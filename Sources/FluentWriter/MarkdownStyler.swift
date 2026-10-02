import AppKit
import WriterCore

/// Applies iA-style Markdown presentation: one monospaced size, bold headings, grey markup,
/// and block markers that hang into the left margin so prose stays on one vertical edge.
final class MarkdownStyler {
    private(set) var fontSize: CGFloat
    private(set) var regular: NSFont
    private(set) var bold: NSFont
    private(set) var italic: NSFont
    private(set) var boldItalic: NSFont
    private(set) var lineHeight: CGFloat
    private(set) var baselineOffset: CGFloat
    private(set) var characterWidth: CGFloat

    /// Width reserved left of the prose edge for hanging markers like `## ` and `- `.
    var hangWidth: CGFloat { characterWidth * 4 }

    init(fontSize: CGFloat) {
        self.fontSize = fontSize
        regular = Theme.font(size: fontSize)
        bold = regular; italic = regular; boldItalic = regular
        lineHeight = 0; baselineOffset = 0; characterWidth = 0
        setFontSize(fontSize)
    }

    func setFontSize(_ size: CGFloat) {
        fontSize = size
        regular = Theme.font(size: size)
        bold = Theme.font(size: size, bold: true)
        italic = Theme.font(size: size, italic: true)
        boldItalic = Theme.font(size: size, bold: true, italic: true)
        characterWidth = Theme.characterWidth(for: regular)
        lineHeight = (size * Theme.lineHeightMultiple).rounded()
        let natural = NSLayoutManager().defaultLineHeight(for: regular)
        baselineOffset = max(0, ((lineHeight - natural) / 2).rounded())
    }

    func paragraphStyle(firstLineIndent: CGFloat? = nil, headIndent: CGFloat? = nil) -> NSParagraphStyle {
        let p = NSMutableParagraphStyle()
        p.minimumLineHeight = lineHeight
        p.maximumLineHeight = lineHeight
        p.firstLineHeadIndent = firstLineIndent ?? hangWidth
        p.headIndent = headIndent ?? hangWidth
        p.lineBreakMode = .byWordWrapping
        p.lineBreakStrategy = []
        p.allowsDefaultTighteningForTruncation = false
        return p
    }

    var baseAttributes: [NSAttributedString.Key: Any] {
        [
            .font: regular,
            .foregroundColor: Theme.text,
            .paragraphStyle: paragraphStyle(),
            .baselineOffset: baselineOffset,
        ]
    }

    /// Styles the lines intersecting `range`, or the whole document when `range` is nil.
    func style(_ storage: NSTextStorage, lines: [MarkdownLine], in range: NSRange?) {
        let length = storage.length
        let target: NSRange
        if let range {
            let start = lines.firstIndex { NSMaxRange($0.range) >= range.location } ?? 0
            let end = lines.lastIndex { $0.range.location <= NSMaxRange(range) } ?? lines.count - 1
            guard start <= end, !lines.isEmpty else { return }
            let lo = lines[start].range.location
            let hi = min(length, end + 1 < lines.count ? lines[end + 1].range.location : length)
            target = NSRange(location: lo, length: hi - lo)
            storage.beginEditing()
            storage.setAttributes(baseAttributes, range: target)
            for line in lines[start...end] { styleLine(line, storage) }
            storage.endEditing()
        } else {
            target = NSRange(location: 0, length: length)
            storage.beginEditing()
            storage.setAttributes(baseAttributes, range: target)
            for line in lines { styleLine(line, storage) }
            storage.endEditing()
        }
    }

    private func width(_ r: NSRange?) -> CGFloat { CGFloat(r?.length ?? 0) * characterWidth }

    private func styleLine(_ line: MarkdownLine, _ s: NSTextStorage) {
        let markup = Theme.markup
        let fullLine = NSRange(location: line.range.location, length: min(line.range.length + 1, s.length - line.range.location))
        switch line.kind {
        case .heading:
            s.addAttribute(.font, value: bold, range: line.range)
            if let m = line.markerRange { s.addAttribute(.foregroundColor, value: markup, range: m) }
            let indent = max(0, hangWidth - width(line.markerRange))
            s.addAttribute(.paragraphStyle, value: paragraphStyle(firstLineIndent: indent, headIndent: hangWidth), range: fullLine)
        case .bulletItem, .orderedItem, .taskItem, .blockquote:
            if let m = line.markerRange { s.addAttribute(.foregroundColor, value: markup, range: m) }
            let indentW = width(line.indentRange)
            let first = max(0, hangWidth - width(line.markerRange))
            let head = first + indentW + width(line.markerRange)
            s.addAttribute(.paragraphStyle, value: paragraphStyle(firstLineIndent: first, headIndent: head), range: fullLine)
        case .codeFence, .horizontalRule:
            s.addAttribute(.foregroundColor, value: markup, range: line.range)
            return
        case .codeBlock:
            s.addAttribute(.foregroundColor, value: Theme.quiet, range: line.range)
            return
        case .paragraph, .blank:
            break
        }
        for span in line.inlines { styleInline(span, s) }
    }

    private func applyTrait(bold makeBold: Bool, italic makeItalic: Bool, in range: NSRange, _ s: NSTextStorage) {
        s.enumerateAttribute(.font, in: range) { value, r, _ in
            let f = value as? NSFont ?? regular
            let traits = f.fontDescriptor.symbolicTraits
            let b = makeBold || traits.contains(.bold)
            let i = makeItalic || traits.contains(.italic)
            s.addAttribute(.font, value: b ? (i ? boldItalic : bold) : (i ? italic : regular), range: r)
        }
    }

    private func styleInline(_ span: MarkdownInlineSpan, _ s: NSTextStorage) {
        let markup = Theme.markup
        switch span.kind {
        case .strong:
            applyTrait(bold: true, italic: false, in: span.range, s)
        case .emphasis:
            applyTrait(bold: false, italic: true, in: span.range, s)
        case .strikethrough:
            s.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: span.contentRange)
            s.addAttribute(.strikethroughColor, value: Theme.quiet, range: span.contentRange)
        case .code:
            s.addAttribute(.backgroundColor, value: Theme.codeBackground, range: span.range)
        case .link:
            s.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: span.contentRange)
            s.addAttribute(.underlineColor, value: Theme.markup, range: span.contentRange)
            if let u = span.urlRange { s.addAttribute(.foregroundColor, value: markup, range: u) }
        case .autolink, .bareURL:
            s.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: span.contentRange)
            s.addAttribute(.underlineColor, value: Theme.markup, range: span.contentRange)
        }
        for m in span.markerRanges { s.addAttribute(.foregroundColor, value: markup, range: m) }
    }
}
