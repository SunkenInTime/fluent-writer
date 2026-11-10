import Foundation
import WriterAgents
import WriterCore

// fluent-writer-mcp: an MCP server (stdio) that lets outside agents such as Claude Code or Codex see
// the draft open in Fluent Writer, its comments, versions and Assist history, and leave comments or
// propose versions the author reviews in the app.
//
// Usage: fluent-writer-mcp [--support <dir>] [--variant dev]

setvbuf(stdout, nil, _IOLBF, 0)

var supportDirectory = AppVariant.standard.supportDirectory
var args = CommandLine.arguments.dropFirst()
while let a = args.popFirst() {
    switch a {
    case "--support": if let v = args.popFirst() { supportDirectory = URL(fileURLWithPath: (v as NSString).expandingTildeInPath, isDirectory: true) }
    case "--variant": if let v = args.popFirst(), let variant = AppVariant(rawValue: v) { supportDirectory = variant.supportDirectory }
    case "--help", "-h":
        print("fluent-writer-mcp [--support <dir>] [--variant standard|dev]\nSpeaks the Model Context Protocol over stdio.")
        exit(0)
    default: break
    }
}

let store = NotesStore(supportDirectory: supportDirectory)
var agentName = "Outside agent"

struct ToolError: Error { var message: String }

// MARK: Helpers

func send(_ v: JSONValue) {
    FileHandle.standardOutput.write(Data((v.serialized() + "\n").utf8))
}

func lineNumber(of location: Int, in text: NSString) -> Int {
    var line = 1
    let end = min(location, text.length)
    var i = 0
    while i < end {
        if text.character(at: i) == 0x0A { line += 1 }
        i += 1
    }
    return line
}

func iso(_ d: Date) -> JSONValue { .string(ISO8601DateFormatter().string(from: d)) }

func resolvePath(_ args: JSONValue) throws -> String {
    if let p = args["path"]?.stringValue, !p.isEmpty {
        return URL(fileURLWithPath: (p as NSString).expandingTildeInPath).standardizedFileURL.path
    }
    if let live = store.liveState(), let p = live.path { return p }
    if store.liveState() != nil {
        throw ToolError(message: "The draft open in Fluent Writer hasn't been saved to a file yet. Ask the author to type a little (it saves within a second), or pass `path`.")
    }
    throw ToolError(message: "Fluent Writer isn't running, so there's no open draft. Pass `path` to a Markdown file, or use list_documents.")
}

func readText(_ path: String) throws -> String {
    guard let data = FileManager.default.contents(atPath: path) else { throw ToolError(message: "Couldn't read \(path).") }
    return String(data: data, encoding: .utf8) ?? String(decoding: data, as: UTF8.self)
}

func commentJSON(_ c: Comment, text: NSString) -> JSONValue {
    var anchored = c
    anchored.reanchor(in: text)
    var o: [String: JSONValue] = [
        "id": .string(c.id),
        "quote": .string(c.quote),
        "comment": .string(c.body),
        "author": .string(c.author),
        "created": iso(c.created),
        "status": .string(c.isOpen ? "open" : "resolved"),
    ]
    if anchored.detached { o["note"] = "The quoted text is no longer in the draft." } else { o["line"] = .number(Double(lineNumber(of: anchored.location, in: text))) }
    if !c.replies.isEmpty {
        o["replies"] = .array(c.replies.map { ["author": .string($0.author), "text": .string($0.text), "date": iso($0.date)] })
    }
    return .object(o)
}

func versionSummary(_ v: DraftVersion) -> JSONValue {
    var o: [String: JSONValue] = [
        "id": .string(v.id), "label": .string(v.label), "source": .string(v.source),
        "kind": .string(v.kind == .snapshot ? "saved draft" : "proposal"), "created": iso(v.created),
        "words": .number(Double(TextStats.counts(forMarkdown: v.body).words)),
    ]
    if let u = v.usedAt { o["used"] = iso(u) }
    if let f = v.feedback { o["summary"] = .string(f) }
    if !v.resolvesCommentIDs.isEmpty { o["resolves_comment_ids"] = .array(v.resolvesCommentIDs.map { .string($0) }) }
    return .object(o)
}

func exchangeJSON(_ e: AssistExchange) -> JSONValue {
    var o: [String: JSONValue] = [
        "date": iso(e.date), "provider": .string(e.provider), "action": .string(e.action),
        "instruction": .string(e.instruction), "reply": .string(e.feedback),
    ]
    if let v = e.versionID { o["version_id"] = .string(v) }
    if !e.commentIDs.isEmpty { o["comment_ids"] = .array(e.commentIDs.map { .string($0) }) }
    return .object(o)
}

/// Sends a change through the running app, or applies it directly when the app is closed.
func mutate(_ command: InboxCommand) throws -> String {
    if store.liveState() != nil {
        try store.post(command)
        guard let r = store.awaitResult(id: command.id, timeout: 4) else {
            return "Queued. Fluent Writer hasn't picked it up yet; it will apply it shortly."
        }
        if !r.ok { throw ToolError(message: r.message) }
        return r.message
    }
    let text = try readText(command.path)
    let r = try store.withLock { () throws -> InboxResult in
        var notes = store.load(path: command.path)
        let r = NotesStore.apply(command, to: &notes, text: text)
        if r.ok { try store.save(notes) }
        return r
    }
    if !r.ok { throw ToolError(message: r.message) }
    return r.message + " (Fluent Writer is closed; the author will see this next time they open the draft.)"
}

// MARK: Tools

let pathProp: JSONValue = ["type": "string", "description": "Absolute path to the Markdown file. Defaults to the draft open in Fluent Writer."]

let tools: [JSONValue] = [
    [
        "name": "get_context",
        "description": "Start here. Returns the draft open in Fluent Writer: its path and title, what the author has selected, their open comments (each pinned to quoted text), the versions waiting for review, and recent Assist requests and replies.",
        "inputSchema": ["type": "object", "properties": [:]],
    ],
    [
        "name": "read_document",
        "description": "Returns the full text of a draft. The open draft autosaves within a second of typing, so this matches what the author sees.",
        "inputSchema": ["type": "object", "properties": [
            "path": pathProp,
            "line_numbers": ["type": "boolean", "description": "Prefix each line with its number."],
        ]],
    ],
    [
        "name": "list_comments",
        "description": "The author's comments on a draft. Each has an id, the quoted text it's pinned to, its line, the comment, and any replies. Comments are notes about what to change; resolve them by proposing a version.",
        "inputSchema": ["type": "object", "properties": [
            "path": pathProp,
            "include_resolved": ["type": "boolean"],
        ]],
    ],
    [
        "name": "add_comment",
        "description": "Pins a comment to text in the draft, the way a reviewer would. `quote` must be copied exactly from the draft.",
        "inputSchema": ["type": "object", "required": ["quote", "comment"], "properties": [
            "path": pathProp,
            "quote": ["type": "string", "description": "Exact text from the draft to attach the comment to."],
            "comment": ["type": "string"],
            "occurrence": ["type": "integer", "description": "Which appearance of `quote` to use when it appears more than once (1-based)."],
        ]],
    ],
    [
        "name": "reply_to_comment",
        "description": "Answers one of the author's comments without changing the text.",
        "inputSchema": ["type": "object", "required": ["comment_id", "reply"], "properties": [
            "path": pathProp, "comment_id": ["type": "string"], "reply": ["type": "string"],
        ]],
    ],
    [
        "name": "resolve_comment",
        "description": "Marks a comment resolved, with an optional note. Prefer listing comment ids in propose_version so they resolve only if the author uses that version.",
        "inputSchema": ["type": "object", "required": ["comment_id"], "properties": [
            "path": pathProp, "comment_id": ["type": "string"], "note": ["type": "string"],
        ]],
    ],
    [
        "name": "propose_version",
        "description": "The way to change a draft that's open in Fluent Writer. Creates a new version from edits (or full text). The author previews it as a diff and decides whether to use it; their draft isn't touched until then. Don't write the open file directly: the app holds the live text and would overwrite your change.",
        "inputSchema": ["type": "object", "required": ["label"], "properties": [
            "path": pathProp,
            "label": ["type": "string", "description": "Short name for the version, e.g. \"Tighter intro\"."],
            "summary": ["type": "string", "description": "A few sentences on what changed and why."],
            "edits": [
                "type": "array",
                "description": "Changes to apply. Each replaces `old` (copied exactly from the draft, long enough to be unique) with `new`, or adds `new` at the start or end when `at` is set.",
                "items": ["type": "object", "required": ["new"], "properties": [
                    "old": ["type": "string"],
                    "new": ["type": "string"],
                    "at": ["type": "string", "enum": ["start", "end"]],
                    "comment_ids": ["type": "array", "items": ["type": "string"]],
                ]],
            ],
            "full_text": ["type": "string", "description": "The complete new draft, instead of edits."],
            "resolves_comment_ids": ["type": "array", "items": ["type": "string"], "description": "Comments this version addresses. They resolve when the author uses it."],
        ]],
    ],
    [
        "name": "list_versions",
        "description": "Versions of a draft: agent proposals and the author's saved drafts, newest first.",
        "inputSchema": ["type": "object", "properties": ["path": pathProp]],
    ],
    [
        "name": "read_version",
        "description": "The full text of one version.",
        "inputSchema": ["type": "object", "required": ["version_id"], "properties": ["path": pathProp, "version_id": ["type": "string"]]],
    ],
    [
        "name": "get_assist_history",
        "description": "What the author asked Fluent Writer's built-in Assist about this draft, and what it replied, oldest first.",
        "inputSchema": ["type": "object", "properties": ["path": pathProp, "limit": ["type": "integer"]]],
    ],
    [
        "name": "list_documents",
        "description": "Drafts the author opened recently in Fluent Writer.",
        "inputSchema": ["type": "object", "properties": ["limit": ["type": "integer"]]],
    ],
]

func pretty(_ v: JSONValue) -> String {
    let e = JSONEncoder()
    e.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
    return (try? e.encode(v)).flatMap { String(data: $0, encoding: .utf8) } ?? v.serialized()
}

func call(_ name: String, _ args: JSONValue) throws -> String {
    switch name {
    case "get_context":
        let live = store.liveState()
        var o: [String: JSONValue] = ["app": live == nil ? "Fluent Writer isn't running." : .string("\(live!.appName) is running.")]
        guard let live, let path = live.path else {
            if let live { o["document"] = .string("“\(live.title)” is open but hasn't been saved to a file yet.") }
            o["recent_documents"] = .array(DraftLibrary.recentPaths(supportDirectory: supportDirectory).prefix(10).map { ["path": .string($0.path), "title": .string($0.title)] })
            return pretty(.object(o))
        }
        let text = (try? readText(path)) ?? ""
        let ns = text as NSString
        let notes = store.load(path: path)
        var doc: [String: JSONValue] = [
            "path": .string(path), "title": .string(live.title),
            "words": .number(Double(TextStats.counts(forMarkdown: text).words)),
            "lines": .number(Double(text.split(separator: "\n", omittingEmptySubsequences: false).count)),
        ]
        if let r = live.remote { doc["remote"] = .string(r) }
        o["document"] = .object(doc)
        if live.selectionLength > 0 {
            o["selection"] = [
                "text": .string(live.selectedText),
                "line": .number(Double(lineNumber(of: live.selectionLocation, in: ns))),
            ]
        } else {
            o["caret_line"] = .number(Double(lineNumber(of: live.selectionLocation, in: ns)))
        }
        o["open_comments"] = .array(notes.openComments.map { commentJSON($0, text: ns) })
        o["versions"] = .array(notes.versions.prefix(8).map(versionSummary))
        if let p = live.previewingVersionID { o["author_is_previewing_version"] = .string(p) }
        o["recent_assist"] = .array(notes.transcript.suffix(5).map(exchangeJSON))
        o["how_to_edit"] = "Use propose_version to change this draft; the author reviews it in the app. Read the full text with read_document."
        return pretty(.object(o))

    case "read_document":
        let path = try resolvePath(args)
        let text = try readText(path)
        guard args["line_numbers"] == .bool(true) else { return text }
        return text.split(separator: "\n", omittingEmptySubsequences: false).enumerated()
            .map { String(format: "%5d  ", $0.offset + 1) + $0.element }.joined(separator: "\n")

    case "list_comments":
        let path = try resolvePath(args)
        let ns = ((try? readText(path)) ?? "") as NSString
        let notes = store.load(path: path)
        let list = args["include_resolved"] == .bool(true) ? notes.comments : notes.openComments
        if list.isEmpty { return "No \(args["include_resolved"] == .bool(true) ? "" : "open ")comments on this draft." }
        return pretty(.array(list.map { commentJSON($0, text: ns) }))

    case "add_comment":
        var c = InboxCommand(kind: .addComment, path: try resolvePath(args), agent: agentName)
        c.quote = args["quote"]?.stringValue
        c.body = args["comment"]?.stringValue
        c.occurrence = args["occurrence"]?.numberValue.map { Int($0) }
        return try mutate(c)

    case "reply_to_comment":
        var c = InboxCommand(kind: .replyToComment, path: try resolvePath(args), agent: agentName)
        c.commentID = args["comment_id"]?.stringValue
        c.body = args["reply"]?.stringValue
        return try mutate(c)

    case "resolve_comment":
        var c = InboxCommand(kind: .resolveComment, path: try resolvePath(args), agent: agentName)
        c.commentID = args["comment_id"]?.stringValue
        c.body = args["note"]?.stringValue
        return try mutate(c)

    case "propose_version":
        var c = InboxCommand(kind: .proposeVersion, path: try resolvePath(args), agent: agentName)
        c.label = args["label"]?.stringValue
        c.body = args["summary"]?.stringValue
        c.fullText = args["full_text"]?.stringValue
        if case let .array(items)? = args["edits"] {
            c.edits = items.map { e in
                let ids: [String]
                if case let .array(a)? = e["comment_ids"] { ids = a.compactMap(\.stringValue) } else { ids = [] }
                return ProposedEdit(old: e["old"]?.stringValue, new: e["new"]?.stringValue ?? "",
                                    position: e["at"]?.stringValue.flatMap { ProposedEdit.Position(rawValue: $0) }, commentIDs: ids)
            }
        }
        if c.fullText == nil && (c.edits ?? []).isEmpty { throw ToolError(message: "Pass `edits` or `full_text`.") }
        if case let .array(a)? = args["resolves_comment_ids"] { c.resolvesCommentIDs = a.compactMap(\.stringValue) }
        return try mutate(c)

    case "list_versions":
        let notes = store.load(path: try resolvePath(args))
        if notes.versions.isEmpty { return "No versions yet." }
        return pretty(.array(notes.versions.map(versionSummary)))

    case "read_version":
        let notes = store.load(path: try resolvePath(args))
        guard let id = args["version_id"]?.stringValue, let v = notes.versions.first(where: { $0.id == id }) else {
            throw ToolError(message: "No version with that id. Use list_versions.")
        }
        return v.body

    case "get_assist_history":
        let notes = store.load(path: try resolvePath(args))
        let limit = args["limit"]?.numberValue.map { Int($0) } ?? 20
        if notes.transcript.isEmpty { return "No Assist requests for this draft yet." }
        return pretty(.array(notes.transcript.suffix(max(1, limit)).map(exchangeJSON)))

    case "list_documents":
        let limit = args["limit"]?.numberValue.map { Int($0) } ?? 20
        let recent = DraftLibrary.recentPaths(supportDirectory: supportDirectory).prefix(max(1, limit))
        if recent.isEmpty { return "No recent drafts." }
        return pretty(.array(recent.map { r in
            let notes = store.load(path: r.path)
            return ["path": .string(r.path), "title": .string(r.title), "last_opened": iso(r.lastOpened),
                    "open_comments": .number(Double(notes.openComments.count)), "versions": .number(Double(notes.versions.count))]
        }))

    default:
        throw ToolError(message: "Unknown tool \(name).")
    }
}

// MARK: Protocol loop

let instructions = """
Fluent Writer is the author's Markdown writing app. Call get_context first to see the open draft, the \
author's selection and comments, versions awaiting review, and their recent Assist conversation. \
Comments are the author's notes about what to change, pinned to quoted text. To change the open draft, \
call propose_version: the author previews it in the app and chooses whether to use it. Don't edit the \
open file directly.
"""

func handle(_ msg: JSONValue) {
    guard let method = msg["method"]?.stringValue else { return }
    let id = msg["id"]
    let params = msg["params"] ?? .null
    func reply(_ result: JSONValue) { if let id { send(["jsonrpc": "2.0", "id": id, "result": result]) } }
    switch method {
    case "initialize":
        let client = params["clientInfo"]
        let raw = (client?["title"]?.stringValue ?? client?["name"]?.stringValue ?? "").lowercased()
        if raw.contains("claude") { agentName = "Claude Code" }
        else if raw.contains("codex") { agentName = "Codex CLI" }
        else if let t = client?["title"]?.stringValue ?? client?["name"]?.stringValue, !t.isEmpty { agentName = t }
        reply([
            "protocolVersion": params["protocolVersion"] ?? "2025-06-18",
            "capabilities": ["tools": ["listChanged": false]],
            "serverInfo": ["name": "fluent-writer", "title": "Fluent Writer", "version": "0.2.0"],
            "instructions": .string(instructions),
        ])
    case "ping":
        reply([:])
    case "tools/list":
        reply(["tools": .array(tools)])
    case "tools/call":
        let name = params["name"]?.stringValue ?? ""
        do {
            let text = try call(name, params["arguments"] ?? [:])
            reply(["content": [["type": "text", "text": .string(text)]], "isError": false])
        } catch let e as ToolError {
            reply(["content": [["type": "text", "text": .string(e.message)]], "isError": true])
        } catch {
            reply(["content": [["type": "text", "text": .string(error.localizedDescription)]], "isError": true])
        }
    default:
        if method.hasPrefix("notifications/") { return }
        if let id { send(["jsonrpc": "2.0", "id": id, "error": ["code": -32601, "message": .string("Method not found: \(method)")]]) }
    }
}

while let line = readLine(strippingNewline: true) {
    guard !line.isEmpty, let msg = JSONValue.parse(line) else { continue }
    if case let .array(batch) = msg { batch.forEach(handle) } else { handle(msg) }
}
