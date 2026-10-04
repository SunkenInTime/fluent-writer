import Foundation

/// A file or folder on another machine, reached with the system `ssh` and the user's own SSH config and keys.
public struct RemoteLocation: Codable, Hashable, Sendable {
    /// `host`, `user@host`, or an alias from `~/.ssh/config`.
    public var host: String
    public var port: Int?
    /// Absolute, or relative to the remote home when it starts with `~`.
    public var path: String

    public init(host: String, port: Int? = nil, path: String) {
        self.host = host
        self.port = port
        self.path = path
    }

    /// Accepts `host:path`, `user@host:~/notes.md`, and `ssh://user@host:2222/path`.
    public static func parse(_ input: String) -> RemoteLocation? {
        let text = input.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.lowercased().hasPrefix("ssh://") {
            guard let comps = URLComponents(string: text), let host = comps.host, !host.isEmpty else { return nil }
            let user = comps.user.map { "\($0)@" } ?? ""
            var path = comps.path
            if path.hasPrefix("/~") { path.removeFirst() }
            if path.isEmpty { path = "~" }
            return validated(RemoteLocation(host: user + host, port: comps.port, path: path))
        }
        guard let colon = text.firstIndex(of: ":") else { return nil }
        let host = String(text[..<colon])
        var path = String(text[text.index(after: colon)...])
        if path.isEmpty { path = "~" }
        return validated(RemoteLocation(host: host, path: path))
    }

    private static func validated(_ loc: RemoteLocation) -> RemoteLocation? {
        let bad = CharacterSet.whitespacesAndNewlines.union(CharacterSet(charactersIn: "/'\"`$;|&<>()\\"))
        guard !loc.host.isEmpty, !loc.host.hasPrefix("-"), loc.host.rangeOfCharacter(from: bad) == nil else { return nil }
        guard !loc.path.contains("\n"), !loc.path.contains("\0") else { return nil }
        return loc
    }

    public var display: String {
        if let port { return "ssh://\(host):\(port)/\(path.hasPrefix("/") ? String(path.dropFirst()) : path)" }
        return "\(host):\(path)"
    }

    /// The host without the user, for compact labels.
    public var hostName: String { host.split(separator: "@").last.map(String.init) ?? host }

    public var name: String {
        let trimmed = path.hasSuffix("/") && path.count > 1 ? String(path.dropLast()) : path
        return (trimmed as NSString).lastPathComponent
    }

    public func appending(_ component: String) -> RemoteLocation {
        var copy = self
        copy.path = path.hasSuffix("/") ? path + component : path + "/" + component
        return copy
    }

    /// The path as a remote shell word. A leading `~` stays unquoted so the remote shell expands it.
    public var shellPath: String {
        if path == "~" { return "~" }
        if path.hasPrefix("~/") { return "~/" + RemoteLocation.shellQuote(String(path.dropFirst(2))) }
        return RemoteLocation.shellQuote(path)
    }

    public static func shellQuote(_ s: String) -> String {
        "'" + s.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }
}

public struct RemoteEntry: Hashable, Sendable {
    public var name: String
    public var isDirectory: Bool

    public init(name: String, isDirectory: Bool) {
        self.name = name
        self.isDirectory = isDirectory
    }
}

public enum RemoteKind: Sendable { case file, directory, missing }

/// Runs `ssh` non-interactively. Login must work without a prompt (keys, agent, or an existing control master).
public struct SSHTransport: Sendable {
    public var executable: String
    public var connectTimeout: Int

    public init(executable: String = "/usr/bin/ssh", connectTimeout: Int = 10) {
        self.executable = executable
        self.connectTimeout = connectTimeout
    }

    public func arguments(for loc: RemoteLocation, command: String) -> [String] {
        var args = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=\(connectTimeout)", "-o", "ServerAliveInterval=5", "-o", "ServerAliveCountMax=2"]
        if let port = loc.port { args += ["-p", String(port)] }
        args += [loc.host, command]
        return args
    }

    public func read(_ loc: RemoteLocation) throws -> Data {
        try run(loc, "cat -- \(loc.shellPath)")
    }

    /// Writes to a temporary file beside the target, then renames it over the target.
    public func write(_ data: Data, to loc: RemoteLocation) throws {
        let tmp = loc.appendingSuffix(".fwtmp-\(UUID().uuidString.prefix(8))").shellPath
        let target = loc.shellPath
        _ = try run(loc, "cat > \(tmp) && mv -f -- \(tmp) \(target) || { rm -f -- \(tmp); exit 1; }", input: data)
    }

    public func kind(_ loc: RemoteLocation) throws -> RemoteKind {
        let p = loc.shellPath
        let out = String(decoding: try run(loc, "if [ -d \(p) ]; then echo d; elif [ -f \(p) ]; then echo f; else echo n; fi"), as: UTF8.self)
        switch out.trimmingCharacters(in: .whitespacesAndNewlines) {
        case "d": return .directory
        case "f": return .file
        default: return .missing
        }
    }

    public func list(_ loc: RemoteLocation) throws -> [RemoteEntry] {
        let out = try run(loc, "cd -- \(loc.shellPath) && ls -1Ap")
        return SSHTransport.parseListing(String(decoding: out, as: UTF8.self))
    }

    public static func parseListing(_ output: String) -> [RemoteEntry] {
        output.split(whereSeparator: \.isNewline).compactMap { line -> RemoteEntry? in
            var name = String(line)
            guard !name.hasPrefix(".") else { return nil }
            let isDir = name.hasSuffix("/")
            if isDir { name.removeLast() }
            guard !name.isEmpty else { return nil }
            return RemoteEntry(name: name, isDirectory: isDir)
        }
    }

    @discardableResult
    func run(_ loc: RemoteLocation, _ command: String, input: Data? = nil) throws -> Data {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = arguments(for: loc, command: command)
        let out = Pipe(), err = Pipe(), inp = Pipe()
        process.standardOutput = out
        process.standardError = err
        process.standardInput = inp
        do { try process.run() } catch {
            throw DraftStoreError(message: "Couldn't start ssh: \(error.localizedDescription)")
        }
        let errBox = DataBox()
        let group = DispatchGroup()
        group.enter()
        DispatchQueue.global().async {
            errBox.data = err.fileHandleForReading.readDataToEndOfFile()
            group.leave()
        }
        group.enter()
        DispatchQueue.global().async {
            if let input { try? inp.fileHandleForWriting.write(contentsOf: input) }
            try? inp.fileHandleForWriting.close()
            group.leave()
        }
        let data = out.fileHandleForReading.readDataToEndOfFile()
        group.wait()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw DraftStoreError(message: SSHTransport.describeFailure(String(decoding: errBox.data, as: UTF8.self), host: loc.hostName, status: process.terminationStatus))
        }
        return data
    }

    static func describeFailure(_ stderr: String, host: String, status: Int32) -> String {
        let lines = stderr.split(whereSeparator: \.isNewline).map(String.init).filter { !$0.hasPrefix("Warning: Permanently added") }
        let detail = lines.last?.trimmingCharacters(in: .whitespaces) ?? ""
        if detail.contains("Permission denied") {
            return "\(host) refused the login. Fluent Writer can't type passwords, so use an SSH key or ssh-agent (ssh-copy-id \(host))."
        }
        if detail.contains("Host key verification failed") {
            return "\(host) isn't a known host yet. Run `ssh \(host)` in Terminal once to trust it."
        }
        if detail.isEmpty { return "ssh to \(host) failed (exit \(status))." }
        return "\(host): \(detail)"
    }
}

private final class DataBox: @unchecked Sendable { var data = Data() }

extension RemoteLocation {
    func appendingSuffix(_ suffix: String) -> RemoteLocation {
        var copy = self
        copy.path += suffix
        return copy
    }
}
