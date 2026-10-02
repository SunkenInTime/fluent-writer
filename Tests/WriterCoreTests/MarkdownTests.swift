import XCTest
@testable import WriterCore

final class MarkdownTests: XCTestCase {
    func testHeadingAndListKinds() {
        let lines = MarkdownParser.parse("# Title\n\n- one\n2. two\n- [x] done\n> quote\n---\ntext" as NSString)
        XCTAssertEqual(lines.map(\.kind), [.heading(level: 1), .blank, .bulletItem, .orderedItem(number: 2, delimiter: "."), .taskItem(checked: true), .blockquote, .horizontalRule, .paragraph])
        XCTAssertEqual(lines[0].markerRange, NSRange(location: 0, length: 2))
    }

    func testFenceTracking() {
        let lines = MarkdownParser.parse("```\n# not heading\n```\n# heading" as NSString)
        XCTAssertEqual(lines.map(\.kind), [.codeFence, .codeBlock, .codeFence, .heading(level: 1)])
    }

    func testTrailingNewlineProducesEmptyLine() {
        let lines = MarkdownParser.parse("a\n" as NSString)
        XCTAssertEqual(lines.count, 2)
        XCTAssertEqual(lines[1].range, NSRange(location: 2, length: 0))
    }

    func testInlineSpans() {
        let text = "a **bold** and *it* `co*de*` [link](https://x.com) ~~gone~~" as NSString
        let spans = MarkdownParser.parseInlines(text, in: NSRange(location: 0, length: text.length))
        let kinds = spans.map(\.kind)
        XCTAssertEqual(kinds, [.strong, .emphasis, .code, .link, .strikethrough])
        let strong = spans[0]
        XCTAssertEqual(text.substring(with: strong.contentRange), "bold")
    }

    func testBoldItalicCombined() {
        let text = "***both***" as NSString
        let spans = MarkdownParser.parseInlines(text, in: NSRange(location: 0, length: text.length))
        XCTAssertTrue(spans.contains { $0.kind == .strong })
        XCTAssertTrue(spans.contains { $0.kind == .emphasis })
    }
}

final class TextUnitsTests: XCTestCase {
    let text = "First sentence here. Second one is longer. Third!\nNext paragraph." as NSString

    func testSentenceContainingCaret() {
        let r = TextUnits.sentenceRange(in: text, at: 25)
        XCTAssertEqual(text.substring(with: r), "Second one is longer.")
    }

    func testCaretAfterFinalPunctuationBelongsToSentence() {
        let loc = text.range(of: "here.").location + 5
        XCTAssertEqual(text.substring(with: TextUnits.sentenceRange(in: text, at: loc)), "First sentence here.")
    }

    func testParagraph() {
        XCTAssertEqual(text.substring(with: TextUnits.paragraphRange(in: text, at: 55)), "Next paragraph.")
        XCTAssertEqual(text.substring(with: TextUnits.paragraphRange(in: text, at: text.length)), "Next paragraph.")
    }

    func testEmptyLineAtEnd() {
        let t = "abc\n" as NSString
        XCTAssertEqual(TextUnits.paragraphRange(in: t, at: 4), NSRange(location: 4, length: 0))
        XCTAssertEqual(TextUnits.sentenceRange(in: t, at: 4).length, 0)
    }
}

final class ExportTests: XCTestCase {
    func testCleanPlainText() {
        let md = """
        # Heading

        Some **bold** and *italic* text with a [link](https://example.com) and https://bare.dev 🎉.


        - first
        - second
          - nested
        1. one
        2. two

        > quoted `code`
        """
        let expected = """
        Heading

        Some bold and italic text with a link (https://example.com) and https://bare.dev 🎉.

        • first
        • second
          • nested
        1. one
        2. two

        quoted code
        """
        XCTAssertEqual(PlainTextExporter.export(md), expected)
    }

    func testLinkWithURLText() {
        XCTAssertEqual(PlainTextExporter.export("[https://a.com](https://a.com)"), "https://a.com")
        XCTAssertEqual(PlainTextExporter.export("<https://a.com>"), "https://a.com")
    }

    func testEscapesAndNoDecorativeUnicode() {
        let out = PlainTextExporter.export("2 \\* 3 = **six**")
        XCTAssertEqual(out, "2 * 3 = six")
        XCTAssertFalse(out.unicodeScalars.contains { (0x1D400...0x1D7FF).contains($0.value) })
    }

    func testCountsIgnoreDelimiters() {
        let c = TextStats.counts(forMarkdown: "# Hi **there**")
        XCTAssertEqual(c, TextCounts(words: 2, characters: 8))
    }

    func testEmojiCountsAsOneCharacter() {
        XCTAssertEqual(TextStats.counts(forPlainText: "👍🏽").characters, 1)
    }
}

final class TitleTests: XCTestCase {
    func testDerivedTitle() {
        XCTAssertEqual(TitleDeriver.derivedTitle(from: "\n\n# The **big** idea\nbody"), "The big idea")
        XCTAssertEqual(TitleDeriver.derivedTitle(from: "   \n"), "Untitled")
        XCTAssertEqual(TitleDeriver.fileNameStem(for: "a/b: c"), "a-b- c")
    }
}

final class EditingTests: XCTestCase {
    func apply(_ text: String, _ edit: TextEdit?) -> String { edit!.applied(to: text) }

    func testBulletContinuation() {
        let t = "- one"
        let e = ListContinuation.newlineEdit(in: t as NSString, selection: NSRange(location: 5, length: 0))
        XCTAssertEqual(apply(t, e), "- one\n- ")
        XCTAssertEqual(e?.selectionAfter.location, 8)
    }

    func testOrderedContinuation() {
        let t = "  9. nine"
        XCTAssertEqual(apply(t, ListContinuation.newlineEdit(in: t as NSString, selection: NSRange(location: 9, length: 0))), "  9. nine\n  10. ")
    }

    func testTaskContinuation() {
        let t = "- [x] done"
        XCTAssertEqual(apply(t, ListContinuation.newlineEdit(in: t as NSString, selection: NSRange(location: 10, length: 0))), "- [x] done\n- [ ] ")
    }

    func testEmptyItemEndsList() {
        let t = "- one\n- "
        let e = ListContinuation.newlineEdit(in: t as NSString, selection: NSRange(location: 8, length: 0))
        XCTAssertEqual(apply(t, e), "- one\n")
    }

    func testSplitItem() {
        let t = "- onetwo"
        XCTAssertEqual(apply(t, ListContinuation.newlineEdit(in: t as NSString, selection: NSRange(location: 5, length: 0))), "- one\n- two")
    }

    func testPlainParagraphUsesDefault() {
        XCTAssertNil(ListContinuation.newlineEdit(in: "hello" as NSString, selection: NSRange(location: 5, length: 0)))
    }

    func testToggleBold() {
        let t = "make this bold"
        let on = InlineFormatting.toggle("**", in: t as NSString, selection: NSRange(location: 10, length: 4))
        let bolded = on.applied(to: t)
        XCTAssertEqual(bolded, "make this **bold**")
        let off = InlineFormatting.toggle("**", in: bolded as NSString, selection: on.selectionAfter)
        XCTAssertEqual(off.applied(to: bolded), t)
    }

    func testItalicDoesNotUnwrapBold() {
        let t = "**word**"
        let e = InlineFormatting.toggle("*", in: t as NSString, selection: NSRange(location: 2, length: 4))
        XCTAssertEqual(e.applied(to: t), "***word***")
    }

    func testToggleExpandsToWord() {
        let t = "a word here"
        let e = InlineFormatting.toggle("*", in: t as NSString, selection: NSRange(location: 4, length: 0))
        XCTAssertEqual(e.applied(to: t), "a *word* here")
    }

    func testHeadingToggle() {
        let t = "Title"
        let e = InlineFormatting.setLinePrefix(.heading(2), in: t as NSString, selection: NSRange(location: 2, length: 0))
        XCTAssertEqual(e.applied(to: t), "## Title")
        XCTAssertEqual(e.selectionAfter.location, 5)
        let back = InlineFormatting.setLinePrefix(.heading(2), in: "## Title" as NSString, selection: e.selectionAfter)
        XCTAssertEqual(back.applied(to: "## Title"), "Title")
    }

    func testNumberedListPrefix() {
        let t = "a\nb"
        let e = InlineFormatting.setLinePrefix(.numbered, in: t as NSString, selection: NSRange(location: 0, length: 3))
        XCTAssertEqual(e.applied(to: t), "1. a\n2. b")
    }

    func testLink() {
        let t = "see docs"
        let e = InlineFormatting.link(in: t as NSString, selection: NSRange(location: 4, length: 4))
        XCTAssertEqual(e.applied(to: t), "see [docs]()")
        XCTAssertEqual(e.selectionAfter.location, 11)
    }

    func testIndent() {
        let t = "- a"
        let e = ListContinuation.indentEdit(in: t as NSString, selection: NSRange(location: 3, length: 0), outdent: false)
        XCTAssertEqual(e?.applied(to: t), "  - a")
        XCTAssertNil(ListContinuation.indentEdit(in: "plain" as NSString, selection: NSRange(location: 0, length: 0), outdent: false))
    }
}

final class DiffTests: XCTestCase {
    func testRoundTrip() {
        let old = "The quick brown fox jumps."
        let new = "The fast brown fox leaps over."
        let segs = WordDiff.diff(old, new)
        XCTAssertEqual(segs.filter { $0.kind != .insert }.map(\.text).joined(), old)
        XCTAssertEqual(segs.filter { $0.kind != .delete }.map(\.text).joined(), new)
        XCTAssertTrue(segs.contains { $0.kind == .delete && $0.text.contains("quick") })
    }

    func testFrozenScopeTracking() {
        var scope = FrozenScope(range: NSRange(location: 10, length: 5), original: "hello")
        scope.noteEdit(editedRange: NSRange(location: 0, length: 3), changeInLength: 3)
        XCTAssertEqual(scope.range.location, 13)
        XCTAssertFalse(scope.isStale)
        scope.noteEdit(editedRange: NSRange(location: 30, length: 1), changeInLength: 1)
        XCTAssertFalse(scope.isStale)
        scope.noteEdit(editedRange: NSRange(location: 14, length: 1), changeInLength: 1)
        XCTAssertTrue(scope.isStale)
    }

    func testFrozenScopeVerify() {
        var scope = FrozenScope(range: NSRange(location: 0, length: 5), original: "hello")
        scope.verify(against: "hello world")
        XCTAssertFalse(scope.isStale)
        scope.verify(against: "jello world")
        XCTAssertTrue(scope.isStale)
    }
}
