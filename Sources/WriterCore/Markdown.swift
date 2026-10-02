import Foundation

public enum MarkdownBlockKind: Equatable, Sendable {
    case paragraph
    case blank
    case heading(level: Int)
    case bulletItem
    case orderedItem(number: Int, delimiter: Character)
    case taskItem(checked: Bool)
    case blockquote
    case codeFence
    case codeBlock
    case horizontalRule

    public var isListItem: Bool {
        switch self {
        case .bulletItem, .orderedItem, .taskItem: return true
        default: return false
        }
    }
}

public enum MarkdownInlineKind: Equatable, Sendable {
    case strong
    case emphasis
    case strikethrough
    case code
    case link
    case autolink
    case bareURL
}

public struct MarkdownInlineSpan: Equatable, Sendable {
    public var kind: MarkdownInlineKind
    /// Full range including delimiters.
    public var range: NSRange
    /// The visible text range (for links, the link text).
    public var contentRange: NSRange
    /// Delimiter ranges (e.g. `**`, `[`, `](url)`).
    public var markerRanges: [NSRange]
    /// For links and URLs, the range of the destination.
    public var urlRange: NSRange?
}

public struct MarkdownLine: Equatable, Sendable {
    /// Range of the line, excluding its line terminator.
    public var range: NSRange
    public var kind: MarkdownBlockKind
    /// Leading indentation (spaces/tabs before the marker).
    public var indentRange: NSRange
    /// The block marker including its trailing whitespace, e.g. `## `, `- `, `1. `, `> `, `- [ ] `.
    public var markerRange: NSRange?
    /// Text after the marker.
    public var contentRange: NSRange
    public var inlines: [MarkdownInlineSpan]
}

public enum MarkdownParser {
    private static func regex(_ pattern: String, _ options: NSRegularExpression.Options = []) -> NSRegularExpression {
        // Patterns are compile-time constants.
        return try! NSRegularExpression(pattern: pattern, options: options)
    }

    static let headingRegex = regex(#"^(#{1,6})(?:[ \t]+|$)"#)
    static let taskRegex = regex(#"^([ \t]*)([-*+])[ \t]+\[([ xX])\](?:[ \t]+|$)"#)
    static let bulletRegex = regex(#"^([ \t]*)([-*+])(?:[ \t]+)"#)
    static let orderedRegex = regex(#"^([ \t]*)(\d{1,9})([.)])(?:[ \t]+)"#)
    static let quoteRegex = regex(#"^(?:>[ \t]?)+"#)
    static let ruleRegex = regex(#"^[ \t]{0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$"#)
    static let fenceRegex = regex(#"^[ \t]{0,3}(`{3,}|~{3,})"#)

    static let codeRegex = regex(#"(`+)(?!`)(.+?)(?<!`)\1(?!`)"#)
    static let linkRegex = regex(#"(!?)\[([^\]\n]*)\]\(([^)\s]+)(?:[ \t]+"[^"\n]*")?\)"#)
    static let autolinkRegex = regex(#"<((?:https?|mailto):[^>\s]+)>"#)
    static let bareURLRegex = regex(#"(?<![\w(\[<])(?:https?://|www\.)[^\s<>]*[^\s<>.,;:!?'")\]*_~]"#)
    static let tripleRegex = regex(#"(?<!\*)\*\*\*(?=[^\s*])(.+?)(?<=[^\s*])\*\*\*(?!\*)"#)
    static let strongRegex = regex(#"(\*\*|__)(?=\S)(.+?)(?<=\S)\1"#)
    static let emphasisStarRegex = regex(#"(?<![\*\\])\*(?=[^\s\*])(.+?)(?<=[^\s\*\\])\*(?!\*)"#)
    static let emphasisUnderscoreRegex = regex(#"(?<![\w_\\])_(?=[^\s_])(.+?)(?<=[^\s_\\])_(?![\w_])"#)
    static let strikeRegex = regex(#"~~(?=\S)(.+?)(?<=\S)~~"#)

    /// Parses every line of `text`. Code-fence state is tracked across lines.
    public static func parse(_ text: NSString) -> [MarkdownLine] {
        var lines: [MarkdownLine] = []
        var inFence = false
        var fenceMarker = ""
        var location = 0
        let length = text.length
        while location <= length {
            var lineEnd = 0
            var contentsEnd = 0
            if location == length {
                // Trailing empty line after a final newline (or empty document).
                if location > 0 && !isNewline(text.character(at: location - 1)) { break }
                lineEnd = length
                contentsEnd = length
            } else {
                text.getLineStart(nil, end: &lineEnd, contentsEnd: &contentsEnd, for: NSRange(location: location, length: 0))
            }
            let lineRange = NSRange(location: location, length: contentsEnd - location)
            let line = text.substring(with: lineRange) as NSString
            var parsed = parseLine(line, offset: location, inFence: &inFence, fenceMarker: &fenceMarker)
            parsed.range = lineRange
            lines.append(parsed)
            if lineEnd == location { break }
            location = lineEnd
        }
        return lines
    }

    private static func isNewline(_ c: unichar) -> Bool {
        return c == 0x0A || c == 0x0D || c == 0x2028 || c == 0x2029
    }

    /// Parses a single line in isolation (no fence context).
    public static func parseLine(_ line: NSString, offset: Int = 0) -> MarkdownLine {
        var inFence = false
        var marker = ""
        return parseLine(line, offset: offset, inFence: &inFence, fenceMarker: &marker)
    }

    static func parseLine(_ line: NSString, offset: Int, inFence: inout Bool, fenceMarker: inout String) -> MarkdownLine {
        let full = NSRange(location: 0, length: line.length)
        func shifted(_ r: NSRange) -> NSRange { NSRange(location: r.location + offset, length: r.length) }
        let lineRange = NSRange(location: offset, length: line.length)
        let emptyIndent = NSRange(location: offset, length: 0)

        if let m = fenceRegex.firstMatch(in: line as String, range: full) {
            let marker = line.substring(with: m.range(at: 1))
            if !inFence {
                inFence = true
                fenceMarker = String(marker.prefix(1))
                return MarkdownLine(range: lineRange, kind: .codeFence, indentRange: emptyIndent, markerRange: shifted(m.range), contentRange: shifted(NSRange(location: NSMaxRange(m.range), length: line.length - NSMaxRange(m.range))), inlines: [])
            } else if marker.hasPrefix(fenceMarker) {
                inFence = false
                return MarkdownLine(range: lineRange, kind: .codeFence, indentRange: emptyIndent, markerRange: shifted(m.range), contentRange: shifted(NSRange(location: NSMaxRange(m.range), length: line.length - NSMaxRange(m.range))), inlines: [])
            }
        }
        if inFence {
            return MarkdownLine(range: lineRange, kind: .codeBlock, indentRange: emptyIndent, markerRange: nil, contentRange: lineRange, inlines: [])
        }

        let trimmed = (line as String).trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty {
            return MarkdownLine(range: lineRange, kind: .blank, indentRange: emptyIndent, markerRange: nil, contentRange: lineRange, inlines: [])
        }
        if ruleRegex.firstMatch(in: line as String, range: full) != nil {
            return MarkdownLine(range: lineRange, kind: .horizontalRule, indentRange: emptyIndent, markerRange: lineRange, contentRange: NSRange(location: NSMaxRange(lineRange), length: 0), inlines: [])
        }

        var kind: MarkdownBlockKind = .paragraph
        var indent = NSRange(location: 0, length: 0)
        var marker: NSRange?
        if let m = headingRegex.firstMatch(in: line as String, range: full) {
            kind = .heading(level: m.range(at: 1).length)
            marker = m.range
        } else if let m = taskRegex.firstMatch(in: line as String, range: full) {
            let check = line.substring(with: m.range(at: 3))
            kind = .taskItem(checked: check.lowercased() == "x")
            indent = m.range(at: 1)
            marker = NSRange(location: NSMaxRange(indent), length: NSMaxRange(m.range) - NSMaxRange(indent))
        } else if let m = bulletRegex.firstMatch(in: line as String, range: full) {
            kind = .bulletItem
            indent = m.range(at: 1)
            marker = NSRange(location: NSMaxRange(indent), length: NSMaxRange(m.range) - NSMaxRange(indent))
        } else if let m = orderedRegex.firstMatch(in: line as String, range: full) {
            let number = Int(line.substring(with: m.range(at: 2))) ?? 1
            let delimiter = Character(line.substring(with: m.range(at: 3)))
            kind = .orderedItem(number: number, delimiter: delimiter)
            indent = m.range(at: 1)
            marker = NSRange(location: NSMaxRange(indent), length: NSMaxRange(m.range) - NSMaxRange(indent))
        } else if let m = quoteRegex.firstMatch(in: line as String, range: full) {
            kind = .blockquote
            marker = m.range
        }

        let contentStart = marker.map { NSMaxRange($0) } ?? 0
        let content = NSRange(location: contentStart, length: line.length - contentStart)
        let inlines = parseInlines(line, in: content).map { span -> MarkdownInlineSpan in
            var s = span
            s.range = shifted(s.range)
            s.contentRange = shifted(s.contentRange)
            s.markerRanges = s.markerRanges.map(shifted)
            s.urlRange = s.urlRange.map(shifted)
            return s
        }
        return MarkdownLine(
            range: lineRange,
            kind: kind,
            indentRange: shifted(indent.length > 0 ? indent : NSRange(location: 0, length: 0)),
            markerRange: marker.map(shifted),
            contentRange: shifted(content),
            inlines: inlines
        )
    }

    /// Finds inline spans in `text` within `range`. Ranges are relative to `text`.
    public static func parseInlines(_ text: NSString, in range: NSRange) -> [MarkdownInlineSpan] {
        let string = text as String
        var spans: [MarkdownInlineSpan] = []
        var protected: [NSRange] = []

        func overlapsProtected(_ r: NSRange) -> Bool {
            protected.contains { NSIntersectionRange($0, r).length > 0 }
        }

        for m in codeRegex.matches(in: string, range: range) {
            let ticks = m.range(at: 1).length
            let open = NSRange(location: m.range.location, length: ticks)
            let close = NSRange(location: NSMaxRange(m.range) - ticks, length: ticks)
            spans.append(MarkdownInlineSpan(kind: .code, range: m.range, contentRange: m.range(at: 2), markerRanges: [open, close], urlRange: nil))
            protected.append(m.range)
        }
        for m in linkRegex.matches(in: string, range: range) where !overlapsProtected(m.range) {
            let bang = m.range(at: 1)
            let textRange = m.range(at: 2)
            let url = m.range(at: 3)
            let open = NSRange(location: m.range.location, length: bang.length + 1)
            let close = NSRange(location: NSMaxRange(textRange), length: NSMaxRange(m.range) - NSMaxRange(textRange))
            spans.append(MarkdownInlineSpan(kind: .link, range: m.range, contentRange: textRange, markerRanges: [open, close], urlRange: url))
            // Only the destination is protected; the link text may carry emphasis.
            protected.append(close)
        }
        for m in autolinkRegex.matches(in: string, range: range) where !overlapsProtected(m.range) {
            let open = NSRange(location: m.range.location, length: 1)
            let close = NSRange(location: NSMaxRange(m.range) - 1, length: 1)
            spans.append(MarkdownInlineSpan(kind: .autolink, range: m.range, contentRange: m.range(at: 1), markerRanges: [open, close], urlRange: m.range(at: 1)))
            protected.append(m.range)
        }
        for m in bareURLRegex.matches(in: string, range: range) where !overlapsProtected(m.range) {
            spans.append(MarkdownInlineSpan(kind: .bareURL, range: m.range, contentRange: m.range, markerRanges: [], urlRange: m.range))
            protected.append(m.range)
        }

        for m in tripleRegex.matches(in: string, range: range) where !overlapsProtected(m.range) {
            let r = m.range
            let inner = NSRange(location: r.location + 2, length: r.length - 4)
            spans.append(MarkdownInlineSpan(kind: .strong, range: r, contentRange: inner, markerRanges: [NSRange(location: r.location, length: 2), NSRange(location: NSMaxRange(r) - 2, length: 2)], urlRange: nil))
            spans.append(MarkdownInlineSpan(kind: .emphasis, range: inner, contentRange: m.range(at: 1), markerRanges: [NSRange(location: inner.location, length: 1), NSRange(location: NSMaxRange(inner) - 1, length: 1)], urlRange: nil))
            protected.append(NSRange(location: r.location, length: 3))
            protected.append(NSRange(location: NSMaxRange(r) - 3, length: 3))
        }
        var strongRanges: [NSRange] = []
        for m in strongRegex.matches(in: string, range: range) where !overlapsProtected(m.range) {
            let d = m.range(at: 1).length
            let open = NSRange(location: m.range.location, length: d)
            let close = NSRange(location: NSMaxRange(m.range) - d, length: d)
            spans.append(MarkdownInlineSpan(kind: .strong, range: m.range, contentRange: m.range(at: 2), markerRanges: [open, close], urlRange: nil))
            strongRanges.append(open)
            strongRanges.append(close)
        }
        func overlapsStrongMarker(_ r: NSRange) -> Bool {
            strongRanges.contains { NSIntersectionRange($0, r).length > 0 }
        }
        for regex in [emphasisStarRegex, emphasisUnderscoreRegex] {
            for m in regex.matches(in: string, range: range) where !overlapsProtected(m.range) {
                let open = NSRange(location: m.range.location, length: 1)
                let close = NSRange(location: NSMaxRange(m.range) - 1, length: 1)
                if overlapsStrongMarker(open) || overlapsStrongMarker(close) { continue }
                spans.append(MarkdownInlineSpan(kind: .emphasis, range: m.range, contentRange: m.range(at: 1), markerRanges: [open, close], urlRange: nil))
            }
        }
        for m in strikeRegex.matches(in: string, range: range) where !overlapsProtected(m.range) {
            let open = NSRange(location: m.range.location, length: 2)
            let close = NSRange(location: NSMaxRange(m.range) - 2, length: 2)
            spans.append(MarkdownInlineSpan(kind: .strikethrough, range: m.range, contentRange: m.range(at: 1), markerRanges: [open, close], urlRange: nil))
        }
        spans.sort { $0.range.location < $1.range.location }
        return spans
    }
}
