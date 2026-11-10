import CryptoKit
import Foundation

public struct CommentReply: Codable, Equatable, Sendable {
    public var author: String
    public var text: String
    public var date: Date

    public init(author: String, text: String, date: Date = Date()) {
        self.author = author
        self.text = text
        self.date = date
    }
}

/// A note pinned to a span of the draft. The span follows edits while the draft is open and is found
/// again by its quoted text when the file changed elsewhere.
public struct Comment: Codable, Equatable, Identifiable, Sendable {
    public var id: String
    public var location: Int
    public var length: Int
    public var quote: String
    public var prefix: String
    public var suffix: String
    public var body: String
    public var author: String
    public var created: Date
    public var replies: [CommentReply]
    public var resolved: Date?
    /// The quoted text is gone from the draft.
    public var detached: Bool

    public init(id: String = Comment.newID(), range: NSRange, in text: NSString, body: String, author: String = "you", created: Date = Date()) {
        self.id = id
        location = range.location
        length = range.length
        quote = ""
        prefix = ""
        suffix = ""
        self.body = body
        self.author = author
        self.created = created
        replies = []
        resolved = nil
        detached = false
        capture(from: text)
    }

    public static func newID() -> String { String(UUID().uuidString.prefix(8)).lowercased() }

    public var range: NSRange {
        get { NSRange(location: location, length: length) }
        set { location = newValue.location; length = newValue.length }
    }

    public var isOpen: Bool { resolved == nil }

    /// Refreshes the quote and its surrounding context from the current text.
    public mutating func capture(from text: NSString) {
        guard !detached, NSMaxRange(range) <= text.length else { return }
        quote = text.substring(with: range)
        let pre = max(0, location - 32)
        prefix = text.substring(with: NSRange(location: pre, length: location - pre))
        let end = NSMaxRange(range)
        suffix = text.substring(with: NSRange(location: end, length: min(32, text.length - end)))
    }

    /// Follows an edit. `editedRange` is the post-edit range of new text; `changeInLength` the length delta.
    public mutating func noteEdit(editedRange: NSRange, changeInLength delta: Int) {
        guard !detached else { return }
        let pre = NSRange(location: editedRange.location, length: max(0, editedRange.length - delta))
        let start = location, end = location + length
        if NSMaxRange(pre) <= start { location += delta; return }
        if pre.location >= end { return }
        // The edit overlaps the span: keep whatever part of the span survives plus the new text inside it.
        let newStart = pre.location <= start ? editedRange.location : start
        let newEnd = NSMaxRange(pre) >= end ? NSMaxRange(editedRange) : end + delta
        location = newStart
        length = max(0, newEnd - newStart)
        if length == 0 { detached = true }
    }

    /// Finds the span again in `text`. Returns false when the quote no longer appears.
    @discardableResult
    public mutating func reanchor(in text: NSString) -> Bool {
        if NSMaxRange(range) <= text.length, text.substring(with: range) == quote, !quote.isEmpty {
            detached = false
            return true
        }
        guard !quote.isEmpty else { detached = true; return false }
        var best: NSRange?
        var bestScore = Int.min
        var search = NSRange(location: 0, length: text.length)
        while search.length > 0 {
            let r = text.range(of: quote, options: [.literal], range: search)
            if r.location == NSNotFound { break }
            let before = text.substring(with: NSRange(location: max(0, r.location - prefix.utf16.count), length: min(prefix.utf16.count, r.location)))
            let afterStart = NSMaxRange(r)
            let after = text.substring(with: NSRange(location: afterStart, length: min(suffix.utf16.count, text.length - afterStart)))
            let score = (before == prefix ? 2 : 0) + (after == suffix ? 2 : 0) - abs(r.location - location) / 1000
            if score > bestScore { bestScore = score; best = r }
            search = NSRange(location: r.location + 1, length: text.length - r.location - 1)
        }
        guard let best else { detached = true; return false }
        range = best
        detached = false
        return true
    }
}

/// An alternative to the draft. Agent replies arrive as proposals; using one keeps the text it replaced
/// as a snapshot, so every switch can be undone from the list.
public struct DraftVersion: Codable, Equatable, Identifiable, Sendable {
    public enum Kind: String, Codable, Sendable { case proposal, snapshot }

    public var id: String
    public var kind: Kind
    public var label: String
    /// "you", "Codex", "Claude", or the name an outside agent gave.
    public var source: String
    public var created: Date
    public var body: String
    public var feedback: String?
    public var resolvesCommentIDs: [String]
    public var usedAt: Date?

    public init(id: String = Comment.newID(), kind: Kind, label: String, source: String, created: Date = Date(), body: String, feedback: String? = nil, resolvesCommentIDs: [String] = []) {
        self.id = id
        self.kind = kind
        self.label = label
        self.source = source
        self.created = created
        self.body = body
        self.feedback = feedback
        self.resolvesCommentIDs = resolvesCommentIDs
        usedAt = nil
    }
}

/// One request to Assist and what came back.
public struct AssistExchange: Codable, Equatable, Sendable {
    public var date: Date
    public var provider: String
    public var action: String
    public var instruction: String
    public var feedback: String
    public var versionID: String?
    public var commentIDs: [String]

    public init(date: Date = Date(), provider: String, action: String, instruction: String, feedback: String, versionID: String?, commentIDs: [String]) {
        self.date = date
        self.provider = provider
        self.action = action
        self.instruction = instruction
        self.feedback = feedback
        self.versionID = versionID
        self.commentIDs = commentIDs
    }
}

/// Everything Fluent Writer keeps about a draft besides its text.
public struct DocumentNotes: Codable, Equatable, Sendable {
    public var path: String
    public var comments: [Comment] = []
    public var versions: [DraftVersion] = []
    public var transcript: [AssistExchange] = []

    public static let maxVersions = 40
    public static let maxTranscript = 200

    public init(path: String) { self.path = path }

    public var isEmpty: Bool { comments.isEmpty && versions.isEmpty && transcript.isEmpty }
    public var openComments: [Comment] { comments.filter(\.isOpen) }

    /// Adds a version, newest first, dropping the oldest unused ones past the cap.
    public mutating func addVersion(_ v: DraftVersion) {
        versions.insert(v, at: 0)
        while versions.count > Self.maxVersions, let i = versions.lastIndex(where: { $0.usedAt == nil }) ?? versions.indices.last {
            versions.remove(at: i)
        }
    }

    public mutating func addExchange(_ e: AssistExchange) {
        transcript.append(e)
        if transcript.count > Self.maxTranscript { transcript.removeFirst(transcript.count - Self.maxTranscript) }
    }

    public mutating func resolve(_ ids: [String], note: String?, author: String) {
        for i in comments.indices where ids.contains(comments[i].id) && comments[i].isOpen {
            if let note, !note.isEmpty { comments[i].replies.append(CommentReply(author: author, text: note)) }
            comments[i].resolved = Date()
        }
    }

    /// Adds a comment on the `occurrence`-th appearance (1-based) of `quote`.
    public mutating func addComment(quote: String, occurrence: Int = 1, body: String, author: String, in text: NSString) throws -> Comment {
        let ranges = EditPlacer.occurrences(of: quote, in: text)
        guard !ranges.isEmpty else { throw DraftStoreError(message: "That text isn't in the draft.") }
        guard occurrence >= 1, occurrence <= ranges.count else {
            throw DraftStoreError(message: "The text appears \(ranges.count) time\(ranges.count == 1 ? "" : "s"); occurrence must be 1–\(ranges.count).")
        }
        let c = Comment(range: ranges[occurrence - 1], in: text, body: body, author: author)
        comments.append(c)
        return c
    }
}

// MARK: Store

/// Notes live in the support folder, one file per draft path, so drafts stay plain Markdown.
/// The app is the only writer while it runs; outside agents send it commands through the inbox.
public final class NotesStore {
    public let supportDirectory: URL
    public var notesDirectory: URL { supportDirectory.appendingPathComponent("Notes", isDirectory: true) }
    public var inboxDirectory: URL { supportDirectory.appendingPathComponent("Inbox", isDirectory: true) }
    public var resultsDirectory: URL { inboxDirectory.appendingPathComponent("Results", isDirectory: true) }
    public var liveStateURL: URL { supportDirectory.appendingPathComponent("Live.json") }

    public init(supportDirectory: URL) {
        self.supportDirectory = supportDirectory
    }

    /// ISO 8601 with milliseconds, so inbox commands keep their order.
    static let encoder: JSONEncoder = {
        let e = JSONEncoder()
        e.outputFormatting = [.prettyPrinted, .sortedKeys]
        e.dateEncodingStrategy = .custom { date, encoder in
            let f = ISO8601DateFormatter()
            f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
            var c = encoder.singleValueContainer()
            try c.encode(f.string(from: date))
        }
        return e
    }()

    static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .custom { decoder in
            let s = try decoder.singleValueContainer().decode(String.self)
            let f = ISO8601DateFormatter()
            f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
            if let date = f.date(from: s) { return date }
            f.formatOptions = [.withInternetDateTime]
            if let date = f.date(from: s) { return date }
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "Bad date \(s)"))
        }
        return d
    }()

    public static func key(for path: String) -> String {
        let digest = SHA256.hash(data: Data(URL(fileURLWithPath: path).standardizedFileURL.path.utf8))
        return digest.prefix(12).map { String(format: "%02x", $0) }.joined()
    }

    public func url(for path: String) -> URL {
        notesDirectory.appendingPathComponent(Self.key(for: path) + ".json")
    }

    public func load(path: String) -> DocumentNotes {
        guard let data = try? Data(contentsOf: url(for: path)),
              let notes = try? Self.decoder.decode(DocumentNotes.self, from: data) else { return DocumentNotes(path: path) }
        return notes
    }

    public func save(_ notes: DocumentNotes) throws {
        let target = url(for: notes.path)
        if notes.isEmpty {
            try? FileManager.default.removeItem(at: target)
            return
        }
        try FileManager.default.createDirectory(at: notesDirectory, withIntermediateDirectories: true)
        try AtomicFile.write(try Self.encoder.encode(notes), to: target)
    }

    /// Runs `body` holding an exclusive lock on the notes folder, so separate processes
    /// (the app, several MCP servers) can read-modify-write notes without losing each other's changes.
    public func withLock<T>(_ body: () throws -> T) rethrows -> T {
        try? FileManager.default.createDirectory(at: notesDirectory, withIntermediateDirectories: true)
        let fd = open(notesDirectory.appendingPathComponent(".lock").path, O_RDWR | O_CREAT, 0o644)
        if fd >= 0 { flock(fd, LOCK_EX) }
        defer { if fd >= 0 { flock(fd, LOCK_UN); close(fd) } }
        return try body()
    }

    /// Carries notes along when a draft's file is renamed.
    public func move(from old: String, to new: String) {
        guard old != new else { return }
        var notes = load(path: old)
        guard !notes.isEmpty else { return }
        notes.path = new
        if (try? save(notes)) != nil { try? FileManager.default.removeItem(at: url(for: old)) }
    }

    // MARK: Live state

    public func writeLiveState(_ state: LiveState) {
        try? FileManager.default.createDirectory(at: supportDirectory, withIntermediateDirectories: true)
        guard let data = try? Self.encoder.encode(state) else { return }
        try? AtomicFile.write(data, to: liveStateURL)
    }

    public func clearLiveState() {
        try? FileManager.default.removeItem(at: liveStateURL)
    }

    /// The running app's state, or nil when it isn't running.
    public func liveState() -> LiveState? {
        guard let data = try? Data(contentsOf: liveStateURL),
              let state = try? Self.decoder.decode(LiveState.self, from: data) else { return nil }
        return kill(state.pid, 0) == 0 ? state : nil
    }

    // MARK: Inbox

    public func post(_ command: InboxCommand) throws {
        try FileManager.default.createDirectory(at: inboxDirectory, withIntermediateDirectories: true)
        try AtomicFile.write(try Self.encoder.encode(command), to: inboxDirectory.appendingPathComponent("\(command.id).json"))
    }

    /// Pending commands, oldest first.
    public func pendingCommands() -> [(URL, InboxCommand)] {
        let fm = FileManager.default
        guard let names = try? fm.contentsOfDirectory(atPath: inboxDirectory.path) else { return [] }
        return names.filter { $0.hasSuffix(".json") && !$0.hasPrefix(".") }.compactMap { name in
            let u = inboxDirectory.appendingPathComponent(name)
            guard let data = try? Data(contentsOf: u), let c = try? Self.decoder.decode(InboxCommand.self, from: data) else { return nil }
            return (u, c)
        }.sorted { $0.1.created < $1.1.created }
    }

    public func writeResult(_ result: InboxResult) {
        try? FileManager.default.createDirectory(at: resultsDirectory, withIntermediateDirectories: true)
        guard let data = try? Self.encoder.encode(result) else { return }
        try? AtomicFile.write(data, to: resultsDirectory.appendingPathComponent("\(result.id).json"))
    }

    /// Waits for the app to answer a command. Returns nil on timeout.
    public func awaitResult(id: String, timeout: TimeInterval) -> InboxResult? {
        let u = resultsDirectory.appendingPathComponent("\(id).json")
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if let data = try? Data(contentsOf: u), let r = try? Self.decoder.decode(InboxResult.self, from: data) {
                try? FileManager.default.removeItem(at: u)
                return r
            }
            usleep(50_000)
        }
        return nil
    }

    // MARK: Applying commands

    /// Applies a command to `notes`, given the draft's current text. Shared by the app and by
    /// outside agents when the app isn't running.
    public static func apply(_ command: InboxCommand, to notes: inout DocumentNotes, text: String) -> InboxResult {
        let ns = text as NSString
        do {
            switch command.kind {
            case .addComment:
                guard let quote = command.quote, !quote.isEmpty, let body = command.body, !body.isEmpty else {
                    throw DraftStoreError(message: "add_comment needs the quoted text and a comment.")
                }
                let c = try notes.addComment(quote: quote, occurrence: command.occurrence ?? 1, body: body, author: command.agent, in: ns)
                return InboxResult(id: command.id, ok: true, message: "Added comment \(c.id).", commentID: c.id)
            case .replyToComment, .resolveComment:
                guard let id = command.commentID, let i = notes.comments.firstIndex(where: { $0.id == id }) else {
                    throw DraftStoreError(message: "No comment with that id.")
                }
                if command.kind == .resolveComment {
                    notes.resolve([id], note: command.body, author: command.agent)
                    return InboxResult(id: command.id, ok: true, message: "Resolved comment \(id).", commentID: id)
                }
                guard let body = command.body, !body.isEmpty else { throw DraftStoreError(message: "The reply is empty.") }
                notes.comments[i].replies.append(CommentReply(author: command.agent, text: body))
                return InboxResult(id: command.id, ok: true, message: "Replied to comment \(id).", commentID: id)
            case .proposeVersion:
                let body: String
                var failures: [EditFailure] = []
                var placedIDs: [String] = []
                if let full = command.fullText {
                    body = full
                    placedIDs = (command.edits ?? []).flatMap(\.commentIDs)
                } else {
                    let placement = EditPlacer.place(command.edits ?? [], in: text)
                    failures = placement.failed
                    guard !placement.placed.isEmpty else {
                        throw DraftStoreError(message: failures.first?.reason ?? "The version has no edits.")
                    }
                    body = placement.applied(to: text)
                    placedIDs = placement.placed.flatMap { (command.edits ?? [])[$0.index].commentIDs }
                }
                guard body != text else { throw DraftStoreError(message: "The version is identical to the draft.") }
                // Only edits that landed resolve their comments.
                let ids = Array(Set((command.resolvesCommentIDs ?? []) + placedIDs))
                let v = DraftVersion(kind: .proposal, label: command.label ?? "\(command.agent) version", source: command.agent, body: body, feedback: command.body, resolvesCommentIDs: ids)
                notes.addVersion(v)
                var message = "Created version \(v.id). The author can preview it in Fluent Writer and choose whether to use it."
                if !failures.isEmpty {
                    message += " \(failures.count) edit\(failures.count == 1 ? "" : "s") couldn't be placed: " + failures.map { "#\($0.index + 1) \($0.reason)" }.joined(separator: " ")
                }
                return InboxResult(id: command.id, ok: true, message: message, versionID: v.id)
            }
        } catch {
            return InboxResult(id: command.id, ok: false, message: error.localizedDescription)
        }
    }
}

/// A change an outside agent asks the app to make.
public struct InboxCommand: Codable, Equatable, Sendable {
    public enum Kind: String, Codable, Sendable { case addComment, replyToComment, resolveComment, proposeVersion }

    public var id: String
    public var created: Date
    public var kind: Kind
    public var path: String
    public var agent: String
    public var quote: String?
    public var occurrence: Int?
    public var body: String?
    public var commentID: String?
    public var label: String?
    public var edits: [ProposedEdit]?
    public var fullText: String?
    public var resolvesCommentIDs: [String]?

    public init(kind: Kind, path: String, agent: String) {
        id = UUID().uuidString
        created = Date()
        self.kind = kind
        self.path = path
        self.agent = agent
    }
}

public struct InboxResult: Codable, Equatable, Sendable {
    public var id: String
    public var ok: Bool
    public var message: String
    public var commentID: String?
    public var versionID: String?

    public init(id: String, ok: Bool, message: String, commentID: String? = nil, versionID: String? = nil) {
        self.id = id
        self.ok = ok
        self.message = message
        self.commentID = commentID
        self.versionID = versionID
    }
}

/// What the running app is showing, for outside agents.
public struct LiveState: Codable, Equatable, Sendable {
    public var pid: Int32
    public var appName: String
    public var path: String?
    public var remote: String?
    public var title: String
    public var selectionLocation: Int
    public var selectionLength: Int
    public var selectedText: String
    public var previewingVersionID: String?
    public var updated: Date

    public init(pid: Int32, appName: String, path: String?, remote: String?, title: String, selection: NSRange, selectedText: String, previewingVersionID: String?, updated: Date = Date()) {
        self.pid = pid
        self.appName = appName
        self.path = path
        self.remote = remote
        self.title = title
        selectionLocation = selection.location
        selectionLength = selection.length
        self.selectedText = selectedText
        self.previewingVersionID = previewingVersionID
        self.updated = updated
    }
}
