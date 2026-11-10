import Foundation
import WriterCore

public enum AgentProvider: String, CaseIterable, Codable, Sendable {
    case codex
    case claude

    public var displayName: String {
        switch self {
        case .codex: return "Codex"
        case .claude: return "Claude"
        }
    }
}

public enum AgentAction: String, CaseIterable, Sendable {
    case critique
    case clarify
    case shorten
    case strongerOpening
    case custom
    case resolveComments

    /// Actions offered as one-click buttons.
    public static var quickActions: [AgentAction] { [.critique, .clarify, .shorten, .strongerOpening] }

    public var label: String {
        switch self {
        case .critique: return "Critique"
        case .clarify: return "Clarify"
        case .shorten: return "Shorten"
        case .strongerOpening: return "Stronger opening"
        case .custom: return "Ask"
        case .resolveComments: return "Resolve comments"
        }
    }

    /// Whether this action may change the text.
    public var proposesEdit: Bool { self != .critique }

    var task: String {
        switch self {
        case .critique:
            return "Critique the text. Point out what works, what is unclear, and what to cut. Do not rewrite it."
        case .clarify:
            return "Edit the text so every sentence is clear and direct, keeping the author's voice, meaning and length."
        case .shorten:
            return "Shorten the text by roughly a third without losing its meaning or voice."
        case .strongerOpening:
            return "Rewrite only the opening so it pulls the reader in. Leave the rest unchanged."
        case .custom:
            return "Follow the author's instruction. It may ask you to change any part of the draft, add new text anywhere (including at the end), or just answer a question."
        case .resolveComments:
            return "The author left comments on the draft. Address each one: change the text it points at when the comment asks for a change, or answer it when it asks a question."
        }
    }
}

/// A comment as the agent sees it.
public struct PromptComment: Equatable, Sendable {
    public var id: String
    public var quote: String
    public var body: String
    public var replies: [String]

    public init(id: String, quote: String, body: String, replies: [String] = []) {
        self.id = id
        self.quote = quote
        self.body = body
        self.replies = replies
    }
}

public struct AgentRequest: Equatable, Sendable {
    public var provider: AgentProvider
    public var action: AgentAction
    public var instruction: String
    /// The whole draft. Edits may land anywhere in it.
    public var document: String
    /// The passage the author selected, if any. It focuses the request; it doesn't limit it.
    public var selection: String?
    public var title: String
    public var comments: [PromptComment]
    /// Earlier instructions and replies for this draft, oldest first.
    public var history: [(instruction: String, feedback: String)]

    public init(provider: AgentProvider, action: AgentAction, instruction: String, document: String, selection: String? = nil,
                title: String, comments: [PromptComment] = [], history: [(instruction: String, feedback: String)] = []) {
        self.provider = provider
        self.action = action
        self.instruction = instruction
        self.document = document
        self.selection = selection
        self.title = title
        self.comments = comments
        self.history = history
    }

    public static func == (a: AgentRequest, b: AgentRequest) -> Bool {
        a.provider == b.provider && a.action == b.action && a.instruction == b.instruction && a.document == b.document
            && a.selection == b.selection && a.title == b.title && a.comments == b.comments
            && a.history.map(\.instruction) == b.history.map(\.instruction) && a.history.map(\.feedback) == b.history.map(\.feedback)
    }
}

public enum AgentEvent: Equatable, Sendable {
    /// Accumulated raw model output so far.
    case progress(String)
    case completed(String)
    case failed(String)
}

public struct AgentReply: Equatable, Sendable {
    public var feedback: String
    public var edits: [ProposedEdit]
    /// Answers to comments, keyed by comment id.
    public var replies: [String: String]

    public init(feedback: String, edits: [ProposedEdit] = [], replies: [String: String] = [:]) {
        self.feedback = feedback
        self.edits = edits
        self.replies = replies
    }
}

public enum AgentPrompt {
    public static let feedbackTag = "feedback"
    public static let editTag = "edit"

    public static func build(_ request: AgentRequest) -> String {
        let titled = request.title.isEmpty ? "" : " titled \"\(request.title)\""
        var lines: [String] = [
            "You are a careful writing editor working with an author on their Markdown draft\(titled).",
            "Work only from the text in this message. Do not run commands, read or write files, browse, or use any tools.",
            "",
            "Task: \(request.action.task)",
        ]
        let instruction = request.instruction.trimmingCharacters(in: .whitespacesAndNewlines)
        if !instruction.isEmpty {
            lines.append("Author's instruction: \(instruction)")
        }
        if let selection = request.selection, !selection.isEmpty {
            lines += [
                "",
                "The author selected this passage, so focus on it. You may still change other parts of the draft when the instruction calls for it.",
                "<selection>", selection, "</selection>",
            ]
        }
        if !request.comments.isEmpty {
            lines.append("")
            lines.append(request.action == .resolveComments
                ? "The author's comments. Each one is attached to the quoted text:"
                : "Comments the author left on the draft, for context. Act on them only if the instruction asks you to:")
            for c in request.comments {
                lines.append("<comment id=\"\(c.id)\">")
                lines += ["<on>", c.quote, "</on>", "<note>", c.body, "</note>"]
                for r in c.replies { lines += ["<earlier-reply>", r, "</earlier-reply>"] }
                lines.append("</comment>")
            }
        }
        let history = request.history.suffix(4)
        if !history.isEmpty {
            lines += ["", "Earlier in this session (for context; the draft below already reflects whatever the author kept):"]
            for h in history {
                let ask = h.instruction.isEmpty ? "(a quick action)" : h.instruction
                lines.append("- Author asked: \(ask.prefix(400))")
                if !h.feedback.isEmpty { lines.append("  You replied: \(h.feedback.prefix(600))") }
            }
        }
        lines += [
            "",
            "Reply in exactly this format:",
            "<\(feedbackTag)>",
            "Brief notes for the author in plain prose. Two to six short sentences or bullet points.",
            "</\(feedbackTag)>",
        ]
        if request.action.proposesEdit {
            lines += [
                "Then zero or more edits. Each edit changes one passage, anywhere in the draft:",
                "<edit>",
                "<old>exact text copied from the draft</old>",
                "<new>the replacement</new>",
                "</edit>",
                "To add text at the end of the draft (or at the start), use:",
                "<edit at=\"end\">",
                "<new>the text to add</new>",
                "</edit>",
                "Rules for edits:",
                "- Copy <old> character for character from the draft, and make it long enough to appear only once. Keep it to the sentences you change, not the whole draft.",
                "- To insert text after a passage, put that passage in <old> and repeat it at the start of <new>.",
                "- To delete text, leave <new> empty.",
                "- Never repeat the draft or add a second copy of it. Edits replace text in place; the author previews the result as a separate version.",
                "- Leave out edits entirely if no change is warranted.",
            ]
            if !request.comments.isEmpty {
                lines += [
                    "- When an edit addresses a comment, name it: <edit for=\"COMMENT_ID\">.",
                    "- To answer a comment without changing the text, write <reply for=\"COMMENT_ID\">your answer</reply>.",
                ]
            }
        } else {
            lines.append("Do not include any edits.")
            if !request.comments.isEmpty {
                lines.append("You may answer comments with <reply for=\"COMMENT_ID\">your answer</reply>.")
            }
        }
        lines += ["", "Draft:", "<draft>", request.document, "</draft>"]
        return lines.joined(separator: "\n")
    }

    /// Splits model output into feedback, edits and comment replies. Tolerates partial streams:
    /// feedback shows as it arrives, edits and replies only once their closing tag is in.
    public static func parse(_ output: String) -> AgentReply {
        let feedback = partialSection(feedbackTag, in: output)
        let edits = blocks("edit", in: output).compactMap(edit(from:))
        var replies: [String: String] = [:]
        for b in blocks("reply", in: output) {
            guard let id = attribute("for", in: b.attributes)?.split(separator: " ").first.map(String.init) else { continue }
            let text = b.body.trimmingCharacters(in: .whitespacesAndNewlines)
            if !text.isEmpty { replies[id] = text }
        }
        if feedback == nil && edits.isEmpty && replies.isEmpty && !output.contains("<") {
            return AgentReply(feedback: output.trimmingCharacters(in: .whitespacesAndNewlines))
        }
        return AgentReply(feedback: (feedback ?? "").trimmingCharacters(in: .whitespacesAndNewlines), edits: edits, replies: replies)
    }

    public static func isDraftingEdits(_ output: String) -> Bool { output.contains("<edit") }

    static func edit(from block: (attributes: String, body: String)) -> ProposedEdit? {
        let at = attribute("at", in: block.attributes).flatMap { ProposedEdit.Position(rawValue: $0.lowercased()) }
        let ids = attribute("for", in: block.attributes)?.split(whereSeparator: { $0 == " " || $0 == "," }).map(String.init) ?? []
        let old = innerSection("old", in: block.body).map(trimNewlines)
        guard let new = innerSection("new", in: block.body).map(trimNewlines) else { return nil }
        if old == nil && at == nil { return nil }
        return ProposedEdit(old: old, new: new, position: (old?.isEmpty ?? true) ? (at ?? .end) : nil, commentIDs: ids)
    }

    /// Complete `<tag …>…</tag>` blocks, in order.
    static func blocks(_ tag: String, in text: String) -> [(attributes: String, body: String)] {
        guard let re = try? NSRegularExpression(pattern: "<\(tag)(\\s[^>]*)?>([\\s\\S]*?)</\(tag)>") else { return [] }
        let ns = text as NSString
        return re.matches(in: text, range: NSRange(location: 0, length: ns.length)).map { m in
            let attrs = m.range(at: 1).location == NSNotFound ? "" : ns.substring(with: m.range(at: 1))
            return (attrs, ns.substring(with: m.range(at: 2)))
        }
    }

    static func attribute(_ name: String, in attrs: String) -> String? {
        guard let re = try? NSRegularExpression(pattern: "\(name)\\s*=\\s*[\"']([^\"']*)[\"']") else { return nil }
        let ns = attrs as NSString
        guard let m = re.firstMatch(in: attrs, range: NSRange(location: 0, length: ns.length)) else { return nil }
        return ns.substring(with: m.range(at: 1)).trimmingCharacters(in: .whitespaces)
    }

    static func innerSection(_ tag: String, in text: String) -> String? {
        guard let open = text.range(of: "<\(tag)>"), let close = text.range(of: "</\(tag)>", range: open.upperBound..<text.endIndex) else { return nil }
        return String(text[open.upperBound..<close.lowerBound])
    }

    /// A section that may still be streaming: everything after the open tag up to its close, or up to
    /// the next tag when the close hasn't arrived.
    static func partialSection(_ tag: String, in text: String) -> String? {
        guard let open = text.range(of: "<\(tag)>") else { return nil }
        let rest = text[open.upperBound...]
        if let close = rest.range(of: "</\(tag)>") { return String(rest[..<close.lowerBound]) }
        if let next = rest.range(of: "<") { return String(rest[..<next.lowerBound]) }
        return String(rest)
    }

    static func trimNewlines(_ s: String) -> String {
        var t = Substring(s)
        while t.first == "\n" || t.first == "\r" { t.removeFirst() }
        while t.last == "\n" || t.last == "\r" { t.removeLast() }
        return String(t)
    }
}

/// One in-flight request to a writing agent.
public protocol AgentSession: AnyObject {
    /// Starts the request. `onEvent` is called on the main queue until a terminal event or `cancel()`.
    func start(prompt: String, onEvent: @escaping (AgentEvent) -> Void)
    /// Stops the request. No events are delivered after this returns.
    func cancel()
}
