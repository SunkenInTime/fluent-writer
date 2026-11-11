//! Rules from the Swift WriterCore that PARITY.md lists and the ported tests didn't pin down:
//! format commands (`TextEditing.swift`), the plain-text export (`PlainTextExporter.swift`),
//! titles and file names (`TitleDeriver.swift`), counts (`TextStats.swift`) and the draft store
//! (`DraftStore.swift`). Each expected value is what the Swift code produces.

use std::fs;

use writer_core::text_editing::{self, LinePrefix};
use writer_core::{DraftLibrary, SaveResult, plain_text_exporter, text_stats, title_deriver};

fn edit(e: &text_editing::TextEdit, text: &str) -> (String, std::ops::Range<usize>) {
    (e.applied(text), e.selection_after.clone())
}

// MARK: Bold, Italic, Strikethrough

#[test]
fn a_caret_inside_a_word_wraps_the_word_with_apostrophes_in_it() {
    let t = "don't stop";
    assert_eq!(edit(&text_editing::toggle("**", t, 2..2), t), ("**don't** stop".into(), 2..7));
    let t = "it’s here";
    assert_eq!(edit(&text_editing::toggle("~~", t, 1..1), t), ("~~it’s~~ here".into(), 2..2 + "it’s".len()));
}

#[test]
fn a_caret_at_a_word_edge_or_between_words_inserts_an_empty_pair() {
    let t = "a word";
    assert_eq!(edit(&text_editing::toggle("**", t, 2..2), t), ("a ****word".into(), 4..4));
    let t = "a  b";
    assert_eq!(edit(&text_editing::toggle("*", t, 2..2), t), ("a ** b".into(), 3..3));
}

#[test]
fn a_caret_between_an_empty_pair_removes_it() {
    let t = "x **** y";
    assert_eq!(edit(&text_editing::toggle("**", t, 4..4), t), ("x  y".into(), 2..2));
}

#[test]
fn a_selection_with_or_inside_the_delimiters_unwraps() {
    let t = "a **bold** c";
    assert_eq!(edit(&text_editing::toggle("**", t, 2..10), t), ("a bold c".into(), 2..6), "delimiters selected too");
    assert_eq!(edit(&text_editing::toggle("**", t, 4..8), t), ("a bold c".into(), 2..6), "delimiters around the selection");
    let t = "a ~~gone~~";
    assert_eq!(edit(&text_editing::toggle("~~", t, 4..8), t), ("a gone".into(), 2..6));
}

#[test]
fn a_single_star_is_a_delimiter_only_when_not_half_of_a_double() {
    // `*word*` inside `***word***`: each `*` sits in a run on both sides, so it counts.
    let t = "***word***";
    assert_eq!(edit(&text_editing::toggle("*", t, 2..8), t), ("**word**".into(), 2..6));
    // `**` alone is not a single `*`: italic wraps the bold instead.
    let t = "**word**";
    assert_eq!(edit(&text_editing::toggle("*", t, 2..6), t), ("***word***".into(), 3..7));
}

#[test]
fn wrapping_keeps_spaces_outside_the_delimiters() {
    let t = "a bold b";
    assert_eq!(edit(&text_editing::toggle("**", t, 1..7), t), ("a **bold** b".into(), 4..8));
}

// MARK: Link

#[test]
fn link_with_nothing_selected_or_a_url_selected_puts_the_caret_in_the_brackets() {
    let t = "go ";
    assert_eq!(edit(&text_editing::link(t, 3..3), t), ("go []()".into(), 4..4));
    for url in ["http://x.com", "https://x.com", "www.x.com"] {
        let t = format!("see {url}");
        assert_eq!(edit(&text_editing::link(&t, 4..t.len()), &t), (format!("see []({url})"), 5..5));
    }
    let t = "see docs";
    assert_eq!(edit(&text_editing::link(t, 4..8), t), ("see [docs]()".into(), 11..11));
}

// MARK: Headings, lists, quotes, Body

#[test]
fn block_prefixes_replace_each_touched_lines_marker() {
    let t = "# a\n- b\n> c";
    assert_eq!(text_editing::set_line_prefix(LinePrefix::Bullet, t, 0..t.len()).applied(t), "- a\n- b\n- c");
    assert_eq!(text_editing::set_line_prefix(LinePrefix::Heading(3), t, 0..t.len()).applied(t), "### a\n### b\n### c");
    assert_eq!(text_editing::set_line_prefix(LinePrefix::Quote, t, 0..t.len()).applied(t), "> a\n> b\n> c");
    assert_eq!(text_editing::set_line_prefix(LinePrefix::Numbered, t, 0..t.len()).applied(t), "1. a\n2. b\n3. c");
}

#[test]
fn the_same_prefix_on_every_line_toggles_off() {
    let t = "* a\n+ b";
    assert_eq!(text_editing::set_line_prefix(LinePrefix::Bullet, t, 0..t.len()).applied(t), "a\nb", "any bullet");
    let t = "3. a\n7) b";
    assert_eq!(text_editing::set_line_prefix(LinePrefix::Numbered, t, 0..t.len()).applied(t), "a\nb", "any number");
    let t = ">a\n> b";
    assert_eq!(text_editing::set_line_prefix(LinePrefix::Quote, t, 0..t.len()).applied(t), "a\nb", "any quote");
    let t = "## a";
    assert_eq!(text_editing::set_line_prefix(LinePrefix::Heading(1), t, 0..0).applied(t), "# a", "another level replaces");
    assert_eq!(text_editing::set_line_prefix(LinePrefix::Body, t, 0..0).applied(t), "a", "Body removes any prefix");
}

#[test]
fn blank_lines_in_a_selection_stay_and_the_block_stays_selected() {
    let t = "a\n\nb";
    let e = text_editing::set_line_prefix(LinePrefix::Numbered, t, 0..t.len());
    assert_eq!(edit(&e, t), ("1. a\n\n2. b".into(), 0..10));
}

#[test]
fn a_caret_moves_by_the_prefix_change() {
    let t = "- x";
    assert_eq!(text_editing::set_line_prefix(LinePrefix::Heading(2), t, 3..3).selection_after, 4..4);
    let t = "## x";
    assert_eq!(text_editing::set_line_prefix(LinePrefix::Body, t, 4..4).selection_after, 1..1);
}

// MARK: Indent, outdent and continuing lists

#[test]
fn indent_touches_only_list_items_and_tab_falls_through_without_one() {
    let t = "- a\nplain";
    assert_eq!(edit(&text_editing::indent_edit(t, 0..t.len(), false).unwrap(), t), ("  - a\nplain".into(), 0..11));
    assert!(text_editing::indent_edit("plain", 2..2, false).is_none(), "Tab types a tab");
    let t = "    - a";
    assert_eq!(edit(&text_editing::indent_edit(t, 7..7, true).unwrap(), t), ("  - a".into(), 5..5), "up to two spaces");
    let t = "\t- a";
    assert_eq!(edit(&text_editing::indent_edit(t, 4..4, true).unwrap(), t), ("- a".into(), 3..3), "or one tab");
}

#[test]
fn continuing_a_list_drops_spaces_before_the_caret_and_keeps_the_marker_style() {
    let t = "- one   ";
    assert_eq!(edit(&text_editing::newline_edit(t, 8..8).unwrap(), t), ("- one\n- ".into(), 8..8));
    let t = "1) a";
    assert_eq!(text_editing::newline_edit(t, 4..4).unwrap().applied(t), "1) a\n2) ");
    let t = "  * [ ] a";
    assert_eq!(text_editing::newline_edit(t, 9..9).unwrap().applied(t), "  * [ ] a\n  * [ ] ");
    let t = ">q";
    assert_eq!(text_editing::newline_edit(t, 2..2).unwrap().applied(t), ">q\n> ");
    let t = "> > q";
    assert_eq!(text_editing::newline_edit(t, 5..5).unwrap().applied(t), "> > q\n> > ");
    assert!(text_editing::newline_edit("- one", 1..1).is_none(), "a caret before the marker's end types a newline");
}

// MARK: Plain-text export (Copy for Twitter, counts, previews)

#[test]
fn export_drops_block_markers_and_keeps_list_shapes() {
    let x = plain_text_exporter::export;
    assert_eq!(x("# Head"), "Head");
    assert_eq!(x("> quote"), "quote");
    assert_eq!(x("- item\n* two\n- [x] task"), "• item\n• two\n• task");
    assert_eq!(x("3. a\n4) b"), "3. a\n4) b");
    assert_eq!(x("- a\n  - b\n\t- c"), "• a\n  • b\n    • c");
    assert_eq!(x("```\n**code**\n```"), "**code**", "fences dropped, code kept as is");
    assert_eq!(x("a\n\n---\n\nb"), "a\n\nb", "a rule is a blank line");
    assert_eq!(x("a\n\n\n\nb\n\n\n"), "a\n\nb", "blank runs collapse, trailing ones go");
    assert_eq!(x("a   "), "a", "trailing spaces go");
}

#[test]
fn export_inline_rules() {
    let x = plain_text_exporter::export;
    assert_eq!(x("**b** *i* ~~s~~"), "b i s");
    assert_eq!(x("x ![alt](a.png) y"), "x  y", "images go");
    assert_eq!(x("[label](http://u)"), "label (http://u)");
    assert_eq!(x("[](http://u)"), "http://u", "empty label");
    assert_eq!(x("[http://u](http://u)"), "http://u", "label is the url");
    assert_eq!(x("[u.com](https://u.com)"), "https://u.com", "label is the url without its scheme");
    assert_eq!(x("[http stuff](http://u)"), "http://u", "label starting with http");
    assert_eq!(x(r"\*not em\*"), "*not em*", "escapes removed");
    assert_eq!(x(r"`a` \*x\*"), r"a \*x\*", "but not on a line with code");
}

// MARK: Titles and file names

#[test]
fn derived_titles_skip_fences_and_rules_and_drop_markup() {
    assert_eq!(title_deriver::derived_title("```\n---\n\n## **Big**   idea\n"), "Big idea");
    assert_eq!(title_deriver::derived_title("- [ ] task one"), "task one");
    assert_eq!(title_deriver::derived_title("> [a link](http://x)"), "a link (http://x)");
}

#[test]
fn derived_titles_are_cut_at_60_characters_on_a_late_space() {
    let long = "abcdefghi, ".repeat(7);
    assert_eq!(title_deriver::derived_title(&long), "abcdefghi, abcdefghi, abcdefghi, abcdefghi, abcdefghi");
    let unbroken = "a".repeat(70);
    assert_eq!(title_deriver::derived_title(&unbroken), "a".repeat(60), "no space past the halfway point");
}

#[test]
fn file_name_stems_are_safe() {
    assert_eq!(title_deriver::file_name_stem("a/b:c"), "a-b-c");
    assert_eq!(title_deriver::file_name_stem("  .hidden. "), "hidden");
    assert_eq!(title_deriver::file_name_stem(""), "Untitled");
    assert_eq!(title_deriver::file_name_stem("\u{7}bell"), "bell");
    let long = "é".repeat(150);
    assert_eq!(title_deriver::file_name_stem(&long), "é".repeat(120), "over 200 bytes: 120 characters");
}

// MARK: Counts

#[test]
fn counts_read_the_plain_text_in_grapheme_clusters() {
    let c = text_stats::counts_for_markdown("# **Hi** there");
    assert_eq!((c.words, c.characters), (2, 8));
    assert_eq!(text_stats::counts_for_plain_text("e\u{301}👍🏽").characters, 2);
    assert_eq!(text_stats::format(text_stats::counts_for_plain_text("one"), None), "1 word  ·  3 characters");
    let all = text_stats::counts_for_plain_text("one two");
    let some = text_stats::counts_for_plain_text("o");
    assert_eq!(text_stats::format(all, Some(some)), "1 of 2 words  ·  1 of 7 characters", "plural follows the total");
}

// MARK: Draft store

fn library() -> (DraftLibrary, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let lib = DraftLibrary::new(dir.path().join("drafts"), dir.path().join("support")).unwrap();
    (lib, dir)
}

#[test]
fn files_that_arent_utf8_are_read_as_latin_1() {
    let (mut lib, dir) = library();
    let path = dir.path().join("old.txt");
    fs::write(&path, [b'c', b'a', b'f', 0xE9]).unwrap();
    assert_eq!(lib.open(&path).unwrap().body, "café");
}

#[test]
fn a_draft_with_nothing_new_isnt_written_again() {
    let (mut lib, _dir) = library();
    let mut doc = lib.new_draft();
    doc.body = "Same".into();
    doc.refresh_derived_title();
    assert_eq!(lib.save(&mut doc).unwrap(), SaveResult::Saved);
    assert_eq!(lib.save(&mut doc).unwrap(), SaveResult::Unchanged);
}

#[test]
fn numbered_names_stay_and_files_elsewhere_keep_their_names() {
    let (mut lib, dir) = library();
    for _ in 0..2 {
        let mut d = lib.new_draft();
        d.body = "Plan".into();
        d.refresh_derived_title();
        lib.save(&mut d).unwrap();
    }
    let mut second = lib.open(&dir.path().join("drafts/Plan 2.md")).unwrap();
    second.body = "Plan\nmore".into();
    lib.save(&mut second).unwrap();
    assert!(second.path().unwrap().ends_with("Plan 2.md"), "the title plus \" N\" counts as the title");

    let elsewhere = dir.path().join("notes.md");
    fs::write(&elsewhere, "First line").unwrap();
    let mut doc = lib.open(&elsewhere).unwrap();
    doc.body = "Another first line".into();
    doc.refresh_derived_title();
    lib.save(&mut doc).unwrap();
    assert_eq!(doc.path().unwrap(), elsewhere.as_path(), "outside the drafts folder the name stays");
}

#[test]
fn recovery_snapshots_hold_the_draft_and_go_once_it_is_saved() {
    let (mut lib, dir) = library();
    let mut doc = lib.new_draft();
    doc.body = "Kept text".into();
    doc.refresh_derived_title();
    doc.selection = 4..4;
    lib.write_recovery(&doc).unwrap();
    let file = fs::read_dir(dir.path().join("support/Recovery")).unwrap().next().unwrap().unwrap().path();
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    let mut keys: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, ["body", "draftID", "hasManualTitle", "savedAt", "selectionLocation", "title"], "no path before the first save");
    assert_eq!(json["body"], "Kept text");
    lib.save(&mut doc).unwrap();
    assert!(lib.recovery_snapshots().is_empty(), "a successful save deletes it");
}

#[test]
fn a_snapshot_matching_the_file_is_deleted_on_open_and_a_different_one_is_loaded() {
    let (mut lib, _dir) = library();
    let mut doc = lib.new_draft();
    doc.body = "On disk".into();
    doc.refresh_derived_title();
    lib.save(&mut doc).unwrap();
    let path = doc.path().unwrap().to_path_buf();
    lib.write_recovery(&doc).unwrap();
    let reopened = lib.open(&path).unwrap();
    assert!(reopened.recovered_at().is_none());
    assert!(lib.recovery_snapshots().is_empty(), "identical: deleted");

    doc.body = "Newer words".into();
    doc.selection = 5..5;
    lib.write_recovery(&doc).unwrap();
    let recovered = lib.open(&path).unwrap();
    assert!(recovered.recovered_at().is_some());
    assert_eq!((recovered.body.as_str(), recovered.selection.clone()), ("Newer words", 5..5));
}

// MARK: Notes on disk

/// `WriterCore/DocumentNotes.swift:168-169`, `:224-284`
#[test]
fn notes_keep_200_exchanges_write_iso_dates_with_milliseconds_and_go_when_empty() {
    use writer_core::{AssistExchange, DocumentNotes, NotesStore};
    let dir = tempfile::tempdir().unwrap();
    let store = NotesStore::new(dir.path());
    let mut notes = DocumentNotes::new("/x/a.md");
    for i in 0..205 {
        notes.add_exchange(AssistExchange {
            date: std::time::SystemTime::now(),
            provider: "Codex".into(),
            action: "ask".into(),
            instruction: format!("{i}"),
            feedback: String::new(),
            version_id: None,
            comment_ids: Vec::new(),
        });
    }
    assert_eq!(notes.transcript.len(), 200);
    assert_eq!(notes.transcript[0].instruction, "5", "the oldest go");
    store.save(&notes, "text").unwrap();
    let file = store.path_for("/x/a.md");
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(name.len(), 24 + ".json".len(), "12 bytes of SHA-256 in hex");
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    let date = json["transcript"][0]["date"].as_str().unwrap();
    assert!(date.len() == 24 && date.ends_with('Z') && date.as_bytes()[19] == b'.', "ISO 8601 with milliseconds: {date}");
    notes.transcript.clear();
    store.save(&notes, "text").unwrap();
    assert!(!file.exists(), "empty notes leave no file");
}
