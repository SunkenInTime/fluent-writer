//! The Assist panel's state and what it does (Swift `AssistController.swift`): opening and
//! closing, the provider, sending a request to Codex or Claude, following the reply as it
//! streams in, and turning a finished reply into comment replies, a new version and an entry in
//! the draft's Assist history. Nothing leaves the machine until Send is pressed, and an agent's
//! edits only reach the draft when the version they produce is used.
//!
//! The panel is drawn by `assist_view.rs` (header, tabs, the Ask tab) and `assist_lists.rs` (the
//! Comments and Versions tabs).

use std::ops::Range;
use std::time::Duration;

use rune::{TextEdit, Tx, Waker};
use writer_agents::{AgentAction, AgentEvent, AgentHandle, AgentPrompt, AgentProvider, AgentReply, AgentRequest, HistoryEntry, PromptComment};
use writer_core::{AssistExchange, DraftVersion, VersionKind, edit_placement, text_stats};

use crate::app::{App, DRAFT, SIDEBAR_WIDTH};
use crate::notes::AssistTab;

/// The key of the instruction field.
pub const INSTRUCTION: &str = "assist-instruction";

/// What the in-flight request was about, kept to place its edits when the reply arrives.
/// `AssistController.swift:146-152`
#[derive(Clone, Debug)]
struct Sent {
    action: AgentAction,
    instruction: String,
    /// The selection sent as the passage to focus on, as it was when sent.
    selection: Option<Range<usize>>,
    based_on_version_id: Option<String>,
    comment_ids: Vec<String>,
}

/// Makes the session for a request. Tests replace it to replay canned replies.
pub type SessionMaker = Box<dyn Fn(AgentProvider) -> Box<dyn writer_agents::AgentSession>>;

pub struct Assist {
    pub provider: AgentProvider,
    /// The instruction field.
    pub instruction: TextEdit,
    pub running: bool,
    handle: Option<AgentHandle>,
    sent: Option<Sent>,
    /// The line under the instruction field, and whether it is an error (drawn in `warning`).
    pub status: String,
    pub status_warning: bool,
    /// The reply's notes as they stream in, then what the finished reply says.
    pub feedback: String,
    /// The version the last reply produced, for the Preview Version button.
    pub last_version_id: Option<String>,
    /// The Comments tab lists resolved comments too.
    pub show_resolved: bool,
    /// The scope line and what screen readers hear for it.
    pub scope: String,
    pub scope_label: String,
    /// The provider menu drawn in the app, where the platform has no pop-up menus.
    pub provider_menu_open: bool,
    /// The selection and text the scope line last followed.
    seen: ((usize, usize), u64),
    waker: Option<Waker>,
    pub session_maker: Option<SessionMaker>,
    pub connect: crate::connect_agent::Connect,
}

impl Assist {
    pub fn new(provider: AgentProvider) -> Assist {
        Assist {
            provider,
            instruction: crate::editing::text_edit(""),
            running: false,
            handle: None,
            sent: None,
            status: String::new(),
            status_warning: false,
            feedback: String::new(),
            last_version_id: None,
            show_resolved: false,
            scope: String::new(),
            scope_label: String::new(),
            provider_menu_open: false,
            seen: ((usize::MAX, usize::MAX), u64::MAX),
            waker: None,
            session_maker: None,
            connect: Default::default(),
        }
    }

    /// Lets a streaming reply wake the window.
    pub fn set_waker(&mut self, waker: Waker) {
        self.connect.set_waker(waker.clone());
        self.waker = Some(waker);
    }

    /// Clears the status and feedback. `AssistController.swift:833-837`
    fn clear_result(&mut self) {
        self.feedback.clear();
        self.status.clear();
        self.status_warning = false;
    }
}

/// "1 word", "2 words": `n` and the noun, singular for exactly one.
pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A version's label from an instruction: one line, at most 48 characters (47 and "…").
/// `AssistController.swift:828-831`
pub fn short_label(instruction: &str) -> String {
    let one = instruction.replace('\n', " ");
    let one = one.trim_matches(|c: char| c == ' ' || c == '\t');
    if one.chars().count() > 48 { format!("{}…", one.chars().take(47).collect::<String>()) } else { one.to_string() }
}

/// Moves `i` down to a character boundary of `text`, within it.
fn floor_boundary(text: &str, i: usize) -> usize {
    let mut i = i.min(text.len());
    while !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

impl App {
    // MARK: Opening and closing

    /// Opens the panel, with `provider` selected and on `tab` when given: a provider alone opens
    /// on Ask, nothing keeps the last tab. Focuses the instruction field on Ask.
    /// `AssistController.swift:377-391`
    pub fn assist_show(&mut self, provider: Option<AgentProvider>, tab: Option<AssistTab>, tx: &mut Tx) {
        if let Some(p) = provider {
            self.assist_set_provider(p);
        }
        if !self.assist_visible {
            self.make_room_for_panel(tx);
            self.assist_visible = true;
        }
        let tab = tab.unwrap_or(if provider.is_some() { AssistTab::Ask } else { self.notes.assist_tab });
        self.assist_select(tab);
        self.assist_refresh_scope();
        if tab == AssistTab::Ask {
            self.assist.instruction.focused = true;
            tx.focus(INSTRUCTION, false);
        }
    }

    /// Opening the panel in a narrow window widens it so the draft keeps a usable column: to
    /// 720 pt, plus the sidebar when it is pinned; not in full screen. Rune keeps the window on
    /// its screen and skips the animation under Reduce Motion. `ChromeControls.swift:147-157`
    fn make_room_for_panel(&mut self, tx: &mut Tx) {
        if let Some(width) = self.panel_room_needed() {
            tx.resize_window((width, self.window.1));
        }
    }

    /// The width opening the panel asks the window for, if it asks.
    pub fn panel_room_needed(&self) -> Option<f32> {
        let needed = 720.0 + if self.sidebar_visible { SIDEBAR_WIDTH } else { 0.0 };
        (!self.win.full_screen && self.window.0 < needed).then_some(needed)
    }

    /// Stops any request, hides the panel and gives the keyboard to the version preview or the
    /// draft. `AssistController.swift:393-400`
    pub fn assist_close(&mut self, tx: &mut Tx) {
        self.assist_stop();
        self.assist_visible = false;
        self.assist.provider_menu_open = false;
        if self.preview.is_active() {
            self.preview.edit.focused = true;
            tx.focus(crate::version_preview::PREVIEW, false);
        } else {
            self.edit.focused = true;
            tx.focus(DRAFT, false);
        }
    }

    /// ⌥⌘J: closes the panel when open, otherwise opens it on its last tab.
    /// `AssistController.swift:866-868`
    pub fn assist_toggle(&mut self, tx: &mut Tx) {
        if self.assist_visible { self.assist_close(tx) } else { self.assist_show(None, None, tx) }
    }

    /// A toolbar button: closes the panel when it is open on `tab`, otherwise opens it there.
    /// `ChromeControls.swift:159-162`
    pub fn assist_toggle_tab(&mut self, tab: AssistTab, tx: &mut Tx) {
        if self.assist_visible && self.notes.assist_tab == tab { self.assist_close(tx) } else { self.assist_show(None, Some(tab), tx) }
    }

    /// Switches tabs. `AssistController.swift:414-427`
    pub fn assist_select(&mut self, tab: AssistTab) {
        if self.notes.assist_tab != tab {
            self.notes.assist_tab = tab;
        }
    }

    pub fn assist_set_provider(&mut self, p: AgentProvider) {
        self.assist.provider = p;
        self.prefs.set_assist_provider(p.raw_value());
        self.assist.provider_menu_open = false;
        self.assist_refresh_scope();
    }

    // MARK: Following the draft

    /// What the scope line says: the previewed version, the selection, or the whole draft.
    /// `AssistController.swift:446-468`
    pub fn assist_refresh_scope(&mut self) {
        let name = self.assist.provider.display_name();
        let total = if self.preview.version_id().is_none() && !self.edit.text.is_empty() { self.draft_total_counts().words } else { 0 };
        let a = &mut self.assist;
        if let Some(v) = self.preview.version_id().and_then(|id| self.notes.version(id)) {
            let words = text_stats::counts_for_markdown(&v.body).words;
            a.scope = format!("Version “{}” · {words} words → {name}", v.label);
            a.scope_label = format!("Will send the previewed version, {words} words, to {name}");
            return;
        }
        let text = &self.edit.text;
        if text.is_empty() {
            // The accessibility label keeps the sentence before, as the Swift app's did.
            a.scope = "Write something first".into();
            return;
        }
        let (x, y) = self.edit.range();
        let sel = x.min(y)..x.max(y).min(text.len());
        let selected = if sel.is_empty() { 0 } else { text.get(sel).map_or(0, |s| text_stats::counts_for_markdown(s).words) };
        if selected > 0 {
            a.scope = format!("Draft · focus on {} → {name}", plural(selected, "selected word", "selected words"));
            a.scope_label = format!("Will send the draft, {total} words, focused on {selected} selected words, to {name}");
        } else {
            a.scope = format!("Full draft · {} → {name}", plural(total, "word", "words"));
            a.scope_label = format!("Will send the full draft, {total} words, to {name}");
        }
    }

    /// Another draft was opened: stops the request and clears the result.
    /// `AssistController.swift:431-436`
    pub fn assist_document_changed(&mut self) {
        self.assist_stop();
        self.assist.clear_result();
        self.assist_refresh_scope();
    }

    // MARK: Sending

    /// Return in the instruction field, and Send: sends the instruction unless it is blank.
    /// `AssistController.swift:684-687`
    pub fn assist_send_instruction(&mut self, tx: &mut Tx) {
        if self.assist.instruction.text.trim().is_empty() {
            return;
        }
        self.assist_send(AgentAction::Custom, tx);
    }

    /// The Send button, which reads Stop while a request runs. `AssistController.swift:689-691`
    pub fn assist_send_or_stop(&mut self, tx: &mut Tx) {
        if self.assist.running { self.assist_stop() } else { self.assist_send_instruction(tx) }
    }

    /// Sends the draft (or the previewed version) with `action`. `AssistController.swift:705-747`
    pub fn assist_send(&mut self, action: AgentAction, tx: &mut Tx) {
        let based_on = self.previewed_version().cloned();
        let base = based_on.as_ref().map_or_else(|| self.edit.text.clone(), |v| v.body.clone());
        if base.trim().is_empty() {
            return;
        }
        self.assist_stop();
        self.assist.clear_result();
        self.notes.flush_save();
        let text = &self.edit.text;
        let (x, y) = self.edit.range();
        let sel = x.min(y)..x.max(y);
        let selection = (based_on.is_none() && !sel.is_empty() && sel.end <= text.len() && !text[sel.clone()].trim().is_empty()).then_some(sel);
        let comments: Vec<PromptComment> = match action {
            AgentAction::ResolveComments | AgentAction::Custom => self
                .notes
                .attached_open_comments()
                .iter()
                .map(|c| PromptComment {
                    id: c.id.clone(),
                    quote: c.quote.clone(),
                    body: c.body.clone(),
                    replies: c.replies.iter().map(|r| format!("{}: {}", r.author, r.text)).collect(),
                })
                .collect(),
            _ => Vec::new(),
        };
        if action == AgentAction::ResolveComments && comments.is_empty() {
            return;
        }
        let instruction = if action == AgentAction::Custom { self.assist.instruction.text.clone() } else { String::new() };
        let transcript = self.notes.transcript();
        let history = transcript[transcript.len().saturating_sub(4)..]
            .iter()
            .map(|e| HistoryEntry::new(if e.instruction.is_empty() { e.action.clone() } else { e.instruction.clone() }, e.feedback.clone()))
            .collect();
        let provider = self.assist.provider;
        let request = AgentRequest {
            provider,
            action,
            instruction: instruction.clone(),
            document: base,
            selection: selection.clone().map(|r| text[r].to_string()),
            title: self.draft.title.clone(),
            comments: comments.clone(),
            history,
        };
        let count = comments.len();
        self.assist.sent = Some(Sent {
            action,
            instruction,
            selection,
            based_on_version_id: based_on.as_ref().map(|v| v.id.clone()),
            comment_ids: comments.into_iter().map(|c| c.id).collect(),
        });
        self.assist.running = true;
        if action == AgentAction::ResolveComments {
            self.assist_select(AssistTab::Ask);
        }
        let what = if action == AgentAction::ResolveComments {
            plural(count, "comment", "comments")
        } else if based_on.is_some() {
            "the version".into()
        } else {
            "the draft".into()
        };
        self.assist.status = format!("{} is reading {what}…", provider.display_name());
        self.assist.status_warning = false;
        let waker = self.assist.waker.clone();
        let wake = move || {
            if let Some(w) = &waker {
                w();
            }
        };
        let handle = match &self.assist.session_maker {
            Some(make) => AgentHandle::start(make(provider), AgentPrompt::build(&request), wake),
            None => AgentHandle::start_request(&request, wake),
        };
        self.assist.handle = Some(handle);
        tx.wake_after(Duration::ZERO);
    }

    /// Stops a running request: "Stopped. Nothing was changed." `AssistController.swift:402-412`
    pub fn assist_stop(&mut self) {
        let a = &mut self.assist;
        if !a.running {
            return;
        }
        if let Some(h) = a.handle.take() {
            h.stop();
        }
        a.running = false;
        a.sent = None;
        a.status = "Stopped. Nothing was changed.".into();
        a.status_warning = false;
        self.assist_refresh_scope();
    }

    // MARK: Replies

    /// Runs before every frame: takes the reply's events, follows the selection for the scope
    /// line, and hears back from the connect sheet. Returns whether anything shown changed.
    pub fn assist_poll(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        while let Some(event) = self.assist.handle.as_ref().and_then(|h| h.try_recv()) {
            changed = true;
            self.assist_handle(event, tx);
        }
        // `AssistController.swift:438-439`: the scope follows the selection unless a request runs.
        let seen = (self.edit.range(), self.notes.text_revision());
        if seen != self.assist.seen {
            self.assist.seen = seen;
            if !self.assist.running {
                self.assist_refresh_scope();
                changed = true;
            }
        }
        changed |= self.connect_poll(tx);
        changed
    }

    /// `AssistController.swift:749-768`
    fn assist_handle(&mut self, event: AgentEvent, tx: &mut Tx) {
        match event {
            AgentEvent::Progress(text) => {
                self.assist.feedback = AgentPrompt::parse(&text).feedback;
                if AgentPrompt::is_drafting_edits(&text) {
                    self.assist.status = "Drafting edits…".into();
                }
            }
            AgentEvent::Completed(text) => {
                self.assist.running = false;
                self.assist.handle = None;
                self.assist_finish(AgentPrompt::parse(&text), tx);
            }
            AgentEvent::Failed(message) => {
                let a = &mut self.assist;
                a.running = false;
                a.handle = None;
                a.sent = None;
                a.feedback.clear();
                a.status = message;
                a.status_warning = true;
            }
        }
        if !self.assist.running {
            self.assist_refresh_scope();
        }
    }

    /// Turns a finished reply into comment replies, a version and an Assist history entry.
    /// `AssistController.swift:770-826`
    fn assist_finish(&mut self, reply: AgentReply, tx: &mut Tx) {
        let Some(sent) = self.assist.sent.take() else { return };
        let name = self.assist.provider.display_name();
        let mut answered: Vec<(&String, &String)> = reply.replies.iter().collect();
        answered.sort();
        for (id, answer) in answered {
            if self.notes.comment(id).is_some() {
                self.reply_to_comment(id, name, answer);
            }
        }
        // Place edits on the text as it is now, so anything typed while waiting is kept.
        let based_on = sent.based_on_version_id.as_deref().and_then(|id| self.notes.version(id)).cloned();
        let base = based_on.as_ref().map_or_else(|| self.edit.text.clone(), |v| v.body.clone());
        let focus = if based_on.is_none() {
            sent.selection.clone().map(|r| floor_boundary(&base, r.start)..floor_boundary(&base, r.end))
        } else {
            None
        };
        let notes = &self.notes;
        let comment_range = |edit: &writer_core::ProposedEdit| -> Option<Range<usize>> {
            if based_on.is_some() {
                return None;
            }
            let r = notes.comment(edit.comment_ids.first()?)?.range.clone();
            Some(floor_boundary(&base, r.start)..floor_boundary(&base, r.end))
        };
        let placement = edit_placement::place(&reply.edits, &base, focus, Some(&comment_range));
        let mut version_id = None;
        if !placement.placed.is_empty() {
            let body = placement.applied(&base);
            if body != base {
                let mut resolves: Vec<String> = Vec::new();
                for p in &placement.placed {
                    for id in &reply.edits[p.index].comment_ids {
                        if !resolves.contains(id) {
                            resolves.push(id.clone());
                        }
                    }
                }
                let label = match sent.action {
                    AgentAction::Custom => short_label(&sent.instruction),
                    AgentAction::ResolveComments => format!("Resolve {}", plural(sent.comment_ids.len(), "comment", "comments")),
                    other => other.label().to_string(),
                };
                let label = match &based_on {
                    Some(v) => format!("{label} (from “{}”)", v.label),
                    None => label,
                };
                let mut v = DraftVersion::new(VersionKind::Proposal, label, name, body);
                v.feedback = (!reply.feedback.is_empty()).then(|| reply.feedback.clone());
                v.resolves_comment_ids = resolves;
                version_id = Some(v.id.clone());
                self.assist.last_version_id = Some(v.id.clone());
                self.add_version(v);
            }
        }
        let mut shown = reply.feedback.clone();
        if shown.is_empty() && version_id.is_none() && reply.replies.is_empty() {
            shown = "No feedback.".into();
        }
        if !reply.replies.is_empty() {
            if !shown.is_empty() {
                shown.push_str("\n\n");
            }
            shown.push_str(&format!("Answered {} — see Comments.", plural(reply.replies.len(), "comment", "comments")));
        }
        if !placement.failed.is_empty() {
            shown.push_str(&format!("\n\n{} couldn't be placed:", plural(placement.failed.len(), "edit", "edits")));
            for f in &placement.failed {
                shown.push_str(&format!("\n• {}\n  “{}”", f.reason, f.edit.new.chars().take(300).collect::<String>()));
            }
        }
        self.assist.feedback = shown;
        self.add_exchange(AssistExchange {
            date: std::time::SystemTime::now(),
            provider: name.to_string(),
            action: sent.action.label().to_string(),
            instruction: sent.instruction.clone(),
            feedback: reply.feedback.clone(),
            version_id: version_id.clone(),
            comment_ids: sent.comment_ids.clone(),
        });
        if let Some(id) = &version_id {
            let n = placement.placed.len();
            self.assist.status = format!(
                "{} ready as a new version, shown in the editor. ← → switch versions · ↩ use it · esc back to your draft.",
                plural(n, "edit", "edits")
            );
            self.assist.status_warning = false;
            self.preview_version(id, tx);
        } else {
            self.assist.status.clear();
        }
        if sent.action == AgentAction::ResolveComments && !reply.replies.is_empty() && version_id.is_none() {
            self.assist_select(AssistTab::Comments);
        }
    }

    // MARK: Lists

    /// The Preview Version button shows while the last reply's version still exists.
    pub fn assist_preview_target(&self) -> Option<&str> {
        self.assist.last_version_id.as_deref().filter(|id| self.notes.version(id).is_some())
    }

    /// Deletes a version from its card. `AssistController.swift:640-646`
    pub fn assist_delete_version(&mut self, id: &str, tx: &mut Tx) {
        self.delete_version(id, tx);
        if self.assist.last_version_id.as_deref() == Some(id) {
            self.assist.last_version_id = None;
        }
    }

    /// "Send N Comments to …": enabled with attached open comments and no request running.
    /// `AssistController.swift:488-490`
    pub fn assist_can_send_comments(&self) -> bool {
        !self.notes.attached_open_comments().is_empty() && !self.assist.running
    }
}

#[cfg(test)]
#[path = "assist_tests.rs"]
mod app_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_one_line_and_cut_at_48_characters() {
        assert_eq!(short_label("  Make it\nshorter "), "Make it shorter");
        let long = "a".repeat(49);
        assert_eq!(short_label(&long), format!("{}…", "a".repeat(47)));
        assert_eq!(short_label(&"b".repeat(48)), "b".repeat(48));
    }

    #[test]
    fn plurals() {
        assert_eq!(plural(1, "word", "words"), "1 word");
        assert_eq!(plural(0, "word", "words"), "0 words");
    }
}
