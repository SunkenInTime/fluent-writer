import Foundation

/// One change an agent wants, anchored by text rather than offsets so it can land anywhere in the draft.
/// Either `old` names existing text to replace, or `position` inserts at the start or end of the draft.
public struct ProposedEdit: Codable, Equatable, Sendable {
    public enum Position: String, Codable, Sendable { case start, end }

    public var old: String?
    public var new: String
    public var position: Position?
    /// Comment ids this edit addresses.
    public var commentIDs: [String]

    public init(old: String? = nil, new: String, position: Position? = nil, commentIDs: [String] = []) {
        self.old = old
        self.new = new
        self.position = position
        self.commentIDs = commentIDs
    }

    enum CodingKeys: String, CodingKey { case old, new, position, commentIDs }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        old = try c.decodeIfPresent(String.self, forKey: .old)
        new = try c.decodeIfPresent(String.self, forKey: .new) ?? ""
        position = try c.decodeIfPresent(Position.self, forKey: .position)
        commentIDs = try c.decodeIfPresent([String].self, forKey: .commentIDs) ?? []
    }
}

/// An edit resolved to a range of the current text.
public struct PlacedEdit: Equatable, Sendable {
    public var index: Int
    public var range: NSRange
    public var replacement: String
}

public struct EditFailure: Equatable, Sendable {
    public var index: Int
    public var edit: ProposedEdit
    public var reason: String
}

public struct EditPlacement: Equatable, Sendable {
    public var placed: [PlacedEdit]
    public var failed: [EditFailure]

    /// The text with every placed edit applied.
    public func applied(to text: String) -> String {
        let ns = NSMutableString(string: text)
        // Back to front. At the same offset, replacements go before insertions (so they can't eat
        // inserted text), and insertions go in reverse so they read in the order they were given.
        let order = placed.sorted { a, b in
            if a.range.location != b.range.location { return a.range.location > b.range.location }
            if a.range.length != b.range.length { return a.range.length > b.range.length }
            return a.index > b.index
        }
        for e in order {
            ns.replaceCharacters(in: e.range, with: e.replacement)
        }
        return ns as String
    }
}

public enum EditPlacer {
    /// Resolves each edit against `text`. When `old` appears more than once, the occurrence nearest
    /// `focus` wins (the selection or commented passage the request was about).
    public static func place(_ edits: [ProposedEdit], in text: String, focus: NSRange? = nil,
                             focusFor: ((ProposedEdit) -> NSRange?)? = nil) -> EditPlacement {
        let ns = text as NSString
        var placed: [PlacedEdit] = []
        var failed: [EditFailure] = []
        for (i, edit) in edits.enumerated() {
            if let position = edit.position, edit.old?.isEmpty ?? true {
                placed.append(insertion(at: position, of: edit.new, in: ns, index: i))
                continue
            }
            guard let old = edit.old, !old.isEmpty else {
                failed.append(EditFailure(index: i, edit: edit, reason: "The edit didn't say what text to change."))
                continue
            }
            let taken = placed.map(\.range)
            let candidates = occurrences(of: old, in: ns).filter { r in !taken.contains { overlaps($0, r) } }
            guard let range = nearest(candidates, to: focusFor?(edit) ?? focus) else {
                let reason = occurrences(of: old, in: ns).isEmpty
                    ? "Couldn't find the text this edit changes. It may have been edited since."
                    : "This edit overlaps another one."
                failed.append(EditFailure(index: i, edit: edit, reason: reason))
                continue
            }
            placed.append(PlacedEdit(index: i, range: range, replacement: edit.new))
        }
        return EditPlacement(placed: placed, failed: failed)
    }

    static func overlaps(_ a: NSRange, _ b: NSRange) -> Bool {
        if a.length == 0 || b.length == 0 { return a.location > b.location && a.location < NSMaxRange(b) || b.location > a.location && b.location < NSMaxRange(a) }
        return NSIntersectionRange(a, b).length > 0
    }

    static func nearest(_ ranges: [NSRange], to focus: NSRange?) -> NSRange? {
        guard let focus else { return ranges.first }
        return ranges.min { distance($0, focus) < distance($1, focus) }
    }

    static func distance(_ r: NSRange, _ focus: NSRange) -> Int {
        if NSIntersectionRange(r, focus).length > 0 || (r.location >= focus.location && NSMaxRange(r) <= NSMaxRange(focus)) { return 0 }
        return min(abs(r.location - NSMaxRange(focus)), abs(NSMaxRange(r) - focus.location))
    }

    /// Adds a block at the start or end, keeping one blank line between it and the existing text.
    static func insertion(at position: ProposedEdit.Position, of new: String, in ns: NSString, index: Int) -> PlacedEdit {
        let block = new.trimmingCharacters(in: .newlines)
        let text = ns as String
        switch position {
        case .end:
            guard ns.length > 0 else { return PlacedEdit(index: index, range: NSRange(location: 0, length: 0), replacement: block) }
            let separator = text.hasSuffix("\n\n") ? "" : (text.hasSuffix("\n") ? "\n" : "\n\n")
            return PlacedEdit(index: index, range: NSRange(location: ns.length, length: 0), replacement: separator + block)
        case .start:
            guard ns.length > 0 else { return PlacedEdit(index: index, range: NSRange(location: 0, length: 0), replacement: block) }
            let separator = text.hasPrefix("\n") ? "\n" : "\n\n"
            return PlacedEdit(index: index, range: NSRange(location: 0, length: 0), replacement: block + separator)
        }
    }

    /// Exact matches first. Failing that, matches that differ only in whitespace runs or curly quotes.
    public static func occurrences(of needle: String, in ns: NSString) -> [NSRange] {
        var exact: [NSRange] = []
        var search = NSRange(location: 0, length: ns.length)
        while search.length > 0 {
            let r = ns.range(of: needle, options: [.literal], range: search)
            if r.location == NSNotFound { break }
            exact.append(r)
            let next = r.location + max(1, r.length)
            search = NSRange(location: next, length: max(0, ns.length - next))
        }
        if !exact.isEmpty { return exact }

        let hay = normalize(ns)
        let pin = normalize(needle as NSString)
        let trimmed = pin.text.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return [] }
        let hayNS = hay.text as NSString
        var found: [NSRange] = []
        search = NSRange(location: 0, length: hayNS.length)
        while search.length > 0 {
            let r = hayNS.range(of: trimmed, options: [.literal], range: search)
            if r.location == NSNotFound { break }
            let start = hay.map[r.location]
            let end = hay.map[NSMaxRange(r) - 1] + 1
            found.append(NSRange(location: start, length: end - start))
            let next = r.location + max(1, r.length)
            search = NSRange(location: next, length: max(0, hayNS.length - next))
        }
        return found
    }

    /// Collapses whitespace runs to one space and straightens quotes. `map[i]` is the source offset of
    /// normalized unit `i`.
    static func normalize(_ ns: NSString) -> (text: String, map: [Int]) {
        var units: [unichar] = []
        var map: [Int] = []
        var lastWasSpace = false
        for i in 0..<ns.length {
            var c = ns.character(at: i)
            let isSpace = c == 0x20 || c == 0x09 || c == 0x0A || c == 0x0D || c == 0xA0
            if isSpace {
                if lastWasSpace { continue }
                c = 0x20
            }
            switch c {
            case 0x2018, 0x2019, 0x201B: c = 0x27
            case 0x201C, 0x201D, 0x201F: c = 0x22
            case 0x2013, 0x2014: c = 0x2D
            default: break
            }
            lastWasSpace = isSpace
            units.append(c)
            map.append(i)
        }
        return (String(utf16CodeUnits: units, count: units.count), map)
    }
}
