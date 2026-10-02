import Foundation

/// Converts a Markdown draft into clean plain text suitable for pasting into a post composer.
/// Delimiters are removed, links keep their URLs, lists and paragraph breaks survive.
/// No decorative Unicode styling is ever produced.
public enum PlainTextExporter {
    static let escapeRegex = try! NSRegularExpression(pattern: #"\\([\\`*_{}\[\]()#+\-.!~>|])"#)

    public static func export(_ markdown: String) -> String {
        let ns = markdown as NSString
        let lines = MarkdownParser.parse(ns)
        var output: [String] = []
        for line in lines {
            switch line.kind {
            case .codeFence:
                continue
            case .codeBlock:
                output.append(ns.substring(with: line.range))
            case .blank:
                output.append("")
            case .horizontalRule:
                output.append("")
            case .heading, .paragraph, .blockquote:
                output.append(renderInline(ns, line: line).trimmingTrailingWhitespace())
            case .bulletItem, .taskItem:
                output.append(indent(ns, line) + "• " + renderInline(ns, line: line).trimmingTrailingWhitespace())
            case let .orderedItem(number, delimiter):
                output.append(indent(ns, line) + "\(number)\(delimiter) " + renderInline(ns, line: line).trimmingTrailingWhitespace())
            }
        }
        // Collapse runs of blank lines and trim the ends.
        var collapsed: [String] = []
        for l in output {
            if l.isEmpty, collapsed.last?.isEmpty ?? true { continue }
            collapsed.append(l)
        }
        while collapsed.last?.isEmpty == true { collapsed.removeLast() }
        return collapsed.joined(separator: "\n")
    }

    private static func indent(_ ns: NSString, _ line: MarkdownLine) -> String {
        guard line.indentRange.length > 0 else { return "" }
        let raw = ns.substring(with: line.indentRange).replacingOccurrences(of: "\t", with: "    ")
        let level = raw.count / 2
        return String(repeating: "  ", count: level)
    }

    static func renderInline(_ ns: NSString, line: MarkdownLine) -> String {
        return renderInline(ns, range: line.contentRange, spans: line.inlines)
    }

    /// Renders an arbitrary single-line Markdown fragment.
    public static func renderInlineFragment(_ text: String) -> String {
        let ns = text as NSString
        let range = NSRange(location: 0, length: ns.length)
        return renderInline(ns, range: range, spans: MarkdownParser.parseInlines(ns, in: range))
    }

    static func renderInline(_ ns: NSString, range: NSRange, spans: [MarkdownInlineSpan]) -> String {
        var replacements: [(NSRange, String)] = []
        var claimed: [NSRange] = []
        func isClaimed(_ r: NSRange) -> Bool { claimed.contains { NSIntersectionRange($0, r).length > 0 || ($0.location <= r.location && NSMaxRange(r) <= NSMaxRange($0) && r.length == 0) } }

        let codeRanges = spans.filter { $0.kind == .code }.map { $0.range }
        for span in spans where span.kind == .link {
            let label = renderInlineFragment(ns.substring(with: span.contentRange))
            let url = span.urlRange.map { ns.substring(with: $0) } ?? ""
            let trimmedLabel = label.trimmingCharacters(in: .whitespaces)
            let replacement: String
            if trimmedLabel.isEmpty || trimmedLabel == url || "https://\(trimmedLabel)" == url || "http://\(trimmedLabel)" == url || trimmedLabel.hasPrefix("http") {
                replacement = url
            } else {
                replacement = "\(label) (\(url))"
            }
            replacements.append((span.range, replacement))
            claimed.append(span.range)
        }
        for span in spans where span.kind != .link {
            if isClaimed(span.range) { continue }
            switch span.kind {
            case .bareURL:
                continue
            case .code:
                for m in span.markerRanges { replacements.append((m, "")) }
            default:
                if codeRanges.contains(where: { NSIntersectionRange($0, span.range).length > 0 }) { continue }
                for m in span.markerRanges { replacements.append((m, "")) }
            }
        }
        // Apply replacements back to front.
        let mutable = NSMutableString(string: ns.substring(with: range))
        let unique = Dictionary(replacements.map { ($0.0.location * 100_000 + $0.0.length, $0) }, uniquingKeysWith: { a, _ in a }).values
        let sorted = unique.sorted { $0.0.location > $1.0.location }
        var lastStart = Int.max
        for (r, s) in sorted {
            guard NSMaxRange(r) <= lastStart else { continue }
            let local = NSRange(location: r.location - range.location, length: r.length)
            guard local.location >= 0, NSMaxRange(local) <= mutable.length else { continue }
            mutable.replaceCharacters(in: local, with: s)
            lastStart = r.location
        }
        let result = mutable as String
        if codeRanges.isEmpty {
            return escapeRegex.stringByReplacingMatches(in: result, range: NSRange(location: 0, length: (result as NSString).length), withTemplate: "$1")
        }
        return result
    }
}

extension String {
    func trimmingTrailingWhitespace() -> String {
        var s = Substring(self)
        while let last = s.last, last == " " || last == "\t" { s.removeLast() }
        return String(s)
    }
}
