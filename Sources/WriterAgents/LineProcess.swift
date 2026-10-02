import Foundation

/// Child process speaking newline-delimited text over stdio. Callbacks arrive on the main queue.
public final class LineProcess {
    public var onLine: ((String) -> Void)?
    public var onExit: ((Int32, String) -> Void)?

    private let process = Process()
    private let stdin = Pipe()
    private let stdout = Pipe()
    private let stderr = Pipe()
    private var buffer = Data()
    private var errorTail = ""
    private var closed = false

    public init(executable: URL, arguments: [String], workingDirectory: URL?, environment: [String: String]? = nil) {
        process.executableURL = executable
        process.arguments = arguments
        if let workingDirectory { process.currentDirectoryURL = workingDirectory }
        if let environment { process.environment = environment }
        process.standardInput = stdin
        process.standardOutput = stdout
        process.standardError = stderr
    }

    public func run() throws {
        stdout.fileHandleForReading.readabilityHandler = { [weak self] h in
            let data = h.availableData
            if data.isEmpty { h.readabilityHandler = nil; return }
            DispatchQueue.main.async { self?.consume(data) }
        }
        stderr.fileHandleForReading.readabilityHandler = { [weak self] h in
            let data = h.availableData
            if data.isEmpty { h.readabilityHandler = nil; return }
            guard let s = String(data: data, encoding: .utf8), !s.isEmpty else { return }
            DispatchQueue.main.async {
                guard let self else { return }
                self.errorTail = String((self.errorTail + s).suffix(4000))
            }
        }
        process.terminationHandler = { [weak self] p in
            let status = p.terminationStatus
            DispatchQueue.main.async {
                guard let self else { return }
                self.stdout.fileHandleForReading.readabilityHandler = nil
                self.stderr.fileHandleForReading.readabilityHandler = nil
                if !self.closed { self.onExit?(status, self.errorTail) }
            }
        }
        try process.run()
    }

    public func send(_ line: String) {
        guard !closed, process.isRunning, let data = (line + "\n").data(using: .utf8) else { return }
        try? stdin.fileHandleForWriting.write(contentsOf: data)
    }

    /// Detaches callbacks and stops the process; nothing is delivered afterwards.
    public func terminate() {
        guard !closed else { return }
        closed = true
        onLine = nil
        onExit = nil
        stdout.fileHandleForReading.readabilityHandler = nil
        stderr.fileHandleForReading.readabilityHandler = nil
        try? stdin.fileHandleForWriting.close()
        if process.isRunning {
            process.terminate()
            let p = process
            DispatchQueue.global().asyncAfter(deadline: .now() + 2) {
                if p.isRunning { kill(p.processIdentifier, SIGKILL) }
            }
        }
    }

    private func consume(_ data: Data) {
        guard !closed, !data.isEmpty else { return }
        buffer.append(data)
        while let nl = buffer.firstIndex(of: 0x0A) {
            let lineData = buffer[buffer.startIndex..<nl]
            buffer.removeSubrange(buffer.startIndex...nl)
            if let line = String(data: lineData, encoding: .utf8), !line.isEmpty {
                onLine?(line)
                if closed { return }
            }
        }
    }
}

/// Finds command-line tools the way a login shell would, since GUI apps start with a bare PATH.
public enum ExecutableLocator {
    public static func find(_ name: String, extraDirectories: [String] = []) -> URL? {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        var dirs = extraDirectories
        dirs += (ProcessInfo.processInfo.environment["PATH"] ?? "").split(separator: ":").map(String.init)
        dirs += [
            "/opt/homebrew/bin", "/usr/local/bin", "/usr/bin",
            "\(home)/.local/bin", "\(home)/.npm-global/bin", "\(home)/.bun/bin",
            "\(home)/.volta/bin", "\(home)/.claude/local", "\(home)/.codex/bin",
        ]
        for dir in dirs {
            let path = (dir as NSString).appendingPathComponent(name)
            if FileManager.default.isExecutableFile(atPath: path) { return URL(fileURLWithPath: path) }
        }
        return loginShellLookup(name)
    }

    static func loginShellLookup(_ name: String) -> URL? {
        guard name.allSatisfy({ $0.isLetter || $0.isNumber || $0 == "-" || $0 == "_" }) else { return nil }
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/zsh")
        p.arguments = ["-lc", "command -v \(name)"]
        let out = Pipe()
        p.standardOutput = out
        p.standardError = Pipe()
        do { try p.run() } catch { return nil }
        p.waitUntilExit()
        let s = String(data: out.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard p.terminationStatus == 0, s.hasPrefix("/"), FileManager.default.isExecutableFile(atPath: s) else { return nil }
        return URL(fileURLWithPath: s)
    }

    /// PATH for child processes: the inherited one plus the tool's own directory.
    public static func environment(adding dirs: [URL]) -> [String: String] {
        var env = ProcessInfo.processInfo.environment
        let extra = dirs.map(\.path)
        let base = env["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin"
        env["PATH"] = (extra + ["/opt/homebrew/bin", "/usr/local/bin", base]).joined(separator: ":")
        return env
    }

    /// An empty scratch directory so agents start with nothing on disk to see.
    public static func scratchDirectory() -> URL {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("fluent-writer-agent-\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }
}
