import XCTest
@testable import WriterCore

final class RemoteTests: XCTestCase {
    var root: URL!

    override func setUpWithError() throws {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("fw-remote-\(UUID().uuidString)")
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: root)
    }

    func makeLibrary() throws -> DraftLibrary {
        try DraftLibrary(draftsDirectory: root.appendingPathComponent("drafts"), supportDirectory: root.appendingPathComponent("support"))
    }

    func testParsesScpStyle() {
        XCTAssertEqual(RemoteLocation.parse("me@box:~/notes/a.md"), RemoteLocation(host: "me@box", path: "~/notes/a.md"))
        XCTAssertEqual(RemoteLocation.parse(" box:/srv/x.md "), RemoteLocation(host: "box", path: "/srv/x.md"))
        XCTAssertEqual(RemoteLocation.parse("box:"), RemoteLocation(host: "box", path: "~"))
    }

    func testParsesSSHURL() {
        XCTAssertEqual(RemoteLocation.parse("ssh://me@box:2222/srv/a.md"), RemoteLocation(host: "me@box", port: 2222, path: "/srv/a.md"))
        XCTAssertEqual(RemoteLocation.parse("ssh://box/~/a.md"), RemoteLocation(host: "box", path: "~/a.md"))
        XCTAssertEqual(RemoteLocation.parse("ssh://box"), RemoteLocation(host: "box", path: "~"))
    }

    func testRejectsUnsafeOrLocalInput() {
        XCTAssertNil(RemoteLocation.parse("/Users/me/a.md"))
        XCTAssertNil(RemoteLocation.parse("-oProxyCommand=x:a"))
        XCTAssertNil(RemoteLocation.parse("box;rm:a"))
        XCTAssertNil(RemoteLocation.parse("no colon"))
    }

    func testShellPathQuotesButKeepsTilde() {
        XCTAssertEqual(RemoteLocation(host: "b", path: "~/it's here.md").shellPath, "~/'it'\\''s here.md'")
        XCTAssertEqual(RemoteLocation(host: "b", path: "/a b/$x").shellPath, "'/a b/$x'")
        XCTAssertEqual(RemoteLocation(host: "b", path: "~").shellPath, "~")
    }

    func testArgumentsAreNonInteractive() {
        let args = SSHTransport().arguments(for: RemoteLocation(host: "me@box", port: 2222, path: "~"), command: "true")
        XCTAssertTrue(args.contains("BatchMode=yes"))
        XCTAssertEqual(Array(args.suffix(4)), ["-p", "2222", "me@box", "true"])
    }

    func testParsesListingAndSkipsHidden() {
        let entries = SSHTransport.parseListing(".git/\n.DS_Store\nchapters/\nnotes.md\nhello world.txt\n")
        XCTAssertEqual(entries, [
            RemoteEntry(name: "chapters", isDirectory: true),
            RemoteEntry(name: "notes.md", isDirectory: false),
            RemoteEntry(name: "hello world.txt", isDirectory: false),
        ])
    }

    func testMirrorPathStaysInsideSupportDirectory() throws {
        let lib = try makeLibrary()
        let mirror = lib.mirrorURL(for: RemoteLocation(host: "me@box", port: 22, path: "/../../etc/passwd"))
        XCTAssertTrue(mirror.path.hasPrefix(lib.remoteMirrorDirectory.standardizedFileURL.path))
        XCTAssertEqual(mirror.lastPathComponent, "passwd")
        XCTAssertNotEqual(lib.mirrorURL(for: RemoteLocation(host: "box", path: "~/a.md")), lib.mirrorURL(for: RemoteLocation(host: "box", path: "/a.md")))
    }

    func testRemoteSaveMarksUploadPendingUntilUploaded() throws {
        let lib = try makeLibrary()
        let loc = RemoteLocation(host: "box", path: "~/notes/a.md")
        let doc = try lib.openRemote(loc, contents: Data("hello".utf8))
        XCTAssertEqual(doc.body, "hello")
        XCTAssertEqual(doc.title, "a.md")
        XCTAssertFalse(doc.remoteUploadPending)
        doc.body = "hello there"
        XCTAssertEqual(try lib.save(doc), .saved)
        XCTAssertTrue(doc.remoteUploadPending)
        XCTAssertEqual(try String(contentsOf: lib.mirrorURL(for: loc), encoding: .utf8), "hello there")
        lib.remoteUploadFinished(doc, uploadedBody: "hello", succeeded: true)
        XCTAssertTrue(doc.remoteUploadPending, "an older upload must not clear newer saves")
        lib.remoteUploadFinished(doc, uploadedBody: "hello there", succeeded: true)
        XCTAssertFalse(doc.remoteUploadPending)
        XCTAssertEqual(try lib.save(doc), .unchanged)
    }

    func testPendingLocalEditsSurviveReopen() throws {
        let loc = RemoteLocation(host: "box", path: "/srv/a.md")
        do {
            let lib = try makeLibrary()
            let doc = try lib.openRemote(loc, contents: Data("v1".utf8))
            doc.body = "local edit"
            try lib.save(doc)
            lib.remoteUploadFinished(doc, uploadedBody: "local edit", succeeded: false)
        }
        let lib = try makeLibrary()
        let reopened = try lib.openRemote(loc, contents: Data("v1".utf8))
        XCTAssertEqual(reopened.body, "local edit")
        XCTAssertTrue(reopened.remoteUploadPending)
        XCTAssertEqual(try lib.save(reopened), .saved, "pending text is offered for upload again")
    }

    func testRemoteDraftsAppearInRecentsWithLocation() throws {
        let lib = try makeLibrary()
        let loc = RemoteLocation(host: "box", path: "~/a.md")
        _ = try lib.openRemote(loc, contents: Data("hi\nthere".utf8))
        let summary = try XCTUnwrap(lib.recentDrafts().first)
        XCTAssertEqual(summary.remote, loc)
        XCTAssertEqual(summary.title, "a.md")
        let viaMirror = try lib.open(summary.url)
        XCTAssertEqual(viaMirror.remote, loc)
    }

    func testRenamingRemoteFileKeepsMirrorPath() throws {
        let lib = try makeLibrary()
        let loc = RemoteLocation(host: "box", path: "~/a.md")
        let doc = try lib.openRemote(loc, contents: Data("hi".utf8))
        try lib.rename(doc, to: "Better")
        XCTAssertEqual(doc.title, "Better")
        XCTAssertEqual(doc.url, lib.mirrorURL(for: loc))
    }

    /// Runs against a real server when FLUENT_WRITER_SSH_TEST_DIR is set, e.g. `ssh://me@localhost:2222/tmp/fw`.
    func testRoundTripOverSSH() throws {
        guard let raw = ProcessInfo.processInfo.environment["FLUENT_WRITER_SSH_TEST_DIR"], let dir = RemoteLocation.parse(raw) else {
            throw XCTSkip("FLUENT_WRITER_SSH_TEST_DIR not set")
        }
        let ssh = SSHTransport()
        let file = dir.appending("round trip 'quoted'.md")
        try ssh.write(Data("# Hi\n\nüñï".utf8), to: file)
        XCTAssertEqual(try ssh.kind(file), .file)
        XCTAssertEqual(try ssh.kind(dir), .directory)
        XCTAssertEqual(try ssh.kind(dir.appending("nope")), .missing)
        XCTAssertEqual(String(decoding: try ssh.read(file), as: UTF8.self), "# Hi\n\nüñï")
        XCTAssertTrue(try ssh.list(dir).contains(RemoteEntry(name: "round trip 'quoted'.md", isDirectory: false)))
        XCTAssertFalse(try ssh.list(dir).contains { $0.name.contains("fwtmp") })
    }
}
