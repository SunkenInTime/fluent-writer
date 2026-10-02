import Foundation

public enum DiffSegmentKind: Equatable, Sendable {
    case equal
    case insert
    case delete
}

public struct DiffSegment: Equatable, Sendable {
    public var kind: DiffSegmentKind
    public var text: String

    public init(kind: DiffSegmentKind, text: String) {
        self.kind = kind
        self.text = text
    }
}

public enum WordDiff {
    static let tokenRegex = try! NSRegularExpression(pattern: #"\s+|[\p{L}\p{N}'’]+|[^\s\p{L}\p{N}]"#)

    static func tokens(_ s: String) -> [String] {
        let ns = s as NSString
        return tokenRegex.matches(in: s, range: NSRange(location: 0, length: ns.length)).map { ns.substring(with: $0.range) }
    }

    public static func diff(_ old: String, _ new: String) -> [DiffSegment] {
        let a = tokens(old)
        let b = tokens(new)
        let difference = b.difference(from: a)
        var removed = Set<Int>()
        var inserted = Set<Int>()
        for change in difference {
            switch change {
            case let .remove(offset, _, _): removed.insert(offset)
            case let .insert(offset, _, _): inserted.insert(offset)
            }
        }
        var segments: [DiffSegment] = []
        func push(_ kind: DiffSegmentKind, _ text: String) {
            if let last = segments.last, last.kind == kind {
                segments[segments.count - 1].text += text
            } else {
                segments.append(DiffSegment(kind: kind, text: text))
            }
        }
        var i = 0
        var j = 0
        while i < a.count || j < b.count {
            if i < a.count, removed.contains(i) {
                push(.delete, a[i]); i += 1
            } else if j < b.count, inserted.contains(j) {
                push(.insert, b[j]); j += 1
            } else if i < a.count, j < b.count {
                push(.equal, a[i]); i += 1; j += 1
            } else if i < a.count {
                push(.delete, a[i]); i += 1
            } else {
                push(.insert, b[j]); j += 1
            }
        }
        return mergeWhitespaceOnlyEquals(segments)
    }

    /// Absorbs single-space "equal" runs between changes so the diff reads as phrases, not confetti.
    static func mergeWhitespaceOnlyEquals(_ segments: [DiffSegment]) -> [DiffSegment] {
        guard segments.count >= 3 else { return segments }
        var out: [DiffSegment] = []
        var idx = 0
        while idx < segments.count {
            let s = segments[idx]
            if s.kind == .equal, s.text.allSatisfy({ $0 == " " }), !out.isEmpty, idx + 1 < segments.count,
               out.last?.kind != .equal, segments[idx + 1].kind != .equal {
                // Fold the space into both the deletion and the insertion runs.
                var dels = ""
                var ins = ""
                // Collect trailing changes already in `out`.
                while let last = out.last, last.kind != .equal {
                    if last.kind == .delete { dels = last.text + dels } else { ins = last.text + ins }
                    out.removeLast()
                }
                dels += s.text
                ins += s.text
                var k = idx + 1
                while k < segments.count, segments[k].kind != .equal {
                    if segments[k].kind == .delete { dels += segments[k].text } else { ins += segments[k].text }
                    k += 1
                }
                if !dels.isEmpty { out.append(DiffSegment(kind: .delete, text: dels)) }
                if !ins.isEmpty { out.append(DiffSegment(kind: .insert, text: ins)) }
                idx = k
                continue
            }
            out.append(s)
            idx += 1
        }
        return out
    }
}

/// Tracks a frozen text range through later edits so a returned proposal can be marked stale.
public struct FrozenScope: Equatable, Sendable {
    public private(set) var range: NSRange
    public let original: String
    public private(set) var isStale: Bool = false

    public init(range: NSRange, original: String) {
        self.range = range
        self.original = original
    }

    /// `editedRange` is the post-edit range of new text; `changeInLength` the length delta.
    public mutating func noteEdit(editedRange: NSRange, changeInLength: Int) {
        guard !isStale else { return }
        let pre = NSRange(location: editedRange.location, length: max(0, editedRange.length - changeInLength))
        if NSMaxRange(pre) <= range.location {
            range.location += changeInLength
        } else if pre.location >= NSMaxRange(range) {
            return
        } else {
            isStale = true
        }
    }

    /// Confirms that the tracked range still holds the frozen text.
    public mutating func verify(against text: NSString) {
        guard !isStale else { return }
        if NSMaxRange(range) > text.length || text.substring(with: range) != original {
            isStale = true
        }
    }
}
