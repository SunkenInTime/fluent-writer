import AppKit

/// NSTextView with iA's wide, bright caret and hooks the editor controller needs.
class EditorTextView: NSTextView {
    var caretWidth: CGFloat = 2
    var onKeyTyping: (() -> Void)?
    var onMouseSelection: (() -> Void)?
    /// Takes pasted or dropped images (location is the drop point, nil for a paste). Returns false to
    /// let the normal text paste or drop happen.
    var onImagePaste: ((NSPasteboard, Int?) -> Bool)?
    /// Height of the visible caret; the line height includes leading we don't want to cover.
    var caretHeight: CGFloat = 0

    /// Extra scrollable space below the last line so it can rise to the reading position.
    var bottomOverscroll: CGFloat = 0

    override var acceptsFirstResponder: Bool { true }

    override func setFrameSize(_ newSize: NSSize) {
        var size = newSize
        if let lm = layoutManager, let tc = textContainer {
            let used = lm.usedRect(for: tc).height + textContainerInset.height * 2
            size.height = max(minSize.height, used + bottomOverscroll)
        }
        super.setFrameSize(size)
    }

    override func paste(_ sender: Any?) {
        if isEditable, onImagePaste?(.general, nil) == true { return }
        super.paste(sender)
    }

    /// A plain-text view disables Paste when the clipboard only holds an image; allow it so ⌘V reaches us.
    override func validateMenuItem(_ item: NSMenuItem) -> Bool {
        if item.action == #selector(paste(_:)), isEditable, onImagePaste != nil, carriesImages(.general) { return true }
        return super.validateMenuItem(item)
    }

    override var acceptableDragTypes: [NSPasteboard.PasteboardType] {
        super.acceptableDragTypes + [.fileURL, .png, .tiff]
    }

    private func carriesImages(_ pb: NSPasteboard) -> Bool {
        if pb.canReadObject(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true, .urlReadingContentsConformToTypes: ["public.image"]]) { return true }
        return pb.string(forType: .string) == nil && (pb.data(forType: .png) != nil || pb.data(forType: .tiff) != nil)
    }

    override func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation {
        let op = super.draggingUpdated(sender)
        return isEditable && onImagePaste != nil && carriesImages(sender.draggingPasteboard) ? .copy : op
    }

    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        if isEditable, carriesImages(sender.draggingPasteboard) {
            let index = characterIndexForInsertion(at: convert(sender.draggingLocation, from: nil))
            if onImagePaste?(sender.draggingPasteboard, index) == true { return true }
        }
        return super.performDragOperation(sender)
    }

    override func keyDown(with event: NSEvent) {
        onKeyTyping?()
        super.keyDown(with: event)
    }

    override func mouseDown(with event: NSEvent) {
        super.mouseDown(with: event)
        onMouseSelection?()
    }

    private func caretRect(from rect: NSRect) -> NSRect {
        var r = rect
        r.size.width = caretWidth
        if caretHeight > 0 && caretHeight < r.height {
            r.origin.y += ((r.height - caretHeight) / 2).rounded()
            r.size.height = caretHeight
        }
        return r
    }

    override func drawInsertionPoint(in rect: NSRect, color: NSColor, turnedOn flag: Bool) {
        let r = caretRect(from: rect)
        if flag {
            Theme.caret.setFill()
            NSBezierPath(roundedRect: r, xRadius: 1, yRadius: 1).fill()
        } else {
            setNeedsDisplay(r.insetBy(dx: -1, dy: -1), avoidAdditionalLayout: true)
        }
    }

    override func setNeedsDisplay(_ rect: NSRect, avoidAdditionalLayout flag: Bool) {
        var r = rect
        if r.width <= 2 { r.size.width += caretWidth + 1 }
        super.setNeedsDisplay(r, avoidAdditionalLayout: flag)
    }

    /// Caret rectangle in this view's coordinates for the current insertion point.
    func insertionRect() -> NSRect? {
        guard let lm = layoutManager, let tc = textContainer else { return nil }
        let loc = selectedRange().location
        let length = textStorage?.length ?? 0
        var rect: NSRect
        if length == 0 || (loc >= length && lm.extraLineFragmentTextContainer != nil) {
            rect = lm.extraLineFragmentRect
            if rect == .zero, length > 0 {
                let g = lm.glyphIndexForCharacter(at: length - 1)
                rect = lm.lineFragmentRect(forGlyphAt: g, effectiveRange: nil)
            }
        } else {
            let g = lm.glyphIndexForCharacter(at: min(loc, length - 1))
            rect = lm.lineFragmentRect(forGlyphAt: g, effectiveRange: nil)
        }
        _ = tc
        rect.origin.x += textContainerOrigin.x
        rect.origin.y += textContainerOrigin.y
        return rect
    }
}
