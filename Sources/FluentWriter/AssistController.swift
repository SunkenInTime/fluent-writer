import AppKit
import WriterAgents
import WriterCore

/// Flipped container so the assist panel lays out top-down.
final class AssistPanel: NSView {
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        Theme.canvas.setFill()
        bounds.fill()
        Theme.hairline.setFill()
        NSRect(x: 0, y: 0, width: 1, height: bounds.height).fill()
    }
}

final class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

/// A clickable card in the comment and version lists.
final class CardView: NSView {
    var onClick: (() -> Void)?
    var isSelected = false { didSet { needsDisplay = true } }
    var isDimmed = false { didSet { alphaValue = isDimmed ? 0.6 : 1 } }
    private var hovering = false { didSet { needsDisplay = true } }
    private var tracking: NSTrackingArea?

    override var isFlipped: Bool { true }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        let t = NSTrackingArea(rect: bounds, options: [.mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect], owner: self)
        addTrackingArea(t)
        tracking = t
    }

    override func mouseEntered(with event: NSEvent) { hovering = true }
    override func mouseExited(with event: NSEvent) { hovering = false }

    override func mouseUp(with event: NSEvent) {
        if bounds.contains(convert(event.locationInWindow, from: nil)) { onClick?() }
    }

    override func draw(_ dirtyRect: NSRect) {
        let path = NSBezierPath(roundedRect: bounds.insetBy(dx: 0.5, dy: 0.5), xRadius: 8, yRadius: 8)
        (isSelected ? Theme.cardSelected : hovering ? Theme.cardHover : Theme.card).setFill()
        path.fill()
        (isSelected ? Theme.caret.withAlphaComponent(0.5) : Theme.hairline).setStroke()
        path.lineWidth = 1
        path.stroke()
    }

    override func accessibilityRole() -> NSAccessibility.Role? { .button }
    override func accessibilityPerformPress() -> Bool { onClick?(); return true }
    override func isAccessibilityElement() -> Bool { true }
}

/// Vertical list of cards that lays itself out to a fixed width.
final class CardList: NSScrollView {
    private let content = FlippedView()
    private var cards: [NSView] = []

    init() {
        super.init(frame: .zero)
        documentView = content
        drawsBackground = false
        hasVerticalScroller = true
        autohidesScrollers = true
        scrollerStyle = .overlay
        borderType = .noBorder
    }

    required init?(coder: NSCoder) { fatalError() }

    func set(_ views: [NSView]) {
        cards.forEach { $0.removeFromSuperview() }
        cards = views
        views.forEach(content.addSubview)
        relayout()
    }

    /// Each card reports its height for a width through `layoutCard`.
    var layoutCard: ((NSView, CGFloat) -> CGFloat)?

    func relayout() {
        let w = contentSize.width
        var y: CGFloat = 0
        for c in cards {
            let h = layoutCard?(c, w) ?? 40
            c.frame = NSRect(x: 0, y: y, width: w, height: h)
            y += h + 8
        }
        content.frame = NSRect(x: 0, y: 0, width: w, height: max(y, contentSize.height))
    }
}

/// Side panel for writing help, comments and versions. Nothing leaves the Mac until Send is pressed,
/// and agent edits only reach the draft when you use the version they produce.
final class AssistController: NSObject, NSTextFieldDelegate {
    enum Tab: Int { case ask, comments, versions }

    weak var editor: EditorController?
    let panel = AssistPanel()
    private(set) var tab: Tab = .ask

    private let providerPopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let closeButton = NSButton()
    private let tabs = NSSegmentedControl(labels: ["Ask", "Comments", "Versions"], trackingMode: .selectOne, target: nil, action: nil)

    // Ask
    private let askViews = FlippedView()
    private let scopeLabel = NSTextField(labelWithString: "")
    private let privacyLabel = NSTextField(wrappingLabelWithString: "")
    private var actionButtons: [NSButton] = []
    private let instructionField = NSTextField()
    private let sendButton = NSButton(title: "Send", target: nil, action: nil)
    private let statusLabel = NSTextField(wrappingLabelWithString: "")
    private let feedbackHeader = NSTextField(labelWithString: "Feedback")
    private let feedbackView = NSTextView()
    private let feedbackScroll = NSScrollView()
    private let previewButton = NSButton(title: "Preview Version", target: nil, action: nil)

    // Comments
    private let commentViews = FlippedView()
    private let commentsHeader = NSTextField(wrappingLabelWithString: "")
    private let addCommentButton = NSButton(title: "Add Comment", target: nil, action: nil)
    private let showResolvedButton = NSButton(checkboxWithTitle: "Show resolved", target: nil, action: nil)
    private let commentList = CardList()
    private let sendCommentsButton = NSButton(title: "Send Comments", target: nil, action: nil)
    private let commentsEmpty = NSTextField(wrappingLabelWithString: "")

    // Versions
    private let versionViews = FlippedView()
    private let versionsHeader = NSTextField(wrappingLabelWithString: "")
    private let versionList = CardList()

    private var provider: AgentProvider = AgentProvider(rawValue: UserDefaults.standard.string(forKey: "assist.provider") ?? "") ?? .codex
    private var session: AgentSession?
    private var generation = 0
    private var running = false
    private var lastVersionID: String?
    private var showResolved = false

    /// What the in-flight request was about, kept to place its edits when the reply arrives.
    private struct Sent {
        var action: AgentAction
        var instruction: String
        var selection: NSRange?
        var basedOnVersionID: String?
        var commentIDs: [String]
    }
    private var sent: Sent?

    init(editor: EditorController) {
        self.editor = editor
        super.init()
        build()
        refreshScope()
        updateControls()
    }

    private var notes: NotesController? { editor?.notes }

    // MARK: Building

    private func label(_ f: NSTextField, size: CGFloat = 12, color: NSColor = Theme.quiet) {
        f.font = Theme.uiFont(size: size)
        f.textColor = color
        f.isSelectable = false
    }

    private func smallButton(_ b: NSButton, _ action: Selector) {
        b.target = self
        b.action = action
        b.bezelStyle = .rounded
        b.controlSize = .small
        b.font = Theme.uiFont(size: 11)
    }

    private func build() {
        panel.setAccessibilityRole(.group)
        panel.setAccessibilityLabel("Assist")

        providerPopup.addItems(withTitles: AgentProvider.allCases.map(\.displayName))
        providerPopup.selectItem(at: AgentProvider.allCases.firstIndex(of: provider) ?? 0)
        providerPopup.target = self
        providerPopup.action = #selector(providerChanged)
        providerPopup.font = Theme.uiFont(size: 12)
        providerPopup.isBordered = false
        providerPopup.setAccessibilityLabel("Provider")

        closeButton.title = "Close"
        closeButton.isBordered = false
        closeButton.font = Theme.uiFont(size: 12)
        closeButton.contentTintColor = Theme.quiet
        closeButton.target = self
        closeButton.action = #selector(closeClicked)
        closeButton.setAccessibilityLabel("Close assist")

        tabs.target = self
        tabs.action = #selector(tabChanged)
        tabs.selectedSegment = 0
        tabs.controlSize = .small
        tabs.font = Theme.uiFont(size: 11)
        tabs.segmentDistribution = .fillEqually
        tabs.setAccessibilityLabel("Assist section")

        for v: NSView in [providerPopup, closeButton, tabs, askViews, commentViews, versionViews] { panel.addSubview(v) }

        // Ask
        label(scopeLabel, color: Theme.text)
        scopeLabel.lineBreakMode = .byTruncatingTail
        label(privacyLabel, size: 11)
        privacyLabel.stringValue = "Nothing is sent until you press Send. The agent gets this draft, your selection and comments, with no files, commands or web access."
        for action in AgentAction.quickActions {
            let b = NSButton(title: action.label, target: self, action: #selector(actionClicked(_:)))
            b.bezelStyle = .recessed
            b.font = Theme.uiFont(size: 12)
            b.tag = AgentAction.allCases.firstIndex(of: action) ?? 0
            b.setAccessibilityLabel("\(action.label) with the selected provider")
            actionButtons.append(b)
            askViews.addSubview(b)
        }
        instructionField.placeholderString = "Ask anything, or say what to change or add"
        instructionField.font = Theme.uiFont(size: 13)
        instructionField.focusRingType = .none
        instructionField.bezelStyle = .roundedBezel
        instructionField.delegate = self
        instructionField.target = self
        instructionField.action = #selector(sendCustom)
        instructionField.cell?.wraps = true
        instructionField.cell?.isScrollable = false
        instructionField.usesSingleLineMode = false
        instructionField.setAccessibilityLabel("Instruction")
        sendButton.target = self
        sendButton.action = #selector(sendOrStop)
        sendButton.bezelStyle = .rounded
        sendButton.keyEquivalent = ""
        sendButton.font = Theme.uiFont(size: 12)
        label(statusLabel)
        label(feedbackHeader, size: 11)
        feedbackView.isEditable = false
        feedbackView.isSelectable = true
        feedbackView.drawsBackground = false
        feedbackView.textContainerInset = NSSize(width: 0, height: 4)
        feedbackView.textContainer?.lineFragmentPadding = 0
        feedbackView.isVerticallyResizable = true
        feedbackView.autoresizingMask = [.width]
        feedbackView.textContainer?.widthTracksTextView = true
        feedbackView.setAccessibilityLabel("Feedback")
        feedbackScroll.documentView = feedbackView
        feedbackScroll.drawsBackground = false
        feedbackScroll.hasVerticalScroller = true
        feedbackScroll.autohidesScrollers = true
        feedbackScroll.borderType = .noBorder
        smallButton(previewButton, #selector(previewClicked))
        for v: NSView in [scopeLabel, privacyLabel, instructionField, sendButton, statusLabel, feedbackHeader, feedbackScroll, previewButton] {
            askViews.addSubview(v)
        }

        // Comments
        label(commentsHeader, size: 11)
        smallButton(addCommentButton, #selector(addCommentClicked))
        addCommentButton.toolTip = "Comment on the selection or the current line (⌥⌘M)"
        showResolvedButton.target = self
        showResolvedButton.action = #selector(toggleResolved)
        showResolvedButton.font = Theme.uiFont(size: 11)
        showResolvedButton.controlSize = .small
        smallButton(sendCommentsButton, #selector(sendCommentsClicked))
        sendCommentsButton.font = Theme.uiFont(size: 12)
        sendCommentsButton.controlSize = .regular
        label(commentsEmpty)
        commentsEmpty.alignment = .center
        commentsEmpty.stringValue = "Select some text, or put the caret on a line, and press ⌥⌘M to leave a comment.\n\nWrite as many as you like, then send them all at once. The agent's fixes come back as a version you can preview."
        commentList.layoutCard = { [weak self] v, w in self?.layoutCommentCard(v, width: w) ?? 40 }
        for v: NSView in [commentsHeader, addCommentButton, showResolvedButton, commentList, sendCommentsButton, commentsEmpty] { commentViews.addSubview(v) }

        // Versions
        label(versionsHeader, size: 11)
        versionsHeader.stringValue = "Each agent reply becomes a version. Click one to preview it as a diff, then use it or go back. Using one keeps your current draft here too."
        versionList.layoutCard = { [weak self] v, w in self?.layoutVersionCard(v, width: w) ?? 40 }
        for v: NSView in [versionsHeader, versionList] { versionViews.addSubview(v) }

        NotificationCenter.default.addObserver(self, selector: #selector(layout), name: NSView.frameDidChangeNotification, object: panel)
        panel.postsFrameChangedNotifications = true
        select(.ask)
    }

    // MARK: Layout

    @objc func layout() {
        let w = panel.bounds.width
        let pad: CGFloat = 22
        let inner = w - pad * 2
        providerPopup.frame = NSRect(x: pad - 4, y: 40, width: 120, height: 24)
        closeButton.frame = NSRect(x: w - pad - 50, y: 42, width: 50, height: 20)
        tabs.frame = NSRect(x: pad, y: 72, width: inner, height: 22)
        let content = NSRect(x: 0, y: 106, width: w, height: max(0, panel.bounds.height - 106))
        for v in [askViews, commentViews, versionViews] { v.frame = content }
        switch tab {
        case .ask: layoutAsk(width: w, height: content.height, pad: pad)
        case .comments: layoutComments(width: w, height: content.height, pad: pad)
        case .versions: layoutVersions(width: w, height: content.height, pad: pad)
        }
    }

    private func wrappedHeight(_ f: NSTextField, _ width: CGFloat) -> CGFloat {
        guard !f.stringValue.isEmpty else { return 0 }
        return ceil(f.cell?.cellSize(forBounds: NSRect(x: 0, y: 0, width: width, height: 1000)).height ?? 16)
    }

    private func layoutAsk(width w: CGFloat, height h: CGFloat, pad: CGFloat) {
        let inner = w - pad * 2
        var y: CGFloat = 0
        scopeLabel.frame = NSRect(x: pad, y: y, width: inner, height: 18)
        y += 22
        let privacyH = wrappedHeight(privacyLabel, inner)
        privacyLabel.frame = NSRect(x: pad, y: y, width: inner, height: privacyH)
        y += privacyH + 14
        var x = pad
        for b in actionButtons {
            b.sizeToFit()
            let bw = b.frame.width + 8
            if x + bw > w - pad { x = pad; y += 28 }
            b.frame = NSRect(x: x, y: y, width: bw, height: 22)
            x += bw + 6
        }
        y += 34
        instructionField.frame = NSRect(x: pad, y: y, width: inner - 74, height: 44)
        sendButton.frame = NSRect(x: w - pad - 66, y: y + 8, width: 66, height: 28)
        y += 54
        let statusH = wrappedHeight(statusLabel, inner)
        statusLabel.frame = NSRect(x: pad, y: y, width: inner, height: statusH)
        if statusH > 0 { y += statusH + 8 }
        let showsPreview = lastVersionID != nil && notes?.version(lastVersionID!) != nil
        previewButton.isHidden = !showsPreview
        if showsPreview {
            previewButton.sizeToFit()
            previewButton.frame = NSRect(x: pad, y: y, width: previewButton.frame.width + 10, height: 24)
            y += 34
        }
        feedbackHeader.isHidden = feedbackView.string.isEmpty && !running
        feedbackHeader.frame = NSRect(x: pad, y: y, width: inner, height: 16)
        feedbackScroll.frame = NSRect(x: pad, y: y + 18, width: inner, height: max(0, h - y - 18 - 20))
    }

    private func layoutComments(width w: CGFloat, height h: CGFloat, pad: CGFloat) {
        let inner = w - pad * 2
        var y: CGFloat = 0
        addCommentButton.sizeToFit()
        addCommentButton.frame = NSRect(x: pad, y: y, width: addCommentButton.frame.width + 10, height: 24)
        showResolvedButton.sizeToFit()
        showResolvedButton.frame = NSRect(x: w - pad - showResolvedButton.frame.width, y: y + 3, width: showResolvedButton.frame.width, height: 18)
        y += 32
        let headerH = wrappedHeight(commentsHeader, inner)
        commentsHeader.frame = NSRect(x: pad, y: y, width: inner, height: headerH)
        if headerH > 0 { y += headerH + 8 }
        let bottom: CGFloat = 52
        sendCommentsButton.frame = NSRect(x: pad, y: h - bottom + 6, width: inner, height: 30)
        commentList.frame = NSRect(x: pad, y: y, width: inner, height: max(0, h - y - bottom))
        commentList.relayout()
        commentsEmpty.frame = NSRect(x: pad + 8, y: y + 30, width: inner - 16, height: wrappedHeight(commentsEmpty, inner - 16))
    }

    private func layoutVersions(width w: CGFloat, height h: CGFloat, pad: CGFloat) {
        let inner = w - pad * 2
        let headerH = wrappedHeight(versionsHeader, inner)
        versionsHeader.frame = NSRect(x: pad, y: 0, width: inner, height: headerH)
        let y = headerH + 12
        versionList.frame = NSRect(x: pad, y: y, width: inner, height: max(0, h - y - 16))
        versionList.relayout()
    }

    // MARK: Lifecycle

    func show(provider p: AgentProvider?, tab t: Tab? = nil) {
        if let p { setProvider(p) }
        guard let editor else { return }
        if panel.superview == nil {
            editor.makeRoomForPanel()
            editor.canvas.addSubview(panel)
            editor.frameChanged()
        }
        select(t ?? (p != nil ? .ask : tab))
        refreshScope()
        refreshLists()
        layout()
        editor.updateToolbarState()
        if tab == .ask { editor.window?.makeFirstResponder(instructionField) }
    }

    func close() {
        stop()
        panel.removeFromSuperview()
        guard let editor else { return }
        editor.frameChanged()
        editor.updateToolbarState()
        editor.window?.makeFirstResponder(editor.preview.isActive ? editor.preview.textView : editor.textView)
    }

    func stop() {
        guard running else { return }
        generation += 1
        session?.cancel()
        session = nil
        running = false
        sent = nil
        statusLabel.stringValue = "Stopped. Nothing was changed."
        updateControls()
        layout()
    }

    private func select(_ t: Tab) {
        tab = t
        tabs.selectedSegment = t.rawValue
        askViews.isHidden = t != .ask
        commentViews.isHidden = t != .comments
        versionViews.isHidden = t != .versions
        layout()
    }

    @objc private func tabChanged() {
        select(Tab(rawValue: tabs.selectedSegment) ?? .ask)
        refreshLists()
        editor?.updateToolbarState()
    }

    // MARK: Draft tracking

    func documentChanged() {
        stop()
        clearResult()
        refreshScope()
        refreshLists()
    }

    func selectionChanged() {
        if !running { refreshScope() }
        if tab == .comments, let editor {
            let id = notes?.comment(at: editor.textView.selectedRange().location)?.id
            if id != notes?.activeCommentID { notes?.activeCommentID = id; refreshLists() }
        }
    }

    private func refreshScope() {
        let name = provider.displayName
        guard let editor else { return }
        if let id = editor.preview.versionID, let v = notes?.version(id) {
            let words = TextStats.counts(forMarkdown: v.body).words
            scopeLabel.stringValue = "Version “\(v.label)” · \(words) words → \(name)"
            scopeLabel.setAccessibilityLabel("Will send the previewed version, \(words) words, to \(name)")
            return
        }
        let ns = editor.textView.string as NSString
        guard ns.length > 0 else { scopeLabel.stringValue = "Write something first"; return }
        let sel = editor.textView.selectedRange()
        let total = TextStats.counts(forMarkdown: ns as String).words
        let selectedWords = sel.length > 0 && NSMaxRange(sel) <= ns.length ? TextStats.counts(forMarkdown: ns.substring(with: sel)).words : 0
        if selectedWords > 0 {
            let words = selectedWords
            scopeLabel.stringValue = "Draft · focus on \(words) selected \(words == 1 ? "word" : "words") → \(name)"
            scopeLabel.setAccessibilityLabel("Will send the draft, \(total) words, focused on \(words) selected words, to \(name)")
        } else {
            scopeLabel.stringValue = "Full draft · \(total) \(total == 1 ? "word" : "words") → \(name)"
            scopeLabel.setAccessibilityLabel("Will send the full draft, \(total) words, to \(name)")
        }
    }

    // MARK: Lists

    func refreshLists() {
        guard let notes else { return }
        let open = notes.openComments.count
        tabs.setLabel(open > 0 ? "Comments \(open)" : "Comments", forSegment: Tab.comments.rawValue)
        let versions = notes.versions.count
        tabs.setLabel(versions > 0 ? "Versions \(versions)" : "Versions", forSegment: Tab.versions.rawValue)
        guard panel.superview != nil else { return }
        if tab == .comments { reloadComments() }
        if tab == .versions { reloadVersions() }
        if tab == .ask { layout() }
    }

    private func reloadComments() {
        guard let notes else { return }
        let shown = notes.openComments + (showResolved ? notes.resolvedComments : [])
        commentList.set(shown.map(commentCard))
        let open = notes.openComments.filter { !$0.detached }
        sendCommentsButton.title = open.isEmpty ? "Send Comments" : "Send \(open.count) Comment\(open.count == 1 ? "" : "s") to \(provider.displayName)"
        sendCommentsButton.isEnabled = !open.isEmpty && !running
        commentsEmpty.isHidden = !shown.isEmpty
        showResolvedButton.isHidden = notes.resolvedComments.isEmpty
        commentsHeader.stringValue = ""
        layout()
    }

    private func reloadVersions() {
        guard let notes, let editor else { return }
        var cards: [NSView] = [versionCard(nil)]
        cards += notes.versions.map { versionCard($0) }
        versionList.set(cards)
        _ = editor
        layout()
    }

    private func text(_ s: String, size: CGFloat = 12, color: NSColor = Theme.text, font: NSFont? = nil, lines: Int = 0) -> NSTextField {
        let f = NSTextField(wrappingLabelWithString: s)
        f.font = font ?? Theme.uiFont(size: size)
        f.textColor = color
        f.isSelectable = false
        f.maximumNumberOfLines = lines
        f.lineBreakMode = .byWordWrapping
        f.cell?.truncatesLastVisibleLine = true
        return f
    }

    private func iconButton(_ symbol: String, _ tip: String, _ action: Selector, id: String) -> NSButton {
        let b = NSButton()
        b.image = NSImage(systemSymbolName: symbol, accessibilityDescription: tip)
        b.isBordered = false
        b.contentTintColor = Theme.quiet
        b.toolTip = tip
        b.target = self
        b.action = action
        b.identifier = NSUserInterfaceItemIdentifier(id)
        b.setAccessibilityLabel(tip)
        return b
    }

    private func commentCard(_ c: Comment) -> NSView {
        let card = CardView()
        card.isSelected = c.id == notes?.activeCommentID
        card.isDimmed = !c.isOpen
        card.identifier = NSUserInterfaceItemIdentifier(c.id)
        let quote = c.quote.replacingOccurrences(of: "\n", with: " ")
        let q = text(c.detached ? "“\(quote)” — no longer in the draft" : "“\(quote)”", color: Theme.quiet, font: Theme.font(size: 11, italic: true), lines: 2)
        let body = text(c.body, size: 12)
        var views: [NSView] = [q, body]
        for r in c.replies {
            let reply = text("\(r.author): \(r.text)", size: 11, color: Theme.quiet)
            reply.identifier = NSUserInterfaceItemIdentifier("reply")
            views.append(reply)
        }
        let resolve = iconButton(c.isOpen ? "checkmark.circle" : "arrow.uturn.backward.circle", c.isOpen ? "Resolve" : "Reopen", #selector(resolveComment(_:)), id: c.id)
        let edit = iconButton("pencil", "Edit comment", #selector(editComment(_:)), id: c.id)
        let delete = iconButton("trash", "Delete comment", #selector(deleteComment(_:)), id: c.id)
        for b in [resolve, edit, delete] { b.tag = 1; views.append(b) }
        views.forEach(card.addSubview)
        card.onClick = { [weak self] in self?.editor?.reveal(comment: c.id); self?.reloadComments() }
        card.setAccessibilityLabel("Comment on “\(quote.prefix(60))”: \(c.body)")
        return card
    }

    private func layoutCommentCard(_ card: NSView, width w: CGFloat) -> CGFloat {
        let pad: CGFloat = 12
        var y: CGFloat = 10
        var buttonX = w - pad - 18
        for v in card.subviews {
            if let b = v as? NSButton, b.tag == 1 { continue }
            guard let f = v as? NSTextField else { continue }
            let isQuote = v === card.subviews.first
            let width = isQuote ? w - pad * 2 - 72 : w - pad * 2
            f.preferredMaxLayoutWidth = width
            let h = wrappedHeight(f, width)
            f.frame = NSRect(x: pad, y: y, width: width, height: h)
            y += h + (isQuote ? 4 : 6)
        }
        for v in card.subviews.reversed() {
            guard let b = v as? NSButton, b.tag == 1 else { continue }
            b.frame = NSRect(x: buttonX, y: 8, width: 18, height: 18)
            buttonX -= 24
        }
        return y + 4
    }

    private func versionCard(_ v: DraftVersion?) -> NSView {
        let card = CardView()
        let previewing = editor?.preview.versionID
        card.isSelected = v == nil ? previewing == nil : v!.id == previewing
        guard let v else {
            let title = text("Your draft", size: 12, font: Theme.uiFont(size: 12, weight: .medium))
            let detail = text(previewing == nil ? "What you're editing now" : "Back to editing (→ or esc)", size: 11, color: Theme.quiet)
            [title, detail].forEach(card.addSubview)
            card.onClick = { [weak self] in self?.editor?.preview.close() }
            card.setAccessibilityLabel("Your draft")
            return card
        }
        let title = text(v.label, size: 12, font: Theme.uiFont(size: 12, weight: .medium), lines: 2)
        var bits = [v.kind == .snapshot ? "Saved draft" : v.source, VersionPreviewController.relative(v.created)]
        if v.usedAt != nil { bits.append("used") }
        let detail = text(bits.joined(separator: " · "), size: 11, color: Theme.quiet)
        var views: [NSView] = [title, detail]
        if let fb = v.feedback, !fb.isEmpty {
            views.append(text(fb, size: 11, color: Theme.quiet, lines: 3))
        }
        let delete = iconButton("xmark", "Delete version", #selector(deleteVersion(_:)), id: v.id)
        delete.tag = 1
        views.append(delete)
        views.forEach(card.addSubview)
        card.onClick = { [weak self] in self?.editor?.preview.show(v.id) }
        card.setAccessibilityLabel("\(v.label), \(bits.joined(separator: ", "))")
        return card
    }

    private func layoutVersionCard(_ card: NSView, width w: CGFloat) -> CGFloat {
        let pad: CGFloat = 12
        var y: CGFloat = 9
        for v in card.subviews {
            if let b = v as? NSButton, b.tag == 1 {
                b.frame = NSRect(x: w - pad - 14, y: 9, width: 14, height: 14)
                continue
            }
            guard let f = v as? NSTextField else { continue }
            let width = w - pad * 2 - (v === card.subviews.first ? 20 : 0)
            f.preferredMaxLayoutWidth = width
            let h = wrappedHeight(f, width)
            f.frame = NSRect(x: pad, y: y, width: width, height: h)
            y += h + 3
        }
        return y + 7
    }

    @objc private func resolveComment(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue, let c = notes?.comment(id) else { return }
        notes?.setResolved(id, c.isOpen)
        refreshLists()
    }

    @objc private func editComment(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue else { return }
        editor?.editComment(id)
    }

    @objc private func deleteComment(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue else { return }
        notes?.deleteComment(id)
        refreshLists()
    }

    @objc private func deleteVersion(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue else { return }
        if editor?.preview.versionID == id { editor?.preview.close() }
        notes?.deleteVersion(id)
        if lastVersionID == id { lastVersionID = nil }
        refreshLists()
    }

    @objc private func toggleResolved() {
        showResolved = showResolvedButton.state == .on
        reloadComments()
    }

    @objc private func addCommentClicked() { editor?.addComment(nil) }
    @objc private func sendCommentsClicked() { send(.resolveComments) }

    @objc private func previewClicked() {
        if let id = lastVersionID { editor?.preview.show(id) }
    }

    // MARK: Actions

    @objc private func providerChanged() {
        let i = providerPopup.indexOfSelectedItem
        guard i >= 0, i < AgentProvider.allCases.count else { return }
        setProvider(AgentProvider.allCases[i])
    }

    private func setProvider(_ p: AgentProvider) {
        provider = p
        UserDefaults.standard.set(p.rawValue, forKey: "assist.provider")
        providerPopup.selectItem(at: AgentProvider.allCases.firstIndex(of: p) ?? 0)
        refreshScope()
        if tab == .comments { reloadComments() }
    }

    @objc private func closeClicked() { close() }

    @objc private func actionClicked(_ sender: NSButton) {
        let all = AgentAction.allCases
        guard sender.tag >= 0, sender.tag < all.count else { return }
        send(all[sender.tag])
    }

    @objc private func sendCustom() {
        guard !instructionField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        send(.custom)
    }

    @objc private func sendOrStop() {
        if running { stop() } else { sendCustom() }
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(NSResponder.insertNewline(_:)) {
            sendCustom()
            return true
        }
        if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
            if running { stop() } else { close() }
            return true
        }
        return false
    }

    private func send(_ action: AgentAction) {
        guard let editor, let notes else { return }
        let basedOn = editor.preview.versionID.flatMap { notes.version($0) }
        let base = basedOn?.body ?? editor.textView.string
        guard !base.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        stop()
        clearResult()
        notes.flushSave()
        let ns = editor.textView.string as NSString
        let sel = editor.textView.selectedRange()
        let selection: NSRange? = basedOn == nil && sel.length > 0 && NSMaxRange(sel) <= ns.length
            && !ns.substring(with: sel).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? sel : nil
        let comments: [Comment]
        switch action {
        case .resolveComments, .custom: comments = notes.openComments.filter { !$0.detached }
        default: comments = []
        }
        if action == .resolveComments && comments.isEmpty { return }
        let instruction = action == .custom ? instructionField.stringValue : ""
        let history = notes.transcript.suffix(4).map { ($0.instruction.isEmpty ? $0.action : $0.instruction, $0.feedback) }
        let request = AgentRequest(
            provider: provider, action: action, instruction: instruction, document: base,
            selection: selection.map { ns.substring(with: $0) }, title: editor.draft.title,
            comments: comments.map { PromptComment(id: $0.id, quote: $0.quote, body: $0.body, replies: $0.replies.map { "\($0.author): \($0.text)" }) },
            history: history
        )
        sent = Sent(action: action, instruction: instruction, selection: selection, basedOnVersionID: basedOn?.id, commentIDs: comments.map(\.id))
        generation += 1
        let token = generation
        running = true
        if action == .resolveComments { select(.ask) }
        let what = action == .resolveComments ? "\(comments.count) comment\(comments.count == 1 ? "" : "s")" : (basedOn != nil ? "the version" : "the draft")
        statusLabel.stringValue = "\(provider.displayName) is reading \(what)…"
        statusLabel.textColor = Theme.quiet
        updateControls()
        layout()
        let s = AgentFactory.make(provider)
        session = s
        s.start(prompt: AgentPrompt.build(request)) { [weak self] event in
            guard let self, token == self.generation else { return }
            self.handle(event)
        }
    }

    private func handle(_ event: AgentEvent) {
        switch event {
        case let .progress(text):
            setFeedback(AgentPrompt.parse(text).feedback)
            if AgentPrompt.isDraftingEdits(text) { statusLabel.stringValue = "Drafting edits…" }
        case let .completed(text):
            running = false
            session = nil
            finish(AgentPrompt.parse(text))
        case let .failed(message):
            running = false
            session = nil
            sent = nil
            setFeedback("")
            statusLabel.stringValue = message
            statusLabel.textColor = Theme.warning
        }
        updateControls()
        layout()
    }

    private func finish(_ reply: AgentReply) {
        guard let editor, let notes, let sent else { return }
        self.sent = nil
        let name = provider.displayName
        for (id, answer) in reply.replies where notes.comment(id) != nil {
            notes.addReply(to: id, author: name, text: answer)
        }
        // Place edits on the text as it is now, so anything typed while waiting is kept.
        let basedOn = sent.basedOnVersionID.flatMap { notes.version($0) }
        let base = basedOn?.body ?? editor.textView.string
        let placement = EditPlacer.place(reply.edits, in: base, focus: basedOn == nil ? sent.selection : nil) { edit in
            guard basedOn == nil, let id = edit.commentIDs.first else { return nil }
            return notes.comment(id)?.range
        }
        var versionID: String?
        if !placement.placed.isEmpty {
            let body = placement.applied(to: base)
            if body != base {
                let resolves = Array(Set(placement.placed.flatMap { reply.edits[$0.index].commentIDs }))
                let label: String
                switch sent.action {
                case .custom: label = Self.shortLabel(sent.instruction)
                case .resolveComments: label = "Resolve \(sent.commentIDs.count) comment\(sent.commentIDs.count == 1 ? "" : "s")"
                default: label = sent.action.label
                }
                let v = DraftVersion(kind: .proposal, label: basedOn.map { "\(label) (from “\($0.label)”)" } ?? label, source: name,
                                     body: body, feedback: reply.feedback.isEmpty ? nil : reply.feedback, resolvesCommentIDs: resolves)
                notes.addVersion(v)
                versionID = v.id
                lastVersionID = v.id
            }
        }
        var shown = reply.feedback
        if shown.isEmpty && versionID == nil && reply.replies.isEmpty { shown = "No feedback." }
        if !reply.replies.isEmpty {
            shown += (shown.isEmpty ? "" : "\n\n") + "Answered \(reply.replies.count) comment\(reply.replies.count == 1 ? "" : "s") — see Comments."
        }
        if !placement.failed.isEmpty {
            shown += "\n\n\(placement.failed.count) edit\(placement.failed.count == 1 ? "" : "s") couldn't be placed:"
            for f in placement.failed {
                shown += "\n• \(f.reason)\n  “\(f.edit.new.prefix(300))”"
            }
        }
        setFeedback(shown)
        notes.addExchange(AssistExchange(provider: name, action: sent.action.label, instruction: sent.instruction,
                                         feedback: reply.feedback, versionID: versionID, commentIDs: sent.commentIDs))
        if let versionID {
            let n = placement.placed.count
            statusLabel.stringValue = "\(n) edit\(n == 1 ? "" : "s") ready as a new version, shown in the editor. ← → switch versions · ↩ use it · esc back to your draft."
            statusLabel.textColor = Theme.quiet
            editor.preview.show(versionID)
        } else {
            statusLabel.stringValue = ""
        }
        if sent.action == .resolveComments && !reply.replies.isEmpty && versionID == nil { select(.comments) }
        refreshLists()
    }

    static func shortLabel(_ instruction: String) -> String {
        let one = instruction.replacingOccurrences(of: "\n", with: " ").trimmingCharacters(in: .whitespaces)
        return one.count > 48 ? String(one.prefix(47)) + "…" : one
    }

    private func clearResult() {
        feedbackView.string = ""
        statusLabel.stringValue = ""
        statusLabel.textColor = Theme.quiet
    }

    private func setFeedback(_ text: String) {
        let p = NSMutableParagraphStyle()
        p.lineHeightMultiple = 1.35
        feedbackView.textStorage?.setAttributedString(NSAttributedString(string: text, attributes: [
            .font: Theme.font(size: 13), .foregroundColor: Theme.text, .paragraphStyle: p,
        ]))
    }

    private func updateControls() {
        sendButton.title = running ? "Stop" : "Send"
        sendButton.setAccessibilityLabel(running ? "Stop request" : "Send instruction")
        for b in actionButtons { b.isEnabled = !running }
        providerPopup.isEnabled = !running
        if tab == .comments { reloadComments() }
        if !running { refreshScope() }
    }
}

extension EditorController {
    @discardableResult
    func ensureAssist() -> AssistController {
        if let assist { return assist }
        let a = AssistController(editor: self)
        assist = a
        return a
    }

    @objc func openAssist(_ sender: Any?) {
        if assistVisible { assist?.close() } else { ensureAssist().show(provider: nil) }
    }

    @objc func askCodex(_ sender: Any?) { ensureAssist().show(provider: .codex, tab: .ask) }
    @objc func askClaude(_ sender: Any?) { ensureAssist().show(provider: .claude, tab: .ask) }
    @objc func showComments(_ sender: Any?) { ensureAssist().show(provider: nil, tab: .comments) }
}
