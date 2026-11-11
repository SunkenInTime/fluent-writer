import AppKit
import WriterCore

/// Text view that saves on Return and cancels on Escape. Shift- or Option-Return adds a line.
final class ComposerTextView: NSTextView {
    var onSubmit: (() -> Void)?
    var onCancel: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        if (event.keyCode == 36 || event.keyCode == 76), !hasMarkedText() {
            if event.modifierFlags.intersection([.shift, .option]).isEmpty { onSubmit?(); return }
            insertNewlineIgnoringFieldEditor(nil)
            return
        }
        if event.keyCode == 53 { onCancel?(); return }
        super.keyDown(with: event)
    }
}

/// Popover for writing (or editing) a comment on a span of the draft.
final class CommentComposer: NSObject, NSPopoverDelegate {
    private let popover = NSPopover()
    private let textView = ComposerTextView()
    private var onSave: ((String) -> Void)?
    private var textObserver: NSObjectProtocol?

    func present(quote: String, initial: String = "", relativeTo rect: NSRect, of view: NSView, onSave: @escaping (String) -> Void) {
        self.onSave = onSave
        let width: CGFloat = 320
        let root = NSView(frame: NSRect(x: 0, y: 0, width: width, height: 150))

        let excerpt = quote.replacingOccurrences(of: "\n", with: " ")
        let quoteLabel = NSTextField(labelWithString: "“\(excerpt.count > 90 ? excerpt.prefix(90) + "…" : excerpt)”")
        quoteLabel.font = Theme.font(size: 11, italic: true)
        quoteLabel.textColor = .secondaryLabelColor
        quoteLabel.lineBreakMode = .byTruncatingTail
        quoteLabel.frame = NSRect(x: 14, y: 124, width: width - 28, height: 16)
        root.addSubview(quoteLabel)

        let scroll = NSScrollView(frame: NSRect(x: 12, y: 34, width: width - 24, height: 84))
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.borderType = .noBorder
        scroll.drawsBackground = false
        textView.frame = NSRect(origin: .zero, size: scroll.contentSize)
        textView.isRichText = false
        textView.font = Theme.uiFont(size: 13)
        textView.textColor = .labelColor
        textView.typingAttributes = [.font: Theme.uiFont(size: 13), .foregroundColor: NSColor.labelColor]
        textView.drawsBackground = false
        textView.insertionPointColor = Theme.caret
        textView.isVerticallyResizable = true
        textView.autoresizingMask = [.width]
        textView.textContainer?.widthTracksTextView = true
        textView.textContainerInset = NSSize(width: 2, height: 4)
        textView.string = initial
        textView.setAccessibilityLabel("Comment")
        textView.onSubmit = { [weak self] in self?.submit() }
        textView.onCancel = { [weak self] in self?.popover.performClose(nil) }
        scroll.documentView = textView
        root.addSubview(scroll)

        let placeholder = NSTextField(labelWithString: initial.isEmpty ? "What should change here?" : "")
        placeholder.font = Theme.uiFont(size: 13)
        placeholder.textColor = .secondaryLabelColor
        placeholder.frame = NSRect(x: 16, y: 96, width: width - 32, height: 18)
        placeholder.isHidden = !initial.isEmpty
        root.addSubview(placeholder)
        if let textObserver { NotificationCenter.default.removeObserver(textObserver) }
        textObserver = NotificationCenter.default.addObserver(forName: NSText.didChangeNotification, object: textView, queue: .main) { [weak placeholder, weak textView] _ in
            placeholder?.isHidden = !(textView?.string.isEmpty ?? true)
        }

        let hint = NSTextField(labelWithString: "↩ save · esc cancel")
        hint.toolTip = "Shift-Return adds a new line"
        hint.font = Theme.uiFont(size: 10)
        hint.textColor = .secondaryLabelColor
        hint.lineBreakMode = .byTruncatingTail
        root.addSubview(hint)

        let save = NSButton(title: initial.isEmpty ? "Comment" : "Save", target: self, action: #selector(submit))
        save.bezelStyle = .rounded
        save.controlSize = .small
        save.font = Theme.uiFont(size: 11)
        save.sizeToFit()
        save.frame.origin = NSPoint(x: width - 12 - save.frame.width, y: 6)
        root.addSubview(save)
        hint.frame = NSRect(x: 14, y: 10, width: max(0, save.frame.minX - 14 - 10), height: 14)

        let vc = NSViewController()
        vc.view = root
        popover.contentViewController = vc
        popover.behavior = .transient
        popover.animates = true
        popover.delegate = self
        popover.show(relativeTo: rect, of: view, preferredEdge: .maxY)
        popover.contentViewController?.view.window?.makeFirstResponder(textView)
    }

    @objc private func submit() {
        let body = textView.string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !body.isEmpty else { NSSound.beep(); return }
        let save = onSave
        onSave = nil
        popover.performClose(nil)
        save?(body)
    }

    func popoverDidClose(_ notification: Notification) {
        onSave = nil
        if let textObserver { NotificationCenter.default.removeObserver(textObserver) }
        textObserver = nil
    }
}

extension EditorController {
    /// The span a new comment attaches to: the selection, or the current line when nothing is selected.
    func commentTargetRange() -> NSRange? {
        let ns = textView.string as NSString
        var r = textView.selectedRange()
        if r.length == 0 { r = TextUnits.paragraphRange(in: ns, at: r.location) }
        // Trim surrounding whitespace so the highlight hugs the words.
        while r.length > 0, let s = UnicodeScalar(ns.character(at: r.location)), CharacterSet.whitespacesAndNewlines.contains(s) { r.location += 1; r.length -= 1 }
        while r.length > 0, let s = UnicodeScalar(ns.character(at: NSMaxRange(r) - 1)), CharacterSet.whitespacesAndNewlines.contains(s) { r.length -= 1 }
        return r.length > 0 ? r : nil
    }

    /// Rectangle of `range` in the text view, for anchoring popovers.
    func rect(for range: NSRange) -> NSRect {
        guard let lm = textView.layoutManager, let tc = textView.textContainer else { return .zero }
        let glyphs = lm.glyphRange(forCharacterRange: NSRange(location: range.location, length: min(range.length, 1)), actualCharacterRange: nil)
        var r = lm.boundingRect(forGlyphRange: glyphs, in: tc)
        r.origin.x += textView.textContainerOrigin.x
        r.origin.y += textView.textContainerOrigin.y
        return r
    }

    @objc func addComment(_ sender: Any?) {
        if preview.isActive { preview.close() }
        guard let range = commentTargetRange() else { NSSound.beep(); return }
        let quote = (textView.string as NSString).substring(with: range)
        textView.setSelectedRange(range)
        textView.scrollRangeToVisible(range)
        showChrome(autoHide: false)
        composer.present(quote: quote, relativeTo: rect(for: range), of: textView) { [weak self] body in
            guard let self else { return }
            let c = self.notes.addComment(range: range, body: body)
            self.textView.setSelectedRange(NSRange(location: NSMaxRange(range), length: 0))
            self.window?.makeFirstResponder(self.textView)
            if let c { self.notes.activeCommentID = c.id }
            self.assist?.refreshLists()
            let n = self.notes.openComments.count
            self.flashMessage("\(n) open comment\(n == 1 ? "" : "s") · ⇧⌘M to review and send")
        }
    }

    func editComment(_ id: String) {
        guard let c = notes.comment(id), !c.detached else { return }
        textView.setSelectedRange(c.range)
        textView.scrollRangeToVisible(c.range)
        composer.present(quote: c.quote, initial: c.body, relativeTo: rect(for: c.range), of: textView) { [weak self] body in
            self?.notes.updateComment(id, body: body)
            self?.assist?.refreshLists()
        }
    }

    /// Selects a comment's text in the editor.
    func reveal(comment id: String) {
        if preview.isActive { preview.close() }
        guard let c = notes.comment(id), !c.detached else { return }
        notes.activeCommentID = id
        textView.setSelectedRange(c.range)
        textView.scrollRangeToVisible(c.range)
        if focus.centersCaret { centerCaret(animated: true) }
    }
}
