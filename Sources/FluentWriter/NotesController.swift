import AppKit
import WriterCore

/// Comments, versions and the Assist transcript for the open draft. Also publishes what the app is
/// showing for outside agents and applies the commands they leave in the inbox.
final class NotesController {
    weak var editor: EditorController?
    let store: NotesStore
    private(set) var notes = DocumentNotes(path: "")
    /// The path `notes` is filed under; nil while the draft has never been saved.
    private var path: String?
    /// The draft `notes` belong to. Path changes only carry notes along for this same draft.
    private var documentID: UUID?
    var activeCommentID: String? { didSet { if oldValue != activeCommentID { refreshHighlights() } } }
    /// Called whenever comments or versions change, so panels can refresh.
    var onChange: (() -> Void)?

    private var saveTimer: Timer?
    private var liveTimer: Timer?
    private var inboxSource: DispatchSourceFileSystemObject?
    private var inboxFD: Int32 = -1

    init(editor: EditorController, supportDirectory: URL) {
        self.editor = editor
        store = NotesStore(supportDirectory: supportDirectory)
        startInbox()
    }

    deinit {
        inboxSource?.cancel()
    }

    // MARK: Document lifecycle

    /// Call after the editor shows a new draft. The previous draft's notes must already be flushed.
    func documentLoaded() {
        saveTimer?.invalidate()
        saveTimer = nil
        guard let editor else { return }
        documentID = editor.draft.id
        path = editor.draft.url?.path
        notes = path.map { store.load(path: $0) } ?? DocumentNotes(path: "")
        let text = editor.textView.string as NSString
        for i in notes.comments.indices { notes.comments[i].reanchor(in: text) }
        activeCommentID = nil
        refreshHighlights()
        changed(save: false)
        scheduleLiveState()
        processInbox()
    }

    /// Files the notes under the draft's current path after a first save or a rename.
    func syncPath() {
        guard let editor, editor.draft.id == documentID, let newPath = editor.draft.url?.path, newPath != path else { return }
        if let old = path {
            flushSave()
            store.move(from: old, to: newPath)
        }
        path = newPath
        notes.path = newPath
        scheduleSave()
        scheduleLiveState()
    }

    func noteEdit(editedRange: NSRange, changeInLength delta: Int) {
        guard !notes.comments.isEmpty else { return }
        var anyDetached = false
        for i in notes.comments.indices {
            let was = notes.comments[i].detached
            notes.comments[i].noteEdit(editedRange: editedRange, changeInLength: delta)
            if notes.comments[i].detached && !was { anyDetached = true }
        }
        scheduleSave()
        if anyDetached { onChange?() }
    }

    private func changed(save: Bool = true) {
        if save { scheduleSave() }
        onChange?()
    }

    private func scheduleSave() {
        saveTimer?.invalidate()
        saveTimer = Timer.scheduledTimer(withTimeInterval: 0.6, repeats: false) { [weak self] _ in self?.flushSave() }
    }

    func flushSave() {
        saveTimer?.invalidate()
        saveTimer = nil
        guard let path, let editor, editor.draft.id == documentID else { return }
        let text = editor.textView.string as NSString
        for i in notes.comments.indices { notes.comments[i].capture(from: text) }
        notes.path = path
        let snapshot = notes
        store.withLock { try? store.save(snapshot) }
    }

    /// Detached comments come back when their text does, e.g. after undoing a deletion.
    func reanchorDetached() {
        guard let editor, notes.comments.contains(where: { $0.detached && $0.isOpen }) else { return }
        let text = editor.textView.string as NSString
        var any = false
        for i in notes.comments.indices where notes.comments[i].detached && notes.comments[i].isOpen {
            if notes.comments[i].reanchor(in: text) { any = true }
        }
        if any { refreshHighlights(); changed() }
    }

    // MARK: Comments

    var openComments: [Comment] { notes.comments.filter(\.isOpen) }
    var resolvedComments: [Comment] { notes.comments.filter { !$0.isOpen } }

    func comment(_ id: String) -> Comment? { notes.comments.first { $0.id == id } }

    @discardableResult
    func addComment(range: NSRange, body: String, author: String = "you") -> Comment? {
        guard let editor else { return nil }
        let text = editor.textView.string as NSString
        guard range.length > 0, NSMaxRange(range) <= text.length else { return nil }
        let c = Comment(range: range, in: text, body: body, author: author)
        notes.comments.append(c)
        notes.comments.sort { $0.location < $1.location }
        refreshHighlights()
        changed()
        scheduleLiveState()
        return c
    }

    func updateComment(_ id: String, body: String) {
        guard let i = notes.comments.firstIndex(where: { $0.id == id }) else { return }
        notes.comments[i].body = body
        changed()
    }

    func deleteComment(_ id: String) {
        notes.comments.removeAll { $0.id == id }
        if activeCommentID == id { activeCommentID = nil }
        refreshHighlights()
        changed()
    }

    func setResolved(_ id: String, _ resolved: Bool) {
        guard let i = notes.comments.firstIndex(where: { $0.id == id }) else { return }
        notes.comments[i].resolved = resolved ? Date() : nil
        refreshHighlights()
        changed()
    }

    func addReply(to id: String, author: String, text: String) {
        guard let i = notes.comments.firstIndex(where: { $0.id == id }) else { return }
        notes.comments[i].replies.append(CommentReply(author: author, text: text))
        changed()
    }

    /// The open comment covering `location`, preferring the shortest span.
    func comment(at location: Int) -> Comment? {
        openComments.filter { !$0.detached && $0.location <= location && location <= NSMaxRange($0.range) }
            .min { $0.length < $1.length }
    }

    func refreshHighlights() {
        guard let editor, let lm = editor.textView.layoutManager, let storage = editor.textView.textStorage else { return }
        let full = NSRange(location: 0, length: storage.length)
        lm.removeTemporaryAttribute(.backgroundColor, forCharacterRange: full)
        for c in openComments where !c.detached {
            let r = NSIntersectionRange(c.range, full)
            guard r.length > 0 else { continue }
            lm.addTemporaryAttribute(.backgroundColor, value: c.id == activeCommentID ? Theme.commentHighlightActive : Theme.commentHighlight, forCharacterRange: r)
        }
    }

    // MARK: Versions

    var versions: [DraftVersion] { notes.versions }
    func version(_ id: String) -> DraftVersion? { notes.versions.first { $0.id == id } }

    func addVersion(_ v: DraftVersion) {
        notes.addVersion(v)
        changed()
    }

    func deleteVersion(_ id: String) {
        notes.versions.removeAll { $0.id == id }
        changed()
    }

    /// Keeps the current draft as a snapshot unless an identical version already exists.
    func snapshotCurrentDraft() {
        guard let editor else { return }
        let body = editor.textView.string
        guard !body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, !notes.versions.contains(where: { $0.body == body }) else { return }
        let f = DateFormatter()
        f.dateStyle = .none
        f.timeStyle = .short
        notes.addVersion(DraftVersion(kind: .snapshot, label: "Your draft · \(f.string(from: Date()))", source: "you", body: body))
    }

    func markUsed(_ id: String) {
        guard let i = notes.versions.firstIndex(where: { $0.id == id }) else { return }
        notes.versions[i].usedAt = Date()
        let resolves = notes.versions[i].resolvesCommentIDs
        if !resolves.isEmpty { notes.resolve(resolves, note: nil, author: notes.versions[i].source) }
        refreshHighlights()
        changed()
    }

    /// Settles comment anchors after a version replaced the text. Spans tracked through the edits keep
    /// their place and take on the new wording; only lost ones are searched for by quote. When the
    /// text was replaced wholesale (`tracked` false) every span is searched for.
    func settleAfterRewrite(tracked: Bool) {
        guard let editor else { return }
        let text = editor.textView.string as NSString
        for i in notes.comments.indices where notes.comments[i].isOpen {
            let c = notes.comments[i]
            if tracked && !c.detached && c.length > 0 && NSMaxRange(c.range) <= text.length {
                notes.comments[i].capture(from: text)
            } else {
                notes.comments[i].reanchor(in: text)
            }
        }
        refreshHighlights()
        changed()
    }

    // MARK: Transcript

    var transcript: [AssistExchange] { notes.transcript }

    func addExchange(_ e: AssistExchange) {
        notes.addExchange(e)
        changed()
    }

    // MARK: Live state

    func scheduleLiveState() {
        liveTimer?.invalidate()
        liveTimer = Timer.scheduledTimer(withTimeInterval: 0.4, repeats: false) { [weak self] _ in self?.writeLiveState() }
    }

    func writeLiveState() {
        guard let editor else { return }
        let ns = editor.textView.string as NSString
        let sel = editor.textView.selectedRange()
        let selected = sel.length > 0 && NSMaxRange(sel) <= ns.length ? ns.substring(with: sel) : ""
        store.writeLiveState(LiveState(
            pid: ProcessInfo.processInfo.processIdentifier, appName: AppVariant.current.appName,
            path: editor.draft.url?.path, remote: editor.draft.remote?.display, title: editor.draft.title,
            selection: sel, selectedText: selected, previewingVersionID: editor.preview.versionID
        ))
    }

    func appWillTerminate() {
        flushSave()
        store.clearLiveState()
    }

    // MARK: Inbox

    private func startInbox() {
        try? FileManager.default.createDirectory(at: store.inboxDirectory, withIntermediateDirectories: true)
        inboxFD = open(store.inboxDirectory.path, O_EVTONLY)
        guard inboxFD >= 0 else { return }
        let source = DispatchSource.makeFileSystemObjectSource(fileDescriptor: inboxFD, eventMask: [.write, .rename], queue: .main)
        source.setEventHandler { [weak self] in self?.processInbox() }
        let fd = inboxFD
        source.setCancelHandler { close(fd) }
        source.resume()
        inboxSource = source
    }

    func processInbox() {
        guard let editor else { return }
        for (url, command) in store.pendingCommands() {
            try? FileManager.default.removeItem(at: url)
            let target = URL(fileURLWithPath: command.path).standardizedFileURL.path
            let result: InboxResult
            if target == path {
                // The open draft: apply to the live text and notes.
                let wasVersions = notes.versions.count
                result = NotesStore.apply(command, to: &notes, text: editor.textView.string)
                if result.ok {
                    refreshHighlights()
                    changed()
                    flushSave()
                    if command.kind == .proposeVersion, notes.versions.count > wasVersions {
                        editor.flashMessage("\(command.agent) proposed a version · ⌥⌘V to see it")
                        editor.assist?.refreshLists()
                    } else if command.kind == .addComment {
                        editor.flashMessage("\(command.agent) added a comment")
                    }
                }
            } else {
                guard let text = try? String(contentsOfFile: target, encoding: .utf8) else {
                    store.writeResult(InboxResult(id: command.id, ok: false, message: "Couldn't read \(target)."))
                    continue
                }
                result = store.withLock {
                    var other = store.load(path: target)
                    let r = NotesStore.apply(command, to: &other, text: text)
                    if r.ok { try? store.save(other) }
                    return r
                }
            }
            store.writeResult(result)
        }
    }
}
