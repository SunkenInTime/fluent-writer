//! Port of Tests/WriterCoreTests/ImageTests.swift.

use std::path::{Path, PathBuf};

use writer_core::markdown;
use writer_core::markdown_images::{self, ImageSource, MarkdownImage};
use writer_core::plain_text_exporter;

fn blocks(s: &str) -> Vec<MarkdownImage> {
    markdown_images::blocks(s, &markdown::parse(s))
}

fn file(source: &str, base: Option<&Path>) -> Option<PathBuf> {
    match markdown_images::resolve(source, base) {
        Some(ImageSource::File(p)) => Some(p),
        _ => None,
    }
}

#[test]
fn finds_image_lines_only() {
    let text = "Intro ![inline](a.png) text.\n\n![Team offsite](images/team%20offsite.png)\n\n  ![t](<my pic.jpg> \"Title\")\n\n```\n![code](x.png)\n```";
    let found = blocks(text);
    assert_eq!(
        found.iter().map(|i| i.alt.as_str()).collect::<Vec<_>>(),
        ["Team offsite", "t"]
    );
    assert_eq!(
        found.iter().map(|i| i.source.as_str()).collect::<Vec<_>>(),
        ["images/team%20offsite.png", "my pic.jpg"]
    );
    assert_eq!(
        &text[found[0].line_range.clone()],
        "![Team offsite](images/team%20offsite.png)"
    );
}

#[cfg(unix)]
#[test]
fn resolve() {
    let base = Path::new("/Users/me/Drafts");
    assert_eq!(
        file("images/team%20offsite.png", Some(base)),
        Some(PathBuf::from("/Users/me/Drafts/images/team offsite.png"))
    );
    assert_eq!(
        file("/tmp/x.png", Some(base)),
        Some(PathBuf::from("/tmp/x.png"))
    );
    match markdown_images::resolve("https://example.com/a.png", Some(base)) {
        Some(ImageSource::Web(u)) => assert!(u.starts_with("https://example.com/")),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        markdown_images::resolve("data:image/png;base64,AAA", Some(base)),
        None
    );
    assert_eq!(markdown_images::resolve("rel.png", None), None);
    assert_eq!(
        file("file:///tmp/a%20b.png", None),
        Some(PathBuf::from("/tmp/a b.png"))
    );
}

#[test]
fn markdown_escapes_path_and_round_trips() {
    let md = markdown_images::markdown("A [b]", "images/Pasted image (1).png");
    assert_eq!(md, "![A b](images/Pasted%20image%20%281%29.png)");
    let found = blocks(&md);
    assert_eq!(found.len(), 1);
    let resolved = file(&found[0].source, Some(Path::new("/d"))).unwrap();
    assert_eq!(resolved.file_name().unwrap(), "Pasted image (1).png");
}

#[test]
fn markdown_encodes_non_ascii() {
    assert_eq!(
        markdown_images::markdown("图", "images/照片 1.png"),
        "![图](images/%E7%85%A7%E7%89%87%201.png)"
    );
}

#[cfg(unix)]
#[test]
fn alt_and_relative_path() {
    assert_eq!(
        markdown_images::alt_text_for_file_name("team-offsite_2026.png"),
        "team offsite 2026"
    );
    assert_eq!(
        markdown_images::relative_path(Path::new("/a/b/images/x.png"), Path::new("/a/b")),
        Some("images/x.png".to_string())
    );
    assert_eq!(
        markdown_images::relative_path(Path::new("/a/bc/x.png"), Path::new("/a/b")),
        None
    );
}

#[test]
fn plain_text_drops_images() {
    assert_eq!(
        plain_text_exporter::export("Hi.\n\n![pic](a.png)\n\nBye [link](https://x.com)."),
        plain_text_exporter::export("Hi.\n\n\n\nBye [link](https://x.com).")
    );
}

#[test]
fn block_insertion_and_unique_names() {
    assert_eq!(
        markdown_images::block_insertion("![](a.png)", 3, "abc"),
        "\n\n![](a.png)"
    );
    assert_eq!(
        markdown_images::block_insertion("![](a.png)", 0, "abc"),
        "![](a.png)\n\n"
    );
    assert_eq!(
        markdown_images::block_insertion("![](a.png)", 4, "abc\n"),
        "![](a.png)"
    );
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("photo.png"), b"x").unwrap();
    assert_eq!(
        markdown_images::unique_file_path("photo.png", dir.path()),
        dir.path().join("photo 2.png")
    );
    assert_eq!(
        markdown_images::unique_file_path("new.png", dir.path()),
        dir.path().join("new.png")
    );
}
