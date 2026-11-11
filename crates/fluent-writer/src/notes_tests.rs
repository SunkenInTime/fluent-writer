//! Headless tests for comments, the composer, version previews and the inbox, driving the
//! whole app through Rune's `Runtime` as `tests.rs` does.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rune::{Key, KeyEvent, Mods, Runtime};
use writer_core::{DraftVersion, InboxCommand, InboxCommandKind, InboxResult, NotesStore, VersionKind};

use crate::app::App;
use crate::tests::{command, harness, press, settle, type_text};

fn primary(shift: bool, alt: bool) -> Mods {
    Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), logo: cfg!(target_os = "macos"), shift, alt }
}

fn select(rt: &mut Runtime<App>, a: usize, b: usize) {
    rt.update(|s, _| s.edit.set_selection(a, b));
    rt.frame(1.0 / 60.0);
}

/// Adds a comment on the first appearance of `quote` and returns its id.
fn comment_on(rt: &mut Runtime<App>, quote: &str, body: &str) -> String {
    let start = rt.state.edit.text.find(quote).expect("quote is in the draft");
    let mut id = None;
    rt.update(|s, _| id = s.add_comment(start..start + quote.len(), body));
    rt.frame(1.0 / 60.0);
    id.expect("comment added")
}

fn span(rt: &Runtime<App>, id: &str) -> (String, bool) {
    let c = rt.state.notes.comment(id).expect("comment exists");
    let text = rt.state.edit.text.get(c.range.clone()).unwrap_or("").to_string();
    (text, c.detached)
}

fn backspace(rt: &mut Runtime<App>) {
    press(rt, Key::Backspace, Mods::default());
}

// MARK: Comments following edits

#[test]
fn comments_follow_typing_before_inside_and_after_their_text() {
    let (mut rt, _dir) = harness("Say 😀 hello world to me");
    let id = comment_on(&mut rt, "hello world", "Too plain");
    // Before the span, an emoji.
    select(&mut rt, 0, 0);
    type_text(&mut rt, "🎉 ");
    assert_eq!(span(&rt, &id), ("hello world".into(), false));
    // Inside the span.
    let at = rt.state.edit.text.find("world").unwrap();
    select(&mut rt, at, at);
    type_text(&mut rt, "big ");
    assert_eq!(span(&rt, &id), ("hello big world".into(), false));
    // Right after the span: not part of it.
    let end = rt.state.edit.text.find("world").unwrap() + "world".len();
    select(&mut rt, end, end);
    type_text(&mut rt, "!");
    assert_eq!(span(&rt, &id), ("hello big world".into(), false));
    // Right at its start: not part of it either.
    let start = rt.state.edit.text.find("hello").unwrap();
    select(&mut rt, start, start);
    type_text(&mut rt, "👋");
    assert_eq!(span(&rt, &id), ("hello big world".into(), false));
    assert_eq!(rt.state.edit.text, "🎉 Say 😀 👋hello big world! to me");
}

#[test]
fn comments_follow_deletions_before_inside_and_across_their_text() {
    let (mut rt, _dir) = harness("ab 😀😀 one two three end");
    let id = comment_on(&mut rt, "one two three", "Cut");
    // Delete an emoji before the span.
    let after_emoji = "ab 😀😀".len();
    select(&mut rt, after_emoji, after_emoji);
    backspace(&mut rt);
    assert_eq!(rt.state.edit.text, "ab 😀 one two three end");
    assert_eq!(span(&rt, &id), ("one two three".into(), false));
    // Delete a word inside.
    let two = rt.state.edit.text.find("two ").unwrap();
    select(&mut rt, two, two + 4);
    backspace(&mut rt);
    assert_eq!(span(&rt, &id), ("one three".into(), false));
    // Delete across the end: the part left keeps the comment.
    let three = rt.state.edit.text.find("three end").unwrap();
    select(&mut rt, three + 2, three + 6);
    backspace(&mut rt);
    assert_eq!(rt.state.edit.text, "ab 😀 one thend");
    assert_eq!(span(&rt, &id), ("one th".into(), false));
    // Delete after it: untouched.
    let len = rt.state.edit.text.len();
    select(&mut rt, len, len);
    backspace(&mut rt);
    assert_eq!(span(&rt, &id), ("one th".into(), false));
    // Once saved, the notes keep the quote as it now reads.
    save(&mut rt);
    settle(&mut rt, 0.7);
    assert_eq!(rt.state.notes.comment(&id).unwrap().quote, "one th");
    // Delete the whole span: detached, no highlight.
    let one = rt.state.edit.text.find("one th").unwrap();
    select(&mut rt, one, one + "one th".len());
    backspace(&mut rt);
    assert!(span(&rt, &id).1, "a comment whose text is deleted is detached");
    assert!(rt.state.notes.highlights(rt.state.edit.text.len(), &crate::theme::Palette::new(false)).is_empty());
    // Undo brings the text back and the comment re-attaches by its quote.
    command(&mut rt, 'z', false, false);
    assert_eq!(span(&rt, &id), ("one th".into(), false));
}

#[test]
fn typing_a_repeated_character_inside_a_span_lands_where_the_caret_is() {
    let (mut rt, _dir) = harness("aaaa");
    let mut id = None;
    rt.update(|s, _| id = s.add_comment(2..4, "x"));
    let id = id.unwrap();
    // Caret between the first two "a"s: the new "a" is before the span, which moves right.
    select(&mut rt, 1, 1);
    type_text(&mut rt, "a");
    assert_eq!(rt.state.notes.comment(&id).unwrap().range, 3..5);
    // Backspace at the span's end removes the span's last "a".
    select(&mut rt, 5, 5);
    backspace(&mut rt);
    assert_eq!(rt.state.notes.comment(&id).unwrap().range, 3..4);
}

#[test]
fn format_commands_move_comments_by_their_exact_edit() {
    let (mut rt, _dir) = harness("one two three");
    let id = comment_on(&mut rt, "three", "x");
    select(&mut rt, 4, 7);
    command(&mut rt, 'b', false, false);
    assert_eq!(rt.state.edit.text, "one **two** three");
    assert_eq!(span(&rt, &id), ("three".into(), false));
}

#[test]
fn highlights_mark_open_comments_and_the_active_one() {
    let (mut rt, _dir) = harness("alpha beta gamma");
    let a = comment_on(&mut rt, "alpha", "x");
    let b = comment_on(&mut rt, "gamma", "y");
    let p = crate::theme::Palette::new(false);
    rt.update(|s, _| s.notes.set_active_comment(Some(b.clone())));
    let spans = rt.state.notes.highlights(rt.state.edit.text.len(), &p);
    assert_eq!(spans.len(), 2);
    assert_eq!((spans[0].start, spans[0].end, spans[0].bg), (0, 5, Some(p.comment_highlight)));
    assert_eq!((spans[1].start, spans[1].end, spans[1].bg), (11, 16, Some(p.comment_highlight_active)));
    // Resolved comments aren't highlighted; Esc in the editor clears the active one.
    rt.update(|s, _| s.set_comment_resolved(&a, true));
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.notes.active_comment_id(), None);
    let spans = rt.state.notes.highlights(rt.state.edit.text.len(), &p);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].bg, Some(p.comment_highlight));
}

#[test]
fn comment_at_prefers_the_shortest_span() {
    let (mut rt, _dir) = harness("one two three");
    let _outer = comment_on(&mut rt, "one two three", "outer");
    let inner = comment_on(&mut rt, "two", "inner");
    assert_eq!(rt.state.notes.comment_at(5).map(|c| c.id.clone()), Some(inner));
    // With the Comments tab open, moving the caret makes the comment there active.
    command(&mut rt, 'm', true, false);
    assert!(rt.state.assist_visible);
    select(&mut rt, 1, 1);
    settle(&mut rt, 0.05);
    assert_eq!(rt.state.notes.active_comment_id().map(|i| rt.state.notes.comment(i).unwrap().body.clone()), Some("outer".into()));
}

// MARK: Composer

fn open_composer(rt: &mut Runtime<App>) {
    command(rt, 'm', false, true);
    settle(rt, 0.1);
}

#[test]
fn add_comment_comments_on_the_current_line_and_saves_on_return() {
    let (mut rt, _dir) = harness("First line\n   Second line here  \nThird");
    let caret = rt.state.edit.text.find("line here").unwrap();
    select(&mut rt, caret, caret);
    open_composer(&mut rt);
    let open = rt.state.composer.open.clone().expect("the composer is open");
    assert_eq!(open.quote, "Second line here");
    assert_eq!(rt.state.edit.selected(), "Second line here");
    assert!(rt.state.composer.shows_placeholder());
    assert!(rt.find("comment-composer").is_some());
    // Return with nothing typed beeps and stays open.
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.notes.beeps, 1);
    assert!(rt.state.composer.open.is_some());
    type_text(&mut rt, "Make it");
    press(&mut rt, Key::Enter, Mods { shift: true, ..Mods::default() });
    type_text(&mut rt, "shorter ");
    press(&mut rt, Key::Enter, Mods::default());
    assert!(rt.state.composer.open.is_none());
    let comments = rt.state.notes.open_comments();
    assert_eq!(comments.len(), 1);
    let c = comments[0];
    assert_eq!((c.body.as_str(), c.quote.as_str(), c.author.as_str()), ("Make it\nshorter", "Second line here", "you"));
    assert_eq!(rt.state.notes.active_comment_id(), Some(c.id.as_str()));
    let end = c.range.end;
    assert_eq!(rt.state.edit.range(), (end, end));
    let keys = crate::notes::shortcut_label(crate::commands::Command::ShowComments);
    assert_eq!(rt.state.status().0, format!("1 open comment · {keys} to review and send"));
}

#[test]
fn escape_closes_the_composer_without_saving() {
    let (mut rt, _dir) = harness("Some words");
    select(&mut rt, 0, 4);
    open_composer(&mut rt);
    assert_eq!(rt.state.composer.open.as_ref().unwrap().quote, "Some");
    type_text(&mut rt, "never mind");
    press(&mut rt, Key::Escape, Mods::default());
    assert!(rt.state.composer.open.is_none());
    assert!(rt.state.notes.open_comments().is_empty());
}

#[test]
fn add_comment_on_a_blank_line_beeps() {
    let (mut rt, _dir) = harness("Words\n\n");
    select(&mut rt, 6, 6);
    open_composer(&mut rt);
    assert!(rt.state.composer.open.is_none());
    assert_eq!(rt.state.notes.beeps, 1);
}

#[test]
fn editing_a_comment_prefills_the_composer() {
    let (mut rt, _dir) = harness("alpha beta");
    let id = comment_on(&mut rt, "beta", "Old");
    rt.update(|s, tx| s.edit_comment(&id, tx));
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.composer.edit.text, "Old");
    assert!(!rt.state.composer.shows_placeholder());
    type_text(&mut rt, " and new");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.notes.comment(&id).unwrap().body, "Old and new");
}

// MARK: Versions

fn add_version(rt: &mut Runtime<App>, label: &str, body: &str) -> String {
    let v = DraftVersion::new(VersionKind::Proposal, label, "Codex", body);
    let id = v.id.clone();
    rt.update(|s, _| s.add_version(v));
    rt.frame(1.0 / 60.0);
    id
}

#[test]
fn previewing_steps_through_versions_and_back_to_the_draft() {
    let (mut rt, _dir) = harness("The dog sat on the mat.");
    let older = add_version(&mut rt, "Older", "The cat sat on the mat.");
    let newer = add_version(&mut rt, "Newer", "The dog sat.");
    // Without a preview, Newer and Use are disabled and → beeps.
    assert!(!rt.state.command_enabled(crate::commands::Command::NewerVersion));
    assert!(rt.state.command_enabled(crate::commands::Command::OlderVersion));
    // ⌥⌘← opens the newest version.
    press(&mut rt, Key::Left, primary(false, true));
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.preview.version_id(), Some(newer.as_str()));
    let r = &rt.state.preview.rendered;
    assert_eq!((r.position, r.total), (2, 2));
    assert_eq!(r.combined, "The dog sat on the mat.");
    assert_eq!(r.detail, "From Codex · just now · +0 −3 words");
    assert!(rt.find("version-bar").is_some());
    // ← in the diff goes older; ← at the oldest beeps.
    press(&mut rt, Key::Left, Mods::default());
    assert_eq!(rt.state.preview.version_id(), Some(older.as_str()));
    assert_eq!(rt.state.preview.rendered.combined, "The dog cat sat on the mat.");
    press(&mut rt, Key::Left, Mods::default());
    assert_eq!(rt.state.notes.beeps, 1);
    // Typing in the diff changes nothing.
    type_text(&mut rt, "x");
    assert_eq!(rt.state.preview.edit.text, "The dog cat sat on the mat.");
    assert_eq!(rt.state.edit.text, "The dog sat on the mat.");
    // → twice returns to the draft; → again beeps.
    press(&mut rt, Key::Right, Mods::default());
    press(&mut rt, Key::Right, Mods::default());
    assert!(!rt.state.preview.is_active());
    assert!(rt.find("version-bar").is_none());
    rt.update(|s, tx| s.preview_step(1, tx));
    assert_eq!(rt.state.notes.beeps, 3);
    // Esc goes back too.
    rt.update(|s, tx| s.preview_version(&older, tx));
    settle(&mut rt, 0.05);
    press(&mut rt, Key::Escape, Mods::default());
    assert!(!rt.state.preview.is_active());
}

#[test]
fn using_a_version_keeps_the_draft_as_a_version_and_moves_comments_with_the_words() {
    let (mut rt, _dir) = harness("Keep this. The dog sat on the mat. And keep this too.");
    let kept = comment_on(&mut rt, "keep this too", "Nice");
    let fixed = comment_on(&mut rt, "dog", "Which animal?");
    let mut v = DraftVersion::new(VersionKind::Proposal, "Cat", "Claude", "Keep this. The cat sat on the mat. And keep this too.");
    v.resolves_comment_ids = vec![fixed.clone()];
    let vid = v.id.clone();
    rt.update(|s, _| s.add_version(v));
    rt.update(|s, tx| s.preview_version(&vid, tx));
    settle(&mut rt, 0.1);
    // ⌥⌘↩ uses it.
    press(&mut rt, Key::Enter, primary(false, true));
    assert!(!rt.state.preview.is_active());
    assert_eq!(rt.state.edit.text, "Keep this. The cat sat on the mat. And keep this too.");
    assert_eq!(span(&rt, &kept), ("keep this too".into(), false));
    assert!(!rt.state.notes.comment(&fixed).unwrap().is_open(), "the comment the version addresses is resolved");
    let versions = rt.state.notes.versions();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].kind, VersionKind::Snapshot);
    assert!(versions[0].label.starts_with("Your draft · "), "{}", versions[0].label);
    assert_eq!(versions[0].source, "you");
    assert_eq!(versions[0].body, "Keep this. The dog sat on the mat. And keep this too.");
    assert!(versions[1].used_at.is_some());
    let undo = crate::notes::shortcut_label(crate::commands::Command::Undo);
    assert_eq!(rt.state.status().0, format!("Version used · {undo} or Versions to go back"));
    // The caret sits at the first change, and one undo restores the draft.
    assert_eq!(rt.state.edit.range(), (15, 15));
    command(&mut rt, 'z', false, false);
    assert_eq!(rt.state.edit.text, "Keep this. The dog sat on the mat. And keep this too.");
    assert_eq!(span(&rt, &kept), ("keep this too".into(), false));
}

#[test]
fn use_is_disabled_for_a_version_identical_to_the_draft_and_deleting_the_previewed_one_closes_it() {
    let (mut rt, _dir) = harness("Same text");
    let id = add_version(&mut rt, "Same", "Same text");
    rt.update(|s, tx| s.preview_version(&id, tx));
    settle(&mut rt, 0.05);
    assert!(rt.state.preview.rendered.same_as_draft());
    assert_eq!(rt.state.preview.rendered.detail, "From Codex · just now · same as your draft");
    rt.update(|s, tx| s.delete_version(&id, tx));
    settle(&mut rt, 0.05);
    assert!(!rt.state.preview.is_active());
    assert!(rt.state.notes.versions().is_empty());
}

#[test]
fn adding_a_comment_while_previewing_closes_the_preview_first() {
    let (mut rt, _dir) = harness("Some words here");
    let id = add_version(&mut rt, "V", "Other words here");
    rt.update(|s, tx| s.preview_version(&id, tx));
    settle(&mut rt, 0.05);
    open_composer(&mut rt);
    assert!(!rt.state.preview.is_active());
    assert!(rt.state.composer.open.is_some());
}

#[test]
fn show_comments_and_versions_open_the_assist_panel_on_that_tab() {
    let (mut rt, _dir) = harness("Text");
    command(&mut rt, 'v', false, true);
    assert!(rt.state.assist_visible);
    assert_eq!(rt.state.notes.assist_tab, crate::notes::AssistTab::Versions);
    command(&mut rt, 'm', true, false);
    assert_eq!(rt.state.notes.assist_tab, crate::notes::AssistTab::Comments);
}

// MARK: Saving, the live state and the inbox

/// Saves the draft to a file, so its notes have a path.
fn save(rt: &mut Runtime<App>) -> PathBuf {
    rt.update(|s, _| {
        s.flush();
    });
    settle(rt, 0.05);
    rt.state.draft.path().expect("saved").to_path_buf()
}

fn support(dir: &Path) -> PathBuf {
    dir.join("support")
}

#[test]
fn notes_are_saved_after_a_pause_and_follow_the_first_save() {
    let (mut rt, dir) = harness("A draft about cats");
    let id = comment_on(&mut rt, "cats", "Dogs?");
    let path = save(&mut rt);
    assert_eq!(rt.state.notes.path(), Some(path.to_str().unwrap()));
    settle(&mut rt, 0.7);
    rt.state.notes.wait_for_disk();
    let store = NotesStore::new(support(dir.path()));
    let text = std::fs::read_to_string(&path).unwrap();
    let saved = store.load(path.to_str().unwrap(), &text);
    assert_eq!(saved.comments.len(), 1);
    assert_eq!(saved.comments[0].id, id);
    assert_eq!(saved.comments[0].quote, "cats");
}

#[test]
fn the_live_state_follows_the_selection() {
    let (mut rt, dir) = harness("Hello there world");
    let path = save(&mut rt);
    select(&mut rt, 6, 11);
    settle(&mut rt, 0.2);
    rt.state.notes.wait_for_disk();
    let store = NotesStore::new(support(dir.path()));
    // 0.4 s after the change, not before.
    let before = store.live_state().map(|l| l.selected_text);
    assert_ne!(before.as_deref(), Some("there"));
    settle(&mut rt, 0.3);
    rt.state.notes.wait_for_disk();
    let live = store.live_state().expect("the live state is written");
    assert_eq!(live.path.as_deref(), path.to_str());
    assert_eq!(live.selected_text, "there");
    assert_eq!((live.selection_location, live.selection_length), (6, 5));
    assert_eq!(live.pid, std::process::id());
    drop(rt);
    assert!(store.live_state().is_none(), "the live state goes when the app quits");
}

fn await_result(rt: &mut Runtime<App>, store: &NotesStore, id: &str) -> InboxResult {
    let deadline = Instant::now() + Duration::from_secs(5);
    let file = store.results_directory().join(format!("{id}.json"));
    while Instant::now() < deadline {
        rt.frame(1.0 / 60.0);
        rt.state.notes.wait_for_disk();
        if let Ok(data) = std::fs::read(&file) {
            return serde_json::from_slice(&data).unwrap();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no result for {id}");
}

#[test]
fn inbox_commands_apply_to_the_open_draft_and_answer() {
    let (mut rt, dir) = harness("The opening line is weak. The rest is fine.");
    let path = save(&mut rt);
    settle(&mut rt, 0.1);
    let store = NotesStore::new(support(dir.path()));
    let p = path.to_str().unwrap();

    let mut add = InboxCommand::new(InboxCommandKind::AddComment, p, "Claude Code");
    add.quote = Some("opening line".into());
    add.body = Some("Start with the claim.".into());
    store.post(&add).unwrap();
    let r = await_result(&mut rt, &store, &add.id);
    assert!(r.ok, "{}", r.message);
    let cid = r.comment_id.clone().unwrap();
    assert_eq!(r.message, format!("Added comment {cid}."));
    assert_eq!(rt.state.status().0, "Claude Code added a comment");
    assert_eq!(span(&rt, &cid), ("opening line".into(), false));
    assert!(!store.inbox_directory().join(format!("{}.json", add.id)).exists(), "the command file is removed");

    let mut reply = InboxCommand::new(InboxCommandKind::ReplyToComment, p, "Claude Code");
    reply.comment_id = Some(cid.clone());
    reply.body = Some("Done in the version.".into());
    store.post(&reply).unwrap();
    assert!(await_result(&mut rt, &store, &reply.id).ok);
    assert_eq!(rt.state.notes.comment(&cid).unwrap().replies[0].text, "Done in the version.");

    let mut propose = InboxCommand::new(InboxCommandKind::ProposeVersion, p, "Claude Code");
    propose.edits = Some(vec![writer_core::ProposedEdit::replace("is weak", "is strong")]);
    store.post(&propose).unwrap();
    let r = await_result(&mut rt, &store, &propose.id);
    assert!(r.ok, "{}", r.message);
    let vid = r.version_id.unwrap();
    assert_eq!(rt.state.notes.versions()[0].body, "The opening line is strong. The rest is fine.");
    assert_eq!(rt.state.notes.versions()[0].label, "Claude Code version");
    let keys = crate::notes::shortcut_label(crate::commands::Command::ShowVersions);
    assert_eq!(rt.state.status().0, format!("Claude Code proposed a version · {keys} to see it"));
    // The notes file already holds it when the agent hears back.
    let saved = store.load(p, &std::fs::read_to_string(&path).unwrap());
    assert!(saved.versions.iter().any(|v| v.id == vid));

    let mut resolve = InboxCommand::new(InboxCommandKind::ResolveComment, p, "Claude Code");
    resolve.comment_id = Some(cid.clone());
    store.post(&resolve).unwrap();
    assert!(await_result(&mut rt, &store, &resolve.id).ok);
    assert!(!rt.state.notes.comment(&cid).unwrap().is_open());

    let mut bad = InboxCommand::new(InboxCommandKind::AddComment, p, "Claude Code");
    bad.quote = Some("not in the draft".into());
    bad.body = Some("x".into());
    store.post(&bad).unwrap();
    let r = await_result(&mut rt, &store, &bad.id);
    assert!(!r.ok);
    assert_eq!(r.message, "That text isn't in the draft.");
}

#[test]
fn inbox_commands_for_other_drafts_go_to_their_notes() {
    let (mut rt, dir) = harness("Open draft");
    save(&mut rt);
    let other = dir.path().join("drafts/Other.md");
    std::fs::write(&other, "Another draft entirely").unwrap();
    let store = NotesStore::new(support(dir.path()));
    let mut add = InboxCommand::new(InboxCommandKind::AddComment, other.to_str().unwrap(), "Codex CLI");
    add.quote = Some("Another".into());
    add.body = Some("Hm".into());
    store.post(&add).unwrap();
    assert!(await_result(&mut rt, &store, &add.id).ok);
    let saved = store.load(other.to_str().unwrap(), "Another draft entirely");
    assert_eq!(saved.comments[0].author, "Codex CLI");
    assert!(rt.state.notes.open_comments().is_empty());

    let missing = dir.path().join("drafts/Missing.md");
    let mut add = InboxCommand::new(InboxCommandKind::AddComment, missing.to_str().unwrap(), "Codex CLI");
    add.quote = Some("x".into());
    add.body = Some("y".into());
    store.post(&add).unwrap();
    let r = await_result(&mut rt, &store, &add.id);
    assert_eq!(r.message, format!("Couldn't read {}.", missing.display()));
}

#[test]
fn opening_a_draft_reanchors_its_comments_by_quote() {
    let (mut rt, dir) = harness("Alpha beta gamma");
    let id = comment_on(&mut rt, "gamma", "x");
    let path = save(&mut rt);
    rt.update(|s, _| s.notes.flush_save());
    rt.state.notes.wait_for_disk();
    // The file changes elsewhere while it's closed.
    rt.update(|s, tx| s.new_draft(tx));
    std::fs::write(&path, "New start. Alpha beta gamma").unwrap();
    let doc = rt.state.library.open(&path).unwrap();
    rt.update(|s, _| s.load(doc));
    settle(&mut rt, 0.05);
    assert_eq!(span(&rt, &id), ("gamma".into(), false));
    let _ = dir;
}

#[test]
fn keys_reach_the_preview_through_the_window_too() {
    let (mut rt, _dir) = harness("one two");
    let id = add_version(&mut rt, "V", "one three");
    rt.on_key = Some(crate::keys::window_key);
    rt.update(|s, tx| s.preview_version(&id, tx));
    settle(&mut rt, 0.05);
    rt.update(|_, tx| tx.blur());
    rt.frame(1.0 / 60.0);
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(1.0 / 60.0);
    assert!(!rt.state.preview.is_active());
}
