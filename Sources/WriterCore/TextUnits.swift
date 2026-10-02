import Foundation
import NaturalLanguage

public enum FocusUnit: String, CaseIterable, Codable, Sendable {
    case off
    case sentence
    case paragraph
    case typewriter

    public var dimsSurroundings: Bool { self == .sentence || self == .paragraph }
    public var centersCaret: Bool { self != .off }
}

public enum TextUnits {
    /// The line (hard-wrapped paragraph) containing `location`, without its terminator.
    public static func paragraphRange(in text: NSString, at location: Int) -> NSRange {
        let loc = max(0, min(location, text.length))
        if text.length == 0 { return NSRange(location: 0, length: 0) }
        if loc == text.length, isNewline(text.character(at: loc - 1)) {
            return NSRange(location: loc, length: 0)
        }
        var start = 0
        var contentsEnd = 0
        text.getParagraphStart(&start, end: nil, contentsEnd: &contentsEnd, for: NSRange(location: loc, length: 0))
        return NSRange(location: start, length: contentsEnd - start)
    }

    /// The sentence containing `location`, trimmed of trailing whitespace.
    /// A caret placed right after a sentence's final punctuation belongs to that sentence.
    public static func sentenceRange(in text: NSString, at location: Int) -> NSRange {
        let paragraph = paragraphRange(in: text, at: location)
        if paragraph.length == 0 { return paragraph }
        let paragraphText = text.substring(with: paragraph)
        let tokenizer = NLTokenizer(unit: .sentence)
        tokenizer.string = paragraphText
        let relative = location - paragraph.location
        let utf16 = paragraphText.utf16
        var result: NSRange?
        var last: NSRange?
        tokenizer.enumerateTokens(in: paragraphText.startIndex..<paragraphText.endIndex) { tokenRange, _ in
            let r = NSRange(tokenRange, in: paragraphText)
            last = r
            if relative >= r.location && relative < NSMaxRange(r) {
                result = r
                return false
            }
            return true
        }
        var range = result ?? last ?? NSRange(location: 0, length: utf16.count)
        // Trim trailing whitespace.
        let ns = paragraphText as NSString
        while range.length > 0, let scalar = Unicode.Scalar(ns.character(at: NSMaxRange(range) - 1)), CharacterSet.whitespacesAndNewlines.contains(scalar) {
            range.length -= 1
        }
        // A caret sitting in the whitespace after a sentence still belongs to it,
        // unless that is the start of the next sentence.
        return NSRange(location: range.location + paragraph.location, length: range.length)
    }

    public static func focusRange(_ unit: FocusUnit, in text: NSString, at location: Int) -> NSRange? {
        switch unit {
        case .sentence: return sentenceRange(in: text, at: location)
        case .paragraph: return paragraphRange(in: text, at: location)
        case .off, .typewriter: return nil
        }
    }

    static func isNewline(_ c: unichar) -> Bool {
        return c == 0x0A || c == 0x0D || c == 0x2028 || c == 0x2029
    }
}
