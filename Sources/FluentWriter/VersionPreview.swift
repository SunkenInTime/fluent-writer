import AppKit
import WriterCore

/// Read-only text view for previews. Arrow keys step between versions, Return uses the shown one,
/// Escape goes back to the draft.
final class PreviewTextView: EditorTextView {
    var onStep: ((Int) -> Void)?
    var onUse: (() -> Void)?
    var onBack: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        switch event.keyCode {
        case 123: onStep?(-1)
        case 124: onStep?(1)
        case 53: onBack?()
        case 36, 76: onUse?()
        default: super.keyDown(with: event)
        }
    }
}

final class PreviewBanner: NSView {
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        Theme.banner.setFill()
        bounds.fill()
        Theme.hairline.setFill()
        NSRect(x: 0, y: bounds.height - 1, width: bounds.width, height: 1).fill()
    }
}

/// Shows a version in place of the draft as an inline diff against it. Versions line up oldest to
/// newest with your draft at the end, so → walks toward the present and ← back in time.
final class VersionPreviewController: NSObject {
    weak var editor: EditorController?
    let scroll = NSScrollView()
    let textView: PreviewTextView
    let banner = PreviewBanner()
    private let titleLabel = NSTextField(labelWithString: "")
    private let detailLabel = NSTextField(labelWithString: "")
    private let positionLabel = NSTextField(labelWithString: "")
    private let prevButton = NSButton()
    private let nextButton = NSButton()
    private let backButton = NSButton(title: "Back to Draft", target: nil, action: nil)
    private let useButton = NSButton(title: "Use This Version", target: nil, action: nil)
    private(set) var versionID: String?
    static let bannerHeight: CGFloat = 44

    var isActive: Bool { versionID != nil }

    init(editor: EditorController) {
        self.editor = editor
        let storage = NSTextStorage()
        let lm = NSLayoutManager()
        lm.allowsNonContiguousLayout = false
        lm.usesFontLeading = false
        storage.addLayoutManager(lm)
        let container = NSTextContainer(size: NSSize(width: 600, height: CGFloat.greatestFiniteMagnitude))
        container.widthTracksTextView = false
        container.lineFragmentPadding = 0
        lm.addTextContainer(container)
        textView = PreviewTextView(frame: NSRect(x: 0, y: 0, width: 900, height: 700), textContainer: container)
        super.init()
        build()
    }

    private func build() {
        let tv = textView
        tv.isEditable = false
        tv.isSelectable = true
        tv.isRichText = false
        tv.drawsBackground = false
        tv.isVerticallyResizable = true
        tv.isHorizontallyResizable = false
        tv.autoresizingMask = [.width]
        tv.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        tv.selectedTextAttributes = [.backgroundColor: Theme.selection]
        tv.setAccessibilityLabel("Version preview")
        tv.onStep = { [weak self] in self?.step($0) }
        tv.onUse = { [weak self] in self?.use() }
        tv.onBack = { [weak self] in self?.close() }
        scroll.documentView = tv
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.scrollerStyle = .overlay
        scroll.drawsBackground = false
        scroll.borderType = .noBorder

        titleLabel.font = Theme.uiFont(size: 12, weight: .medium)
        titleLabel.textColor = Theme.text
        titleLabel.lineBreakMode = .byTruncatingTail
        detailLabel.font = Theme.uiFont(size: 11)
        detailLabel.textColor = Theme.quiet
        detailLabel.lineBreakMode = .byTruncatingTail
        for (b, symbol, tip) in [(prevButton, "chevron.left", "Older version (←)"), (nextButton, "chevron.right", "Newer version (→)")] {
            b.image = NSImage(systemSymbolName: symbol, accessibilityDescription: tip)
            b.isBordered = false
            b.contentTintColor = Theme.text
            b.toolTip = tip
            b.target = self
        }
        prevButton.action = #selector(prevClicked)
        nextButton.action = #selector(nextClicked)
        backButton.target = self
        backButton.action = #selector(backClicked)
        backButton.bezelStyle = .rounded
        backButton.controlSize = .small
        backButton.font = Theme.uiFont(size: 11)
        backButton.toolTip = "Keep your draft as it is (esc)"
        useButton.target = self
        useButton.action = #selector(useClicked)
        useButton.bezelStyle = .rounded
        useButton.controlSize = .small
        useButton.font = Theme.uiFont(size: 11, weight: .medium)
        useButton.keyEquivalent = ""
        useButton.toolTip = "Replace your draft with this version (↩). Your current draft is kept in Versions."
        positionLabel.font = Theme.uiFont(size: 11)
        positionLabel.textColor = Theme.quiet
        positionLabel.alignment = .center
        for v: NSView in [prevButton, positionLabel, titleLabel, detailLabel, nextButton, backButton, useButton] { banner.addSubview(v) }
        banner.setAccessibilityRole(.toolbar)
        banner.setAccessibilityLabel("Version preview")
    }

    // MARK: Layout

    func layout(editorFrame: NSRect) {
        guard isActive, let editor else { return }
        let w = editorFrame.width
        banner.frame = NSRect(x: editorFrame.minX, y: editorFrame.minY, width: w, height: Self.bannerHeight)
        scroll.frame = NSRect(x: editorFrame.minX, y: editorFrame.minY + Self.bannerHeight, width: w, height: max(0, editorFrame.height - Self.bannerHeight))
        // Narrow windows drop "Back to Draft" (esc and → still work), then shorten "Use This Version".
        useButton.title = w < 460 ? "Use" : "Use This Version"
        useButton.sizeToFit()
        backButton.sizeToFit()
        let compact = w < 620
        backButton.isHidden = compact
        let useW = useButton.frame.width + 8
        let backW = backButton.frame.width + 4
        useButton.frame = NSRect(x: w - 16 - useW, y: 11, width: useW, height: 22)
        backButton.frame = NSRect(x: useButton.frame.minX - 8 - backW, y: 11, width: backW, height: 22)
        let posW = max(34, ceil(positionLabel.attributedStringValue.size().width) + 6)
        prevButton.frame = NSRect(x: 12, y: 12, width: 20, height: 20)
        positionLabel.frame = NSRect(x: 34, y: 14, width: posW, height: 16)
        nextButton.frame = NSRect(x: 34 + posW, y: 12, width: 20, height: 20)
        let labelX: CGFloat = nextButton.frame.maxX + 12
        let labelW = max(0, (compact ? useButton.frame.minX : backButton.frame.minX) - labelX - 12)
        titleLabel.frame = NSRect(x: labelX, y: 5, width: labelW, height: 16)
        detailLabel.frame = NSRect(x: labelX, y: 22, width: labelW, height: 15)
        titleLabel.isHidden = labelW < 48
        detailLabel.isHidden = labelW < 48
        banner.toolTip = labelW < 140 ? [titleLabel.stringValue, detailLabel.stringValue].joined(separator: "\n") : nil

        // Same column as the editor, without typewriter centering.
        let main = editor.textView
        textView.textContainer?.containerSize = main.textContainer?.containerSize ?? NSSize(width: 600, height: CGFloat.greatestFiniteMagnitude)
        textView.textContainerInset = NSSize(width: main.textContainerInset.width, height: 40)
        textView.bottomOverscroll = (scroll.contentSize.height * 0.35).rounded()
        textView.minSize = NSSize(width: scroll.contentSize.width, height: scroll.contentSize.height)
        textView.setFrameSize(NSSize(width: scroll.contentSize.width, height: textView.frame.height))
        textView.sizeToFit()
    }

    // MARK: Showing

    /// Versions oldest first; the draft itself sits after the last one.
    private var timeline: [DraftVersion] { (editor?.notes.versions ?? []).reversed() }

    func show(_ id: String) {
        guard let editor, let v = editor.notes.version(id) else { return }
        let first = !isActive
        versionID = id
        if first {
            editor.canvas.addSubview(scroll, positioned: .above, relativeTo: editor.scrollView)
            editor.canvas.addSubview(banner, positioned: .above, relativeTo: scroll)
            editor.scrollView.isHidden = true
        }
        render(v, against: editor.textView.string)
        editor.frameChanged()
        editor.window?.makeFirstResponder(textView)
        scrollToFirstChange()
        editor.assist?.refreshLists()
        editor.updateToolbarState()
        editor.notes.scheduleLiveState()
        NSAccessibility.post(element: banner, notification: .announcementRequested, userInfo: [
            .announcement: "Previewing \(v.label). Left and right arrows switch versions, Return uses it, Escape goes back.",
            .priority: NSAccessibilityPriorityLevel.medium.rawValue,
        ])
    }

    func close() {
        guard isActive, let editor else { return }
        versionID = nil
        scroll.removeFromSuperview()
        banner.removeFromSuperview()
        editor.scrollView.isHidden = false
        editor.frameChanged()
        editor.window?.makeFirstResponder(editor.textView)
        editor.assist?.refreshLists()
        editor.updateToolbarState()
        editor.notes.scheduleLiveState()
    }

    /// Re-renders after the draft or the version list changed underneath the preview.
    func refresh() {
        guard let id = versionID else { return }
        guard let editor, let v = editor.notes.version(id) else { close(); return }
        render(v, against: editor.textView.string)
        editor.frameChanged()
    }

    func step(_ delta: Int) {
        guard let editor else { return }
        let line = timeline
        guard !line.isEmpty else { return }
        // Index `line.count` stands for the draft.
        let current = versionID.flatMap { id in line.firstIndex { $0.id == id } } ?? line.count
        let next = current + delta
        if next < 0 { NSSound.beep(); return }
        if next >= line.count {
            if isActive { close() } else { NSSound.beep() }
            return
        }
        show(line[next].id)
        _ = editor
    }

    func use() {
        guard let id = versionID, let editor else { return }
        editor.useVersion(id)
    }

    @objc private func prevClicked() { step(-1) }
    @objc private func nextClicked() { step(1) }
    @objc private func backClicked() { close() }
    @objc private func useClicked() { use() }

    // MARK: Rendering

    private var firstChange: Int?

    private func render(_ v: DraftVersion, against draft: String) {
        guard let editor else { return }
        let segments = WordDiff.diff(draft, v.body)
        var combined = ""
        var marks: [(NSRange, DiffSegmentKind)] = []
        var added = 0, removed = 0
        firstChange = nil
        var previous: DiffSegmentKind = .equal
        for seg in segments {
            if previous == .delete && seg.kind == .insert,
               let last = combined.last, !last.isWhitespace, let first = seg.text.first, !first.isWhitespace {
                combined += " "
            }
            previous = seg.kind
            let start = (combined as NSString).length
            combined += seg.text
            let r = NSRange(location: start, length: (seg.text as NSString).length)
            if seg.kind != .equal {
                marks.append((r, seg.kind))
                if firstChange == nil { firstChange = start }
                let words = TextStats.counts(forPlainText: seg.text).words
                if seg.kind == .insert { added += words } else { removed += words }
            }
        }
        let storage = textView.textStorage!
        storage.beginEditing()
        storage.replaceCharacters(in: NSRange(location: 0, length: storage.length), with: combined)
        storage.endEditing()
        editor.styler.style(storage, lines: MarkdownParser.parse(combined as NSString), in: nil)
        storage.beginEditing()
        let ns = combined as NSString
        for (whole, kind) in marks {
          for r in Self.lineRuns(whole, in: ns) {
            switch kind {
            case .insert:
                storage.addAttributes([.foregroundColor: Theme.inserted, .backgroundColor: Theme.insertedBackground], range: r)
            case .delete:
                storage.addAttributes([
                    .foregroundColor: Theme.deleted, .backgroundColor: Theme.deletedBackground,
                    .strikethroughStyle: NSUnderlineStyle.single.rawValue, .strikethroughColor: Theme.deleted,
                ], range: r)
            case .equal: break
            }
          }
        }
        storage.endEditing()
        textView.setSelectedRange(NSRange(location: 0, length: 0))

        let line = timeline
        let position = (line.firstIndex { $0.id == v.id } ?? 0) + 1
        titleLabel.stringValue = v.label
        var detail = [v.kind == .snapshot ? "Saved draft" : "From \(v.source)", Self.relative(v.created)]
        if marks.isEmpty { detail.append("same as your draft") } else { detail.append("+\(added) −\(removed) words") }
        if v.usedAt != nil { detail.append("used before") }
        positionLabel.stringValue = "\(position)/\(line.count)"
        positionLabel.setAccessibilityLabel("Version \(position) of \(line.count)")

        detailLabel.stringValue = detail.joined(separator: " · ")
        prevButton.isEnabled = position > 1
        useButton.isEnabled = !marks.isEmpty
        textView.setAccessibilityValue(v.body)
        textView.setAccessibilityHelp("Inserted text is green, removed text is struck through.")
    }

    /// The parts of `range` that aren't line breaks, so added or removed blank lines aren't painted.
    static func lineRuns(_ range: NSRange, in text: NSString) -> [NSRange] {
        var runs: [NSRange] = []
        var start: Int?
        for i in range.location..<NSMaxRange(range) {
            let c = text.character(at: i)
            if c == 0x0A || c == 0x0D {
                if let s = start { runs.append(NSRange(location: s, length: i - s)); start = nil }
            } else if start == nil {
                start = i
            }
        }
        if let s = start { runs.append(NSRange(location: s, length: NSMaxRange(range) - s)) }
        return runs
    }

    private func scrollToFirstChange() {
        guard let first = firstChange else { scroll.contentView.scroll(to: .zero); return }
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.textView.scrollRangeToVisible(NSRange(location: first, length: 0))
        }
    }

    static func relative(_ date: Date) -> String {
        let f = RelativeDateTimeFormatter()
        f.unitsStyle = .short
        let s = Date().timeIntervalSince(date)
        return s < 30 ? "just now" : f.localizedString(for: date, relativeTo: Date())
    }
}

extension EditorController {
    /// Replaces the draft with a version as one undoable step, keeping the current draft as a snapshot.
    func useVersion(_ id: String) {
        guard let v = notes.version(id) else { return }
        let current = textView.string
        if current != v.body {
            notes.snapshotCurrentDraft()
            let tracked = applyText(v.body, actionName: "Use Version")
            notes.settleAfterRewrite(tracked: tracked)
        }
        notes.markUsed(id)
        preview.close()
        flashMessage("Version used · ⌘Z or Versions to go back")
    }

    /// Rewrites the draft to `target` with word-level edits, so untouched text (and comments on it) stay put.
    /// Returns false if it had to fall back to replacing everything, which loses comment tracking.
    @discardableResult
    func applyText(_ target: String, actionName: String) -> Bool {
        let current = textView.string
        var edits: [(NSRange, String)] = []
        var pos = 0
        var pending: (start: Int, length: Int, insert: String)?
        func flush() {
            if let p = pending { edits.append((NSRange(location: p.start, length: p.length), p.insert)) }
            pending = nil
        }
        for seg in WordDiff.diff(current, target) {
            let len = (seg.text as NSString).length
            switch seg.kind {
            case .equal:
                flush()
                pos += len
            case .delete:
                if pending == nil { pending = (pos, 0, "") }
                pending!.length += len
                pos += len
            case .insert:
                if pending == nil { pending = (pos, 0, "") }
                pending!.insert += seg.text
            }
        }
        flush()
        guard !edits.isEmpty, let undo = textView.undoManager else { return true }
        var tracked = true
        textView.breakUndoCoalescing()
        undo.beginUndoGrouping()
        for (range, text) in edits.reversed() {
            guard textView.shouldChangeText(in: range, replacementString: text) else { continue }
            textView.textStorage?.replaceCharacters(in: range, with: text)
            textView.didChangeText()
        }
        if textView.string != target {
            // Fall back to a single replacement if the word edits didn't land exactly.
            let full = NSRange(location: 0, length: (textView.string as NSString).length)
            if textView.shouldChangeText(in: full, replacementString: target) {
                textView.textStorage?.replaceCharacters(in: full, with: target)
                textView.didChangeText()
                tracked = false
            }
        }
        undo.endUndoGrouping()
        undo.setActionName(actionName)
        textView.typingAttributes = styler.baseAttributes
        if let first = edits.first {
            textView.setSelectedRange(NSRange(location: min(first.0.location, (textView.string as NSString).length), length: 0))
        }
        return tracked
    }

    @objc func showVersions(_ sender: Any?) {
        ensureAssist().show(provider: nil, tab: .versions)
    }

    @objc func previousVersion(_ sender: Any?) { preview.step(-1) }
    @objc func nextVersion(_ sender: Any?) { preview.step(1) }
    @objc func useShownVersion(_ sender: Any?) { preview.use() }
}
