import Foundation

/// Runs the bundled Node bridge around the Claude Agent SDK with every tool denied.
public final class ClaudeAgent: AgentSession {
    private let node: URL?
    private let bridgeScript: URL?
    private var process: LineProcess?
    private var onEvent: ((AgentEvent) -> Void)?
    private var text = ""
    private var finished = false
    private var scratch: URL?

    public init(node: URL? = ExecutableLocator.find("node"), bridgeScript: URL? = ClaudeAgent.locateBridge()) {
        self.node = node
        self.bridgeScript = bridgeScript
    }

    public static func locateBridge() -> URL? {
        var candidates: [URL] = []
        if let res = Bundle.main.resourceURL { candidates.append(res.appendingPathComponent("bridge/claude/bridge.mjs")) }
        let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        candidates.append(repo.appendingPathComponent("bridge/claude/bridge.mjs"))
        return candidates.first { FileManager.default.fileExists(atPath: $0.path) }
    }

    public func start(prompt: String, onEvent: @escaping (AgentEvent) -> Void) {
        self.onEvent = onEvent
        guard let node else {
            finish(.failed("Claude needs Node.js. Install it from nodejs.org or with `brew install node`."))
            return
        }
        guard let bridgeScript else {
            finish(.failed("The Claude bridge is missing from this build."))
            return
        }
        let dir = ExecutableLocator.scratchDirectory()
        scratch = dir
        var dirs = [node.deletingLastPathComponent()]
        if let claude = ExecutableLocator.find("claude") { dirs.append(claude.deletingLastPathComponent()) }
        var env = ExecutableLocator.environment(adding: dirs)
        if let claude = ExecutableLocator.find("claude") { env["FLUENT_WRITER_CLAUDE_PATH"] = claude.path }
        let p = LineProcess(executable: node, arguments: [bridgeScript.path], workingDirectory: dir, environment: env)
        p.onLine = { [weak self] in self?.receive($0) }
        p.onExit = { [weak self] status, err in
            guard let self, !self.finished else { return }
            let detail = err.split(separator: "\n").last.map(String.init) ?? "exit \(status)"
            self.finish(.failed("Claude stopped unexpectedly: \(detail)"))
        }
        do { try p.run() } catch {
            finish(.failed("Couldn't start Claude: \(error.localizedDescription)"))
            return
        }
        process = p
        let req: JSONValue = ["type": "request", "prompt": .string(prompt), "cwd": .string(dir.path)]
        p.send(req.serialized())
    }

    public func receive(_ line: String) {
        guard !finished, let msg = JSONValue.parse(line) else { return }
        switch msg["type"]?.stringValue {
        case "delta":
            if let t = msg["text"]?.stringValue {
                text += t
                onEvent?(.progress(text))
            }
        case "done":
            finish(.completed(msg["text"]?.stringValue ?? text))
        case "error":
            finish(.failed(msg["message"]?.stringValue ?? "Claude reported an error."))
        default:
            break
        }
    }

    public func cancel() {
        guard !finished else { return }
        finished = true
        onEvent = nil
        process?.send(JSONValue.object(["type": "cancel"]).serialized())
        tearDown()
    }

    private func finish(_ event: AgentEvent) {
        guard !finished else { return }
        finished = true
        let handler = onEvent
        onEvent = nil
        handler?(event)
        tearDown()
    }

    private func tearDown() {
        let p = process
        process = nil
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { p?.terminate() }
        if let scratch {
            try? FileManager.default.removeItem(at: scratch)
            self.scratch = nil
        }
    }
}

public enum AgentFactory {
    public static func make(_ provider: AgentProvider) -> AgentSession {
        if let path = ProcessInfo.processInfo.environment["FLUENT_WRITER_AGENT_FIXTURE"],
           let reply = try? String(contentsOfFile: path, encoding: .utf8) {
            return FixtureAgent(reply: reply)
        }
        switch provider {
        case .codex: return CodexAgent()
        case .claude: return ClaudeAgent()
        }
    }
}

/// Replays a canned reply word by word; used for offline UI testing.
public final class FixtureAgent: AgentSession {
    private let reply: String
    private var cancelled = false

    public init(reply: String) { self.reply = reply }

    public func start(prompt: String, onEvent: @escaping (AgentEvent) -> Void) {
        let chunks = reply.split(separator: " ", omittingEmptySubsequences: false).map(String.init)
        var text = ""
        for (i, chunk) in chunks.enumerated() {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.04 * Double(i + 1)) { [weak self] in
                guard let self, !self.cancelled else { return }
                text += (i == 0 ? "" : " ") + chunk
                if i == chunks.count - 1 { self.cancelled = true; onEvent(.completed(text)) } else { onEvent(.progress(text)) }
            }
        }
    }

    public func cancel() { cancelled = true }
}
