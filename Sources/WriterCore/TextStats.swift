import Foundation

public struct TextCounts: Equatable, Sendable {
    public var words: Int
    public var characters: Int

    public init(words: Int, characters: Int) {
        self.words = words
        self.characters = characters
    }

    public static let zero = TextCounts(words: 0, characters: 0)
}

public enum TextStats {
    /// Counts what would actually be pasted: Markdown delimiters are excluded.
    public static func counts(forMarkdown markdown: String) -> TextCounts {
        return counts(forPlainText: PlainTextExporter.export(markdown))
    }

    public static func counts(forPlainText text: String) -> TextCounts {
        var words = 0
        let ns = text as NSString
        ns.enumerateSubstrings(in: NSRange(location: 0, length: ns.length), options: [.byWords, .substringNotRequired]) { _, _, _, _ in
            words += 1
        }
        return TextCounts(words: words, characters: text.count)
    }

    public static func format(_ counts: TextCounts, selection: TextCounts?) -> String {
        let nf = NumberFormatter()
        nf.numberStyle = .decimal
        func n(_ v: Int) -> String { nf.string(from: NSNumber(value: v)) ?? String(v) }
        func words(_ v: Int) -> String { v == 1 ? "word" : "words" }
        func chars(_ v: Int) -> String { v == 1 ? "character" : "characters" }
        if let s = selection {
            return "\(n(s.words)) of \(n(counts.words)) \(words(counts.words))  ·  \(n(s.characters)) of \(n(counts.characters)) \(chars(counts.characters))"
        }
        return "\(n(counts.words)) \(words(counts.words))  ·  \(n(counts.characters)) \(chars(counts.characters))"
    }
}
