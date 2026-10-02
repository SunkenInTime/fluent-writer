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

/// Transient side panel for optional writing help. Nothing leaves the Mac until Send is pressed,
/// and suggestions only change the draft through Accept.
final class AssistController: NSObject, NSTextFieldDelegate {
    weak var editor: EditorController?
    let panel = AssistPanel()

    private let providerPopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let closeButton = NSButton()
    private let scopeLabel = NSTextField(labelWithString: "")
    private let privacyLabel = NSTextField(wrappingLabelWithString: "")
    private var actionButtons: [NSButton] = []
    private let instructionField = NSTextField()
    private let sendButton = NSButton(title: "Send", target: nil, action: nil)
    private let statusLabel = NSTextField(wrappingLabelWithString: "")
    private let feedbackHeader = NSTextField(labelWithString: "Feedback")
    private let feedbackView = NSTextView()
    private let feedbackScroll = NSScrollView()
    private let editHeader = NSTextField(labelWithString: "Proposed edit")
    private let diffView = NSTextView()
    private let diffScroll = NSScrollView()
    private let acceptButton = NSButton(title: "Accept", target: nil, action: nil)
    private let rejectButton = NSButton(title: "Reject", target: nil, action: nil)

    private var provider: AgentProvider = AgentProvider(rawValue: UserDefaults.standard.string(forKey: "assist.provider") ?? "") ?? .codex
    private var session: AgentSession?
    private var generation = 0
    private var scope: FrozenScope?
    private var revision: String?
    private var running = false

    init(editor: EditorController) {
        self.editor = editor
        super.init()
        build()
        refreshScope()
        updateControls()
    }

    // MARK: Building

    private func label(_ f: NSTextField, size: CGFloat = 12, color: NSColor = Theme.quiet) {
        f.font = Theme.uiFont(size: size)
        f.textColor = color
        f.isSelectable = false
    }

    private func configureReader(_ tv: NSTextView, _ scroll: NSScrollView, name: String) {
        tv.isEditable = false
        tv.isSelectable = true
        tv.drawsBackground = false
        tv.textContainerInset = NSSize(width: 0, height: 4)
        tv.textContainer?.lineFragmentPadding = 0
        tv.isVerticallyResizable = true
        tv.autoresizingMask = [.width]
        tv.textContainer?.widthTracksTextView = true
        tv.setAccessibilityLabel(name)
        scroll.documentView = tv
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.borderType = .noBorder
        panel.addSubview(scroll)
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

        label(scopeLabel, color: Theme.text)
        label(privacyLabel, size: 11)
        privacyLabel.stringValue = "Nothing is sent until you press Send. The agent gets only this text, with no files, commands or web access."

        for action in AgentAction.allCases where action != .custom {
            let b = NSButton(title: action.label, target: self, action: #selector(actionClicked(_:)))
            b.bezelStyle = .recessed
            b.font = Theme.uiFont(size: 12)
            b.tag = AgentAction.allCases.firstIndex(of: action) ?? 0
            b.setAccessibilityLabel("\(action.label) with the selected provider")
            actionButtons.append(b)
            panel.addSubview(b)
        }

        instructionField.placeholderString = "Or ask anything about this text"
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
        label(editHeader, size: 11)
        configureReader(feedbackView, feedbackScroll, name: "Feedback")
        configureReader(diffView, diffScroll, name: "Proposed edit")

        acceptButton.target = self
        acceptButton.action = #selector(accept)
        acceptButton.bezelStyle = .rounded
        acceptButton.font = Theme.uiFont(size: 12)
        rejectButton.target = self
        rejectButton.action = #selector(reject)
        rejectButton.bezelStyle = .rounded
        rejectButton.font = Theme.uiFont(size: 12)

        for v: NSView in [providerPopup, closeButton, scopeLabel, privacyLabel, instructionField, sendButton,
                          statusLabel, feedbackHeader, editHeader, acceptButton, rejectButton] {
            panel.addSubview(v)
        }
        NotificationCenter.default.addObserver(self, selector: #selector(layout), name: NSView.frameDidChangeNotification, object: panel)
        panel.postsFrameChangedNotifications = true
    }

    @objc func layout() {
        let w = panel.bounds.width
        let pad: CGFloat = 22
        let inner = w - pad * 2
        var y: CGFloat = 44
        providerPopup.frame = NSRect(x: pad - 4, y: y - 4, width: 120, height: 24)
        closeButton.frame = NSRect(x: w - pad - 50, y: y - 2, width: 50, height: 20)
        y += 30
        scopeLabel.frame = NSRect(x: pad, y: y, width: inner, height: 18)
        y += 22
        let privacyH = privacyLabel.cell?.cellSize(forBounds: NSRect(x: 0, y: 0, width: inner, height: 200)).height ?? 30
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
        let statusH = statusLabel.stringValue.isEmpty ? 0 : (statusLabel.cell?.cellSize(forBounds: NSRect(x: 0, y: 0, width: inner, height: 200)).height ?? 16)
        statusLabel.frame = NSRect(x: pad, y: y, width: inner, height: statusH)
        if statusH > 0 { y += statusH + 10 }

        let bottomButtons: CGFloat = revision == nil ? 0 : 44
        let available = panel.bounds.height - y - bottomButtons - 20
        let showsEdit = revision != nil
        let feedbackH = showsEdit ? max(60, available * 0.38) : available - 20
        feedbackHeader.frame = NSRect(x: pad, y: y, width: inner, height: 16)
        feedbackScroll.frame = NSRect(x: pad, y: y + 18, width: inner, height: max(0, feedbackH - 18))
        y += feedbackH + 10
        editHeader.isHidden = !showsEdit
        diffScroll.isHidden = !showsEdit
        acceptButton.isHidden = !showsEdit
        rejectButton.isHidden = !showsEdit
        if showsEdit {
            let editH = panel.bounds.height - y - bottomButtons - 20
            editHeader.frame = NSRect(x: pad, y: y, width: inner, height: 16)
            diffScroll.frame = NSRect(x: pad, y: y + 18, width: inner, height: max(0, editH - 18))
            let by = panel.bounds.height - bottomButtons - 6
            acceptButton.frame = NSRect(x: w - pad - 84, y: by, width: 84, height: 30)
            rejectButton.frame = NSRect(x: w - pad - 84 - 90, y: by, width: 84, height: 30)
        }
        feedbackHeader.isHidden = feedbackView.string.isEmpty && !running
    }

    // MARK: Lifecycle

    func show(provider p: AgentProvider?) {
        if let p { setProvider(p) }
        guard let editor else { return }
        if panel.superview == nil {
            editor.canvas.addSubview(panel)
            editor.frameChanged()
        }
        refreshScope()
        layout()
        editor.window?.makeFirstResponder(instructionField)
    }

    func close() {
        stop()
        clearResult()
        panel.removeFromSuperview()
        guard let editor else { return }
        editor.frameChanged()
        editor.window?.makeFirstResponder(editor.textView)
    }

    func stop() {
        guard running else { return }
        generation += 1
        session?.cancel()
        session = nil
        running = false
        statusLabel.stringValue = "Stopped. Nothing was changed."
        updateControls()
        layout()
    }

    // MARK: Draft tracking

    func noteEdit(editedRange: NSRange, changeInLength: Int) {
        guard var s = scope else { return }
        let wasStale = s.isStale
        s.noteEdit(editedRange: editedRange, changeInLength: changeInLength)
        scope = s
        if s.isStale && !wasStale { markStale() }
    }

    func documentChanged() {
        stop()
        clearResult()
        refreshScope()
    }

    func selectionChanged() {
        if !running && scope == nil { refreshScope() }
    }

    private func currentScope() -> (NSRange, Bool)? {
        guard let editor else { return nil }
        let ns = editor.textView.string as NSString
        let sel = editor.textView.selectedRange()
        if sel.length > 0, NSMaxRange(sel) <= ns.length,
           !ns.substring(with: sel).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return (sel, true)
        }
        guard ns.length > 0 else { return nil }
        return (NSRange(location: 0, length: ns.length), false)
    }

    private func refreshScope() {
        let name = provider.displayName
        guard let editor, let (range, isSelection) = currentScope() else {
            scopeLabel.stringValue = "Write something first"
            return
        }
        let text = (editor.textView.string as NSString).substring(with: range)
        let words = TextStats.counts(forMarkdown: text).words
        let what = isSelection ? "Selection" : "Full draft"
        scopeLabel.stringValue = "\(what) · \(words) \(words == 1 ? "word" : "words") → \(name)"
        scopeLabel.setAccessibilityLabel("Will send \(what.lowercased()), \(words) words, to \(name)")
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
        guard let editor, let (range, isSelection) = currentScope() else { return }
        stop()
        clearResult()
        let original = (editor.textView.string as NSString).substring(with: range)
        scope = FrozenScope(range: range, original: original)
        let request = AgentRequest(
            provider: provider, action: action, instruction: instructionField.stringValue,
            scopeText: original, isSelection: isSelection, title: editor.draft.title
        )
        generation += 1
        let token = generation
        running = true
        statusLabel.stringValue = "\(provider.displayName) is reading \(isSelection ? "the selection" : "the draft")…"
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
            let reply = AgentPrompt.parse(text)
            setFeedback(reply.feedback)
            if text.contains("<\(AgentPrompt.revisionTag)>") {
                statusLabel.stringValue = "Drafting a proposed edit…"
            }
        case let .completed(text):
            running = false
            session = nil
            let reply = AgentPrompt.parse(text)
            setFeedback(reply.feedback.isEmpty && reply.revision == nil ? "No feedback." : reply.feedback)
            statusLabel.stringValue = ""
            if let r = reply.revision, let s = scope, r != s.original {
                revision = r
                showDiff(old: s.original, new: r)
                verifyScope()
            } else {
                scope = nil
            }
        case let .failed(message):
            running = false
            session = nil
            scope = nil
            setFeedback("")
            statusLabel.stringValue = message
            statusLabel.textColor = Theme.warning
        }
        updateControls()
        layout()
    }

    private func verifyScope() {
        guard let editor, var s = scope else { return }
        s.verify(against: editor.textView.string as NSString)
        scope = s
        if s.isStale { markStale() }
    }

    private func markStale() {
        guard revision != nil || running else { return }
        statusLabel.stringValue = "You edited this text after sending it, so the suggestion is out of date. Send again for a fresh one."
        statusLabel.textColor = Theme.warning
        updateControls()
        layout()
    }

    @objc private func accept() {
        verifyScope()
        guard let editor, let s = scope, !s.isStale, let r = revision else { return }
        let replacement = r as NSString
        editor.apply(
            TextEdit(range: s.range, replacement: r, selectionAfter: NSRange(location: s.range.location, length: replacement.length)),
            actionName: "Accept Suggestion"
        )
        clearResult()
        statusLabel.stringValue = "Edit applied. Undo restores the original."
        statusLabel.textColor = Theme.quiet
        updateControls()
        layout()
        editor.window?.makeFirstResponder(editor.textView)
    }

    @objc private func reject() {
        clearResult()
        updateControls()
        layout()
    }

    private func clearResult() {
        revision = nil
        scope = nil
        feedbackView.string = ""
        diffView.string = ""
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

    private func showDiff(old: String, new: String) {
        let out = NSMutableAttributedString()
        let p = NSMutableParagraphStyle()
        p.lineHeightMultiple = 1.35
        let base: [NSAttributedString.Key: Any] = [.font: Theme.font(size: 13), .foregroundColor: Theme.text, .paragraphStyle: p]
        for seg in WordDiff.diff(old, new) {
            var a = base
            switch seg.kind {
            case .equal: break
            case .insert:
                a[.foregroundColor] = Theme.inserted
                a[.backgroundColor] = Theme.insertedBackground
            case .delete:
                a[.foregroundColor] = Theme.deleted
                a[.backgroundColor] = Theme.deletedBackground
                a[.strikethroughStyle] = NSUnderlineStyle.single.rawValue
            }
            out.append(NSAttributedString(string: seg.text, attributes: a))
        }
        diffView.textStorage?.setAttributedString(out)
        diffView.setAccessibilityValue("Proposed text: \(new)")
    }

    private func updateControls() {
        sendButton.title = running ? "Stop" : "Send"
        sendButton.setAccessibilityLabel(running ? "Stop request" : "Send instruction")
        for b in actionButtons { b.isEnabled = !running }
        providerPopup.isEnabled = !running
        acceptButton.isEnabled = revision != nil && !(scope?.isStale ?? true)
        if !running && scope == nil { refreshScope() }
    }
}

extension EditorController {
    private func ensureAssist() -> AssistController {
        if let assist { return assist }
        let a = AssistController(editor: self)
        assist = a
        return a
    }

    @objc func openAssist(_ sender: Any?) {
        if assistVisible { assist?.close() } else { ensureAssist().show(provider: nil) }
    }

    @objc func askCodex(_ sender: Any?) { ensureAssist().show(provider: .codex) }
    @objc func askClaude(_ sender: Any?) { ensureAssist().show(provider: .claude) }
}
