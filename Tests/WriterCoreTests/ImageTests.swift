import XCTest
@testable import WriterCore

final class MarkdownImageTests: XCTestCase {
    func blocks(_ s: String) -> [MarkdownImage] {
        let ns = s as NSString
        return MarkdownImages.blocks(in: ns, lines: MarkdownParser.parse(ns))
    }

    func testFindsImageLinesOnly() {
        let text = "Intro ![inline](a.png) text.\n\n![Team offsite](images/team%20offsite.png)\n\n  ![t](<my pic.jpg> \"Title\")\n\n```\n![code](x.png)\n```"
        let found = blocks(text)
        XCTAssertEqual(found.map(\.alt), ["Team offsite", "t"])
        XCTAssertEqual(found.map(\.source), ["images/team%20offsite.png", "my pic.jpg"])
    }

    func testResolve() {
        let base = URL(fileURLWithPath: "/Users/me/Drafts", isDirectory: true)
        XCTAssertEqual(MarkdownImages.resolve("images/team%20offsite.png", relativeTo: base)?.path, "/Users/me/Drafts/images/team offsite.png")
        XCTAssertEqual(MarkdownImages.resolve("/tmp/x.png", relativeTo: base)?.path, "/tmp/x.png")
        XCTAssertEqual(MarkdownImages.resolve("https://example.com/a.png", relativeTo: base)?.host, "example.com")
        XCTAssertNil(MarkdownImages.resolve("data:image/png;base64,AAA", relativeTo: base))
        XCTAssertNil(MarkdownImages.resolve("rel.png", relativeTo: nil))
    }

    func testMarkdownEscapesPathAndRoundTrips() {
        let md = MarkdownImages.markdown(alt: "A [b]", path: "images/Pasted image (1).png")
        XCTAssertEqual(md, "![A b](images/Pasted%20image%20%281%29.png)")
        let found = blocks(md)
        XCTAssertEqual(found.count, 1)
        XCTAssertEqual(MarkdownImages.resolve(found[0].source, relativeTo: URL(fileURLWithPath: "/d"))?.lastPathComponent, "Pasted image (1).png")
    }

    func testAltAndRelativePath() {
        XCTAssertEqual(MarkdownImages.altText(forFileName: "team-offsite_2026.png"), "team offsite 2026")
        XCTAssertEqual(MarkdownImages.relativePath(of: URL(fileURLWithPath: "/a/b/images/x.png"), inside: URL(fileURLWithPath: "/a/b")), "images/x.png")
        XCTAssertNil(MarkdownImages.relativePath(of: URL(fileURLWithPath: "/a/bc/x.png"), inside: URL(fileURLWithPath: "/a/b")))
    }

    func testPlainTextDropsImages() {
        XCTAssertEqual(PlainTextExporter.export("Hi.\n\n![pic](a.png)\n\nBye [link](https://x.com)."), PlainTextExporter.export("Hi.\n\n\n\nBye [link](https://x.com)."))
    }
}
