//! Port of Tests/WriterCoreTests/NotesTests.swift.

use std::time::{SystemTime, UNIX_EPOCH};

use writer_core::edit_placement::{self, EditPosition, ProposedEdit};
use writer_core::{
    Comment, DocumentNotes, DraftVersion, InboxCommand, InboxCommandKind, NotesStore, VersionKind,
};

// MARK: EditPlacementTests

fn place(edits: &[ProposedEdit], text: &str) -> edit_placement::EditPlacement {
    edit_placement::place(edits, text, None, None)
}

#[test]
fn replace_and_append() {
    let text = "First paragraph.\n\nSecond paragraph.";
    let p = place(
        &[
            ProposedEdit::replace("Second paragraph.", "Second, sharper."),
            ProposedEdit::insert("Third, added at the end.", EditPosition::End),
        ],
        text,
    );
    assert!(p.failed.is_empty());
    assert_eq!(
        p.applied(text),
        "First paragraph.\n\nSecond, sharper.\n\nThird, added at the end."
    );
}

#[test]
fn append_keeps_one_blank_line() {
    assert_eq!(
        place(&[ProposedEdit::insert("B", EditPosition::End)], "A\n").applied("A\n"),
        "A\n\nB"
    );
    assert_eq!(
        place(&[ProposedEdit::insert("B", EditPosition::End)], "A\n\n").applied("A\n\n"),
        "A\n\nB"
    );
    assert_eq!(
        place(&[ProposedEdit::insert("Z", EditPosition::Start)], "A").applied("A"),
        "Z\n\nA"
    );
}

#[test]
fn duplicate_text_prefers_focus() {
    let text = "Same line.\nOther.\nSame line.";
    let p = edit_placement::place(
        &[ProposedEdit::replace("Same line.", "Changed.")],
        text,
        Some(18..28),
        None,
    );
    assert_eq!(p.applied(text), "Same line.\nOther.\nChanged.");
}

#[test]
fn duplicate_text_prefers_focus_after_multibyte_text() {
    let text = "Same line.\n中文中文中文。\nSame line.";
    let second = text.rfind("Same").unwrap();
    let p = edit_placement::place(
        &[ProposedEdit::replace("Same line.", "Changed.")],
        text,
        Some(second..text.len()),
        None,
    );
    assert_eq!(p.applied(text), "Same line.\n中文中文中文。\nChanged.");
    let per_edit = |_: &ProposedEdit| Some(0..1);
    let q = edit_placement::place(
        &[ProposedEdit::replace("Same line.", "Changed.")],
        text,
        Some(second..text.len()),
        Some(&per_edit),
    );
    assert_eq!(q.applied(text), "Changed.\n中文中文中文。\nSame line.");
}

#[test]
fn fuzzy_match_ignores_whitespace_and_quotes() {
    let text = "He said \u{201C}hello\u{201D}  to   them.\nNext.";
    let p = place(
        &[ProposedEdit::replace(
            "He said \"hello\" to them.",
            "He waved.",
        )],
        text,
    );
    assert!(p.failed.is_empty());
    assert_eq!(p.applied(text), "He waved.\nNext.");
}

#[test]
fn fuzzy_match_ending_on_curly_quote() {
    let text = "👍 She said \u{2018}yes\u{2019}. Done.";
    let p = place(
        &[ProposedEdit::replace("She said 'yes'", "She agreed")],
        text,
    );
    assert_eq!(p.applied(text), "👍 She agreed. Done.");
}

#[test]
fn same_offset_insert_and_replace_both_land() {
    let text = "Hello world";
    let p = place(
        &[
            ProposedEdit::insert("Intro", EditPosition::Start),
            ProposedEdit::replace("Hello", "Hi"),
        ],
        text,
    );
    assert_eq!(p.applied(text), "Intro\n\nHi world");
    let q = place(
        &[
            ProposedEdit::insert("One", EditPosition::End),
            ProposedEdit::insert("Two", EditPosition::End),
        ],
        text,
    );
    assert_eq!(q.applied(text), "Hello world\n\nOne\n\nTwo");
}

#[test]
fn missing_text_fails() {
    let p = place(&[ProposedEdit::replace("Not there", "x")], "Hello.");
    assert_eq!(p.placed.len(), 0);
    assert_eq!(p.failed.len(), 1);
}

#[test]
fn overlapping_edits_keep_the_first() {
    let text = "One two three.";
    let p = place(
        &[
            ProposedEdit::replace("One two", "1 2"),
            ProposedEdit::replace("two three", "2 3"),
        ],
        text,
    );
    assert_eq!(p.placed.len(), 1);
    assert_eq!(p.failed.len(), 1);
}

#[test]
fn proposed_edit_json_defaults() {
    let e: ProposedEdit = serde_json::from_str(r#"{"old":"a"}"#).unwrap();
    assert_eq!(e, ProposedEdit::replace("a", ""));
    let e: ProposedEdit =
        serde_json::from_str(r#"{"new":"b","position":"end","commentIDs":["x"]}"#).unwrap();
    assert_eq!(
        e,
        ProposedEdit::insert("b", EditPosition::End).with_comment_ids(vec!["x".into()])
    );
}

// MARK: CommentTests

const TEXT: &str = "Alpha beta gamma delta.";

fn comment(word: &str) -> Comment {
    let at = TEXT.find(word).unwrap();
    Comment::new(at..at + word.len(), TEXT, "note", "you")
}

#[test]
fn captures_quote_and_context() {
    let c = comment("gamma");
    assert_eq!(c.quote, "gamma");
    assert_eq!(c.prefix, "Alpha beta ");
    assert_eq!(c.suffix, " delta.");
}

#[test]
fn shifts_with_edits_before() {
    let mut c = comment("gamma");
    // Insert "very " before "beta" (location 6).
    c.note_edit(6..11, 5);
    assert_eq!(c.range, 16..21);
}

#[test]
fn ignores_edits_after() {
    let mut c = comment("beta");
    c.note_edit(17..20, 3);
    assert_eq!(c.range, 6..10);
}

#[test]
fn grows_with_edits_inside() {
    let mut c = comment("gamma");
    c.note_edit(13..15, 2);
    assert_eq!(c.range, 11..18);
}

#[test]
fn detaches_when_text_deleted() {
    let mut c = comment("gamma");
    c.note_edit(11..11, -5);
    assert!(c.detached);
}

#[test]
fn undone_deletion_reanchors() {
    let mut c = Comment::new(6..11, "Hello world", "n", "you");
    c.note_edit(6..6, -5);
    assert!(c.detached);
    c.note_edit(6..11, 5);
    assert!(c.reanchor("Hello world"));
    assert_eq!(c.range, 6..11);
}

#[test]
fn reanchors_after_outside_change() {
    let mut c = comment("gamma");
    let changed = "New start. Alpha beta gamma delta.";
    assert!(c.reanchor(changed));
    assert_eq!(&changed[c.range.clone()], "gamma");
    assert!(!c.reanchor("Nothing here."));
    assert!(c.detached);
}

#[test]
fn context_is_32_utf16_units() {
    let text = format!("{}gamma{}", "😀".repeat(20), "中".repeat(40));
    let at = text.find("gamma").unwrap();
    let c = Comment::new(at..at + 5, &text, "n", "you");
    assert_eq!(c.prefix, "😀".repeat(16));
    assert_eq!(c.suffix, "中".repeat(32));
}

// MARK: NotesStoreTests

fn store() -> (tempfile::TempDir, NotesStore) {
    let dir = tempfile::Builder::new()
        .prefix("fw-notes-")
        .tempdir()
        .unwrap();
    let store = NotesStore::new(dir.path());
    (dir, store)
}

fn secs(t: SystemTime) -> f64 {
    t.duration_since(UNIX_EPOCH).unwrap().as_secs_f64()
}

#[test]
fn save_load_move_round_trip() {
    let (_dir, store) = store();
    let mut notes = DocumentNotes::new("/tmp/a.md");
    notes
        .add_comment("world", 1, "Which one?", "you", "hello world")
        .unwrap();
    notes.add_version(DraftVersion::new(
        VersionKind::Proposal,
        "Shorter",
        "Codex",
        "hi",
    ));
    store.save(&notes, "hello world").unwrap();
    let loaded = store.load("/tmp/a.md", "hello world");
    assert_eq!(
        loaded.comments.iter().map(|c| &c.id).collect::<Vec<_>>(),
        notes.comments.iter().map(|c| &c.id).collect::<Vec<_>>()
    );
    assert_eq!(
        loaded
            .versions
            .iter()
            .map(|v| v.body.as_str())
            .collect::<Vec<_>>(),
        ["hi"]
    );
    assert!((secs(loaded.comments[0].created) - secs(notes.comments[0].created)).abs() < 0.002);
    store.move_notes("/tmp/a.md", "/tmp/b.md");
    assert!(store.load("/tmp/a.md", "").is_empty());
    assert_eq!(store.load("/tmp/b.md", "hello world").comments.len(), 1);
}

#[test]
fn comment_spans_are_utf16_on_disk() {
    let (_dir, store) = store();
    let text = "👍🏽 中文 world";
    let mut notes = DocumentNotes::new("/tmp/u.md");
    let c = notes.add_comment("world", 1, "n", "you", text).unwrap();
    assert_eq!(&text[c.range.clone()], "world");
    store.save(&notes, text).unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(store.path_for("/tmp/u.md")).unwrap()).unwrap();
    assert_eq!(raw["comments"][0]["location"], 8);
    assert_eq!(raw["comments"][0]["length"], 5);
    assert!(
        raw["comments"][0]["created"]
            .as_str()
            .unwrap()
            .ends_with('Z')
    );
    assert!(raw["comments"][0].get("resolved").is_none());
    let loaded = store.load("/tmp/u.md", text);
    assert_eq!(&text[loaded.comments[0].range.clone()], "world");
}

#[test]
fn reads_notes_written_by_swift() {
    let json = r#"{
  "comments" : [
    {
      "author" : "you",
      "body" : "Which one?",
      "created" : "2026-11-10T09:30:00.123Z",
      "detached" : false,
      "id" : "ab12cd34",
      "length" : 5,
      "location" : 3,
      "prefix" : "😀 ",
      "quote" : "world",
      "replies" : [
        {
          "author" : "Claude",
          "date" : "2026-11-10T09:31:00Z",
          "text" : "The second."
        }
      ],
      "resolved" : "2026-11-10T09:32:00.000Z",
      "suffix" : ""
    }
  ],
  "path" : "\/tmp\/a.md",
  "transcript" : [
    {
      "action" : "feedback",
      "commentIDs" : [ "ab12cd34" ],
      "date" : "2026-11-10T09:30:00.000Z",
      "feedback" : "ok",
      "instruction" : "",
      "provider" : "Codex"
    }
  ],
  "versions" : [
    {
      "body" : "hi",
      "created" : "2026-11-10T09:30:00.000Z",
      "id" : "ffff0000",
      "kind" : "snapshot",
      "label" : "Before",
      "resolvesCommentIDs" : [ ],
      "source" : "you",
      "usedAt" : "2026-11-10T09:33:00.000Z"
    }
  ]
}"#;
    let text = "😀 world";
    let notes = DocumentNotes::from_json(json.as_bytes(), text).unwrap();
    assert_eq!(notes.path, "/tmp/a.md");
    assert_eq!(&text[notes.comments[0].range.clone()], "world");
    assert!(notes.comments[0].resolved.is_some());
    assert_eq!(notes.comments[0].replies[0].author, "Claude");
    assert_eq!(notes.versions[0].kind, VersionKind::Snapshot);
    assert!(notes.versions[0].used_at.is_some());
    assert_eq!(notes.transcript[0].version_id, None);
    // Writes back the same keys.
    let again: serde_json::Value = serde_json::from_slice(&notes.to_json(text).unwrap()).unwrap();
    let original: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(
        again["comments"][0]["location"],
        original["comments"][0]["location"]
    );
    assert_eq!(
        again["comments"][0]["created"],
        original["comments"][0]["created"]
    );
    assert_eq!(again["versions"], original["versions"]);
}

#[test]
fn notes_key_matches_swift() {
    // SHA-256("/tmp/a.md") truncated to 12 bytes, as CryptoKit computed it
    // (`printf /tmp/a.md | sha256sum | cut -c1-24`).
    assert_eq!(
        NotesStore::key("/tmp/a.md"),
        NotesStore::key("/tmp/./x/../a.md")
    );
    assert_eq!(NotesStore::key("/tmp/a.md").len(), 24);
    #[cfg(unix)]
    assert_eq!(NotesStore::key("/tmp/a.md"), "15f9566a84264471c85904be");
}

#[test]
fn apply_commands() {
    let mut notes = DocumentNotes::new("/x.md");
    let text = "One. Two. Three.";
    let mut add = InboxCommand::new(InboxCommandKind::AddComment, "/x.md", "Claude Code");
    add.quote = Some("Two.".into());
    add.body = Some("Cut this?".into());
    let r1 = NotesStore::apply(&add, &mut notes, text);
    assert!(r1.ok);
    assert_eq!(notes.comments[0].author, "Claude Code");

    let mut propose = InboxCommand::new(InboxCommandKind::ProposeVersion, "/x.md", "Claude Code");
    propose.label = Some("Cut two".into());
    propose.edits = Some(vec![ProposedEdit::replace("Two. ", "")]);
    propose.resolves_comment_ids = Some(vec![r1.comment_id.clone().unwrap()]);
    let r2 = NotesStore::apply(&propose, &mut notes, text);
    assert!(r2.ok, "{}", r2.message);
    assert_eq!(notes.versions[0].body, "One. Three.");
    assert_eq!(
        notes.versions[0].resolves_comment_ids,
        [r1.comment_id.clone().unwrap()]
    );

    let mut partial = InboxCommand::new(InboxCommandKind::ProposeVersion, "/x.md", "Claude Code");
    partial.label = Some("Partial".into());
    partial.edits = Some(vec![
        ProposedEdit::replace("One.", "1.").with_comment_ids(vec!["a".into()]),
        ProposedEdit::replace("Missing", "x").with_comment_ids(vec!["b".into()]),
    ]);
    let r3 = NotesStore::apply(&partial, &mut notes, text);
    assert!(r3.ok);
    assert!(
        r3.message
            .contains("1 edit couldn't be placed: #2 Couldn't find"),
        "{}",
        r3.message
    );
    assert_eq!(notes.versions[0].resolves_comment_ids, ["a"]);

    let mut bad = InboxCommand::new(InboxCommandKind::ProposeVersion, "/x.md", "Claude Code");
    bad.edits = Some(vec![ProposedEdit::replace("Missing", "x")]);
    assert!(!NotesStore::apply(&bad, &mut notes, text).ok);
}

#[test]
fn reply_and_resolve_commands() {
    let mut notes = DocumentNotes::new("/x.md");
    let text = "One. Two.";
    let c = notes.add_comment("Two.", 1, "Cut?", "you", text).unwrap();
    let mut reply = InboxCommand::new(InboxCommandKind::ReplyToComment, "/x.md", "Codex");
    reply.comment_id = Some(c.id.clone());
    assert!(
        !NotesStore::apply(&reply, &mut notes, text).ok,
        "empty reply"
    );
    reply.body = Some("Yes.".into());
    assert!(NotesStore::apply(&reply, &mut notes, text).ok);
    let mut resolve = InboxCommand::new(InboxCommandKind::ResolveComment, "/x.md", "Codex");
    resolve.comment_id = Some(c.id.clone());
    resolve.body = Some("Done.".into());
    assert!(NotesStore::apply(&resolve, &mut notes, text).ok);
    assert_eq!(notes.comments[0].replies.len(), 2);
    assert!(!notes.comments[0].is_open());
    let err = notes.add_comment("One.", 2, "x", "you", text).unwrap_err();
    assert_eq!(
        err.message,
        "The text appears 1 time; occurrence must be 1–1."
    );
}

#[test]
fn inbox_round_trip() {
    let (_dir, store) = store();
    let mut a = InboxCommand::new(InboxCommandKind::AddComment, "/x.md", "Claude Code");
    a.quote = Some("q".into());
    store.post(&a).unwrap();
    let pending = store.pending_commands();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].1.kind, InboxCommandKind::AddComment);
    let raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&pending[0].0).unwrap()).unwrap();
    assert_eq!(raw["kind"], "addComment");
    let mut r = writer_core::InboxResult::new(&a.id, true, "ok");
    r.comment_id = Some("c".into());
    store.write_result(&r);
    assert_eq!(
        store.await_result(&a.id, std::time::Duration::from_secs(1)),
        Some(r)
    );
}

#[test]
fn live_state_round_trip() {
    let (_dir, store) = store();
    let text = "😀 hello";
    let at = text.find("hello").unwrap();
    let state = writer_core::LiveState::new(
        std::process::id(),
        "Fluent Writer",
        Some("/x.md".into()),
        None,
        "T",
        text,
        at..text.len(),
        None,
    );
    assert_eq!((state.selection_location, state.selection_length), (3, 5));
    assert_eq!(state.selected_text, "hello");
    store.write_live_state(&state);
    let read = store.live_state().expect("this process is running");
    assert_eq!(read.selection(text), at..text.len());
    store.clear_live_state();
    assert!(store.live_state().is_none());
}

#[test]
fn locked_writers_dont_lose_updates() {
    let (_dir, store) = store();
    std::thread::scope(|s| {
        for i in 0..16 {
            let store = &store;
            s.spawn(move || {
                store.with_lock(|| {
                    let mut n = store.load("/c.md", "");
                    n.add_version(DraftVersion::new(
                        VersionKind::Proposal,
                        i.to_string(),
                        "x",
                        i.to_string(),
                    ));
                    let _ = store.save(&n, "");
                });
            });
        }
    });
    assert_eq!(store.load("/c.md", "").versions.len(), 16);
}

#[test]
fn version_cap_drops_oldest_unused() {
    let mut notes = DocumentNotes::new("/x.md");
    let mut used = DraftVersion::new(VersionKind::Snapshot, "kept", "you", "kept");
    used.used_at = Some(SystemTime::now());
    notes.add_version(used);
    for i in 0..DocumentNotes::MAX_VERSIONS + 5 {
        notes.add_version(DraftVersion::new(
            VersionKind::Proposal,
            i.to_string(),
            "Codex",
            i.to_string(),
        ));
    }
    assert_eq!(notes.versions.len(), DocumentNotes::MAX_VERSIONS);
    assert!(notes.versions.iter().any(|v| v.label == "kept"));
}
