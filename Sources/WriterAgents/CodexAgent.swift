import Foundation

/// Talks to `codex app-server` over JSON-RPC: one ephemeral read-only thread, one turn.
public final class CodexAgent: AgentSession {
    public enum Phase: Equatable { case idle, initializing, startingThread, startingTurn, streaming, finished }

    private let executable: URL?
    private var process: LineProcess?
    private var onEvent: ((AgentEvent) -> Void)?
    private var nextID = 1
    private var pending: [Int: String] = [:]
    private var prompt = ""
    private var threadID: String?
    private var turnID: String?
    private var text = ""
    private var finalText: String?
    private var scratch: URL?
    public private(set) var phase: Phase = .idle

    /// Lines written to the server; exposed for tests.
    public var transcript: [String] = []
    /// Overrides the process transport in tests.
    public var sendOverride: ((String) -> Void)?

    public init(executable: URL? = ExecutableLocator.find("codex")) {
        self.executable = executable
    }

    public func start(prompt: String, onEvent: @escaping (AgentEvent) -> Void) {
        self.prompt = prompt
        self.onEvent = onEvent
        if sendOverride == nil {
            guard let executable else {
                finish(.failed("Codex isn't installed. Install it with `npm install -g @openai/codex`, then sign in with `codex login`."))
                return
            }
            let dir = ExecutableLocator.scratchDirectory()
            scratch = dir
            let p = LineProcess(
                executable: executable, arguments: ["app-server"], workingDirectory: dir,
                environment: ExecutableLocator.environment(adding: [executable.deletingLastPathComponent()])
            )
            p.onLine = { [weak self] in self?.receive($0) }
            p.onExit = { [weak self] status, err in
                guard let self, self.phase != .finished else { return }
                let detail = err.split(separator: "\n").last.map(String.init) ?? "exit \(status)"
                self.finish(.failed("Codex stopped unexpectedly: \(detail)"))
            }
            do { try p.run() } catch {
                finish(.failed("Couldn't start Codex: \(error.localizedDescription)"))
                return
            }
            process = p
        }
        phase = .initializing
        request("initialize", [
            "clientInfo": ["name": "fluent_writer", "title": "Fluent Writer", "version": "0.1.0"],
            "capabilities": ["experimentalApi": false],
        ])
    }

    public func cancel() {
        guard phase != .finished else { return }
        if let threadID, let turnID {
            request("turn/interrupt", ["threadId": .string(threadID), "turnId": .string(turnID)])
        }
        phase = .finished
        onEvent = nil
        tearDown()
    }

    // MARK: Protocol

    private func request(_ method: String, _ params: JSONValue) {
        let id = nextID
        nextID += 1
        pending[id] = method
        write(["jsonrpc": "2.0", "id": .number(Double(id)), "method": .string(method), "params": params])
    }

    private func notify(_ method: String) {
        write(["jsonrpc": "2.0", "method": .string(method)])
    }

    private func respond(_ id: JSONValue, result: JSONValue) {
        write(["jsonrpc": "2.0", "id": id, "result": result])
    }

    private func respondError(_ id: JSONValue, _ message: String) {
        write(["jsonrpc": "2.0", "id": id, "error": ["code": -32601, "message": .string(message)]])
    }

    private func write(_ v: JSONValue) {
        let line = v.serialized()
        transcript.append(line)
        if let sendOverride { sendOverride(line) } else { process?.send(line) }
    }

    public func receive(_ line: String) {
        guard phase != .finished, let msg = JSONValue.parse(line) else { return }
        let method = msg["method"]?.stringValue
        if let id = msg["id"], let method {
            handleServerRequest(id: id, method: method)
        } else if let id = msg["id"]?.numberValue {
            handleResponse(id: Int(id), msg)
        } else if let method {
            handleNotification(method, msg["params"] ?? .null)
        }
    }

    private func handleResponse(id: Int, _ msg: JSONValue) {
        guard let method = pending.removeValue(forKey: id) else { return }
        if let err = msg["error"] {
            let message = err["message"]?.stringValue ?? "unknown error"
            if method == "turn/interrupt" { return }
            finish(.failed(Self.friendly(message)))
            return
        }
        let result = msg["result"] ?? .null
        switch method {
        case "initialize":
            notify("initialized")
            phase = .startingThread
            var params: [String: JSONValue] = [
                "approvalPolicy": "never",
                "sandbox": "read-only",
                "ephemeral": true,
            ]
            if let scratch { params["cwd"] = .string(scratch.path) }
            request("thread/start", .object(params))
        case "thread/start":
            guard let id = result["thread"]?["id"]?.stringValue else {
                finish(.failed("Codex didn't start a conversation."))
                return
            }
            threadID = id
            phase = .startingTurn
            request("turn/start", [
                "threadId": .string(id),
                "input": [["type": "text", "text": .string(prompt)]],
                "approvalPolicy": "never",
                "sandboxPolicy": ["type": "readOnly"],
            ])
        case "turn/start":
            turnID = result["turn"]?["id"]?.stringValue
            phase = .streaming
        default:
            break
        }
    }

    private func handleNotification(_ method: String, _ params: JSONValue) {
        switch method {
        case "turn/started":
            if turnID == nil { turnID = params["turn"]?["id"]?.stringValue }
        case "item/agentMessage/delta":
            if let d = params["delta"]?.stringValue {
                text += d
                onEvent?(.progress(text))
            }
        case "item/completed":
            if params["item"]?["type"]?.stringValue == "agentMessage", let t = params["item"]?["text"]?.stringValue {
                finalText = t
            }
        case "turn/completed":
            let turn = params["turn"]
            let status = turn?["status"]?.stringValue
            if status == "failed" {
                let message = turn?["error"]?["message"]?.stringValue ?? "The request failed."
                finish(.failed(Self.friendly(message)))
            } else if status == "interrupted" {
                finish(.failed("Stopped."))
            } else {
                finish(.completed(finalText ?? text))
            }
        case "error":
            if params["willRetry"] == .bool(true) { return }
            let message = params["error"]?["message"]?.stringValue ?? "Codex reported an error."
            finish(.failed(Self.friendly(message)))
        default:
            break
        }
    }

    /// Writing help never needs tools; every approval or input request is declined.
    private func handleServerRequest(id: JSONValue, method: String) {
        switch method {
        case "item/commandExecution/requestApproval", "item/fileChange/requestApproval",
             "execCommandApproval", "applyPatchApproval":
            respond(id, result: ["decision": "decline"])
        case "item/permissions/requestApproval":
            respond(id, result: ["decision": "decline"])
        case "mcpServer/elicitation/request":
            respond(id, result: ["action": "decline"])
        default:
            respondError(id, "Fluent Writer doesn't support \(method).")
        }
    }

    static func friendly(_ message: String) -> String {
        let lower = message.lowercased()
        if lower.contains("login") || lower.contains("auth") || lower.contains("401") || lower.contains("api key") {
            return "Codex isn't signed in. Run `codex login` in Terminal, then try again."
        }
        return message
    }

    private func finish(_ event: AgentEvent) {
        guard phase != .finished else { return }
        phase = .finished
        let handler = onEvent
        onEvent = nil
        handler?(event)
        tearDown()
    }

    private func tearDown() {
        let p = process
        process = nil
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { p?.terminate() }
        if let scratch {
            try? FileManager.default.removeItem(at: scratch)
            self.scratch = nil
        }
    }
}
