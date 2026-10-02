import AppKit
import WriterCore

enum SaveState: Equatable {
    case saved
    case pending
    case failed(String)
    case recovered
}

/// Canvas view that reports mouse movement so chrome can reappear.
final class CanvasView: NSView {
    var onMouseMoved: (() -> Void)?
    private var tracking: NSTrackingArea?

    override var isFlipped: Bool { true }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        let t = NSTrackingArea(rect: bounds, options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect], owner: self)
        addTrackingArea(t)
        tracking = t
    }

    override func mouseMoved(with event: NSEvent) { onMouseMoved?() }

    override func draw(_ dirtyRect: NSRect) {
        Theme.canvas.setFill()
        bounds.fill()
    }
}

final class ChromeBar: NSView {
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        Theme.canvas.setFill()
        bounds.fill()
    }
}

final class EditorController: NSWindowController, NSWindowDelegate, NSTextViewDelegate, NSTextStorageDelegate, NSTextFieldDelegate {
    let library: DraftLibrary
    private(set) var draft: DraftDocument

    let canvas = CanvasView()
    let scrollView = NSScrollView()
    let textView: EditorTextView
    let styler: MarkdownStyler
    let topBar = ChromeBar()
    let footer = ChromeBar()
    let titleField = NSTextField()
    let statusLabel = NSTextField(labelWithString: "")
    let countsLabel = NSTextField(labelWithString: "")
    let focusLabel = NSTextField(labelWithString: "")

    private(set) var focus: FocusUnit = Preferences.focus
    private var lines: [MarkdownLine] = []
    private var fenceCount = 0
    private var dimmedFocusRange: NSRange?
    private var saveState: SaveState = .saved
    private var autosaveTimer: Timer?
    private var selectionTimer: Timer?
    private var countsTimer: Timer?
    private var chromeHideTimer: Timer?
    private var chromeVisible = true
    private var loadingDocument = false
    private var transientMessage: String?
    private var transientTimer: Timer?
    private var lastKeyboardEdit = false

    var assist: AssistController?
    let assistWidth: CGFloat = 380
    var assistVisible: Bool { assist?.panel.superview != nil }

    private let storage: NSTextStorage
    static let topBarHeight: CGFloat = 40
    static let footerHeight: CGFloat = 34

    init(library: DraftLibrary, document: DraftDocument) {
        self.library = library
        self.draft = document
        styler = MarkdownStyler(fontSize: Preferences.textSize)

        storage = NSTextStorage()
        let layout = NSLayoutManager()
        layout.allowsNonContiguousLayout = false
        layout.usesFontLeading = false
        storage.addLayoutManager(layout)
        let container = NSTextContainer(size: NSSize(width: 600, height: CGFloat.greatestFiniteMagnitude))
        container.widthTracksTextView = false
        container.lineFragmentPadding = 0
        layout.addTextContainer(container)
        textView = EditorTextView(frame: NSRect(x: 0, y: 0, width: 900, height: 700), textContainer: container)

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1060, height: 760),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered, defer: false
        )
        super.init(window: window)
        configureWindow(window)
        configureTextView()
        layoutChrome()
        load(document)
    }

    required init?(coder: NSCoder) { fatalError() }

    // MARK: Setup

    private func configureWindow(_ window: NSWindow) {
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.backgroundColor = Theme.canvas
        window.minSize = NSSize(width: 420, height: 320)
        window.tabbingMode = .disallowed
        window.delegate = self
        window.setFrameAutosaveName("FluentWriterMain")
        if !window.setFrameUsingName("FluentWriterMain") { window.center() }
        window.contentView = canvas
        window.acceptsMouseMovedEvents = true
        canvas.onMouseMoved = { [weak self] in self?.showChrome(autoHide: true) }
    }

    private func configureTextView() {
        let tv = textView
        tv.isRichText = false
        tv.importsGraphics = false
        tv.allowsUndo = true
        tv.usesFindBar = true
        tv.isIncrementalSearchingEnabled = true
        tv.isContinuousSpellCheckingEnabled = Preferences.spellCheck
        tv.isAutomaticSpellingCorrectionEnabled = false
        tv.isAutomaticQuoteSubstitutionEnabled = false
        tv.isAutomaticDashSubstitutionEnabled = false
        tv.isAutomaticTextReplacementEnabled = true
        tv.isAutomaticLinkDetectionEnabled = false
        tv.smartInsertDeleteEnabled = false
        tv.drawsBackground = false
        tv.isVerticallyResizable = true
        tv.isHorizontallyResizable = false
        tv.autoresizingMask = [.width]
        tv.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        tv.insertionPointColor = Theme.caret
        tv.selectedTextAttributes = [.backgroundColor: Theme.selection]
        tv.linkTextAttributes = [:]
        tv.delegate = self
        tv.textStorage?.delegate = self
        tv.setAccessibilityLabel("Draft")
        tv.setAccessibilityHelp("The draft text. Use the Format menu or Markdown to style text.")
        tv.onKeyTyping = { [weak self] in self?.userIsTyping() }
        tv.onMouseSelection = { [weak self] in self?.lastKeyboardEdit = false }
        applyTypography()

        scrollView.documentView = tv
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.scrollerStyle = .overlay
        scrollView.drawsBackground = false
        scrollView.borderType = .noBorder
        scrollView.findBarPosition = .aboveContent
        scrollView.contentView.postsBoundsChangedNotifications = true
    }

    private func layoutChrome() {
        canvas.addSubview(scrollView)
        canvas.addSubview(topBar)
        canvas.addSubview(footer)

        titleField.isBordered = false
        titleField.drawsBackground = false
        titleField.focusRingType = .none
        titleField.alignment = .center
        titleField.font = Theme.uiFont(size: 13)
        titleField.textColor = Theme.quiet
        titleField.lineBreakMode = .byTruncatingTail
        titleField.cell?.usesSingleLineMode = true
        titleField.delegate = self
        titleField.placeholderString = TitleDeriver.untitled
        titleField.setAccessibilityLabel("Draft title")
        titleField.toolTip = "Click to rename"
        topBar.addSubview(titleField)

        for label in [statusLabel, countsLabel, focusLabel] {
            label.font = Theme.uiFont(size: 12)
            label.textColor = Theme.quiet
            label.lineBreakMode = .byTruncatingTail
            footer.addSubview(label)
        }
        countsLabel.alignment = .right
        focusLabel.alignment = .center
        statusLabel.setAccessibilityLabel("Save status")
        countsLabel.setAccessibilityLabel("Counts")
        countsLabel.isHidden = !Preferences.showCounts

        NotificationCenter.default.addObserver(self, selector: #selector(frameChanged), name: NSView.frameDidChangeNotification, object: canvas)
        canvas.postsFrameChangedNotifications = true
        frameChanged()
    }

    @objc func frameChanged() {
        let b = canvas.bounds
        let panelW: CGFloat = assistVisible ? assistWidth : 0
        let editorW = b.width - panelW
        topBar.frame = NSRect(x: 0, y: 0, width: editorW, height: Self.topBarHeight)
        footer.frame = NSRect(x: 0, y: b.height - Self.footerHeight, width: editorW, height: Self.footerHeight)
        scrollView.frame = NSRect(x: 0, y: Self.topBarHeight, width: editorW, height: b.height - Self.topBarHeight)
        assist?.panel.frame = NSRect(x: editorW, y: 0, width: panelW, height: b.height)
        let titleW = min(420, editorW - 180)
        titleField.frame = NSRect(x: (editorW - titleW) / 2, y: 11, width: titleW, height: 18)
        let pad: CGFloat = 20
        let third = (editorW - pad * 2) / 3
        statusLabel.frame = NSRect(x: pad, y: 9, width: third + 60, height: 16)
        focusLabel.frame = NSRect(x: pad + third, y: 9, width: third, height: 16)
        countsLabel.frame = NSRect(x: editorW - pad - third - 60, y: 9, width: third + 60, height: 16)
        updateTextLayout()
    }

    // MARK: Typography & layout

    func applyTypography() {
        textView.font = styler.regular
        textView.typingAttributes = styler.baseAttributes
        textView.defaultParagraphStyle = styler.paragraphStyle()
        let natural = NSLayoutManager().defaultLineHeight(for: styler.regular)
        textView.caretHeight = (natural * 1.12).rounded()
        textView.caretWidth = max(2, (styler.fontSize / 9).rounded())
        if let storage = textView.textStorage {
            styler.style(storage, lines: lines, in: nil)
        }
        refreshFocus(force: true)
    }

    func updateTextLayout() {
        let anchor = textView.selectedRange()
        let visible = scrollView.contentSize
        let hang = styler.hangWidth
        let minPad = max(Theme.minimumSidePadding, hang + 12)
        let ideal = (styler.characterWidth * Theme.columnCharacters).rounded()
        let prose = max(160, min(ideal, visible.width - minPad * 2))
        let insetX = max(4, ((visible.width - prose) / 2 - hang).rounded())
        let insetY: CGFloat
        if focus.centersCaret {
            insetY = max(40, (visible.height / 2 - styler.lineHeight / 2).rounded())
            textView.bottomOverscroll = 0
        } else {
            insetY = 56
            textView.bottomOverscroll = (visible.height * 0.35).rounded()
        }
        textView.textContainer?.containerSize = NSSize(width: prose + hang, height: CGFloat.greatestFiniteMagnitude)
        textView.textContainerInset = NSSize(width: insetX, height: insetY)
        textView.minSize = NSSize(width: visible.width, height: visible.height)
        textView.setFrameSize(NSSize(width: visible.width, height: textView.frame.height))
        textView.sizeToFit()
        textView.setSelectedRange(anchor)
        if focus.centersCaret { centerCaret(animated: false) }
    }

    // MARK: Documents

    func load(_ doc: DraftDocument) {
        loadingDocument = true
        draft = doc
        textView.string = doc.body
        reparse(full: true)
        textView.undoManager?.removeAllActions()
        let len = (doc.body as NSString).length
        let sel = NSRange(location: min(doc.selection.location, len), length: min(doc.selection.length, max(0, len - doc.selection.location)))
        textView.setSelectedRange(sel)
        loadingDocument = false
        if doc.recoveredAt != nil {
            saveState = .recovered
            scheduleAutosave(after: 0.2)
        } else {
            saveState = .saved
        }
        updateTitle()
        updateStatus()
        updateCounts()
        updateTextLayout()
        refreshFocus(force: true)
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.textView.scrollRangeToVisible(self.textView.selectedRange())
            if self.focus.centersCaret { self.centerCaret(animated: false) }
            else { self.ensureCaretReadable(animated: false) }
        }
        assist?.documentChanged()
        window?.representedURL = doc.url
        window?.title = doc.title
        window?.makeFirstResponder(textView)
    }

    /// Saves pending text. Returns false when the draft couldn't be written.
    @discardableResult
    func flush() -> Bool {
        autosaveTimer?.invalidate()
        autosaveTimer = nil
        syncDocumentFromView()
        do {
            try library.save(draft)
            library.rememberSelection(draft)
            if saveState != .saved { saveState = .saved; updateStatus() }
            updateTitle()
            return true
        } catch {
            saveState = .failed(error.localizedDescription)
            updateStatus()
            return false
        }
    }

    /// Flushes before replacing the current draft; asks before leaving text that couldn't be saved.
    func confirmLeavingDocument() -> Bool {
        if flush() { return true }
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "This draft couldn't be saved."
        if case let .failed(message) = saveState { alert.informativeText = message + "\n\nA recovery copy is kept and will be offered next time." }
        alert.addButton(withTitle: "Keep Editing")
        alert.addButton(withTitle: "Save As…")
        alert.addButton(withTitle: "Continue")
        switch alert.runModal() {
        case .alertSecondButtonReturn: return saveAs()
        case .alertThirdButtonReturn: return true
        default: return false
        }
    }

    @discardableResult
    func saveAs() -> Bool {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.init(filenameExtension: "md")!]
        panel.nameFieldStringValue = TitleDeriver.fileNameStem(for: draft.title) + ".md"
        panel.directoryURL = library.draftsDirectory
        guard panel.runModal() == .OK, let url = panel.url else { return false }
        syncDocumentFromView()
        do {
            try AtomicFile.write(Data(draft.body.utf8), to: url)
            let reopened = try library.open(url)
            reopened.selection = textView.selectedRange()
            library.discardRecovery(for: draft.id)
            load(reopened)
            return true
        } catch {
            presentError(error)
            return false
        }
    }

    func syncDocumentFromView() {
        draft.body = textView.string
        draft.selection = textView.selectedRange()
    }

    func scheduleAutosave(after delay: TimeInterval = 0.8) {
        autosaveTimer?.invalidate()
        autosaveTimer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { [weak self] _ in
            self?.flush()
        }
    }

    func updateTitle() {
        if titleField.currentEditor() == nil {
            titleField.stringValue = draft.title == TitleDeriver.untitled && draft.body.isEmpty ? "" : draft.title
            titleField.textColor = draft.hasManualTitle ? Theme.text.withAlphaComponent(0.75) : Theme.quiet
        }
        window?.title = draft.title
        window?.representedURL = draft.url
    }

    func updateStatus() {
        if let transientMessage {
            statusLabel.stringValue = transientMessage
            statusLabel.textColor = Theme.quiet
            return
        }
        switch saveState {
        case .saved:
            statusLabel.stringValue = draft.url == nil ? "" : "Saved"
            statusLabel.textColor = Theme.quiet
            statusLabel.toolTip = draft.url?.path
        case .pending:
            statusLabel.stringValue = "Edited"
            statusLabel.textColor = Theme.quiet
        case .recovered:
            statusLabel.stringValue = "Recovered unsaved changes"
            statusLabel.textColor = Theme.quiet
        case let .failed(message):
            statusLabel.stringValue = "Not saved — \(message)"
            statusLabel.textColor = Theme.warning
            statusLabel.toolTip = message
            showChrome(autoHide: false)
        }
        statusLabel.setAccessibilityValue(statusLabel.stringValue)
    }

    func flashMessage(_ message: String) {
        transientMessage = message
        updateStatus()
        showChrome(autoHide: true)
        NSAccessibility.post(element: statusLabel, notification: .announcementRequested, userInfo: [.announcement: message, .priority: NSAccessibilityPriorityLevel.medium.rawValue])
        transientTimer?.invalidate()
        transientTimer = Timer.scheduledTimer(withTimeInterval: 2.2, repeats: false) { [weak self] _ in
            self?.transientMessage = nil
            self?.updateStatus()
        }
    }

    // MARK: Counts

    func updateCounts() {
        countsTimer?.invalidate()
        countsTimer = nil
        let text = textView.string
        let total = TextStats.counts(forMarkdown: text)
        let sel = textView.selectedRange()
        var selection: TextCounts?
        if sel.length > 0 {
            selection = TextStats.counts(forMarkdown: (text as NSString).substring(with: sel))
        }
        countsLabel.stringValue = TextStats.format(total, selection: selection)
        countsLabel.setAccessibilityValue(countsLabel.stringValue)
    }

    func scheduleCounts() {
        guard countsTimer == nil else { return }
        countsTimer = Timer.scheduledTimer(withTimeInterval: 0.15, repeats: false) { [weak self] _ in self?.updateCounts() }
    }

    // MARK: Parsing & styling

    private func reparse(full: Bool, edited: NSRange? = nil) {
        let ns = textView.string as NSString
        lines = MarkdownParser.parse(ns)
        let fences = lines.reduce(0) { $1.kind == .codeFence ? $0 + 1 : $0 }
        let fenceChanged = fences != fenceCount
        fenceCount = fences
        guard let storage = textView.textStorage else { return }
        styler.style(storage, lines: lines, in: (full || fenceChanged) ? nil : edited)
    }

    func textStorage(_ textStorage: NSTextStorage, didProcessEditing editedMask: NSTextStorageEditActions, range editedRange: NSRange, changeInLength delta: Int) {
        guard editedMask.contains(.editedCharacters) else { return }
        assist?.noteEdit(editedRange: editedRange, changeInLength: delta)
        // Attribute changes here are safe; styling is deferred to stay out of the edit transaction.
        let range = editedRange
        DispatchQueue.main.async { [weak self] in
            self?.reparse(full: false, edited: range)
            self?.refreshFocus(force: true)
        }
    }

    // MARK: Focus

    func setFocus(_ unit: FocusUnit) {
        let anchor = textView.selectedRange()
        focus = unit
        Preferences.focus = unit
        if unit.dimsSurroundings { Preferences.lastDimmingFocus = unit }
        updateTextLayout()
        textView.setSelectedRange(anchor)
        refreshFocus(force: true)
        if unit.centersCaret { centerCaret(animated: !reduceMotion) }
        focusLabel.stringValue = unit == .off ? "" : focusName(unit)
        flashMessage(unit == .off ? "Focus off" : "\(focusName(unit)) focus")
    }

    func focusName(_ unit: FocusUnit) -> String {
        switch unit {
        case .off: return "Off"
        case .sentence: return "Sentence"
        case .paragraph: return "Paragraph"
        case .typewriter: return "Typewriter"
        }
    }

    /// Range kept bright in sentence/paragraph focus, covering the whole selection.
    func currentFocusRange() -> NSRange? {
        guard focus.dimsSurroundings else { return nil }
        let ns = textView.string as NSString
        let sel = textView.selectedRange()
        guard let a = TextUnits.focusRange(focus, in: ns, at: sel.location) else { return nil }
        if sel.length == 0 { return a }
        guard let b = TextUnits.focusRange(focus, in: ns, at: NSMaxRange(sel)) else { return a }
        return NSUnionRange(a, b)
    }

    func refreshFocus(force: Bool = false) {
        guard let lm = textView.layoutManager, let storage = textView.textStorage else { return }
        let full = NSRange(location: 0, length: storage.length)
        let range = currentFocusRange()
        if !force && range == dimmedFocusRange { return }
        dimmedFocusRange = range
        lm.removeTemporaryAttribute(.foregroundColor, forCharacterRange: full)
        guard let range else { return }
        let before = NSRange(location: 0, length: range.location)
        let afterStart = min(NSMaxRange(range), storage.length)
        let after = NSRange(location: afterStart, length: storage.length - afterStart)
        for r in [before, after] where r.length > 0 {
            lm.addTemporaryAttribute(.foregroundColor, value: Theme.dimmed, forCharacterRange: r)
        }
    }

    var reduceMotion: Bool { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }

    /// Keeps the caret line at the vertical middle of the window.
    func centerCaret(animated: Bool) {
        guard let caret = textView.insertionRect() else { return }
        let visibleH = scrollView.contentView.bounds.height
        let maxY = max(0, textView.frame.height - visibleH)
        let target = min(maxY, max(0, (caret.midY - visibleH / 2).rounded()))
        scroll(to: target, animated: animated)
    }

    /// Outside typewriter modes, keeps the caret out of the bottom edge without recentering.
    func ensureCaretReadable(animated: Bool) {
        guard let caret = textView.insertionRect() else { return }
        let clip = scrollView.contentView.bounds
        let bottomLimit = clip.maxY - max(Self.footerHeight + styler.lineHeight * 2, clip.height * 0.22)
        let topLimit = clip.minY + styler.lineHeight
        var target: CGFloat?
        if caret.maxY > bottomLimit { target = clip.minY + (caret.maxY - bottomLimit) }
        else if caret.minY < topLimit { target = max(0, caret.minY - styler.lineHeight) }
        if let t = target {
            let maxY = max(0, textView.frame.height - clip.height)
            scroll(to: min(maxY, t), animated: animated)
        }
    }

    private func scroll(to y: CGFloat, animated: Bool) {
        let clip = scrollView.contentView
        if abs(clip.bounds.origin.y - y) < 0.5 { return }
        let origin = NSPoint(x: clip.bounds.origin.x, y: y)
        if animated && !reduceMotion {
            NSAnimationContext.runAnimationGroup { ctx in
                ctx.duration = 0.14
                ctx.timingFunction = CAMediaTimingFunction(name: .easeOut)
                clip.animator().setBoundsOrigin(origin)
            } completionHandler: { [weak self] in
                self?.scrollView.reflectScrolledClipView(clip)
            }
        } else {
            clip.setBoundsOrigin(origin)
            scrollView.reflectScrolledClipView(clip)
        }
    }

    // MARK: Chrome

    func userIsTyping() {
        lastKeyboardEdit = true
        if case .failed = saveState { return }
        if assistVisible { return }
        hideChrome()
    }

    func hideChrome() {
        chromeHideTimer?.invalidate()
        guard chromeVisible, titleField.currentEditor() == nil else { return }
        chromeVisible = false
        animateChrome(to: 0)
    }

    func showChrome(autoHide: Bool) {
        chromeHideTimer?.invalidate()
        if !chromeVisible {
            chromeVisible = true
            animateChrome(to: 1)
        }
        if autoHide {
            chromeHideTimer = Timer.scheduledTimer(withTimeInterval: 3.0, repeats: false) { [weak self] _ in
                guard let self, self.window?.isKeyWindow == true, self.window?.firstResponder === self.textView else { return }
                if case .failed = self.saveState { return }
                let mouse = self.canvas.convert(self.window?.mouseLocationOutsideOfEventStream ?? .zero, from: nil)
                if self.topBar.frame.contains(mouse) || self.footer.frame.contains(mouse) { return }
                self.hideChrome()
            }
        }
    }

    private func animateChrome(to alpha: CGFloat) {
        let views: [NSView] = [titleField, statusLabel, countsLabel, focusLabel] + trafficLights()
        let duration = reduceMotion ? 0 : (alpha > 0 ? 0.25 : 0.6)
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = duration
            for v in views { v.animator().alphaValue = alpha }
        }
    }

    private func trafficLights() -> [NSView] {
        guard let w = window else { return [] }
        return [.closeButton, .miniaturizeButton, .zoomButton].compactMap { w.standardWindowButton($0) }
    }

    // MARK: NSTextViewDelegate

    func textDidChange(_ notification: Notification) {
        guard !loadingDocument else { return }
        draft.body = textView.string
        draft.selection = textView.selectedRange()
        if !draft.hasManualTitle {
            let previous = draft.title
            draft.refreshDerivedTitle()
            if previous != draft.title { updateTitle() }
        }
        if case .failed = saveState {} else { saveState = .pending; updateStatus() }
        scheduleAutosave()
        scheduleCounts()
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            if self.focus.centersCaret { self.centerCaret(animated: true) }
            else { self.ensureCaretReadable(animated: false) }
        }
    }

    func textViewDidChangeSelection(_ notification: Notification) {
        guard !loadingDocument else { return }
        draft.selection = textView.selectedRange()
        refreshFocus()
        scheduleCounts()
        assist?.selectionChanged()
        let event = NSApp.currentEvent?.type
        if event == .keyDown && focus.centersCaret {
            DispatchQueue.main.async { [weak self] in self?.centerCaret(animated: true) }
        }
        selectionTimer?.invalidate()
        selectionTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: false) { [weak self] _ in
            guard let self else { return }
            self.library.rememberSelection(self.draft)
        }
    }

    func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        let ns = textView.string as NSString
        let sel = textView.selectedRange()
        switch selector {
        case #selector(NSResponder.insertNewline(_:)):
            if let edit = ListContinuation.newlineEdit(in: ns, selection: sel) {
                apply(edit, actionName: "Typing")
                return true
            }
        case #selector(NSResponder.insertTab(_:)):
            if let edit = ListContinuation.indentEdit(in: ns, selection: sel, outdent: false) {
                apply(edit, actionName: "Indent")
                return true
            }
        case #selector(NSResponder.insertBacktab(_:)):
            if let edit = ListContinuation.indentEdit(in: ns, selection: sel, outdent: true) {
                apply(edit, actionName: "Outdent")
                return true
            }
        case #selector(NSResponder.cancelOperation(_:)):
            if assistVisible { assist?.close(); return true }
        default:
            break
        }
        return false
    }

    /// Applies an edit through the text system so it lands on the undo stack as one step.
    func apply(_ edit: TextEdit, actionName: String) {
        guard textView.shouldChangeText(in: edit.range, replacementString: edit.replacement) else { return }
        textView.textStorage?.replaceCharacters(in: edit.range, with: edit.replacement)
        textView.didChangeText()
        textView.undoManager?.setActionName(actionName)
        let len = (textView.string as NSString).length
        let s = edit.selectionAfter
        textView.setSelectedRange(NSRange(location: min(s.location, len), length: min(s.length, max(0, len - s.location))))
        textView.typingAttributes = styler.baseAttributes
    }

    // MARK: Title editing

    func controlTextDidBeginEditing(_ obj: Notification) {
        showChrome(autoHide: false)
    }

    func controlTextDidEndEditing(_ obj: Notification) {
        guard (obj.object as? NSTextField) === titleField else { return }
        let newTitle = titleField.stringValue
        if newTitle != draft.title {
            syncDocumentFromView()
            if draft.url == nil && !draft.body.isEmpty { flush() }
            do {
                try library.rename(draft, to: newTitle)
            } catch {
                presentError(error)
            }
            if draft.url == nil && !newTitle.isEmpty {
                // Title is kept with the draft and used when it is first saved.
            }
            updateTitle()
        }
        window?.makeFirstResponder(textView)
        showChrome(autoHide: true)
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        if selector == #selector(NSResponder.cancelOperation(_:)) {
            titleField.stringValue = draft.title
            window?.makeFirstResponder(self.textView)
            return true
        }
        return false
    }

    // MARK: NSWindowDelegate

    func windowDidResize(_ notification: Notification) { frameChanged() }

    func windowDidResignKey(_ notification: Notification) {
        flush()
        showChrome(autoHide: false)
    }

    func windowDidBecomeKey(_ notification: Notification) { showChrome(autoHide: true) }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        return confirmLeavingDocument()
    }

    func windowWillClose(_ notification: Notification) {
        NSApp.terminate(nil)
    }

    @discardableResult
    override func presentError(_ error: Error) -> Bool {
        let alert = NSAlert(error: error)
        if let window { alert.beginSheetModal(for: window) } else { alert.runModal() }
        return true
    }
}
