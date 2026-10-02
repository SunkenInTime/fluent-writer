import Foundation

/// A replacement in UTF-16 coordinates plus the selection to apply afterwards.
public struct TextEdit: Equatable, Sendable {
    public var range: NSRange
    public var replacement: String
    public var selectionAfter: NSRange

    public init(range: NSRange, replacement: String, selectionAfter: NSRange) {
        self.range = range
        self.replacement = replacement
        self.selectionAfter = selectionAfter
    }

    public func applied(to text: String) -> String {
        let ns = NSMutableString(string: text)
        ns.replaceCharacters(in: range, with: replacement)
        return ns as String
    }
}

public enum ListContinuation {
    /// The edit for pressing Return. Returns nil when the default newline behavior applies.
    public static func newlineEdit(in text: NSString, selection: NSRange) -> TextEdit? {
        guard selection.length == 0 else { return nil }
        let caret = selection.location
        let lineRange = TextUnits.paragraphRange(in: text, at: caret)
        let line = MarkdownParser.parseLine(text.substring(with: lineRange) as NSString, offset: lineRange.location)
        guard let marker = line.markerRange else { return nil }
        let continuing: Bool
        switch line.kind {
        case .bulletItem, .orderedItem, .taskItem, .blockquote: continuing = true
        default: continuing = false
        }
        guard continuing, caret >= NSMaxRange(marker) else { return nil }

        let content = text.substring(with: line.contentRange).trimmingCharacters(in: .whitespaces)
        if content.isEmpty {
            // Return on an empty item ends the list.
            let removal = NSRange(location: lineRange.location, length: NSMaxRange(line.contentRange) - lineRange.location)
            return TextEdit(range: removal, replacement: "", selectionAfter: NSRange(location: lineRange.location, length: 0))
        }

        let indent = text.substring(with: line.indentRange)
        let markerText = text.substring(with: marker)
        let next: String
        switch line.kind {
        case let .orderedItem(number, delimiter):
            next = indent + "\(number + 1)\(delimiter) "
        case .taskItem:
            let bullet = markerText.first.map(String.init) ?? "-"
            next = indent + bullet + " [ ] "
        case .bulletItem:
            next = indent + markerText.trimmingCharacters(in: .whitespaces) + " "
        case .blockquote:
            next = markerText.hasSuffix(" ") ? markerText : markerText + " "
        default:
            return nil
        }
        // Drop trailing spaces before the caret so lines don't end in whitespace.
        var start = caret
        while start > NSMaxRange(marker), text.character(at: start - 1) == 0x20 { start -= 1 }
        let insertion = "\n" + next
        let range = NSRange(location: start, length: caret - start)
        return TextEdit(range: range, replacement: insertion, selectionAfter: NSRange(location: start + (insertion as NSString).length, length: 0))
    }

    /// Indent (or outdent) list items touched by the selection by two spaces.
    public static func indentEdit(in text: NSString, selection: NSRange, outdent: Bool) -> TextEdit? {
        let lines = lineRanges(in: text, covering: selection)
        guard !lines.isEmpty else { return nil }
        let block = NSRange(location: lines.first!.location, length: NSMaxRange(lines.last!) - lines.first!.location)
        var anyList = false
        var newLines: [String] = []
        var caretShiftFirst = 0
        var totalShift = 0
        for (i, r) in lines.enumerated() {
            let s = text.substring(with: r)
            let parsed = MarkdownParser.parseLine(s as NSString)
            if parsed.kind.isListItem {
                anyList = true
                if outdent {
                    var removed = 0
                    var t = Substring(s)
                    while removed < 2, t.first == " " { t.removeFirst(); removed += 1 }
                    if removed == 0, t.first == "\t" { t.removeFirst(); removed = 1 }
                    newLines.append(String(t))
                    if i == 0 { caretShiftFirst = -removed }
                    totalShift -= removed
                } else {
                    newLines.append("  " + s)
                    if i == 0 { caretShiftFirst = 2 }
                    totalShift += 2
                }
            } else {
                newLines.append(s)
            }
        }
        guard anyList else { return nil }
        let replacement = newLines.joined(separator: "\n")
        let newSelection: NSRange
        if selection.length == 0 {
            newSelection = NSRange(location: max(block.location, selection.location + caretShiftFirst), length: 0)
        } else {
            newSelection = NSRange(location: block.location, length: (replacement as NSString).length)
        }
        _ = totalShift
        return TextEdit(range: block, replacement: replacement, selectionAfter: newSelection)
    }

    static func lineRanges(in text: NSString, covering selection: NSRange) -> [NSRange] {
        var ranges: [NSRange] = []
        var loc = selection.location
        let end = max(selection.location, NSMaxRange(selection) - (selection.length > 0 ? 1 : 0))
        while true {
            let r = TextUnits.paragraphRange(in: text, at: loc)
            ranges.append(r)
            let next = NSMaxRange(r) + 1
            if next > end || next > text.length { break }
            loc = next
        }
        return ranges
    }
}

public enum InlineFormatting {
    /// Toggles a symmetric delimiter (`**`, `*`, `~~`) around the selection.
    public static func toggle(_ delimiter: String, in text: NSString, selection: NSRange) -> TextEdit {
        let d = delimiter as NSString
        let dl = d.length
        var sel = selection

        if sel.length == 0 {
            // Inside an existing pair with nothing selected: expand to the word.
            let word = wordRange(in: text, at: sel.location)
            if word.length > 0 { sel = word }
        }

        if sel.length == 0 {
            // Directly between an empty pair: remove it.
            if sel.location >= dl, sel.location + dl <= text.length,
               text.substring(with: NSRange(location: sel.location - dl, length: dl)) == delimiter,
               text.substring(with: NSRange(location: sel.location, length: dl)) == delimiter {
                let r = NSRange(location: sel.location - dl, length: dl * 2)
                return TextEdit(range: r, replacement: "", selectionAfter: NSRange(location: r.location, length: 0))
            }
            return TextEdit(range: sel, replacement: delimiter + delimiter, selectionAfter: NSRange(location: sel.location + dl, length: 0))
        }

        let selected = text.substring(with: sel)
        // Selection includes the delimiters.
        if selected.hasPrefix(delimiter), selected.hasSuffix(delimiter), (selected as NSString).length >= dl * 2,
           isExactDelimiter(delimiter, in: selected as NSString, at: 0),
           isExactDelimiter(delimiter, in: selected as NSString, at: (selected as NSString).length - dl) {
            let inner = (selected as NSString).substring(with: NSRange(location: dl, length: (selected as NSString).length - dl * 2))
            return TextEdit(range: sel, replacement: inner, selectionAfter: NSRange(location: sel.location, length: (inner as NSString).length))
        }
        // Delimiters surround the selection.
        if sel.location >= dl, NSMaxRange(sel) + dl <= text.length,
           isExactDelimiter(delimiter, in: text, at: sel.location - dl),
           isExactDelimiter(delimiter, in: text, at: NSMaxRange(sel)) {
            let outer = NSRange(location: sel.location - dl, length: sel.length + dl * 2)
            return TextEdit(range: outer, replacement: selected, selectionAfter: NSRange(location: outer.location, length: sel.length))
        }
        // Keep surrounding whitespace outside the delimiters.
        let leading = selected.prefix(while: { $0 == " " }).count
        let trailing = selected.reversed().prefix(while: { $0 == " " }).count
        let coreStart = sel.location + leading
        let coreLength = max(0, sel.length - leading - trailing)
        let core = NSRange(location: coreStart, length: coreLength)
        let coreText = text.substring(with: core)
        return TextEdit(range: core, replacement: delimiter + coreText + delimiter, selectionAfter: NSRange(location: core.location + dl, length: core.length))
    }

    /// True when `delimiter` sits at `location` and is not part of a longer run of the same character.
    static func isExactDelimiter(_ delimiter: String, in text: NSString, at location: Int) -> Bool {
        let dl = (delimiter as NSString).length
        guard location >= 0, location + dl <= text.length else { return false }
        guard text.substring(with: NSRange(location: location, length: dl)) == delimiter else { return false }
        let ch = (delimiter as NSString).character(at: 0)
        let before = location > 0 ? text.character(at: location - 1) : 0
        let after = location + dl < text.length ? text.character(at: location + dl) : 0
        if delimiter == "*" || delimiter == "_" {
            // `*` must not be half of `**`, unless the run is `***`.
            let runBefore = before == ch
            let runAfter = after == ch
            if runBefore != runAfter { return false }
        }
        return true
    }

    static func wordRange(in text: NSString, at location: Int) -> NSRange {
        guard text.length > 0 else { return NSRange(location: location, length: 0) }
        func isWord(_ c: unichar) -> Bool {
            guard let s = Unicode.Scalar(c) else { return false }
            return CharacterSet.alphanumerics.contains(s) || c == 0x27 || c == 0x2019
        }
        var start = location
        var end = location
        while start > 0, isWord(text.character(at: start - 1)) { start -= 1 }
        while end < text.length, isWord(text.character(at: end)) { end += 1 }
        // Only expand when the caret is inside a word, not at its edge.
        if start == location || end == location { return NSRange(location: location, length: 0) }
        return NSRange(location: start, length: end - start)
    }

    public static func link(in text: NSString, selection: NSRange) -> TextEdit {
        let selected = text.substring(with: selection)
        let looksLikeURL = selected.hasPrefix("http://") || selected.hasPrefix("https://") || selected.hasPrefix("www.")
        if selection.length == 0 {
            return TextEdit(range: selection, replacement: "[]()", selectionAfter: NSRange(location: selection.location + 1, length: 0))
        }
        if looksLikeURL {
            return TextEdit(range: selection, replacement: "[](\(selected))", selectionAfter: NSRange(location: selection.location + 1, length: 0))
        }
        let replacement = "[\(selected)]()"
        return TextEdit(range: selection, replacement: replacement, selectionAfter: NSRange(location: selection.location + (replacement as NSString).length - 1, length: 0))
    }

    public enum LinePrefix: Equatable, Sendable {
        case heading(Int)
        case bullet
        case numbered
        case quote
        case body
    }

    /// Sets (or toggles off) a block prefix on every line touched by the selection.
    public static func setLinePrefix(_ prefix: LinePrefix, in text: NSString, selection: NSRange) -> TextEdit {
        let lines = ListContinuation.lineRanges(in: text, covering: selection)
        let block = NSRange(location: lines.first!.location, length: NSMaxRange(lines.last!) - lines.first!.location)
        let parsed = lines.map { MarkdownParser.parseLine(text.substring(with: $0) as NSString, offset: $0.location) }

        let allMatch = parsed.allSatisfy { line in
            switch (prefix, line.kind) {
            case let (.heading(a), .heading(b)): return a == b
            case (.bullet, .bulletItem), (.numbered, .orderedItem), (.quote, .blockquote): return true
            default: return false
            }
        }
        var newLines: [String] = []
        var number = 1
        var caretDelta = 0
        for (i, line) in parsed.enumerated() {
            let content = text.substring(with: line.contentRange)
            let oldPrefixLength = line.contentRange.location - line.range.location
            if line.kind == .blank && lines.count > 1 {
                newLines.append(text.substring(with: line.range))
                continue
            }
            let newPrefix: String
            if allMatch || prefix == .body {
                newPrefix = ""
            } else {
                switch prefix {
                case let .heading(level): newPrefix = String(repeating: "#", count: level) + " "
                case .bullet: newPrefix = "- "
                case .numbered: newPrefix = "\(number). "; number += 1
                case .quote: newPrefix = "> "
                case .body: newPrefix = ""
                }
            }
            if i == 0 { caretDelta = (newPrefix as NSString).length - oldPrefixLength }
            newLines.append(newPrefix + content)
        }
        let replacement = newLines.joined(separator: "\n")
        let after: NSRange
        if selection.length == 0 {
            after = NSRange(location: max(block.location, selection.location + caretDelta), length: 0)
        } else {
            after = NSRange(location: block.location, length: (replacement as NSString).length)
        }
        return TextEdit(range: block, replacement: replacement, selectionAfter: after)
    }
}
