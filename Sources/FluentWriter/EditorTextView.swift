import AppKit

/// NSTextView with iA's wide, bright caret and hooks the editor controller needs.
final class EditorTextView: NSTextView {
    var caretWidth: CGFloat = 2
    var onKeyTyping: (() -> Void)?
    var onMouseSelection: (() -> Void)?
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
