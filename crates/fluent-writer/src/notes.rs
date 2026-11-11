//! Comments, versions and the Assist transcript for the open draft (Swift `NotesController`).
//! Also publishes what the app is showing for outside agents (`Live.json`) and applies the
//! commands they leave in the inbox.
//!
//! The storage and its rules live in `writer_core::document_notes`; this module keeps the open
//! draft's notes in step with the editor. Everything that touches disk after loading (saving,
//! the live state, reading and answering the inbox) runs on one worker thread, in order.

use std::ops::Range;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

use rune::{Span, Tx, Waker};
use writer_core::{
    AssistExchange, Comment, CommentReply, DocumentNotes, DraftVersion, InboxCommand, InboxCommandKind, InboxResult, LiveState, NotesStore,
    VersionKind, text_units,
};

use crate::app::App;
use crate::commands::Command;
use crate::theme::Palette;

/// Notes are saved this long after the last change. `NotesController.swift:84`
pub const SAVE_DELAY: f64 = 0.6;
/// The live state is written this long after the last selection, comment or preview change.
/// `NotesController.swift:239`
pub const LIVE_DELAY: f64 = 0.4;

/// Which tab of the Assist panel was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AssistTab {
    #[default]
    Ask,
    Comments,
    Versions,
}

/// The open draft's notes and what keeps them in step with the editor.
pub struct Notes {
    store: NotesStore,
    notes: DocumentNotes,
    /// The path `notes` is filed under; `None` while the draft has never been saved.
    path: Option<String>,
    /// The draft `notes` belong to. Path changes only carry notes along for this same draft.
    document_id: Option<String>,
    active_comment_id: Option<String>,
    /// The tab "Show Comments" or "Show Versions" asked for, until the Assist panel holds it.
    pub assist_tab: AssistTab,
    /// The draft text as comments last followed it.
    shadow: String,
    /// The range an edit made through `App::apply` replaced, for the next text change.
    expected: Option<Range<usize>>,
    /// The selection before the next edit, as of the last frame.
    pre_selection: (usize, usize),
    /// The selection the live state last saw.
    live_selection: (usize, usize),
    save_at: Option<f64>,
    live_at: Option<f64>,
    /// Goes up on every change to comments, versions or the transcript, so views can refresh.
    revision: u64,
    /// Goes up on every change to the draft text.
    text_revision: u64,
    /// The app's name for the live state ("Fluent Writer" or "Fluent Writer Dev").
    pub app_name: String,
    /// Times the app would have beeped (Rune has no system beep yet).
    pub beeps: usize,
    /// What screen readers should announce next (Rune has no announcement request yet).
    pub announcement: String,
    worker: Worker,
}

#[allow(dead_code, reason = "the Assist panel (next task) calls these")]
impl Notes {
    pub fn new(support_directory: &Path) -> Notes {
        let store = NotesStore::new(support_directory);
        let worker = Worker::start(store.clone());
        Notes {
            store,
            notes: DocumentNotes::new(""),
            path: None,
            document_id: None,
            active_comment_id: None,
            assist_tab: AssistTab::Ask,
            shadow: String::new(),
            expected: None,
            pre_selection: (0, 0),
            live_selection: (0, 0),
            save_at: None,
            live_at: None,
            revision: 0,
            text_revision: 0,
            app_name: writer_core::AppVariant::Standard.app_name().to_string(),
            beeps: 0,
            announcement: String::new(),
            worker,
        }
    }

    /// Lets the inbox watcher wake the window when an outside agent leaves a command.
    pub fn set_waker(&mut self, waker: Waker) {
        *self.worker.waker.lock().unwrap() = Some(waker);
    }

    // MARK: Reading

    pub fn notes(&self) -> &DocumentNotes {
        &self.notes
    }
    /// The path the notes are filed under, once the draft has a file.
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn text_revision(&self) -> u64 {
        self.text_revision
    }

    /// Open comments in text order, detached ones included.
    pub fn open_comments(&self) -> Vec<&Comment> {
        self.notes.comments.iter().filter(|c| c.is_open()).collect()
    }
    pub fn resolved_comments(&self) -> Vec<&Comment> {
        self.notes.comments.iter().filter(|c| !c.is_open()).collect()
    }
    /// The number the Comments badge and tab show: open comments, detached ones included.
    pub fn open_count(&self) -> usize {
        self.notes.comments.iter().filter(|c| c.is_open()).count()
    }
    /// Open comments still attached to the text: the ones Assist sends.
    pub fn attached_open_comments(&self) -> Vec<&Comment> {
        self.notes.comments.iter().filter(|c| c.is_open() && !c.detached).collect()
    }
    pub fn comment(&self, id: &str) -> Option<&Comment> {
        self.notes.comments.iter().find(|c| c.id == id)
    }
    /// Versions newest first.
    pub fn versions(&self) -> &[DraftVersion] {
        &self.notes.versions
    }
    pub fn version(&self, id: &str) -> Option<&DraftVersion> {
        self.notes.versions.iter().find(|v| v.id == id)
    }
    pub fn transcript(&self) -> &[AssistExchange] {
        &self.notes.transcript
    }
    pub fn active_comment_id(&self) -> Option<&str> {
        self.active_comment_id.as_deref()
    }

    /// The open comment covering `location` (a byte offset), preferring the shortest span.
    /// `NotesController.swift:157-160`
    pub fn comment_at(&self, location: usize) -> Option<&Comment> {
        self.notes
            .comments
            .iter()
            .filter(|c| c.is_open() && !c.detached && c.range.start <= location && location <= c.range.end)
            .min_by_key(|c| c.range.len())
    }

    /// Highlights for every open, attached comment: `commentHighlight` behind the text, and
    /// `commentHighlightActive` for the active one. `NotesController.swift:162-171`
    pub fn highlights(&self, len: usize, p: &Palette) -> Vec<Span> {
        self.notes
            .comments
            .iter()
            .filter(|c| c.is_open() && !c.detached)
            .filter_map(|c| {
                let r = c.range.start.min(len)..c.range.end.min(len);
                let active = self.active_comment_id.as_deref() == Some(c.id.as_str());
                (r.start < r.end).then(|| Span::new(r.start, r.end).bg(if active { p.comment_highlight_active } else { p.comment_highlight }, 0.0))
            })
            .collect()
    }

    // MARK: Changing

    fn changed(&mut self, now: f64) {
        self.revision += 1;
        self.schedule_save(now);
    }

    fn schedule_save(&mut self, now: f64) {
        self.save_at = Some(now + SAVE_DELAY);
    }

    pub fn schedule_live_state(&mut self, now: f64) {
        self.live_at = Some(now + LIVE_DELAY);
    }

    /// Makes `id` the active comment (its highlight darkens), or none.
    pub fn set_active_comment(&mut self, id: Option<String>) {
        if self.active_comment_id != id {
            self.active_comment_id = id;
            self.revision += 1;
        }
    }

    pub fn update_comment(&mut self, id: &str, body: &str, now: f64) {
        let Some(c) = self.notes.comments.iter_mut().find(|c| c.id == id) else { return };
        c.body = body.to_string();
        self.changed(now);
    }

    /// Removes a comment at once, with no confirmation. `NotesController.swift:136-141`
    pub fn delete_comment(&mut self, id: &str, now: f64) {
        self.notes.comments.retain(|c| c.id != id);
        if self.active_comment_id.as_deref() == Some(id) {
            self.active_comment_id = None;
        }
        self.changed(now);
    }

    /// Resolves or reopens a comment. `NotesController.swift:143-148`
    pub fn set_resolved(&mut self, id: &str, resolved: bool, now: f64) {
        let Some(c) = self.notes.comments.iter_mut().find(|c| c.id == id) else { return };
        c.resolved = resolved.then(SystemTime::now);
        self.changed(now);
    }

    pub fn add_reply(&mut self, id: &str, author: &str, text: &str, now: f64) {
        let Some(c) = self.notes.comments.iter_mut().find(|c| c.id == id) else { return };
        c.replies.push(CommentReply::new(author, text));
        self.changed(now);
    }

    /// Adds a version, newest first; the oldest unused ones go past 40.
    pub fn add_version(&mut self, v: DraftVersion, now: f64) {
        self.notes.add_version(v);
        self.changed(now);
    }

    pub fn delete_version(&mut self, id: &str, now: f64) {
        self.notes.versions.retain(|v| v.id != id);
        self.changed(now);
    }

    pub fn add_exchange(&mut self, e: AssistExchange, now: f64) {
        self.notes.add_exchange(e);
        self.changed(now);
    }

    /// Keeps `body` as a "Your draft" snapshot unless it is blank or an identical version
    /// already exists. `NotesController.swift:188-197`
    fn snapshot(&mut self, body: &str) {
        if body.trim().is_empty() || self.notes.versions.iter().any(|v| v.body == body) {
            return;
        }
        let label = format!("Your draft · {}", short_time(SystemTime::now()));
        self.notes.add_version(DraftVersion::new(VersionKind::Snapshot, label, "you", body));
    }

    /// Marks a version used and resolves the comments it addresses. `NotesController.swift:199-206`
    fn mark_used(&mut self, id: &str, now: f64) {
        let Some(i) = self.notes.versions.iter().position(|v| v.id == id) else { return };
        self.notes.versions[i].used_at = Some(SystemTime::now());
        let resolves = self.notes.versions[i].resolves_comment_ids.clone();
        if !resolves.is_empty() {
            let source = self.notes.versions[i].source.clone();
            self.notes.resolve(&resolves, None, &source);
        }
        self.changed(now);
    }

    // MARK: Following edits

    /// The selection before the next edit, so a typed or pasted replacement is placed exactly.
    pub fn note_selection(&mut self, selection: (usize, usize)) {
        self.pre_selection = selection;
    }

    /// The next text change is `range` (of the text before it) being replaced.
    pub fn expect_edit(&mut self, range: Range<usize>) {
        self.expected = Some(range);
    }

    /// Moves every comment for one edit: `edited` is the new text's range afterwards and
    /// `delta` the change in length, as `NSTextStorage` reports them.
    fn follow(&mut self, edited: Range<usize>, delta: isize) -> bool {
        if self.notes.comments.is_empty() {
            return false;
        }
        let mut any_detached = false;
        for c in &mut self.notes.comments {
            let was = c.detached;
            c.note_edit(edited.clone(), delta);
            any_detached |= c.detached && !was;
        }
        if any_detached {
            self.revision += 1;
        }
        true
    }

    /// Follows the editor from the text comments last saw to `text`. `caret` is where the
    /// selection ended up. Returns whether the text changed.
    fn text_changed(&mut self, text: &str, caret: usize, now: f64) -> bool {
        if self.shadow == text {
            self.expected = None;
            return false;
        }
        let hint = self.expected.take().map(|r| (r.start, r.end)).unwrap_or(self.pre_selection);
        let (edited, delta) = edit_between(&self.shadow, text, hint, caret);
        if self.follow(edited, delta) {
            self.schedule_save(now);
        }
        self.shadow = text.to_string();
        self.text_revision += 1;
        self.reanchor_detached(now);
        true
    }

    /// Detached comments come back when their text does, e.g. after undoing a deletion.
    /// `NotesController.swift:98-107`
    fn reanchor_detached(&mut self, now: f64) {
        if !self.notes.comments.iter().any(|c| c.detached && c.is_open()) {
            return;
        }
        let mut any = false;
        for c in self.notes.comments.iter_mut().filter(|c| c.detached && c.is_open()) {
            any |= c.reanchor(&self.shadow);
        }
        if any {
            self.changed(now);
        }
    }

    // MARK: Saving

    /// Saves now (on the worker), refreshing each comment's quote from the text.
    /// `NotesController.swift:87-96`
    pub fn flush_save(&mut self) {
        self.save_at = None;
        let Some(path) = self.path.clone() else { return };
        for c in &mut self.notes.comments {
            c.capture(&self.shadow);
        }
        self.notes.path = path;
        self.worker.send(Job::Save(Box::new(self.notes.clone()), self.shadow.clone()));
    }

    /// Saves the notes and removes the live state, as the app quits. `NotesController.swift:254-257`
    pub fn app_will_terminate(&mut self) {
        if self.worker.jobs.is_none() {
            return;
        }
        // A test's temporary support folder may already be gone; don't create it again.
        if self.store.support_directory.exists() {
            self.flush_save();
            self.worker.send(Job::ClearLive);
        }
        self.worker.stop();
    }

    /// Waits until the worker has done everything sent so far.
    #[cfg(test)]
    pub fn wait_for_disk(&self) {
        let (tx, rx) = mpsc::channel();
        self.worker.send(Job::Barrier(tx));
        let _ = rx.recv_timeout(Duration::from_secs(5));
    }
}

impl Drop for Notes {
    fn drop(&mut self) {
        self.app_will_terminate();
    }
}

#[allow(dead_code, reason = "the Assist panel (next task) calls these")]
impl App {
    // MARK: Document lifecycle

    /// Runs after the editor shows a new draft: saves the previous draft's notes, closes any
    /// preview, loads this draft's notes and re-anchors them. `NotesController.swift:36-50`
    pub fn notes_document_loaded(&mut self) {
        let n = &mut self.notes;
        n.flush_save();
        self.preview.close_quietly();
        self.composer.dismiss();
        let n = &mut self.notes;
        n.document_id = Some(self.draft.id().to_string());
        n.path = self.draft.path().map(path_string);
        n.shadow = self.edit.text.clone();
        n.notes = match &n.path {
            Some(p) => n.store.load(p, &n.shadow),
            None => DocumentNotes::new(""),
        };
        for c in &mut n.notes.comments {
            c.reanchor(&n.shadow);
        }
        n.active_comment_id = None;
        n.expected = None;
        let sel = self.edit.range();
        n.pre_selection = sel;
        n.live_selection = sel;
        n.revision += 1;
        n.text_revision += 1;
        n.schedule_live_state(self.now);
        n.worker.send(Job::Scan);
    }

    /// Files the notes under the draft's current path after a first save or a rename.
    /// `NotesController.swift:52-63`
    fn notes_sync_path(&mut self) {
        let Some(new) = self.draft.path().map(path_string) else { return };
        let n = &mut self.notes;
        if n.document_id.as_deref() != Some(&self.draft.id().to_string()) || n.path.as_deref() == Some(&new) {
            return;
        }
        if let Some(old) = n.path.clone() {
            n.flush_save();
            n.worker.send(Job::Move(old, new.clone()));
        }
        n.path = Some(new.clone());
        n.notes.path = new;
        n.schedule_save(self.now);
        n.schedule_live_state(self.now);
    }

    /// Runs after every change to the draft text: comments follow the edit.
    /// `NotesController.swift:65-75`, `EditorController.swift:538-549`
    pub fn notes_text_changed(&mut self) {
        let caret = self.edit.range().1;
        self.notes.text_changed(&self.edit.text, caret, self.now);
        self.notes.pre_selection = self.edit.range();
    }

    // MARK: Comments

    /// Adds a comment by "you" on `range` and keeps comments in text order.
    /// `NotesController.swift:116-128`
    pub fn add_comment(&mut self, range: Range<usize>, body: &str) -> Option<String> {
        let text = &self.edit.text;
        if range.is_empty() || range.end > text.len() || !text.is_char_boundary(range.start) || !text.is_char_boundary(range.end) {
            return None;
        }
        let c = Comment::new(range, text, body, "you");
        let id = c.id.clone();
        let n = &mut self.notes;
        n.notes.comments.push(c);
        n.notes.comments.sort_by_key(|c| c.range.start);
        n.changed(self.now);
        n.schedule_live_state(self.now);
        Some(id)
    }

    /// Clears the active comment, as Esc in the editor does. Returns whether there was one.
    /// `EditorController.swift:754`
    pub fn clear_active_comment(&mut self) -> bool {
        if self.notes.active_comment_id.is_none() {
            return false;
        }
        self.notes.set_active_comment(None);
        true
    }

    /// Selects a comment's text in the editor, scrolls to it and makes it active, closing any
    /// preview. What clicking a comment card does. `CommentComposer.swift:167-175`
    pub fn reveal_comment(&mut self, id: &str, tx: &mut Tx) {
        if self.preview.is_active() {
            self.close_preview(tx);
        }
        let Some(c) = self.notes.comment(id).filter(|c| !c.detached) else { return };
        let r = c.range.clone();
        self.notes.set_active_comment(Some(id.to_string()));
        let len = self.edit.text.len();
        self.edit.set_selection(r.start.min(len), r.end.min(len));
        self.reveal_caret();
        tx.wake_after(Duration::ZERO);
    }

    /// Opens the composer to edit a comment's text; not for detached comments.
    /// `CommentComposer.swift:157-165`
    pub fn edit_comment(&mut self, id: &str, tx: &mut Tx) {
        let Some(c) = self.notes.comment(id).filter(|c| !c.detached) else { return };
        let (r, quote, body) = (c.range.clone(), c.quote.clone(), c.body.clone());
        self.composer_present(r, quote, body, crate::comment_composer::Target::Edit(id.to_string()), tx);
    }

    pub fn delete_comment(&mut self, id: &str) {
        self.notes.delete_comment(id, self.now);
    }
    pub fn set_comment_resolved(&mut self, id: &str, resolved: bool) {
        self.notes.set_resolved(id, resolved, self.now);
    }
    pub fn reply_to_comment(&mut self, id: &str, author: &str, text: &str) {
        self.notes.add_reply(id, author, text, self.now);
    }

    /// The "Add Comment…" command. `CommentComposer.swift:138-155`
    pub fn add_comment_command(&mut self, tx: &mut Tx) {
        if self.preview.is_active() {
            self.close_preview(tx);
        }
        let Some(range) = comment_target_range(&self.edit.text, self.edit.range()) else {
            self.beep();
            return;
        };
        let quote = self.edit.text[range.clone()].to_string();
        self.composer_present(range.clone(), quote, String::new(), crate::comment_composer::Target::New(range), tx);
    }

    /// "Show Comments" and "Show Versions": opens the Assist panel on that tab.
    /// `AssistController.swift:872`, `VersionPreview.swift:408-410`
    pub fn show_assist_tab(&mut self, tab: AssistTab) {
        self.assist_visible = true;
        self.notes.assist_tab = tab;
        self.notes.revision += 1;
    }

    // MARK: Versions

    /// Keeps the current draft as a version and adds the given one, for Assist and agents.
    pub fn add_version(&mut self, v: DraftVersion) {
        self.notes.add_version(v, self.now);
    }

    pub fn add_exchange(&mut self, e: AssistExchange) {
        self.notes.add_exchange(e, self.now);
    }

    /// Deletes a version; deleting the previewed one closes the preview. `AssistController.swift:640-646`
    pub fn delete_version(&mut self, id: &str, tx: &mut Tx) {
        if self.preview.version_id() == Some(id) {
            self.close_preview(tx);
        }
        self.notes.delete_version(id, self.now);
    }

    /// Replaces the draft with a version as one undo step, keeping the current draft as a
    /// snapshot version first. `VersionPreview.swift:339-351`
    pub fn use_version(&mut self, id: &str, tx: &mut Tx) {
        let Some(body) = self.notes.version(id).map(|v| v.body.clone()) else { return };
        if self.edit.text != body {
            let current = self.edit.text.clone();
            self.notes.snapshot(&current);
            let tracked = self.apply_text(&body, tx);
            self.settle_after_rewrite(tracked);
        }
        self.notes.mark_used(id, self.now);
        self.close_preview(tx);
        let undo = shortcut_label(Command::Undo);
        self.flash_message(format!("Version used · {undo} or Versions to go back"), tx);
    }

    /// Rewrites the draft to `target` with word-level edits as one undo step, so untouched
    /// text, and comments on it, stay put. Returns false when it had to fall back to replacing
    /// everything. `VersionPreview.swift:353-406`
    fn apply_text(&mut self, target: &str, tx: &mut Tx) -> bool {
        let current = self.edit.text.clone();
        let mut edits: Vec<(Range<usize>, String)> = Vec::new();
        let mut pos = 0;
        let mut pending: Option<(usize, usize, String)> = None;
        for seg in writer_core::word_diff::diff(&current, target) {
            let len = seg.text.len();
            match seg.kind {
                writer_core::DiffSegmentKind::Equal => {
                    if let Some((start, length, insert)) = pending.take() {
                        edits.push((start..start + length, insert));
                    }
                    pos += len;
                }
                writer_core::DiffSegmentKind::Delete => {
                    pending.get_or_insert((pos, 0, String::new())).1 += len;
                    pos += len;
                }
                writer_core::DiffSegmentKind::Insert => pending.get_or_insert((pos, 0, String::new())).2.push_str(&seg.text),
            }
        }
        if let Some((start, length, insert)) = pending.take() {
            edits.push((start..start + length, insert));
        }
        if edits.is_empty() {
            return true;
        }
        let mut tracked = true;
        let notes = &mut self.notes;
        self.edit.break_history_group();
        self.edit.undo_group(|e| {
            for (range, text) in edits.iter().rev() {
                let len = e.text.len();
                if range.end > len || !e.text.is_char_boundary(range.start) || !e.text.is_char_boundary(range.end) {
                    continue;
                }
                e.set_selection(range.start, range.end);
                e.insert(text);
                notes.follow(range.start..range.start + text.len(), text.len() as isize - range.len() as isize);
            }
            if e.text != target {
                // Fall back to one replacement if the word edits didn't land exactly.
                let old = e.text.len();
                e.set_selection(0, old);
                e.insert(target);
                notes.follow(0..target.len(), target.len() as isize - old as isize);
                tracked = false;
            }
        });
        self.notes.shadow = self.edit.text.clone();
        self.notes.text_revision += 1;
        let first = edits[0].0.start.min(self.edit.text.len());
        self.edit.set_caret(first);
        self.text_did_change(tx);
        tracked
    }

    /// Settles comment anchors after a version replaced the text: spans tracked through the
    /// edits keep their place and take on the new wording; lost ones are searched for by
    /// quote. `NotesController.swift:208-224`
    fn settle_after_rewrite(&mut self, tracked: bool) {
        let n = &mut self.notes;
        let text = &n.shadow;
        for c in n.notes.comments.iter_mut().filter(|c| c.is_open()) {
            if tracked && !c.detached && !c.range.is_empty() && c.range.end <= text.len() {
                c.capture(text);
            } else {
                c.reanchor(text);
            }
        }
        n.changed(self.now);
    }

    // MARK: Clock

    /// Runs before every frame: path changes, the save and live-state timers, and the inbox.
    pub fn notes_poll(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        self.notes_sync_path();
        let sel = self.edit.range();
        if sel != self.notes.live_selection {
            self.notes.live_selection = sel;
            self.notes.schedule_live_state(self.now);
            // With the Comments tab open, the caret picks the comment it sits in.
            // `AssistController.swift:438-443`
            if self.assist_visible && self.notes.assist_tab == AssistTab::Comments {
                let id = self.notes.comment_at(sel.0.min(sel.1)).map(|c| c.id.clone());
                self.notes.set_active_comment(id);
                changed = true;
            }
        }
        self.notes.pre_selection = sel;
        if self.notes.save_at.is_some_and(|t| t <= self.now) {
            self.notes.flush_save();
        }
        if self.notes.live_at.is_some_and(|t| t <= self.now) {
            self.notes.live_at = None;
            self.write_live_state();
        }
        changed |= self.process_inbox(tx);
        changed |= self.preview_poll(tx);
        changed |= self.composer_poll(tx);
        for at in [self.notes.save_at, self.notes.live_at].into_iter().flatten() {
            tx.wake_after(Duration::from_secs_f64((at - self.now).max(0.0)));
        }
        changed
    }

    /// What the app is showing, for outside agents. `NotesController.swift:242-252`
    pub fn write_live_state(&mut self) {
        let (a, b) = self.edit.range();
        let state = LiveState::new(
            std::process::id(),
            &self.notes.app_name,
            self.draft.path().map(path_string),
            self.draft.remote().map(|r| r.display()),
            &self.draft.title,
            &self.edit.text,
            a.min(b)..a.max(b),
            self.preview.version_id().map(str::to_string),
        );
        self.notes.worker.send(Job::Live(Box::new(state)));
    }

    /// Applies the commands outside agents left in the inbox, oldest first.
    /// `NotesController.swift:273-308`
    fn process_inbox(&mut self, tx: &mut Tx) -> bool {
        let mut any = false;
        while let Ok(command) = self.notes.worker.inbox.try_recv() {
            any = true;
            let target = path_string(&standardize(Path::new(&command.path)));
            if self.notes.path.as_deref() != Some(target.as_str()) {
                self.notes.worker.send(Job::ApplyElsewhere(Box::new(command), target));
                continue;
            }
            // The open draft: apply to the live text and notes.
            let n = &mut self.notes;
            let versions_before = n.notes.versions.len();
            let result = NotesStore::apply(&command, &mut n.notes, &n.shadow);
            if result.ok {
                n.changed(self.now);
                n.flush_save();
                if command.kind == InboxCommandKind::ProposeVersion && n.notes.versions.len() > versions_before {
                    let keys = shortcut_label(Command::ShowVersions);
                    self.flash_message(format!("{} proposed a version · {keys} to see it", command.agent), tx);
                } else if command.kind == InboxCommandKind::AddComment {
                    self.flash_message(format!("{} added a comment", command.agent), tx);
                }
            }
            self.notes.worker.send(Job::Result(result));
        }
        any
    }

    /// Records a beep where the Swift app called `NSSound.beep()`.
    pub fn beep(&mut self) {
        self.notes.beeps += 1;
    }
}

/// The span a new comment attaches to: the selection, or the current line when nothing is
/// selected, trimmed of surrounding whitespace. `CommentComposer.swift:117-126`
pub fn comment_target_range(text: &str, selection: (usize, usize)) -> Option<Range<usize>> {
    let (a, b) = (selection.0.min(selection.1).min(text.len()), selection.0.max(selection.1).min(text.len()));
    let r = if a == b { text_units::paragraph_range(text, a) } else { a..b };
    let s = &text[r.clone()];
    let lead = s.len() - s.trim_start().len();
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| r.start + lead..r.start + lead + trimmed.len())
}

/// The single edit that turns `old` into `new`: the new text's range and the change in length.
/// `hint` is the selection just before the change: when replacing it explains the change (typing,
/// pasting or deleting a selection), that is the edit. Otherwise the smallest changed window
/// is used, slid to end at the caret (or, for a deletion, start there) when the text around it
/// repeats, so typing "a" after "a" lands where the caret was.
pub fn edit_between(old: &str, new: &str, hint: (usize, usize), caret: usize) -> (Range<usize>, isize) {
    let delta = new.len() as isize - old.len() as isize;
    let fits = |a: usize, old_len: usize, new_len: usize| {
        a + old_len <= old.len()
            && a + new_len <= new.len()
            && old.is_char_boundary(a)
            && new.is_char_boundary(a)
            && old.is_char_boundary(a + old_len)
            && new.is_char_boundary(a + new_len)
            && old.as_bytes()[..a] == new.as_bytes()[..a]
            && old.as_bytes()[a + old_len..] == new.as_bytes()[a + new_len..]
    };
    let (x, y) = (hint.0.min(hint.1), hint.0.max(hint.1));
    let inserted = (y - x) as isize + delta;
    if y <= old.len() && inserted >= 0 && fits(x, y - x, inserted as usize) {
        return (x..x + inserted as usize, delta);
    }
    let (ob, nb) = (old.as_bytes(), new.as_bytes());
    let mut p = ob.iter().zip(nb).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(p) {
        p -= 1;
    }
    let room = old.len().min(new.len()) - p;
    let mut s = ob.iter().rev().zip(nb.iter().rev()).take(room).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(old.len() - s) {
        s -= 1;
    }
    let (old_len, new_len) = (old.len() - p - s, new.len() - p - s);
    let wanted = if new_len > 0 { caret.checked_sub(new_len) } else { Some(caret) };
    if let Some(a) = wanted.filter(|a| *a < p && fits(*a, old_len, new_len)) {
        return (a..a + new_len, delta);
    }
    (p..p + new_len, delta)
}

/// A shortcut as the menus print it on this platform: "⇧⌘M" or "Ctrl+Shift+M".
pub fn shortcut_label(command: Command) -> String {
    fn find(entries: &[crate::commands::Entry], command: Command) -> Option<String> {
        entries.iter().find_map(|e| match e {
            crate::commands::Entry::Item(c, _, keys) if *c == command => Some(keys.label()),
            crate::commands::Entry::Submenu(_, inner) => find(inner, command),
            _ => None,
        })
    }
    crate::commands::MENUS.iter().find_map(|(_, entries)| find(entries, command)).unwrap_or_default()
}

/// The time as `DateFormatter`'s short time style shows it in US English: "3:42 PM", with the
/// narrow no-break space macOS 14 puts before AM/PM. `NotesController.swift:193-196`
pub fn short_time(t: SystemTime) -> String {
    let local: chrono::DateTime<chrono::Local> = t.into();
    local.format("%-I:%M\u{202F}%p").to_string()
}

/// `RelativeDateTimeFormatter` with the short units style, or "just now" under 30 seconds.
/// `VersionPreview.swift:330-335`
pub fn relative_time(t: SystemTime) -> String {
    let now = SystemTime::now();
    let (seconds, past) = match now.duration_since(t) {
        Ok(d) => (d.as_secs_f64(), true),
        Err(e) => (e.duration().as_secs_f64(), false),
    };
    if past && seconds < 30.0 {
        return "just now".into();
    }
    let s = seconds as u64;
    let (n, unit) = match s {
        0..60 => (s, "sec."),
        60..3_600 => (s / 60, "min."),
        3_600..86_400 => (s / 3_600, "hr."),
        86_400..604_800 => (s / 86_400, if s / 86_400 == 1 { "day" } else { "days" }),
        604_800..2_629_800 => (s / 604_800, "wk."),
        2_629_800..31_557_600 => (s / 2_629_800, "mo."),
        _ => (s / 31_557_600, "yr."),
    };
    if past { format!("{n} {unit} ago") } else { format!("in {n} {unit}") }
}

fn path_string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// `URL(fileURLWithPath:).standardizedFileURL`: absolute, `.` and `..` removed, symlinks kept.
fn standardize(p: &Path) -> PathBuf {
    let absolute = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().map(|d| d.join(p)).unwrap_or_else(|_| p.to_path_buf()) };
    let mut out = PathBuf::new();
    for c in absolute.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if out.parent().is_some() {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

// MARK: Worker

enum Job {
    Save(Box<DocumentNotes>, String),
    Move(String, String),
    Live(Box<LiveState>),
    ClearLive,
    /// Read the inbox: each command is removed from disk and handed to the window.
    Scan,
    Result(InboxResult),
    /// A command for a draft other than the open one: applied to that file's notes under the lock.
    ApplyElsewhere(Box<InboxCommand>, String),
    #[cfg(test)]
    Barrier(Sender<()>),
}

/// The thread that does the notes' disk work in order, and the watcher on the inbox folder.
struct Worker {
    jobs: Option<Sender<Job>>,
    inbox: Receiver<InboxCommand>,
    waker: Arc<Mutex<Option<Waker>>>,
    watcher: Option<notify::RecommendedWatcher>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    fn start(store: NotesStore) -> Worker {
        let (jobs, rx) = mpsc::channel::<Job>();
        let (inbox_tx, inbox) = mpsc::channel();
        let waker: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));
        let _ = std::fs::create_dir_all(store.inbox_directory());
        // The Swift app watched the inbox folder for writes and renames (`NotesController.swift:261-271`).
        let scan = jobs.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            // Only changes to the folder. On Linux, reading the folder (as a scan does) is
            // itself an event, which would start another scan, forever.
            if event.is_ok_and(|e| !e.kind.is_access()) {
                let _ = scan.send(Job::Scan);
            }
        })
        .ok()
        .and_then(|mut w| {
            use notify::Watcher;
            w.watch(&store.inbox_directory(), notify::RecursiveMode::NonRecursive).ok().map(|_| w)
        });
        let wake = waker.clone();
        let thread = std::thread::Builder::new()
            .name("notes".into())
            .spawn(move || {
                for job in rx {
                    run(&store, job, &inbox_tx, &wake);
                }
            })
            .ok();
        Worker { jobs: Some(jobs), inbox, waker, watcher, thread }
    }

    fn send(&self, job: Job) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(job);
        }
    }

    /// Finishes the queued work and ends the thread.
    fn stop(&mut self) {
        self.watcher = None;
        self.jobs = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(store: &NotesStore, job: Job, inbox: &Sender<InboxCommand>, waker: &Mutex<Option<Waker>>) {
    match job {
        Job::Save(notes, text) => {
            store.with_lock(|| {
                let _ = store.save(&notes, &text);
            });
        }
        Job::Move(old, new) => store.move_notes(&old, &new),
        Job::Live(state) => store.write_live_state(&state),
        Job::ClearLive => store.clear_live_state(),
        Job::Scan => {
            let mut any = false;
            for (file, command) in store.pending_commands() {
                let _ = std::fs::remove_file(file);
                any |= inbox.send(command).is_ok();
            }
            if any && let Some(w) = waker.lock().unwrap().as_ref() {
                w();
            }
        }
        Job::Result(r) => store.write_result(&r),
        Job::ApplyElsewhere(command, target) => {
            let result = match std::fs::read_to_string(&target) {
                Ok(text) => store.with_lock(|| {
                    let mut other = store.load(&target, &text);
                    let r = NotesStore::apply(&command, &mut other, &text);
                    if r.ok {
                        let _ = store.save(&other, &text);
                    }
                    r
                }),
                Err(_) => InboxResult::new(&command.id, false, format!("Couldn't read {target}.")),
            };
            store.write_result(&result);
        }
        #[cfg(test)]
        Job::Barrier(done) => {
            let _ = done.send(());
        }
    }
}

#[cfg(test)]
#[path = "notes_tests.rs"]
mod app_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_between_places_typing_at_the_caret() {
        // "a" typed after the first "a" of "aa": the diff alone can't tell which "a" is new.
        assert_eq!(edit_between("aa", "aaa", (1, 1), 2), (1..2, 1));
        // Backspace at 2 in "aaa".
        assert_eq!(edit_between("aaa", "aa", (2, 2), 1), (1..1, -1));
        // A selection replaced.
        assert_eq!(edit_between("hello world", "help world", (0, 5), 4), (0..4, -1));
        // Undo with an unrelated hint falls back to the smallest window.
        assert_eq!(edit_between("one two", "one", (0, 0), 3), (3..3, -4));
        // Multi-byte text stays on character boundaries.
        assert_eq!(edit_between("a😀b", "a😀😀b", (5, 5), 9), (5..9, 4));
        assert_eq!(edit_between("é", "è", (0, 0), 2), (0..2, 0));
    }

    #[test]
    fn target_is_the_trimmed_line_without_a_selection() {
        let text = "First line\n   second line  \nthird";
        assert_eq!(comment_target_range(text, (15, 15)), Some(14..25));
        assert_eq!(&text[14..25], "second line");
        assert_eq!(comment_target_range(text, (0, 5)), Some(0..5));
        assert_eq!(comment_target_range("a\n\nb", (2, 2)), None);
        assert_eq!(comment_target_range(" word ", (0, 6)), Some(1..5));
    }

    #[test]
    fn relative_times() {
        let ago = |s: u64| relative_time(SystemTime::now() - Duration::from_secs(s));
        assert_eq!(ago(5), "just now");
        assert_eq!(ago(45), "45 sec. ago");
        assert_eq!(ago(125), "2 min. ago");
        assert_eq!(ago(7200), "2 hr. ago");
        assert_eq!(ago(86_400), "1 day ago");
        assert_eq!(ago(3 * 86_400), "3 days ago");
    }
}
