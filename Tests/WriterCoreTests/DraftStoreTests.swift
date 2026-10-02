import XCTest
@testable import WriterCore

final class DraftStoreTests: XCTestCase {
    var root: URL!

    override func setUpWithError() throws {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("fw-tests-\(UUID().uuidString)")
    }

    override func tearDownWithError() throws {
        chmod(root.appendingPathComponent("drafts").path, 0o755)
        try? FileManager.default.removeItem(at: root)
    }

    func makeLibrary() throws -> DraftLibrary {
        try DraftLibrary(draftsDirectory: root.appendingPathComponent("drafts"), supportDirectory: root.appendingPathComponent("support"))
    }

    func longDraft() -> String {
        let sentence = "The quick brown fox jumps over the lazy dog while the writer keeps typing. "
        return (0..<215).map { i in i % 5 == 4 ? sentence + "\n\n" : sentence }.joined()
    }

    func testEmptyNewDraftIsNotWritten() throws {
        let lib = try makeLibrary()
        let doc = lib.newDraft()
        XCTAssertEqual(try lib.save(doc), .unchanged)
        XCTAssertNil(doc.url)
    }

    func testSaveReopenPreservesBodyAndCursor() throws {
        let lib = try makeLibrary()
        let doc = lib.newDraft()
        doc.body = "# My post\n\n" + longDraft()
        doc.refreshDerivedTitle()
        doc.selection = NSRange(location: 1234, length: 0)
        try lib.save(doc)
        XCTAssertEqual(doc.url?.lastPathComponent, "My post.md")
        lib.rememberSelection(doc)
        XCTAssertGreaterThan(TextStats.counts(forMarkdown: doc.body).words, 3000)

        let lib2 = try makeLibrary()
        XCTAssertEqual(lib2.lastOpenedURL, doc.url)
        let reopened = try lib2.open(doc.url!)
        XCTAssertEqual(reopened.body, doc.body)
        XCTAssertEqual(reopened.selection.location, 1234)
        XCTAssertFalse(reopened.isDirty)
    }

    func testDerivedTitleRenamesFileButManualTitleSticks() throws {
        let lib = try makeLibrary()
        let doc = lib.newDraft()
        doc.body = "First idea"
        doc.refreshDerivedTitle()
        try lib.save(doc)
        XCTAssertEqual(doc.url?.lastPathComponent, "First idea.md")
        doc.body = "Better idea\nmore"
        doc.refreshDerivedTitle()
        try lib.save(doc)
        XCTAssertEqual(doc.url?.lastPathComponent, "Better idea.md")
        try lib.rename(doc, to: "Launch thread")
        XCTAssertEqual(doc.url?.lastPathComponent, "Launch thread.md")
        doc.body = "Changed first line"
        doc.refreshDerivedTitle()
        try lib.save(doc)
        XCTAssertEqual(doc.title, "Launch thread")
        XCTAssertEqual(doc.url?.lastPathComponent, "Launch thread.md")
    }

    func testTitleCollisionGetsSuffix() throws {
        let lib = try makeLibrary()
        for _ in 0..<2 {
            let d = lib.newDraft()
            d.body = "Same"
            d.refreshDerivedTitle()
            try lib.save(d)
        }
        let names = try FileManager.default.contentsOfDirectory(atPath: lib.draftsDirectory.path).sorted()
        XCTAssertEqual(names, ["Same 2.md", "Same.md"])
    }

    func testSaveFailureKeepsRecoveryAndReopenRestoresIt() throws {
        let lib = try makeLibrary()
        let doc = lib.newDraft()
        doc.body = "Saved version"
        doc.refreshDerivedTitle()
        try lib.save(doc)
        let url = doc.url!

        chmod(lib.draftsDirectory.path, 0o555)
        doc.body = "Saved version plus unsaved words"
        XCTAssertThrowsError(try lib.save(doc)) { error in
            XCTAssertTrue((error as? FileWriteError) != nil)
            XCTAssertTrue(error.localizedDescription.contains("Couldn't save"))
        }
        XCTAssertTrue(doc.isDirty)
        XCTAssertTrue(lib.hasRecovery(for: doc))
        XCTAssertEqual(try String(contentsOf: url, encoding: .utf8), "Saved version")

        // Simulate quitting and relaunching.
        chmod(lib.draftsDirectory.path, 0o755)
        let lib2 = try makeLibrary()
        let reopened = try lib2.open(url)
        XCTAssertEqual(reopened.body, "Saved version plus unsaved words")
        XCTAssertNotNil(reopened.recoveredAt)
        XCTAssertTrue(reopened.isDirty)
        try lib2.save(reopened)
        XCTAssertEqual(try String(contentsOf: url, encoding: .utf8), "Saved version plus unsaved words")
        XCTAssertTrue(lib2.recoverySnapshots().isEmpty)
    }

    func testInterruptedWriteLeavesPreviousRevision() throws {
        let lib = try makeLibrary()
        let doc = lib.newDraft()
        doc.body = "Complete revision"
        doc.refreshDerivedTitle()
        try lib.save(doc)
        // A crash mid-write leaves only a partial temporary file next to the draft.
        let tmp = lib.draftsDirectory.appendingPathComponent(".Complete revision.md\(AtomicFile.temporaryMarker)crash")
        try Data("Compl".utf8).write(to: tmp)
        let lib2 = try makeLibrary()
        XCTAssertFalse(FileManager.default.fileExists(atPath: tmp.path))
        XCTAssertEqual(try lib2.open(doc.url!).body, "Complete revision")
    }

    func testOrphanedUntitledRecovery() throws {
        let lib = try makeLibrary()
        chmod(lib.draftsDirectory.path, 0o555)
        let doc = lib.newDraft()
        doc.body = "never reached disk"
        XCTAssertThrowsError(try lib.save(doc))
        chmod(lib.draftsDirectory.path, 0o755)
        let lib2 = try makeLibrary()
        let orphans = lib2.orphanedRecoveries()
        XCTAssertEqual(orphans.count, 1)
        let restored = lib2.restore(orphans[0])
        XCTAssertEqual(restored.body, "never reached disk")
        XCTAssertTrue(restored.isDirty)
    }

    func testRecentDrafts() throws {
        let lib = try makeLibrary()
        for t in ["One", "Two"] {
            let d = lib.newDraft(); d.body = t; d.refreshDerivedTitle(); try lib.save(d)
        }
        XCTAssertEqual(Set(lib.recentDrafts().map(\.title)), ["One", "Two"])
    }
}
