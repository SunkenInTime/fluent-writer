import XCTest
@testable import WriterCore

final class EditPlacementTests: XCTestCase {
    func testReplaceAndAppend() {
        let text = "First paragraph.\n\nSecond paragraph."
        let p = EditPlacer.place([
            ProposedEdit(old: "Second paragraph.", new: "Second, sharper."),
            ProposedEdit(new: "Third, added at the end.", position: .end),
        ], in: text)
        XCTAssertTrue(p.failed.isEmpty)
        XCTAssertEqual(p.applied(to: text), "First paragraph.\n\nSecond, sharper.\n\nThird, added at the end.")
    }

    func testAppendKeepsOneBlankLine() {
        XCTAssertEqual(EditPlacer.place([ProposedEdit(new: "B", position: .end)], in: "A\n").applied(to: "A\n"), "A\n\nB")
        XCTAssertEqual(EditPlacer.place([ProposedEdit(new: "B", position: .end)], in: "A\n\n").applied(to: "A\n\n"), "A\n\nB")
        XCTAssertEqual(EditPlacer.place([ProposedEdit(new: "Z", position: .start)], in: "A").applied(to: "A"), "Z\n\nA")
    }

    func testDuplicateTextPrefersFocus() {
        let text = "Same line.\nOther.\nSame line."
        let focus = NSRange(location: 18, length: 10)
        let p = EditPlacer.place([ProposedEdit(old: "Same line.", new: "Changed.")], in: text, focus: focus)
        XCTAssertEqual(p.applied(to: text), "Same line.\nOther.\nChanged.")
    }

    func testFuzzyMatchIgnoresWhitespaceAndQuotes() {
        let text = "He said \u{201C}hello\u{201D}  to   them.\nNext."
        let p = EditPlacer.place([ProposedEdit(old: "He said \"hello\" to them.", new: "He waved.")], in: text)
        XCTAssertTrue(p.failed.isEmpty)
        XCTAssertEqual(p.applied(to: text), "He waved.\nNext.")
    }

    func testSameOffsetInsertAndReplaceBothLand() {
        let text = "Hello world"
        let p = EditPlacer.place([ProposedEdit(new: "Intro", position: .start), ProposedEdit(old: "Hello", new: "Hi")], in: text)
        XCTAssertEqual(p.applied(to: text), "Intro\n\nHi world")
        let q = EditPlacer.place([ProposedEdit(new: "One", position: .end), ProposedEdit(new: "Two", position: .end)], in: text)
        XCTAssertEqual(q.applied(to: text), "Hello world\n\nOne\n\nTwo")
    }

    func testMissingTextFails() {
        let p = EditPlacer.place([ProposedEdit(old: "Not there", new: "x")], in: "Hello.")
        XCTAssertEqual(p.placed.count, 0)
        XCTAssertEqual(p.failed.count, 1)
    }

    func testOverlappingEditsKeepTheFirst() {
        let text = "One two three."
        let p = EditPlacer.place([ProposedEdit(old: "One two", new: "1 2"), ProposedEdit(old: "two three", new: "2 3")], in: text)
        XCTAssertEqual(p.placed.count, 1)
        XCTAssertEqual(p.failed.count, 1)
    }
}

final class CommentTests: XCTestCase {
    let text = "Alpha beta gamma delta." as NSString

    func comment(_ word: String) -> Comment {
        Comment(range: text.range(of: word), in: text, body: "note")
    }

    func testCapturesQuoteAndContext() {
        let c = comment("gamma")
        XCTAssertEqual(c.quote, "gamma")
        XCTAssertEqual(c.prefix, "Alpha beta ")
        XCTAssertEqual(c.suffix, " delta.")
    }

    func testShiftsWithEditsBefore() {
        var c = comment("gamma")
        // Insert "very " before "beta" (location 6).
        c.noteEdit(editedRange: NSRange(location: 6, length: 5), changeInLength: 5)
        XCTAssertEqual(c.range, NSRange(location: 16, length: 5))
    }

    func testIgnoresEditsAfter() {
        var c = comment("beta")
        c.noteEdit(editedRange: NSRange(location: 17, length: 3), changeInLength: 3)
        XCTAssertEqual(c.range, NSRange(location: 6, length: 4))
    }

    func testGrowsWithEditsInside() {
        var c = comment("gamma")
        c.noteEdit(editedRange: NSRange(location: 13, length: 2), changeInLength: 2)
        XCTAssertEqual(c.range, NSRange(location: 11, length: 7))
    }

    func testDetachesWhenTextDeleted() {
        var c = comment("gamma")
        c.noteEdit(editedRange: NSRange(location: 11, length: 0), changeInLength: -5)
        XCTAssertTrue(c.detached)
    }

    func testUndoneDeletionReanchors() {
        var c = Comment(range: NSRange(location: 6, length: 5), in: "Hello world" as NSString, body: "n")
        c.noteEdit(editedRange: NSRange(location: 6, length: 0), changeInLength: -5)
        XCTAssertTrue(c.detached)
        c.noteEdit(editedRange: NSRange(location: 6, length: 5), changeInLength: 5)
        XCTAssertTrue(c.reanchor(in: "Hello world" as NSString))
        XCTAssertEqual(c.range, NSRange(location: 6, length: 5))
    }

    func testReanchorsAfterOutsideChange() {
        var c = comment("gamma")
        let changed = "New start. Alpha beta gamma delta." as NSString
        XCTAssertTrue(c.reanchor(in: changed))
        XCTAssertEqual(changed.substring(with: c.range), "gamma")
        XCTAssertFalse(c.reanchor(in: "Nothing here." as NSString))
        XCTAssertTrue(c.detached)
    }
}

final class NotesStoreTests: XCTestCase {
    var dir: URL!

    override func setUp() {
        dir = FileManager.default.temporaryDirectory.appendingPathComponent("fw-notes-\(UUID().uuidString)")
    }

    override func tearDown() { try? FileManager.default.removeItem(at: dir) }

    func testSaveLoadMoveRoundTrip() throws {
        let store = NotesStore(supportDirectory: dir)
        var notes = DocumentNotes(path: "/tmp/a.md")
        _ = try notes.addComment(quote: "world", body: "Which one?", author: "you", in: "hello world" as NSString)
        notes.addVersion(DraftVersion(kind: .proposal, label: "Shorter", source: "Codex", body: "hi"))
        try store.save(notes)
        let loaded = store.load(path: "/tmp/a.md")
        XCTAssertEqual(loaded.comments.map(\.id), notes.comments.map(\.id))
        XCTAssertEqual(loaded.versions.map(\.body), ["hi"])
        XCTAssertEqual(loaded.comments.first!.created.timeIntervalSince1970, notes.comments.first!.created.timeIntervalSince1970, accuracy: 0.002)
        store.move(from: "/tmp/a.md", to: "/tmp/b.md")
        XCTAssertTrue(store.load(path: "/tmp/a.md").isEmpty)
        XCTAssertEqual(store.load(path: "/tmp/b.md").comments.count, 1)
    }

    func testApplyCommands() {
        var notes = DocumentNotes(path: "/x.md")
        let text = "One. Two. Three."
        var add = InboxCommand(kind: .addComment, path: "/x.md", agent: "Claude Code")
        add.quote = "Two."
        add.body = "Cut this?"
        let r1 = NotesStore.apply(add, to: &notes, text: text)
        XCTAssertTrue(r1.ok)
        XCTAssertEqual(notes.comments.first?.author, "Claude Code")

        var propose = InboxCommand(kind: .proposeVersion, path: "/x.md", agent: "Claude Code")
        propose.label = "Cut two"
        propose.edits = [ProposedEdit(old: "Two. ", new: "")]
        propose.resolvesCommentIDs = [r1.commentID!]
        let r2 = NotesStore.apply(propose, to: &notes, text: text)
        XCTAssertTrue(r2.ok, r2.message)
        XCTAssertEqual(notes.versions.first?.body, "One. Three.")
        XCTAssertEqual(notes.versions.first?.resolvesCommentIDs, [r1.commentID!])

        var partial = InboxCommand(kind: .proposeVersion, path: "/x.md", agent: "Claude Code")
        partial.label = "Partial"
        partial.edits = [ProposedEdit(old: "One.", new: "1.", commentIDs: ["a"]), ProposedEdit(old: "Missing", new: "x", commentIDs: ["b"])]
        XCTAssertTrue(NotesStore.apply(partial, to: &notes, text: text).ok)
        XCTAssertEqual(notes.versions.first?.resolvesCommentIDs, ["a"])

        var bad = InboxCommand(kind: .proposeVersion, path: "/x.md", agent: "Claude Code")
        bad.edits = [ProposedEdit(old: "Missing", new: "x")]
        XCTAssertFalse(NotesStore.apply(bad, to: &notes, text: text).ok)
    }

    func testLockedWritersDontLoseUpdates() {
        let store = NotesStore(supportDirectory: dir)
        DispatchQueue.concurrentPerform(iterations: 16) { i in
            store.withLock {
                var n = store.load(path: "/c.md")
                n.addVersion(DraftVersion(kind: .proposal, label: "\(i)", source: "x", body: "\(i)"))
                try? store.save(n)
            }
        }
        XCTAssertEqual(store.load(path: "/c.md").versions.count, 16)
    }

    func testVersionCapDropsOldestUnused() {
        var notes = DocumentNotes(path: "/x.md")
        var used = DraftVersion(kind: .snapshot, label: "kept", source: "you", body: "kept")
        used.usedAt = Date()
        notes.addVersion(used)
        for i in 0..<(DocumentNotes.maxVersions + 5) {
            notes.addVersion(DraftVersion(kind: .proposal, label: "\(i)", source: "Codex", body: "\(i)"))
        }
        XCTAssertEqual(notes.versions.count, DocumentNotes.maxVersions)
        XCTAssertTrue(notes.versions.contains { $0.label == "kept" })
    }
}
