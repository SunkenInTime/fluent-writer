import Foundation

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

    public var label: String {
        switch self {
        case .critique: return "Critique"
        case .clarify: return "Clarify"
        case .shorten: return "Shorten"
        case .strongerOpening: return "Stronger opening"
        case .custom: return "Ask"
        }
    }

    /// Whether this action is expected to propose replacement text.
    public var proposesEdit: Bool {
        switch self {
        case .critique: return false
        case .clarify, .shorten, .strongerOpening: return true
        case .custom: return true
        }
    }

    var task: String {
        switch self {
        case .critique:
            return "Critique the text. Point out what works, what is unclear, and what to cut. Do not rewrite it."
        case .clarify:
            return "Rewrite the text so every sentence is clear and direct, keeping the author's voice, meaning and length."
        case .shorten:
            return "Shorten the text by roughly a third without losing its meaning or voice."
        case .strongerOpening:
            return "Rewrite only the opening so it pulls the reader in. Keep the rest of the text unchanged."
        case .custom:
            return "Follow the author's instruction."
        }
    }
}

public struct AgentRequest: Equatable, Sendable {
    public var provider: AgentProvider
    public var action: AgentAction
    public var instruction: String
    public var scopeText: String
    public var isSelection: Bool
    public var title: String

    public init(provider: AgentProvider, action: AgentAction, instruction: String, scopeText: String, isSelection: Bool, title: String) {
        self.provider = provider
        self.action = action
        self.instruction = instruction
        self.scopeText = scopeText
        self.isSelection = isSelection
        self.title = title
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
    public var revision: String?

    public init(feedback: String, revision: String?) {
        self.feedback = feedback
        self.revision = revision
    }
}

public enum AgentPrompt {
    public static let feedbackTag = "feedback"
    public static let revisionTag = "revision"

    public static func build(_ request: AgentRequest) -> String {
        let scope = request.isSelection ? "a passage selected from a draft" : "a complete draft"
        var lines: [String] = [
            "You are a careful writing editor helping an author with \(scope)\(request.title.isEmpty ? "" : " titled \"\(request.title)\"").",
            "Work only from the text below. Do not run commands, read or write files, browse, or use any tools.",
            "",
            "Task: \(request.action.task)",
        ]
        let instruction = request.instruction.trimmingCharacters(in: .whitespacesAndNewlines)
        if !instruction.isEmpty {
            lines.append("Author's instruction: \(instruction)")
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
                "<\(revisionTag)>",
                "The full replacement for the text, in the same Markdown, with nothing before or after it. Omit this block entirely if no change is warranted.",
                "</\(revisionTag)>",
            ]
        } else {
            lines.append("Do not include a revision.")
        }
        lines += [
            "",
            "Text:",
            "<text>",
            request.scopeText,
            "</text>",
        ]
        return lines.joined(separator: "\n")
    }

    /// Splits model output into feedback and an optional revision. Tolerates partial streams.
    public static func parse(_ output: String) -> AgentReply {
        let feedback = section(feedbackTag, in: output)
        let revision = section(revisionTag, in: output, requireClose: true)
        if feedback == nil && revision == nil {
            if output.contains("<\(revisionTag)>") {
                return AgentReply(feedback: "", revision: nil)
            }
            return AgentReply(feedback: output.trimmingCharacters(in: .whitespacesAndNewlines), revision: nil)
        }
        let cleanedRevision = revision.map(trimRevision)
        return AgentReply(
            feedback: (feedback ?? "").trimmingCharacters(in: .whitespacesAndNewlines),
            revision: (cleanedRevision?.isEmpty ?? true) ? nil : cleanedRevision
        )
    }

    static func section(_ tag: String, in text: String, requireClose: Bool = false) -> String? {
        guard let open = text.range(of: "<\(tag)>") else { return nil }
        let rest = text[open.upperBound...]
        if let close = rest.range(of: "</\(tag)>") {
            return String(rest[..<close.lowerBound])
        }
        if requireClose { return nil }
        let nextOpen = rest.range(of: "<", options: [], range: rest.startIndex..<rest.endIndex)
        if let n = nextOpen, rest[n.lowerBound...].hasPrefix("</") || rest[n.lowerBound...].hasPrefix("<\(revisionTag)") {
            return String(rest[..<n.lowerBound])
        }
        return String(rest)
    }

    static func trimRevision(_ s: String) -> String {
        var t = s
        while t.hasPrefix("\n") { t.removeFirst() }
        while t.hasSuffix("\n") { t.removeLast() }
        return t
    }
}

/// One in-flight request to a writing agent.
public protocol AgentSession: AnyObject {
    /// Starts the request. `onEvent` is called on the main queue until a terminal event or `cancel()`.
    func start(prompt: String, onEvent: @escaping (AgentEvent) -> Void)
    /// Stops the request. No events are delivered after this returns.
    func cancel()
}
