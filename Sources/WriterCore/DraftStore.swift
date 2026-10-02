import Foundation

public struct DraftRecord: Codable, Equatable, Sendable {
    public var path: String
    public var title: String
    public var hasManualTitle: Bool
    public var selectionLocation: Int
    public var selectionLength: Int
    public var lastOpened: Date
}

struct LibraryState: Codable {
    var records: [String: DraftRecord] = [:]
    var lastOpenedPath: String?
}

/// A copy of unsaved text kept outside the draft file, used after a failed save or an interruption.
public struct RecoverySnapshot: Codable, Equatable, Sendable {
    public var draftID: UUID
    public var path: String?
    public var title: String
    public var hasManualTitle: Bool
    public var body: String
    public var selectionLocation: Int
    public var savedAt: Date
}

public struct DraftSummary: Equatable, Sendable {
    public var url: URL
    public var title: String
    public var modified: Date
    public var preview: String
}

public final class DraftDocument {
    public let id: UUID
    public internal(set) var url: URL?
    public var body: String
    public var title: String
    public var hasManualTitle: Bool
    public var selection: NSRange
    public internal(set) var savedBody: String
    /// Set when the body was restored from a recovery snapshot rather than the file.
    public internal(set) var recoveredAt: Date?

    init(id: UUID = UUID(), url: URL?, body: String, savedBody: String, title: String, hasManualTitle: Bool, selection: NSRange) {
        self.id = id
        self.url = url
        self.body = body
        self.savedBody = savedBody
        self.title = title
        self.hasManualTitle = hasManualTitle
        self.selection = selection
    }

    public var isDirty: Bool { body != savedBody }
    public var isEmptyUntitled: Bool { url == nil && body.isEmpty }

    /// Keeps the title in step with the first line until the writer names the draft.
    public func refreshDerivedTitle() {
        guard !hasManualTitle else { return }
        title = TitleDeriver.derivedTitle(from: body)
    }
}

public struct DraftStoreError: LocalizedError {
    public var message: String
    public var errorDescription: String? { message }
}

public final class DraftLibrary {
    public let draftsDirectory: URL
    public let supportDirectory: URL
    var recoveryDirectory: URL { supportDirectory.appendingPathComponent("Recovery", isDirectory: true) }
    var stateURL: URL { supportDirectory.appendingPathComponent("state.json") }
    private var state = LibraryState()
    public static let fileExtensions: Set<String> = ["md", "markdown", "txt", "text"]

    public init(draftsDirectory: URL, supportDirectory: URL) throws {
        self.draftsDirectory = draftsDirectory
        self.supportDirectory = supportDirectory
        let fm = FileManager.default
        try fm.createDirectory(at: draftsDirectory, withIntermediateDirectories: true)
        try fm.createDirectory(at: supportDirectory, withIntermediateDirectories: true)
        try fm.createDirectory(at: recoveryDirectory, withIntermediateDirectories: true)
        AtomicFile.removeStaleTemporaries(in: draftsDirectory)
        AtomicFile.removeStaleTemporaries(in: supportDirectory)
        AtomicFile.removeStaleTemporaries(in: recoveryDirectory)
        if let data = try? Data(contentsOf: stateURL), let decoded = try? JSONDecoder().decode(LibraryState.self, from: data) {
            state = decoded
        }
    }

    public static func defaultLibrary() throws -> DraftLibrary {
        let fm = FileManager.default
        let docs = fm.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("Fluent Writer", isDirectory: true)
        let support = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("Fluent Writer", isDirectory: true)
        return try DraftLibrary(draftsDirectory: docs, supportDirectory: support)
    }

    // MARK: Opening

    public func newDraft() -> DraftDocument {
        return DraftDocument(url: nil, body: "", savedBody: "", title: TitleDeriver.untitled, hasManualTitle: false, selection: NSRange(location: 0, length: 0))
    }

    public var lastOpenedURL: URL? {
        guard let p = state.lastOpenedPath, FileManager.default.fileExists(atPath: p) else { return nil }
        return URL(fileURLWithPath: p)
    }

    public func open(_ url: URL) throws -> DraftDocument {
        let standardized = url.standardizedFileURL
        let data: Data
        do {
            data = try Data(contentsOf: standardized)
        } catch {
            throw DraftStoreError(message: "Couldn't open “\(standardized.lastPathComponent)”: \(error.localizedDescription)")
        }
        guard let body = String(data: data, encoding: .utf8) ?? String(data: data, encoding: .isoLatin1) else {
            throw DraftStoreError(message: "“\(standardized.lastPathComponent)” isn't a text file.")
        }
        let record = state.records[standardized.path]
        let title = record?.title ?? standardized.deletingPathExtension().lastPathComponent
        let length = (body as NSString).length
        let loc = min(record?.selectionLocation ?? length, length)
        let len = min(record?.selectionLength ?? 0, length - loc)
        let doc = DraftDocument(url: standardized, body: body, savedBody: body, title: title, hasManualTitle: record?.hasManualTitle ?? false, selection: NSRange(location: loc, length: len))
        if let snapshot = recoverySnapshots().first(where: { $0.path == standardized.path }) {
            if snapshot.body != body {
                doc.body = snapshot.body
                doc.recoveredAt = snapshot.savedAt
                doc.title = snapshot.title
                doc.hasManualTitle = snapshot.hasManualTitle
                let l = (snapshot.body as NSString).length
                doc.selection = NSRange(location: min(snapshot.selectionLocation, l), length: 0)
            } else {
                discardRecovery(for: snapshot.draftID)
            }
        }
        markOpened(doc)
        return doc
    }

    /// Restores an unsaved draft that never reached a file.
    public func restore(_ snapshot: RecoverySnapshot) -> DraftDocument {
        let l = (snapshot.body as NSString).length
        let doc = DraftDocument(id: snapshot.draftID, url: nil, body: snapshot.body, savedBody: "", title: snapshot.title, hasManualTitle: snapshot.hasManualTitle, selection: NSRange(location: min(snapshot.selectionLocation, l), length: 0))
        doc.recoveredAt = snapshot.savedAt
        return doc
    }

    /// Recovery snapshots for drafts that were never saved to a file.
    public func orphanedRecoveries() -> [RecoverySnapshot] {
        return recoverySnapshots().filter { $0.path == nil || !FileManager.default.fileExists(atPath: $0.path!) }
    }

    func recoverySnapshots() -> [RecoverySnapshot] {
        let fm = FileManager.default
        guard let names = try? fm.contentsOfDirectory(atPath: recoveryDirectory.path) else { return [] }
        return names.filter { $0.hasSuffix(".json") }.compactMap { name in
            guard let data = try? Data(contentsOf: recoveryDirectory.appendingPathComponent(name)) else { return nil }
            return try? JSONDecoder().decode(RecoverySnapshot.self, from: data)
        }.sorted { $0.savedAt > $1.savedAt }
    }

    // MARK: Saving

    public enum SaveResult: Equatable {
        case unchanged
        case saved
    }

    /// Saves the document atomically. On failure, a recovery snapshot is written and the error is rethrown.
    @discardableResult
    public func save(_ doc: DraftDocument) throws -> SaveResult {
        if doc.isEmptyUntitled { return .unchanged }
        if doc.url != nil && !doc.isDirty && doc.recoveredAt == nil { return .unchanged }
        let body = doc.body
        do {
            if doc.url == nil {
                doc.url = uniqueURL(forTitle: doc.title, in: draftsDirectory, excluding: nil)
            }
            let url = doc.url!
            try AtomicFile.write(Data(body.utf8), to: url)
            doc.savedBody = body
            doc.recoveredAt = nil
            discardRecovery(for: doc.id)
            if let stale = recoverySnapshots().first(where: { $0.path == url.path }) { discardRecovery(for: stale.draftID) }
            renameToMatchDerivedTitleIfNeeded(doc)
            markOpened(doc)
            return .saved
        } catch {
            try? writeRecovery(doc)
            throw error
        }
    }

    public func writeRecovery(_ doc: DraftDocument) throws {
        let snapshot = RecoverySnapshot(draftID: doc.id, path: doc.url?.path, title: doc.title, hasManualTitle: doc.hasManualTitle, body: doc.body, selectionLocation: doc.selection.location, savedAt: Date())
        let data = try JSONEncoder().encode(snapshot)
        try AtomicFile.write(data, to: recoveryDirectory.appendingPathComponent("\(doc.id.uuidString).json"))
    }

    public func discardRecovery(for id: UUID) {
        try? FileManager.default.removeItem(at: recoveryDirectory.appendingPathComponent("\(id.uuidString).json"))
    }

    public func hasRecovery(for doc: DraftDocument) -> Bool {
        return FileManager.default.fileExists(atPath: recoveryDirectory.appendingPathComponent("\(doc.id.uuidString).json").path)
    }

    /// Untitled drafts inside the drafts folder follow their first line; files elsewhere keep their name.
    func renameToMatchDerivedTitleIfNeeded(_ doc: DraftDocument) {
        guard !doc.hasManualTitle, let url = doc.url, isInDraftsDirectory(url) else { return }
        let desiredStem = TitleDeriver.fileNameStem(for: doc.title)
        let currentStem = url.deletingPathExtension().lastPathComponent
        if currentStem == desiredStem { return }
        if let match = currentStem.range(of: #" \d+$"#, options: .regularExpression), String(currentStem[..<match.lowerBound]) == desiredStem { return }
        let target = uniqueURL(forTitle: doc.title, in: url.deletingLastPathComponent(), excluding: url)
        move(doc, to: target)
    }

    public func rename(_ doc: DraftDocument, to newTitle: String) throws {
        let trimmed = newTitle.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            doc.hasManualTitle = false
            doc.refreshDerivedTitle()
        } else {
            doc.title = trimmed
            doc.hasManualTitle = true
        }
        guard let url = doc.url else { return }
        let target = uniqueURL(forTitle: doc.title, in: url.deletingLastPathComponent(), excluding: url, pathExtension: url.pathExtension)
        if target != url {
            do {
                try FileManager.default.moveItem(at: url, to: target)
            } catch {
                throw DraftStoreError(message: "Couldn't rename “\(url.lastPathComponent)”: \(error.localizedDescription)")
            }
            forget(url)
            doc.url = target
        }
        markOpened(doc)
    }

    private func move(_ doc: DraftDocument, to target: URL) {
        guard let url = doc.url, target != url else { return }
        if (try? FileManager.default.moveItem(at: url, to: target)) != nil {
            forget(url)
            doc.url = target
        }
    }

    func isInDraftsDirectory(_ url: URL) -> Bool {
        return url.deletingLastPathComponent().standardizedFileURL.path == draftsDirectory.standardizedFileURL.path
    }

    func uniqueURL(forTitle title: String, in directory: URL, excluding: URL?, pathExtension: String = "md") -> URL {
        let stem = TitleDeriver.fileNameStem(for: title)
        let ext = pathExtension.isEmpty ? "md" : pathExtension
        var candidate = directory.appendingPathComponent(stem).appendingPathExtension(ext)
        var n = 2
        while FileManager.default.fileExists(atPath: candidate.path), candidate.standardizedFileURL != excluding?.standardizedFileURL {
            candidate = directory.appendingPathComponent("\(stem) \(n)").appendingPathExtension(ext)
            n += 1
        }
        return candidate.standardizedFileURL
    }

    // MARK: State

    public func markOpened(_ doc: DraftDocument) {
        guard let url = doc.url else { return }
        state.records[url.path] = DraftRecord(path: url.path, title: doc.title, hasManualTitle: doc.hasManualTitle, selectionLocation: doc.selection.location, selectionLength: doc.selection.length, lastOpened: Date())
        state.lastOpenedPath = url.path
        persistState()
    }

    /// Remembers the caret without touching the draft file.
    public func rememberSelection(_ doc: DraftDocument) {
        guard let url = doc.url, var record = state.records[url.path] else { return }
        record.selectionLocation = doc.selection.location
        record.selectionLength = doc.selection.length
        record.title = doc.title
        record.hasManualTitle = doc.hasManualTitle
        state.records[url.path] = record
        persistState()
    }

    private func forget(_ url: URL) {
        state.records.removeValue(forKey: url.path)
    }

    func persistState() {
        guard let data = try? JSONEncoder().encode(state) else { return }
        try? AtomicFile.write(data, to: stateURL)
    }

    // MARK: Listing

    public func recentDrafts(limit: Int = 200) -> [DraftSummary] {
        let fm = FileManager.default
        var urls = Set<URL>()
        if let names = try? fm.contentsOfDirectory(atPath: draftsDirectory.path) {
            for name in names where !name.hasPrefix(".") {
                let url = draftsDirectory.appendingPathComponent(name).standardizedFileURL
                if DraftLibrary.fileExtensions.contains(url.pathExtension.lowercased()) { urls.insert(url) }
            }
        }
        for path in state.records.keys where fm.fileExists(atPath: path) {
            urls.insert(URL(fileURLWithPath: path).standardizedFileURL)
        }
        var summaries: [DraftSummary] = []
        for url in urls {
            let attrs = try? fm.attributesOfItem(atPath: url.path)
            let modified = (attrs?[.modificationDate] as? Date) ?? .distantPast
            let record = state.records[url.path]
            let opened = record?.lastOpened ?? .distantPast
            let preview = previewText(url)
            summaries.append(DraftSummary(url: url, title: record?.title ?? url.deletingPathExtension().lastPathComponent, modified: max(modified, opened), preview: preview))
        }
        summaries.sort { $0.modified > $1.modified }
        return Array(summaries.prefix(limit))
    }

    private func previewText(_ url: URL) -> String {
        guard let handle = try? FileHandle(forReadingFrom: url) else { return "" }
        defer { try? handle.close() }
        let data = (try? handle.read(upToCount: 600)) ?? Data()
        let text = String(decoding: data, as: UTF8.self)
        let plain = PlainTextExporter.export(text)
        return plain.split(whereSeparator: { $0.isNewline }).dropFirst().joined(separator: " ").prefix(140).description
    }
}
