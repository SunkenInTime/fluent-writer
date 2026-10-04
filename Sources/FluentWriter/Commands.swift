import AppKit
import WriterCore

extension EditorController: NSMenuItemValidation {
    // MARK: File

    @objc func newDraft(_ sender: Any?) {
        if draft.isEmptyUntitled { window?.makeFirstResponder(textView); return }
        guard confirmLeavingDocument() else { return }
        load(library.newDraft())
    }

    @objc func openDraftPicker(_ sender: Any?) {
        guard let window else { return }
        flush()
        OpenPicker.present(on: window, library: library, current: draft.url) { [weak self] choice in
            guard let self else { return }
            switch choice {
            case let .draft(url): self.open(url)
            case .browse: self.browseForFile()
            case .new: self.newDraft(nil)
            }
        }
    }

    @objc func openOtherFile(_ sender: Any?) { browseForFile() }

    func browseForFile() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = DraftLibrary.fileExtensions.compactMap { .init(filenameExtension: $0) }
        panel.allowsMultipleSelection = false
        panel.directoryURL = library.draftsDirectory
        guard let window else { return }
        panel.beginSheetModal(for: window) { [weak self] response in
            guard response == .OK, let url = panel.url else { return }
            self?.open(url)
        }
    }

    func open(_ url: URL) {
        if url.standardizedFileURL == draft.url { window?.makeFirstResponder(textView); return }
        guard confirmLeavingDocument() else { return }
        do {
            load(try library.open(url))
            NSDocumentController.shared.noteNewRecentDocumentURL(url)
        } catch {
            presentError(error)
        }
    }

    @objc func saveNow(_ sender: Any?) {
        if draft.isEmptyUntitled { return }
        if flush() { flashMessage("Saved") }
    }

    @objc func saveDraftAs(_ sender: Any?) { saveAs() }

    @objc func renameDraft(_ sender: Any?) {
        showChrome(autoHide: false)
        window?.makeFirstResponder(titleField)
        titleField.currentEditor()?.selectAll(nil)
    }

    @objc func revealInFinder(_ sender: Any?) {
        if draft.url == nil { flush() }
        guard let url = draft.url else { return }
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }

    @objc func copyForTwitter(_ sender: Any?) {
        let ns = textView.string as NSString
        let sel = textView.selectedRange()
        let source = sel.length > 0 ? ns.substring(with: sel) : textView.string
        let plain = PlainTextExporter.export(source)
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(plain, forType: .string)
        let count = TextStats.counts(forPlainText: plain).characters
        let nf = NumberFormatter(); nf.numberStyle = .decimal
        let what = sel.length > 0 ? "selection" : "draft"
        flashMessage("Copied \(what) as plain text · \(nf.string(from: NSNumber(value: count)) ?? "\(count)") characters")
    }

    // MARK: Format

    private func formatInline(_ delimiter: String, name: String) {
        apply(InlineFormatting.toggle(delimiter, in: textView.string as NSString, selection: textView.selectedRange()), actionName: name)
    }

    @objc func toggleBold(_ sender: Any?) { formatInline("**", name: "Bold") }
    @objc func toggleItalic(_ sender: Any?) { formatInline("*", name: "Italic") }
    @objc func toggleStrikethrough(_ sender: Any?) { formatInline("~~", name: "Strikethrough") }

    @objc func insertLink(_ sender: Any?) {
        apply(InlineFormatting.link(in: textView.string as NSString, selection: textView.selectedRange()), actionName: "Link")
    }

    private func setPrefix(_ p: InlineFormatting.LinePrefix, name: String) {
        apply(InlineFormatting.setLinePrefix(p, in: textView.string as NSString, selection: textView.selectedRange()), actionName: name)
    }

    @objc func formatHeading1(_ sender: Any?) { setPrefix(.heading(1), name: "Heading") }
    @objc func formatHeading2(_ sender: Any?) { setPrefix(.heading(2), name: "Heading") }
    @objc func formatHeading3(_ sender: Any?) { setPrefix(.heading(3), name: "Heading") }
    @objc func formatBody(_ sender: Any?) { setPrefix(.body, name: "Body") }
    @objc func formatBulletList(_ sender: Any?) { setPrefix(.bullet, name: "Bulleted List") }
    @objc func formatNumberedList(_ sender: Any?) { setPrefix(.numbered, name: "Numbered List") }
    @objc func formatQuote(_ sender: Any?) { setPrefix(.quote, name: "Blockquote") }

    @objc func indentLines(_ sender: Any?) {
        if let e = ListContinuation.indentEdit(in: textView.string as NSString, selection: textView.selectedRange(), outdent: false) { apply(e, actionName: "Indent") }
    }

    @objc func outdentLines(_ sender: Any?) {
        if let e = ListContinuation.indentEdit(in: textView.string as NSString, selection: textView.selectedRange(), outdent: true) { apply(e, actionName: "Outdent") }
    }

    // MARK: View

    @objc func toggleFocus(_ sender: Any?) {
        setFocus(focus.dimsSurroundings ? .off : Preferences.lastDimmingFocus)
    }

    @objc func focusOff(_ sender: Any?) { setFocus(.off) }
    @objc func focusSentence(_ sender: Any?) { setFocus(.sentence) }
    @objc func focusParagraph(_ sender: Any?) { setFocus(.paragraph) }
    @objc func focusTypewriter(_ sender: Any?) { setFocus(.typewriter) }

    func setTextSize(_ size: CGFloat) {
        let anchor = textView.selectedRange()
        Preferences.textSize = size
        styler.setFontSize(size)
        applyTypography()
        updateTextLayout()
        textView.setSelectedRange(anchor)
        if focus.centersCaret { centerCaret(animated: false) } else { textView.scrollRangeToVisible(anchor) }
        flashMessage("Text size \(Int(size)) pt")
    }

    @objc func biggerText(_ sender: Any?) {
        if let next = Theme.textSizes.first(where: { $0 > styler.fontSize }) { setTextSize(next) }
    }

    @objc func smallerText(_ sender: Any?) {
        if let next = Theme.textSizes.last(where: { $0 < styler.fontSize }) { setTextSize(next) }
    }

    @objc func defaultTextSize(_ sender: Any?) { setTextSize(Theme.defaultTextSize) }

    @objc func setAppearanceFromMenu(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String, let a = Appearance(rawValue: raw) else { return }
        Preferences.appearance = a
        NSApp.appearance = a.nsAppearance
    }

    @objc func toggleCounts(_ sender: Any?) {
        Preferences.showCounts.toggle()
        countsLabel.isHidden = !Preferences.showCounts
    }

    @objc func toggleSpellCheck(_ sender: Any?) {
        Preferences.spellCheck.toggle()
        textView.isContinuousSpellCheckingEnabled = Preferences.spellCheck
    }

    // MARK: Validation

    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        switch item.action {
        case #selector(focusOff(_:)): item.state = focus == .off ? .on : .off
        case #selector(focusSentence(_:)): item.state = focus == .sentence ? .on : .off
        case #selector(focusParagraph(_:)): item.state = focus == .paragraph ? .on : .off
        case #selector(focusTypewriter(_:)): item.state = focus == .typewriter ? .on : .off
        case #selector(toggleFocus(_:)): item.title = focus.dimsSurroundings ? "Turn Focus Off" : "Turn \(focusName(Preferences.lastDimmingFocus)) Focus On"
        case #selector(setAppearanceFromMenu(_:)): item.state = (item.representedObject as? String) == Preferences.appearance.rawValue ? .on : .off
        case #selector(toggleCounts(_:)): item.state = Preferences.showCounts ? .on : .off
        case #selector(toggleSpellCheck(_:)): item.state = Preferences.spellCheck ? .on : .off
        case #selector(biggerText(_:)): return styler.fontSize < (Theme.textSizes.last ?? 32)
        case #selector(smallerText(_:)): return styler.fontSize > (Theme.textSizes.first ?? 14)
        case #selector(toggleFileSidebar(_:)): item.title = sidebarVisible ? "Hide Sidebar" : "Show Sidebar"
        case #selector(revealInFinder(_:)): return draft.remote == nil && (draft.url != nil || !draft.body.isEmpty)
        case #selector(copyForTwitter(_:)):
            item.title = textView.selectedRange().length > 0 ? "Copy Selection for Twitter" : "Copy for Twitter"
            return !textView.string.isEmpty
        case #selector(openAssist(_:)), #selector(askCodex(_:)), #selector(askClaude(_:)): return !textView.string.isEmpty
        default: break
        }
        return true
    }
}
