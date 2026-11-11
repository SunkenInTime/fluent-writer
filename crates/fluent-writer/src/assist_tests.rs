//! Headless tests for the Assist panel and the connect sheet, driving the whole app through
//! Rune's `Runtime`. Replies come from canned sessions, replayed word by word on a thread as
//! `FLUENT_WRITER_AGENT_FIXTURE` does, so the panel streams them exactly as it would a provider's.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rune::{Key, KeyEvent, Mods, Runtime};
use writer_agents::{AgentEvent, AgentProvider, AgentSession, EventHandler, FixtureAgent, ProcessSink};
use writer_agents::line_process::ProcessEvent;

use crate::app::App;
use crate::assist::INSTRUCTION;
use crate::notes::AssistTab;
use crate::tests::{harness, press, settle, type_text};

fn primary(shift: bool, alt: bool) -> Mods {
    Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), logo: cfg!(target_os = "macos"), shift, alt }
}

fn chord(rt: &mut Runtime<App>, c: char, shift: bool, alt: bool) {
    press(rt, Key::Command(c), primary(shift, alt));
    settle(rt, 0.05);
}

fn click(rt: &mut Runtime<App>, key: &str) {
    settle(rt, 0.05);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("{key} is on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt, 0.05);
}

/// A session that replays `reply` and keeps the prompt it was sent.
struct Recording {
    inner: FixtureAgent,
    prompt: Arc<Mutex<String>>,
}

impl AgentSession for Recording {
    fn start(&mut self, prompt: &str, on_event: EventHandler, sink: ProcessSink) {
        *self.prompt.lock().unwrap() = prompt.to_string();
        self.inner.start(prompt, on_event, sink);
    }
    fn handle(&mut self, event: ProcessEvent) {
        self.inner.handle(event);
    }
    fn cancel(&mut self) {
        self.inner.cancel();
    }
    fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }
}

/// A session that fails at once with `message`.
struct Failing(String, bool);

impl AgentSession for Failing {
    fn start(&mut self, _: &str, mut on_event: EventHandler, _: ProcessSink) {
        self.1 = true;
        on_event(AgentEvent::Failed(self.0.clone()));
    }
    fn handle(&mut self, _: ProcessEvent) {}
    fn cancel(&mut self) {
        self.1 = true;
    }
    fn is_finished(&self) -> bool {
        self.1
    }
}

/// Replies with `reply` to every request; returns where the last prompt lands.
fn replying(rt: &mut Runtime<App>, reply: &str) -> Arc<Mutex<String>> {
    let prompt = Arc::new(Mutex::new(String::new()));
    let (reply, kept) = (reply.to_string(), prompt.clone());
    rt.state.assist.session_maker = Some(Box::new(move |_| Box::new(Recording { inner: FixtureAgent::new(reply.clone()), prompt: kept.clone() })));
    prompt
}

/// Adds a comment through an update, so the view rebuilds.
fn add(rt: &mut Runtime<App>, range: std::ops::Range<usize>, body: &str) -> String {
    let mut id = None;
    rt.update(|s, _| id = s.add_comment(range, body));
    rt.frame(1.0 / 60.0);
    id.expect("comment added")
}

/// Runs frames in real time until `done` holds or five seconds pass.
fn wait(rt: &mut Runtime<App>, mut done: impl FnMut(&App) -> bool) {
    let start = Instant::now();
    while !done(&rt.state) {
        assert!(start.elapsed() < Duration::from_secs(5), "timed out; status {:?}", rt.state.assist.status);
        rt.frame(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(4));
    }
    settle(rt, 0.05);
}

const DRAFT: &str = "The cat sat on the mat. It was a fine day.";

const REPLY: &str = "<feedback>The opening is flat. Lead with the surprise.</feedback>\n<edit>\n<old>The cat sat on the mat.</old>\n<new>On the mat, a cat sat.</new>\n</edit>";

// MARK: Opening and closing

#[test]
fn ask_codex_opens_on_ask_with_the_field_focused_and_escape_closes() {
    let (mut rt, _dir) = harness(DRAFT);
    rt.update(|s, _| s.assist.provider = AgentProvider::Claude);
    chord(&mut rt, 'j', false, false);
    settle(&mut rt, 1.0);
    assert!(rt.state.assist_visible);
    assert_eq!(rt.state.notes.assist_tab, AssistTab::Ask);
    assert_eq!(rt.state.assist.provider, AgentProvider::Codex, "⌘J picks Codex");
    assert!(rt.state.assist.instruction.focused, "the instruction field has the keyboard");
    let ([x, _, w, h], _) = rt.presented("assist-panel").expect("panel drawn");
    assert_eq!((x, w, h), (1060.0 - 380.0, 380.0, 760.0));
    assert_eq!(rt.state.frame().editor_w, 1060.0 - 380.0);
    // Shift-Cmd-J switches to Claude.
    chord(&mut rt, 'j', true, false);
    assert_eq!(rt.state.assist.provider, AgentProvider::Claude);
    assert_eq!(rt.state.prefs.assist_provider(), AgentProvider::Claude, "the choice is kept");
    assert_eq!(rt.state.assist.scope, "Full draft · 11 words → Claude");
    // Escape in the idle field closes the panel and gives the keyboard back to the draft.
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt, 0.05);
    assert!(!rt.state.assist_visible);
    assert!(rt.state.edit.focused);
    // ⌥⌘J reopens it on the last tab, and closes it again.
    rt.update(|s, _| s.notes.assist_tab = AssistTab::Versions);
    chord(&mut rt, 'j', false, true);
    assert!(rt.state.assist_visible);
    assert_eq!(rt.state.notes.assist_tab, AssistTab::Versions);
    chord(&mut rt, 'j', false, true);
    assert!(!rt.state.assist_visible);
}

#[test]
fn escape_in_the_draft_clears_the_active_comment_before_closing_the_panel() {
    let (mut rt, _dir) = harness(DRAFT);
    chord(&mut rt, 'm', true, false);
    assert_eq!(rt.state.notes.assist_tab, AssistTab::Comments);
    let id = add(&mut rt, 4..7, "Dog?");
    rt.update(|s, tx| s.reveal_comment(&id, tx));
    settle(&mut rt, 0.05);
    rt.update(|s, tx| {
        s.edit.focused = true;
        tx.focus(crate::app::DRAFT, false);
    });
    settle(&mut rt, 0.05);
    press(&mut rt, Key::Escape, Mods::default());
    assert!(rt.state.notes.active_comment_id().is_none());
    assert!(rt.state.assist_visible);
    press(&mut rt, Key::Escape, Mods::default());
    assert!(!rt.state.assist_visible);
}

#[test]
fn ask_needs_a_draft_and_the_panel_keeps_the_chrome_up_while_typing() {
    let (mut rt, _dir) = harness("");
    chord(&mut rt, 'j', false, false);
    assert!(!rt.state.assist_visible, "Ask Codex is disabled for an empty draft");
    chord(&mut rt, 'm', true, false);
    assert!(rt.state.assist_visible, "Show Comments is always enabled");
    assert_eq!(rt.state.assist.scope, "Write something first");
    rt.update(|s, tx| {
        s.edit.focused = true;
        tx.focus(crate::app::DRAFT, false);
    });
    settle(&mut rt, 0.05);
    type_text(&mut rt, "Hi");
    assert!(rt.state.chrome_visible, "typing doesn't hide the chrome while the panel is open");
}

// MARK: Scope

#[test]
fn the_scope_line_follows_the_selection_and_the_preview() {
    let (mut rt, _dir) = harness(DRAFT);
    chord(&mut rt, 'j', false, false);
    assert_eq!(rt.state.assist.scope, "Full draft · 11 words → Codex");
    assert_eq!(rt.state.assist.scope_label, "Will send the full draft, 11 words, to Codex");
    rt.update(|s, _| s.edit.set_selection(4, 7));
    settle(&mut rt, 0.05);
    assert_eq!(rt.state.assist.scope, "Draft · focus on 1 selected word → Codex");
    assert_eq!(rt.state.assist.scope_label, "Will send the draft, 11 words, focused on 1 selected words, to Codex");
    // Previewing a version, then a selection change, names the version.
    let v = writer_core::DraftVersion::new(writer_core::VersionKind::Proposal, "Short", "Codex", "A cat sat.");
    let id = v.id.clone();
    rt.update(|s, tx| {
        s.add_version(v);
        s.preview_version(&id, tx);
        s.assist_refresh_scope();
    });
    assert_eq!(rt.state.assist.scope, "Version “Short” · 3 words → Codex");
    assert_eq!(rt.state.assist.scope_label, "Will send the previewed version, 3 words, to Codex");
}

// MARK: Sending

#[test]
fn a_reply_streams_in_becomes_a_version_and_previews_then_use_applies_it() {
    let (mut rt, _dir) = harness(DRAFT);
    let prompt = replying(&mut rt, REPLY);
    chord(&mut rt, 'j', false, false);
    type_text(&mut rt, "Make the opening stronger");
    press(&mut rt, Key::Enter, Mods::default());
    assert!(rt.state.assist.running);
    assert_eq!(rt.state.assist.status, "Codex is reading the draft…");
    assert_eq!(rt.state.assist.scope, "Full draft · 11 words → Codex");
    let send = rt.find("assist-send").unwrap();
    assert!(send.0 > 0.0);
    // Streaming: the feedback grows before the reply finishes, then the status says edits are coming.
    let mut partial = None;
    let mut drafting = false;
    wait(&mut rt, |s| {
        if s.assist.running && !s.assist.feedback.is_empty() && partial.is_none() {
            partial = Some(s.assist.feedback.clone());
        }
        drafting |= s.assist.status == "Drafting edits…";
        !s.assist.running
    });
    let partial = partial.expect("feedback streamed in");
    assert!(partial.len() < "The opening is flat. Lead with the surprise.".len(), "saw a partial reply: {partial:?}");
    assert!(drafting, "the status said Drafting edits…");
    assert!(prompt.lock().unwrap().contains("Author's instruction: Make the opening stronger"));
    let s = &rt.state;
    assert_eq!(s.assist.feedback, "The opening is flat. Lead with the surprise.");
    assert_eq!(s.assist.status, "1 edit ready as a new version, shown in the editor. ← → switch versions · ↩ use it · esc back to your draft.");
    let v = &s.notes.versions()[0];
    assert_eq!(v.label, "Make the opening stronger");
    assert_eq!(v.source, "Codex");
    assert_eq!(v.body, "On the mat, a cat sat. It was a fine day.");
    assert_eq!(v.feedback.as_deref(), Some("The opening is flat. Lead with the surprise."));
    assert_eq!(s.edit.text, DRAFT, "the draft is untouched until the version is used");
    assert_eq!(s.preview.version_id(), Some(v.id.as_str()), "the version is previewed as a diff");
    assert!(!s.preview.rendered.same_as_draft());
    let ex = &s.notes.transcript()[0];
    assert_eq!((ex.provider.as_str(), ex.action.as_str(), ex.instruction.as_str()), ("Codex", "Ask", "Make the opening stronger"));
    assert_eq!(ex.version_id.as_deref(), Some(v.id.as_str()));
    assert!(rt.find("assist-preview").is_some(), "Preview Version shows for the last reply's version");
    // Use it.
    rt.update(|s, tx| s.preview_use(tx));
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.edit.text, "On the mat, a cat sat. It was a fine day.");
    assert!(!rt.state.preview.is_active());
}

#[test]
fn quick_actions_send_no_comments_and_history_goes_with_the_next_request() {
    let (mut rt, _dir) = harness(DRAFT);
    add(&mut rt, 4..7, "Dog?");
    let prompt = replying(&mut rt, "<feedback>Fine as it is.</feedback>");
    chord(&mut rt, 'j', false, false);
    click(&mut rt, "assist-critique");
    assert_eq!(rt.state.assist.status, "Codex is reading the draft…");
    wait(&mut rt, |s| !s.assist.running);
    assert!(!prompt.lock().unwrap().contains("<comment id="), "quick actions send no comments");
    assert_eq!(rt.state.assist.feedback, "Fine as it is.");
    assert_eq!(rt.state.assist.status, "");
    assert!(rt.state.notes.versions().is_empty());
    assert_eq!(rt.state.notes.transcript()[0].action, "Critique");
    // A custom request sends the open comments and the history.
    type_text(&mut rt, "Anything else?");
    press(&mut rt, Key::Enter, Mods::default());
    wait(&mut rt, |s| !s.assist.running);
    let sent = prompt.lock().unwrap().clone();
    assert!(sent.contains("<comment id="), "custom requests send open comments");
    assert!(sent.contains("- Author asked: Critique\n  You replied: Fine as it is."), "{sent}");
}

#[test]
fn a_reply_without_notes_or_edits_says_no_feedback_and_unplaced_edits_are_listed() {
    let (mut rt, _dir) = harness(DRAFT);
    replying(&mut rt, "<feedback></feedback>");
    chord(&mut rt, 'j', false, false);
    click(&mut rt, "assist-clarify");
    wait(&mut rt, |s| !s.assist.running);
    assert_eq!(rt.state.assist.feedback, "No feedback.");
    replying(&mut rt, "<feedback>Two tries.</feedback><edit><old>No such words</old><new>Gone</new></edit>");
    click(&mut rt, "assist-shorten");
    wait(&mut rt, |s| !s.assist.running);
    assert_eq!(
        rt.state.assist.feedback,
        "Two tries.\n\n1 edit couldn't be placed:\n• Couldn't find the text this edit changes. It may have been edited since.\n  “Gone”"
    );
}

#[test]
fn edits_land_on_the_text_as_it_is_when_the_reply_arrives() {
    let (mut rt, _dir) = harness(DRAFT);
    replying(&mut rt, "<feedback>ok ok ok ok ok ok ok ok</feedback><edit><old>fine day</old><new>grey day</new></edit>");
    chord(&mut rt, 'j', false, false);
    type_text(&mut rt, "Darker");
    press(&mut rt, Key::Enter, Mods::default());
    // Type in the draft while waiting.
    rt.update(|s, tx| {
        s.edit.set_caret(0);
        s.edit.focused = true;
        tx.focus(crate::app::DRAFT, false);
    });
    settle(&mut rt, 0.05);
    type_text(&mut rt, "Yes. ");
    assert_eq!(rt.state.assist.scope, "Full draft · 11 words → Codex", "the scope holds still while a request runs");
    wait(&mut rt, |s| !s.assist.running);
    assert_eq!(rt.state.notes.versions()[0].body, "Yes. The cat sat on the mat. It was a grey day.");
    assert_eq!(rt.state.assist.scope, "Version “Darker” · 12 words → Codex", "the finished reply is previewed");
}

#[test]
fn requests_while_previewing_build_on_the_version() {
    let (mut rt, _dir) = harness(DRAFT);
    let v = writer_core::DraftVersion::new(writer_core::VersionKind::Proposal, "Short", "Codex", "A cat sat. Fine day.");
    let id = v.id.clone();
    rt.update(|s, tx| {
        s.add_version(v);
        s.preview_version(&id, tx);
    });
    let prompt = replying(&mut rt, "<feedback>x</feedback><edit><old>Fine day.</old><new>Grey day.</new></edit>");
    chord(&mut rt, 'j', false, false);
    rt.update(|s, _| s.edit.set_selection(0, 3));
    type_text(&mut rt, "Greyer");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.assist.status, "Codex is reading the version…");
    wait(&mut rt, |s| !s.assist.running);
    let sent = prompt.lock().unwrap().clone();
    assert!(sent.contains("<draft>\nA cat sat. Fine day.\n</draft>"));
    assert!(!sent.contains("<selection>"), "no selection while previewing");
    let newest = &rt.state.notes.versions()[0];
    assert_eq!(newest.label, "Greyer (from “Short”)");
    assert_eq!(newest.body, "A cat sat. Grey day.");
}

#[test]
fn stop_and_escape_cancel_and_nothing_changes() {
    let (mut rt, _dir) = harness(DRAFT);
    let long = format!("<feedback>{}</feedback><edit><old>cat</old><new>dog</new></edit>", "word ".repeat(60));
    replying(&mut rt, &long);
    chord(&mut rt, 'j', false, false);
    type_text(&mut rt, "Go");
    press(&mut rt, Key::Enter, Mods::default());
    wait(&mut rt, |s| !s.assist.feedback.is_empty());
    assert!(rt.state.assist.running);
    assert!(rt.find("assist-send").is_some());
    // The provider menu and quick actions are off while it runs.
    click(&mut rt, crate::assist_view::PROVIDER_KEY);
    assert!(!rt.state.assist.provider_menu_open);
    press(&mut rt, Key::Escape, Mods::default());
    assert!(!rt.state.assist.running);
    assert!(rt.state.assist_visible, "Esc while running stops but leaves the panel open");
    assert_eq!(rt.state.assist.status, "Stopped. Nothing was changed.");
    std::thread::sleep(Duration::from_millis(300));
    settle(&mut rt, 0.2);
    assert!(rt.state.notes.versions().is_empty(), "a stopped reply is ignored");
    assert!(rt.state.notes.transcript().is_empty());
    // The Stop button does the same.
    press(&mut rt, Key::Enter, Mods::default());
    assert!(rt.state.assist.running);
    click(&mut rt, "assist-send");
    assert_eq!(rt.state.assist.status, "Stopped. Nothing was changed.");
    // So does opening another draft, which also clears the result.
    press(&mut rt, Key::Enter, Mods::default());
    assert!(rt.state.assist.running);
    rt.update(|s, _| {
        let doc = s.library.new_draft();
        s.load(doc);
    });
    assert!(!rt.state.assist.running);
    assert_eq!(rt.state.assist.status, "");
    // And closing the panel.
    rt.update(|s, _| s.edit.replace_text("Some words."));
    chord(&mut rt, 'j', false, false);
    press(&mut rt, Key::Enter, Mods::default());
    assert!(rt.state.assist.running);
    click(&mut rt, "assist-close");
    assert!(!rt.state.assist.running);
    assert!(!rt.state.assist_visible);
}

#[test]
fn a_failure_shows_the_message_as_a_warning() {
    let (mut rt, _dir) = harness(DRAFT);
    let message = "Codex isn't installed. Install it with `npm install -g @openai/codex`, then sign in with `codex login`.";
    let m = message.to_string();
    rt.state.assist.session_maker = Some(Box::new(move |_| Box::new(Failing(m.clone(), false))));
    chord(&mut rt, 'j', false, false);
    type_text(&mut rt, "Hello");
    press(&mut rt, Key::Enter, Mods::default());
    wait(&mut rt, |s| !s.assist.running);
    assert_eq!(rt.state.assist.status, message);
    assert!(rt.state.assist.status_warning);
    assert_eq!(rt.state.assist.feedback, "");
    assert!(rt.state.notes.transcript().is_empty());
}

#[test]
fn blank_instructions_and_drafts_send_nothing() {
    let (mut rt, _dir) = harness("   \n");
    replying(&mut rt, REPLY);
    chord(&mut rt, 'm', true, false);
    rt.update(|s, tx| s.assist_show(None, Some(AssistTab::Ask), tx));
    type_text(&mut rt, "Go");
    press(&mut rt, Key::Enter, Mods::default());
    assert!(!rt.state.assist.running, "a blank draft isn't sent");
    rt.update(|s, _| s.edit.replace_text("Words here."));
    rt.update(|s, _| s.assist.instruction.replace_text("  "));
    press(&mut rt, Key::Enter, Mods::default());
    assert!(!rt.state.assist.running, "a blank instruction isn't sent");
}

// MARK: Comments

#[test]
fn send_comments_resolves_and_answers_them() {
    let (mut rt, _dir) = harness(DRAFT);
    let a = add(&mut rt, 4..7, "Make it a dog");
    let b = add(&mut rt, 31..35, "Why fine?");
    chord(&mut rt, 'm', true, false);
    let label = rt.state.assist_can_send_comments();
    assert!(label);
    let reply = format!("<feedback>Done.</feedback><edit for=\"{a}\"><old>cat</old><new>dog</new></edit><reply for=\"{b}\">Because it was sunny.</reply>");
    let prompt = replying(&mut rt, &reply);
    click(&mut rt, "assist-send-comments");
    assert_eq!(rt.state.notes.assist_tab, AssistTab::Ask, "sending switches to Ask");
    // The reply streams from another thread, so by now it may already be drafting.
    let status = rt.state.assist.status.clone();
    assert!(status == "Codex is reading 2 comments…" || status == "Drafting edits…", "status {status:?}");
    wait(&mut rt, |s| !s.assist.running);
    assert!(prompt.lock().unwrap().contains("The author's comments. Each one is attached to the quoted text:"));
    let v = &rt.state.notes.versions()[0];
    assert_eq!(v.label, "Resolve 2 comments");
    assert_eq!(v.resolves_comment_ids, vec![a.clone()]);
    let answered = rt.state.notes.comment(&b).unwrap();
    assert_eq!((answered.replies[0].author.as_str(), answered.replies[0].text.as_str()), ("Codex", "Because it was sunny."));
    assert_eq!(rt.state.assist.feedback, "Done.\n\nAnswered 1 comment — see Comments.");
    // Using the version resolves the comment it addressed.
    rt.update(|s, tx| s.preview_use(tx));
    settle(&mut rt, 0.1);
    assert!(!rt.state.notes.comment(&a).unwrap().is_open());
    assert!(rt.state.notes.comment(&b).unwrap().is_open());
}

#[test]
fn replies_alone_switch_to_the_comments_tab() {
    let (mut rt, _dir) = harness(DRAFT);
    let b = add(&mut rt, 31..35, "Why fine?");
    replying(&mut rt, &format!("<reply for=\"{b}\">Sunny.</reply>"));
    chord(&mut rt, 'm', true, false);
    click(&mut rt, "assist-send-comments");
    wait(&mut rt, |s| !s.assist.running);
    assert_eq!(rt.state.notes.assist_tab, AssistTab::Comments);
    assert_eq!(rt.state.assist.feedback, "Answered 1 comment — see Comments.");
    assert!(rt.find(&format!("comment-reply-{b}-0")).is_some(), "the reply shows on the card");
}

#[test]
fn comment_cards_reveal_resolve_reopen_and_delete() {
    let (mut rt, _dir) = harness(DRAFT);
    chord(&mut rt, 'm', true, false);
    assert!(rt.find("assist-comments-empty").is_some());
    assert_eq!(rt.find("assist-send-comments").map(|_| rt.state.assist_can_send_comments()), Some(false));
    let a = add(&mut rt, 4..7, "Dog?");
    settle(&mut rt, 0.05);
    assert!(rt.find("assist-comments-empty").is_none());
    assert!(rt.find("assist-show-resolved").is_none(), "hidden with nothing resolved");
    // Clicking the card selects its text and makes it active.
    rt.update(|s, _| s.notes.set_active_comment(None));
    click(&mut rt, &format!("comment-card-{a}"));
    assert_eq!(rt.state.notes.active_comment_id(), Some(a.as_str()));
    assert_eq!(rt.state.edit.range(), (4, 7));
    // Resolve hides it; Show resolved brings it back, dimmed; Reopen opens it.
    click(&mut rt, &format!("comment-resolve-{a}"));
    assert!(!rt.state.notes.comment(&a).unwrap().is_open());
    assert!(rt.find(&format!("comment-card-{a}")).is_none());
    click(&mut rt, "assist-show-resolved");
    assert!(rt.find(&format!("comment-card-{a}")).is_some());
    click(&mut rt, &format!("comment-resolve-{a}"));
    assert!(rt.state.notes.comment(&a).unwrap().is_open());
    // Edit opens the composer on it.
    click(&mut rt, &format!("comment-edit-{a}"));
    settle(&mut rt, 0.1);
    assert!(rt.state.composer.open.is_some());
    rt.update(|s, tx| s.composer_cancel(tx));
    click(&mut rt, &format!("comment-delete-{a}"));
    assert!(rt.state.notes.comment(&a).is_none());
    // The tab and the button count open comments.
    add(&mut rt, 4..7, "x");
    settle(&mut rt, 0.05);
    assert!(rt.find("assist-tab-comments").is_some());
}

// MARK: Versions

#[test]
fn version_cards_preview_go_back_and_delete() {
    let (mut rt, _dir) = harness(DRAFT);
    let v = writer_core::DraftVersion::new(writer_core::VersionKind::Proposal, "Short", "Codex", "A cat sat.");
    let id = v.id.clone();
    rt.update(|s, _| s.add_version(v));
    chord(&mut rt, 'v', false, true);
    assert_eq!(rt.state.notes.assist_tab, AssistTab::Versions);
    click(&mut rt, &format!("version-card-{id}"));
    assert_eq!(rt.state.preview.version_id(), Some(id.as_str()));
    click(&mut rt, "version-card-draft");
    assert!(!rt.state.preview.is_active());
    click(&mut rt, &format!("version-card-{id}"));
    click(&mut rt, &format!("version-delete-{id}"));
    assert!(rt.state.notes.versions().is_empty());
    assert!(!rt.state.preview.is_active(), "deleting the previewed version closes the preview");
}

#[test]
fn the_provider_menu_switches_providers() {
    let (mut rt, _dir) = harness(DRAFT);
    chord(&mut rt, 'j', false, false);
    click(&mut rt, crate::assist_view::PROVIDER_KEY);
    if rune::popup_menu_supported() {
        // macOS and Windows: the system's pop-up menu.
        assert!(rt.popup_menu().is_some_and(|m| m.find("claude").is_some()));
        rt.resolve_popup(Some("claude")).unwrap();
        settle(&mut rt, 0.1);
    } else {
        // Linux: the menu the app draws itself.
        assert!(rt.state.assist.provider_menu_open);
        click(&mut rt, "assist-provider-claude");
    }
    assert_eq!(rt.state.assist.provider, AgentProvider::Claude);
    assert!(!rt.state.assist.provider_menu_open);
    assert_eq!(rt.state.assist.scope, "Full draft · 11 words → Claude");
}

#[test]
fn the_instruction_field_takes_a_line_break_with_option_return() {
    let (mut rt, _dir) = harness(DRAFT);
    chord(&mut rt, 'j', false, false);
    type_text(&mut rt, "one");
    rt.key(KeyEvent { key: Key::Enter, mods: Mods { alt: true, ..Mods::default() } });
    settle(&mut rt, 0.05);
    type_text(&mut rt, "two");
    assert_eq!(rt.state.assist.instruction.text, "one\ntwo");
    assert!(!rt.state.assist.running);
    let _ = INSTRUCTION;
}

// MARK: Connect sheet

#[test]
fn the_connect_sheet_shows_the_commands_and_copies_them() {
    let (mut rt, dir) = harness(DRAFT);
    rt.update(|s, tx| s.run(crate::commands::Command::ConnectOutsideAgent, tx));
    settle(&mut rt, 0.05);
    // Test binaries have no fluent-writer-mcp beside them.
    assert!(rt.state.assist.connect.sheet.is_none());
    let alert = rt.state.alert().expect("an alert says the connector is missing");
    assert_eq!(alert.message, "The agent connector is missing from this build.");
    assert_eq!(alert.informative, "Build the app with script/build-app.sh, which bundles fluent-writer-mcp.");
    click(&mut rt, "alert-button-0");
    assert!(rt.state.alert().is_none());
    // With a server, the commands.
    let server = dir.path().join("fluent-writer-mcp");
    let support = rt.state.library.support_directory().to_path_buf();
    let claude = crate::connect_agent::claude_command(false, &server, &support);
    let codex = crate::connect_agent::codex_command(false, &server, &support);
    rt.update(|s, _| s.assist.connect.sheet = Some(crate::connect_agent::Sheet::Commands { claude: claude.clone(), codex: codex.clone(), server, support }));
    settle(&mut rt, 0.05);
    for key in ["connect-claude", "connect-codex", "connect-copy", "connect-done", "connect-claude-command", "connect-codex-command"] {
        assert!(rt.find(key).is_some(), "{key} drawn");
    }
    click(&mut rt, "connect-copy");
    assert!(rt.state.assist.connect.sheet.is_none());
    assert_eq!(rt.state.status().0, "Copied the connect commands");
}

#[test]
fn registering_with_a_missing_cli_says_so() {
    let (mut rt, _dir) = harness(DRAFT);
    rt.update(|s, _| s.connect_register_for_test("fluent-writer-no-such-cli", "No Such CLI"));
    wait(&mut rt, |s| s.assist.connect.running == 0);
    assert_eq!(rt.state.status().0, "No Such CLI isn't installed");
}

// MARK: Toolbar

#[test]
fn toolbar_buttons_toggle_their_tab_and_show_state() {
    use crate::chrome::Button;
    let (mut rt, _dir) = harness(DRAFT);
    add(&mut rt, 4..7, "Dog?");
    assert_eq!(rt.state.chrome_button_count(Button::Comments), 1);
    rt.update(|s, tx| s.press_chrome_button(Button::Comments, tx));
    assert!(rt.state.assist_visible);
    assert!(rt.state.chrome_button_active(Button::Comments));
    assert!(!rt.state.chrome_button_active(Button::Assist));
    // Another tab's button switches tabs; the open tab's button closes the panel.
    rt.update(|s, tx| s.press_chrome_button(Button::Assist, tx));
    assert!(rt.state.assist_visible);
    assert!(rt.state.chrome_button_active(Button::Assist));
    rt.update(|s, tx| s.press_chrome_button(Button::Assist, tx));
    assert!(!rt.state.assist_visible);
    // Versions is active while a preview shows, panel or not.
    let v = writer_core::DraftVersion::new(writer_core::VersionKind::Proposal, "Short", "Codex", "A cat sat.");
    let id = v.id.clone();
    rt.update(|s, tx| {
        s.add_version(v);
        s.preview_version(&id, tx);
    });
    assert!(rt.state.chrome_button_active(Button::Versions));
    // The Assist button opens the panel even on an empty draft, unlike Show Assist.
    let (mut rt, _dir) = harness("");
    rt.update(|s, tx| s.press_chrome_button(Button::Assist, tx));
    assert!(rt.state.assist_visible);
}
