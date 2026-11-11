//! Port of Tests/WriterCoreTests/MarkdownTests.swift.

use writer_core::markdown::{self, MarkdownBlockKind as K, MarkdownInlineKind as I};
use writer_core::text_editing::{self, LinePrefix};
use writer_core::word_diff::{self, DiffSegmentKind, FrozenScope};
use writer_core::{TextCounts, plain_text_exporter, text_stats, text_units, title_deriver};

// MARK: MarkdownTests

#[test]
fn heading_and_list_kinds() {
    let lines = markdown::parse("# Title\n\n- one\n2. two\n- [x] done\n> quote\n---\ntext");
    let kinds: Vec<K> = lines.iter().map(|l| l.kind).collect();
    assert_eq!(
        kinds,
        [
            K::Heading { level: 1 },
            K::Blank,
            K::BulletItem,
            K::OrderedItem {
                number: 2,
                delimiter: '.'
            },
            K::TaskItem { checked: true },
            K::Blockquote,
            K::HorizontalRule,
            K::Paragraph
        ]
    );
    assert_eq!(lines[0].marker_range, Some(0..2));
}

#[test]
fn fence_tracking() {
    let lines = markdown::parse("```\n# not heading\n```\n# heading");
    let kinds: Vec<K> = lines.iter().map(|l| l.kind).collect();
    assert_eq!(
        kinds,
        [
            K::CodeFence,
            K::CodeBlock,
            K::CodeFence,
            K::Heading { level: 1 }
        ]
    );
}

#[test]
fn trailing_newline_produces_empty_line() {
    let lines = markdown::parse("a\n");
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1].range, 2..2);
}

#[test]
fn inline_spans() {
    let text = "a **bold** and *it* `co*de*` [link](https://x.com) ~~gone~~";
    let spans = markdown::parse_inlines(text, 0..text.len());
    let kinds: Vec<I> = spans.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        [I::Strong, I::Emphasis, I::Code, I::Link, I::Strikethrough]
    );
    assert_eq!(&text[spans[0].content_range.clone()], "bold");
}

#[test]
fn bold_italic_combined() {
    let text = "***both***";
    let spans = markdown::parse_inlines(text, 0..text.len());
    assert!(spans.iter().any(|s| s.kind == I::Strong));
    assert!(spans.iter().any(|s| s.kind == I::Emphasis));
}

#[test]
fn byte_offsets_with_emoji_and_cjk() {
    let text = "# 你好 👍🏽\n\n- **粗体** and `代码`";
    let lines = markdown::parse(text);
    assert_eq!(lines[0].kind, K::Heading { level: 1 });
    assert_eq!(&text[lines[0].content_range.clone()], "你好 👍🏽");
    let item = &lines[2];
    assert_eq!(item.kind, K::BulletItem);
    let strong = &item.inlines[0];
    assert_eq!(strong.kind, I::Strong);
    assert_eq!(&text[strong.content_range.clone()], "粗体");
    assert_eq!(&text[item.inlines[1].content_range.clone()], "代码");
}

// MARK: TextUnitsTests

const UNITS_TEXT: &str = "First sentence here. Second one is longer. Third!\nNext paragraph.";

#[test]
fn sentence_containing_caret() {
    let r = text_units::sentence_range(UNITS_TEXT, 25);
    assert_eq!(&UNITS_TEXT[r], "Second one is longer.");
}

#[test]
fn caret_after_final_punctuation_belongs_to_sentence() {
    let loc = UNITS_TEXT.find("here.").unwrap() + 5;
    assert_eq!(
        &UNITS_TEXT[text_units::sentence_range(UNITS_TEXT, loc)],
        "First sentence here."
    );
}

#[test]
fn paragraph() {
    assert_eq!(
        &UNITS_TEXT[text_units::paragraph_range(UNITS_TEXT, 55)],
        "Next paragraph."
    );
    assert_eq!(
        &UNITS_TEXT[text_units::paragraph_range(UNITS_TEXT, UNITS_TEXT.len())],
        "Next paragraph."
    );
}

#[test]
fn empty_line_at_end() {
    let t = "abc\n";
    assert_eq!(text_units::paragraph_range(t, 4), 4..4);
    assert_eq!(text_units::sentence_range(t, 4).len(), 0);
}

#[test]
fn sentence_and_paragraph_with_multibyte_text() {
    let t = "Émoji 👍🏽 first. 中文第二句。\r\nNext";
    let caret = t.find("中").unwrap();
    assert_eq!(
        &t[text_units::paragraph_range(t, caret)],
        "Émoji 👍🏽 first. 中文第二句。"
    );
    assert_eq!(&t[text_units::sentence_range(t, 3)], "Émoji 👍🏽 first.");
    assert_eq!(&t[text_units::sentence_range(t, caret)], "中文第二句。");
    // A caret between CR and LF belongs to the paragraph the pair ends.
    let lf = t.find('\n').unwrap();
    assert_eq!(
        &t[text_units::paragraph_range(t, lf)],
        "Émoji 👍🏽 first. 中文第二句。"
    );
    assert_eq!(&t[text_units::paragraph_range(t, t.len())], "Next");
}

// MARK: ExportTests

#[test]
fn clean_plain_text() {
    let md = "# Heading\n\nSome **bold** and *italic* text with a [link](https://example.com) and https://bare.dev 🎉.\n\n\n- first\n- second\n  - nested\n1. one\n2. two\n\n> quoted `code`";
    let expected = "Heading\n\nSome bold and italic text with a link (https://example.com) and https://bare.dev 🎉.\n\n• first\n• second\n  • nested\n1. one\n2. two\n\nquoted code";
    assert_eq!(plain_text_exporter::export(md), expected);
}

#[test]
fn link_with_url_text() {
    assert_eq!(
        plain_text_exporter::export("[https://a.com](https://a.com)"),
        "https://a.com"
    );
    assert_eq!(
        plain_text_exporter::export("<https://a.com>"),
        "https://a.com"
    );
}

#[test]
fn escapes_and_no_decorative_unicode() {
    let out = plain_text_exporter::export("2 \\* 3 = **six**");
    assert_eq!(out, "2 * 3 = six");
    assert!(
        !out.chars()
            .any(|c| (0x1D400..=0x1D7FF).contains(&(c as u32)))
    );
}

#[test]
fn counts_ignore_delimiters() {
    assert_eq!(
        text_stats::counts_for_markdown("# Hi **there**"),
        TextCounts::new(2, 8)
    );
}

#[test]
fn emoji_counts_as_one_character() {
    assert_eq!(text_stats::counts_for_plain_text("👍🏽").characters, 1);
}

// MARK: TitleTests

#[test]
fn derived_title() {
    assert_eq!(
        title_deriver::derived_title("\n\n# The **big** idea\nbody"),
        "The big idea"
    );
    assert_eq!(title_deriver::derived_title("   \n"), "Untitled");
    assert_eq!(title_deriver::file_name_stem("a/b: c"), "a-b- c");
}

// MARK: EditingTests

fn apply(text: &str, edit: Option<text_editing::TextEdit>) -> String {
    edit.expect("an edit").applied(text)
}

#[test]
fn bullet_continuation() {
    let t = "- one";
    let e = text_editing::newline_edit(t, 5..5);
    assert_eq!(e.as_ref().map(|e| e.selection_after.start), Some(8));
    assert_eq!(apply(t, e), "- one\n- ");
}

#[test]
fn ordered_continuation() {
    let t = "  9. nine";
    assert_eq!(
        apply(t, text_editing::newline_edit(t, 9..9)),
        "  9. nine\n  10. "
    );
}

#[test]
fn task_continuation() {
    let t = "- [x] done";
    assert_eq!(
        apply(t, text_editing::newline_edit(t, 10..10)),
        "- [x] done\n- [ ] "
    );
}

#[test]
fn empty_item_ends_list() {
    let t = "- one\n- ";
    assert_eq!(apply(t, text_editing::newline_edit(t, 8..8)), "- one\n");
}

#[test]
fn split_item() {
    let t = "- onetwo";
    assert_eq!(
        apply(t, text_editing::newline_edit(t, 5..5)),
        "- one\n- two"
    );
}

#[test]
fn plain_paragraph_uses_default() {
    assert!(text_editing::newline_edit("hello", 5..5).is_none());
}

#[test]
fn toggle_bold() {
    let t = "make this bold";
    let on = text_editing::toggle("**", t, 10..14);
    let bolded = on.applied(t);
    assert_eq!(bolded, "make this **bold**");
    let off = text_editing::toggle("**", &bolded, on.selection_after.clone());
    assert_eq!(off.applied(&bolded), t);
}

#[test]
fn italic_does_not_unwrap_bold() {
    let t = "**word**";
    assert_eq!(text_editing::toggle("*", t, 2..6).applied(t), "***word***");
}

#[test]
fn toggle_expands_to_word() {
    let t = "a word here";
    assert_eq!(
        text_editing::toggle("*", t, 4..4).applied(t),
        "a *word* here"
    );
}

#[test]
fn toggle_expands_to_multibyte_word() {
    let t = "a café 中文 here";
    let inside = t.find("fé").unwrap();
    assert_eq!(
        text_editing::toggle("*", t, inside..inside).applied(t),
        "a *café* 中文 here"
    );
    let cjk = t.find("文").unwrap();
    let e = text_editing::toggle("**", t, cjk..cjk);
    assert_eq!(e.applied(t), "a café **中文** here");
    assert_eq!(&e.applied(t)[e.selection_after.clone()], "中文");
}

#[test]
fn heading_toggle() {
    let t = "Title";
    let e = text_editing::set_line_prefix(LinePrefix::Heading(2), t, 2..2);
    assert_eq!(e.applied(t), "## Title");
    assert_eq!(e.selection_after.start, 5);
    let back = text_editing::set_line_prefix(
        LinePrefix::Heading(2),
        "## Title",
        e.selection_after.clone(),
    );
    assert_eq!(back.applied("## Title"), "Title");
}

#[test]
fn numbered_list_prefix() {
    let t = "a\nb";
    assert_eq!(
        text_editing::set_line_prefix(LinePrefix::Numbered, t, 0..3).applied(t),
        "1. a\n2. b"
    );
}

#[test]
fn line_prefix_on_crlf_text_terminates() {
    let t = "a\r\nb";
    let e = text_editing::set_line_prefix(LinePrefix::Bullet, t, 0..t.len());
    assert_eq!(e.applied(t), "- a\n- b");
}

#[test]
fn link() {
    let t = "see docs";
    let e = text_editing::link(t, 4..8);
    assert_eq!(e.applied(t), "see [docs]()");
    assert_eq!(e.selection_after.start, 11);
}

#[test]
fn indent() {
    let t = "- a";
    assert_eq!(
        text_editing::indent_edit(t, 3..3, false).map(|e| e.applied(t)),
        Some("  - a".to_string())
    );
    assert!(text_editing::indent_edit("plain", 0..0, false).is_none());
}

// MARK: DiffTests

#[test]
fn diff_round_trip() {
    let old = "The quick brown fox jumps.";
    let new = "The fast brown fox leaps over.";
    let segs = word_diff::diff(old, new);
    let joined = |skip: DiffSegmentKind| {
        segs.iter()
            .filter(|s| s.kind != skip)
            .map(|s| s.text.as_str())
            .collect::<String>()
    };
    assert_eq!(joined(DiffSegmentKind::Insert), old);
    assert_eq!(joined(DiffSegmentKind::Delete), new);
    assert!(
        segs.iter()
            .any(|s| s.kind == DiffSegmentKind::Delete && s.text.contains("quick"))
    );
}

#[test]
fn frozen_scope_tracking() {
    let mut scope = FrozenScope::new(10..15, "hello");
    scope.note_edit(0..3, 3);
    assert_eq!(scope.range().start, 13);
    assert!(!scope.is_stale());
    scope.note_edit(30..31, 1);
    assert!(!scope.is_stale());
    scope.note_edit(14..15, 1);
    assert!(scope.is_stale());
}

#[test]
fn frozen_scope_verify() {
    let mut scope = FrozenScope::new(0..5, "hello");
    scope.verify("hello world");
    assert!(!scope.is_stale());
    scope.verify("jello world");
    assert!(scope.is_stale());
}

#[test]
fn inline_edge_cases() {
    let kinds = |t: &str| {
        markdown::parse_inlines(t, 0..t.len())
            .into_iter()
            .map(|s| (s.kind, t[s.range].to_string()))
            .collect::<Vec<_>>()
    };
    assert_eq!(kinds("snake_case_word"), []);
    assert_eq!(
        kinds("see www.example.com."),
        [(I::BareUrl, "www.example.com".to_string())]
    );
    assert_eq!(kinds("(https://x.com)"), []);
    assert_eq!(
        kinds("__under__ _em_"),
        [
            (I::Strong, "__under__".to_string()),
            (I::Emphasis, "_em_".to_string())
        ]
    );
    assert_eq!(kinds("``a`b``"), [(I::Code, "``a`b``".to_string())]);
    assert_eq!(
        kinds("**a** *b*"),
        [
            (I::Strong, "**a**".to_string()),
            (I::Emphasis, "*b*".to_string())
        ]
    );
    assert_eq!(kinds("2 * 3 * 4"), []);
    // Content after a list marker is matched without seeing the marker (NSRegularExpression bounds).
    let line = markdown::parse_line("- *it*", 0);
    assert_eq!(line.inlines.len(), 1);
    assert_eq!(line.inlines[0].range, 2..6);
}
