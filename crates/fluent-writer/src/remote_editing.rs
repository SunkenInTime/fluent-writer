//! Editing over SSH: the Open over SSH prompt, finding out whether a location is a file or a
//! folder, opening remote files (falling back to the local copy when offline), uploading saves,
//! and making sure edits reached the server before leaving a file. Port of `RemoteEditing.swift`
//! and the remote parts of `EditorController.swift`.
//!
//! Every ssh call runs off the UI thread. Answers come back through `rune::Latest` (reads and
//! checks, newest wins) or the uploader's own worker, and are applied in `App::poll`.
//!
//! The end of this file holds the small modal pieces (sheet, buttons, text field box, alert)
//! the sidebar, the Open picker and this module draw with.

use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rune::{Align, Color, El, Latest, Shadow, TextEdit, Tx, Waker, col, el, row, text, text_input};
use writer_core::{DraftDocument, RemoteKind, RemoteLocation, SshTransport};

use crate::app::{App, DRAFT};
use crate::sidebar::{FolderRoot, Mode};
use crate::theme::{Palette, rgba, ui_font};

#[derive(Clone, Debug, PartialEq)]
pub enum RemoteStatus {
    Idle,
    Uploading,
    Uploaded,
    Offline,
    Failed(String),
}

/// What to do once the current draft may be left.
#[derive(Clone, Debug, PartialEq)]
pub enum Then {
    NewDraft,
    Open(PathBuf),
    OpenRemote(RemoteLocation),
    CloseWindow,
}

/// A message sheet over the window (`NSAlert`).
#[derive(Clone, Debug, PartialEq)]
pub struct Alert {
    pub title: String,
    pub text: String,
    pub kind: AlertKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AlertKind {
    /// One "OK" button.
    Message,
    /// The remote file's edits didn't upload: "Keep Editing" stays, "Continue" goes on.
    UnsentEdits(Then),
}

/// Leaving waits for an upload to finish.
#[derive(Clone, Debug, PartialEq)]
pub struct Leaving {
    pub host: String,
    pub then: Then,
}

// MARK: Uploader

struct Job {
    now: bool,
    doc: DraftDocument,
    body: String,
    loc: RemoteLocation,
}

struct Done {
    now: bool,
    doc: DraftDocument,
    body: String,
    result: Result<(), String>,
}

/// Uploads saved text one at a time on its own thread; while an upload runs, only the newest
/// pending text is kept (`RemoteUploader`).
pub struct Uploader {
    in_flight: bool,
    queued: Option<(DraftDocument, String)>,
    jobs: Option<Sender<Job>>,
    done: Arc<Mutex<Vec<Done>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl Uploader {
    fn new() -> Uploader {
        Uploader { in_flight: false, queued: None, jobs: None, done: Arc::default(), waker: Arc::default() }
    }

    fn send(&mut self, ssh: &SshTransport, job: Job) {
        if self.jobs.is_none() {
            let (send, receive) = channel::<Job>();
            let (ssh, done, waker) = (ssh.clone(), self.done.clone(), self.waker.clone());
            std::thread::Builder::new()
                .name("fluent-writer-upload".into())
                .spawn(move || {
                    for job in receive {
                        let result = ssh.write(job.body.as_bytes(), &job.loc).map_err(|e| e.to_string());
                        done.lock().unwrap().push(Done { now: job.now, doc: job.doc, body: job.body, result });
                        if let Some(w) = waker.lock().unwrap().as_ref() {
                            w();
                        }
                    }
                })
                .expect("the upload thread starts");
            self.jobs = Some(send);
        }
        let _ = self.jobs.as_ref().expect("set above").send(job);
    }

    fn upload(&mut self, ssh: &SshTransport, doc: DraftDocument, body: String) {
        let Some(loc) = doc.remote().cloned() else { return };
        if self.in_flight {
            self.queued = Some((doc, body));
            return;
        }
        self.in_flight = true;
        self.send(ssh, Job { now: false, doc, body, loc });
    }

    /// Uploads `body` once any running upload is done, dropping what was queued.
    fn upload_now(&mut self, ssh: &SshTransport, doc: DraftDocument, body: String) {
        let Some(loc) = doc.remote().cloned() else { return };
        self.queued = None;
        self.send(ssh, Job { now: true, doc, body, loc });
    }

    /// Whether an upload is running or waiting.
    pub fn busy(&self) -> bool {
        self.in_flight || self.queued.is_some()
    }
}

// MARK: State

type Read = (u64, RemoteLocation, Result<Vec<u8>, String>);
type Check = (RemoteLocation, Result<RemoteKind, String>);

pub struct Remote {
    pub ssh: SshTransport,
    pub status: RemoteStatus,
    pub uploader: Uploader,
    /// The Open over SSH prompt is showing.
    pub prompting: bool,
    pub field: TextEdit,
    pub alert: Option<Alert>,
    pub leaving: Option<Leaving>,
    open_generation: u64,
    reads: Latest<Read>,
    checks: Latest<Check>,
}

/// `--ssh PATH` runs that program in place of the system `ssh`, for tests against a private server.
fn ssh_executable() -> Option<String> {
    let args = rune::extra_args();
    args.iter().position(|a| a == "--ssh").and_then(|i| args.get(i + 1)).cloned()
}

impl Remote {
    pub fn new() -> Remote {
        let mut ssh = SshTransport::default();
        if let Some(exe) = ssh_executable() {
            ssh.executable = exe;
        }
        Remote {
            ssh,
            status: RemoteStatus::Idle,
            uploader: Uploader::new(),
            prompting: false,
            field: TextEdit::new(""),
            alert: None,
            leaving: None,
            open_generation: 0,
            reads: Latest::default(),
            checks: Latest::default(),
        }
    }

    pub fn set_waker(&self, waker: Waker) {
        self.reads.set_waker(waker.clone());
        self.checks.set_waker(waker.clone());
        *self.uploader.waker.lock().unwrap() = Some(waker);
    }

    /// An ssh call this module started is still running.
    pub fn is_busy(&self) -> bool {
        self.reads.is_pending() || self.checks.is_pending() || self.uploader.busy() || self.leaving.is_some()
    }

    /// A sheet of this module is up.
    pub fn sheet_open(&self) -> bool {
        self.prompting || self.alert.is_some() || self.leaving.is_some()
    }
}

impl Default for Remote {
    fn default() -> Self {
        Remote::new()
    }
}

// MARK: The app's side

impl App {
    pub fn focus_draft(&mut self, tx: &mut Tx) {
        self.edit.focused = true;
        tx.focus(DRAFT, false);
    }

    /// Saves before the draft is replaced and says whether it may be left. The leave check
    /// with its "couldn't be saved" alert is another branch's; a merge points this at it.
    pub fn can_leave_document(&mut self) -> bool {
        self.flush()
    }

    /// Runs `then` once the current draft may be left: saved, and for a remote file with
    /// edits that haven't reached the server, uploaded (or the writer chose to go on anyway).
    /// The upload runs off the UI thread behind a waiting sheet.
    pub fn leave_then(&mut self, then: Then, tx: &mut Tx) {
        if !self.can_leave_document() {
            return;
        }
        if let Some(loc) = self.draft.remote().cloned()
            && self.draft.remote_upload_pending()
        {
            let body = self.draft.saved_body().to_string();
            self.remote.uploader.upload_now(&self.remote.ssh, self.draft.clone(), body);
            self.remote.leaving = Some(Leaving { host: loc.host_name().to_string(), then });
            return;
        }
        self.proceed(then, tx);
    }

    fn proceed(&mut self, then: Then, tx: &mut Tx) {
        match then {
            Then::NewDraft => {
                let doc = self.library.new_draft();
                self.load(doc);
            }
            Then::Open(path) => match self.library.open(&path) {
                Ok(doc) => self.load(doc),
                Err(e) => self.present_error(e.to_string()),
            },
            Then::OpenRemote(loc) => self.read_remote(loc, tx),
            Then::CloseWindow => tx.close_window(),
        }
    }

    pub fn present_error(&mut self, message: impl Into<String>) {
        self.remote.alert = Some(Alert { title: message.into(), text: String::new(), kind: AlertKind::Message });
    }

    /// After a successful save: uploads a remote file's new text, or refreshes the Recent list.
    pub fn after_save(&mut self, result: writer_core::SaveResult) {
        if self.draft.remote().is_some() && self.draft.remote_upload_pending() {
            self.remote.status = RemoteStatus::Uploading;
            let body = self.draft.saved_body().to_string();
            self.remote.uploader.upload(&self.remote.ssh, self.draft.clone(), body);
        } else if result == writer_core::SaveResult::Saved && self.sidebar.mode == Mode::Recent {
            self.sidebar_document_changed();
        }
    }

    /// The footer status for a saved remote file, or None for every other case.
    pub fn remote_status_text(&self) -> Option<(String, bool)> {
        let loc = self.draft.remote()?;
        let host = loc.host_name();
        Some(match &self.remote.status {
            RemoteStatus::Idle | RemoteStatus::Uploaded if self.draft.remote_upload_pending() => (format!("Saved locally · not yet on {host}"), false),
            RemoteStatus::Idle | RemoteStatus::Uploaded => (format!("Saved to {host}"), false),
            RemoteStatus::Uploading => (format!("Saving to {host}…"), false),
            RemoteStatus::Offline => ("Offline · editing the local copy".into(), false),
            RemoteStatus::Failed(message) => (format!("Not on {host} yet — {message}"), true),
        })
    }

    /// The status label's tooltip for a remote file: the error, or the remote location.
    pub fn remote_status_help(&self) -> Option<String> {
        let loc = self.draft.remote()?;
        Some(match &self.remote.status {
            RemoteStatus::Failed(message) => message.clone(),
            _ => loc.display(),
        })
    }

    // MARK: Open over SSH

    pub fn open_over_ssh(&mut self) {
        let last = self.prefs.last_ssh_location().to_string();
        let mut field = TextEdit::new(last.clone());
        field.set_selection(0, last.len());
        field.focused = true;
        self.remote.field = field;
        self.remote.prompting = true;
    }

    pub fn ssh_prompt_cancel(&mut self, tx: &mut Tx) {
        self.remote.prompting = false;
        self.focus_draft(tx);
    }

    pub fn ssh_prompt_open(&mut self, tx: &mut Tx) {
        self.remote.prompting = false;
        let input = self.remote.field.text.clone();
        let Some(loc) = RemoteLocation::parse(&input) else {
            self.remote.alert = Some(Alert {
                title: "That doesn't look like an SSH location.".into(),
                text: "Use host:path, user@host:path, or ssh://user@host:port/path.".into(),
                kind: AlertKind::Message,
            });
            return;
        };
        self.prefs.set_last_ssh_location(&input);
        self.resolve_remote(loc, tx);
    }

    /// Finds out in the background whether `loc` is a folder, a file or missing.
    fn resolve_remote(&mut self, loc: RemoteLocation, tx: &mut Tx) {
        self.flash_message(format!("Connecting to {}…", loc.host_name()), tx);
        let ssh = self.remote.ssh.clone();
        self.remote.checks.request(loc, move |loc| {
            let kind = ssh.kind(&loc).map_err(|e| e.to_string());
            (loc, kind)
        });
    }

    /// Opens a remote file in place of the current draft.
    pub fn open_remote(&mut self, loc: RemoteLocation, tx: &mut Tx) {
        if self.draft.remote() == Some(&loc) {
            self.focus_draft(tx);
            return;
        }
        self.leave_then(Then::OpenRemote(loc), tx);
    }

    fn read_remote(&mut self, loc: RemoteLocation, tx: &mut Tx) {
        self.remote.open_generation += 1;
        let generation = self.remote.open_generation;
        self.flash_message(format!("Opening {} from {}…", loc.name(), loc.host_name()), tx);
        let ssh = self.remote.ssh.clone();
        self.remote.reads.request(loc, move |loc| {
            let data = ssh.read(&loc).map_err(|e| e.to_string());
            (generation, loc, data)
        });
    }

    fn finish_opening_remote(&mut self, loc: RemoteLocation, result: Result<Vec<u8>, String>, tx: &mut Tx) {
        // A save that fails here drops the open without a word, as the Swift did.
        if !self.flush() {
            return;
        }
        match result {
            Ok(data) => match self.library.open_remote(&loc, &data) {
                Ok(doc) => {
                    let kept = doc.remote_upload_pending();
                    self.load(doc);
                    if kept {
                        self.flash_message(format!("Kept your edits that hadn't reached {}", loc.host_name()), tx);
                    }
                }
                Err(e) => self.present_error(e.to_string()),
            },
            Err(error) => {
                let mirror = self.library.mirror_path(&loc);
                if self.library.record_for_remote(&loc).is_none() || !mirror.exists() {
                    self.present_error(error);
                    return;
                }
                match self.library.open(&mirror) {
                    Ok(doc) => {
                        self.load(doc);
                        self.remote.status = RemoteStatus::Offline;
                    }
                    Err(e) => self.present_error(e.to_string()),
                }
            }
        }
    }

    fn upload_finished(&mut self, done: Done, tx: &mut Tx) {
        let ok = done.result.is_ok();
        if done.doc.id() == self.draft.id() {
            self.library.remote_upload_finished(&mut self.draft, &done.body, ok);
            match &done.result {
                Err(message) => {
                    self.remote.status = RemoteStatus::Failed(message.clone());
                    self.show_chrome(false, tx);
                }
                Ok(()) => self.remote.status = if self.draft.remote_upload_pending() { RemoteStatus::Uploading } else { RemoteStatus::Uploaded },
            }
        } else {
            let mut doc = done.doc;
            self.library.remote_upload_finished(&mut doc, &done.body, ok);
        }
    }

    /// Keep Editing on the waiting sheet: stay on this draft. The upload carries on.
    pub fn stop_leaving(&mut self, tx: &mut Tx) {
        self.remote.leaving = None;
        self.focus_draft(tx);
    }

    pub fn alert_button(&mut self, continue_on: bool, tx: &mut Tx) {
        let Some(alert) = self.remote.alert.take() else { return };
        match alert.kind {
            AlertKind::UnsentEdits(then) if continue_on => self.proceed(then, tx),
            _ => self.focus_draft(tx),
        }
    }

    /// This module's share of `App::poll`. Returns whether anything the view reads changed.
    pub fn poll_remote(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some((loc, kind)) = self.remote.checks.take() {
            changed = true;
            match kind {
                Ok(RemoteKind::Directory) => {
                    if !self.sidebar_visible {
                        self.toggle_sidebar(tx);
                    }
                    self.set_sidebar_folder(FolderRoot::Remote(loc));
                }
                Ok(RemoteKind::File) => self.open_remote(loc, tx),
                Ok(RemoteKind::Missing) => self.present_error(format!("Nothing at {}.", loc.display())),
                Err(e) => self.present_error(e),
            }
        }
        if let Some((generation, loc, result)) = self.remote.reads.take() {
            changed = true;
            if generation == self.remote.open_generation {
                self.finish_opening_remote(loc, result, tx);
            }
        }
        let done: Vec<Done> = std::mem::take(&mut *self.remote.uploader.done.lock().unwrap());
        for d in done {
            changed = true;
            let now = d.now;
            let failure = d.result.clone().err();
            if !now {
                self.remote.uploader.in_flight = false;
            }
            self.upload_finished(d, tx);
            if now {
                if let Some(leaving) = self.remote.leaving.take() {
                    match failure {
                        None => self.proceed(leaving.then, tx),
                        Some(error) => {
                            self.remote.alert = Some(Alert {
                                title: format!("Your latest edits aren't on {} yet.", leaving.host),
                                text: format!("{error}\n\nThey're saved on this Mac and will be sent the next time you open this file."),
                                kind: AlertKind::UnsentEdits(leaving.then),
                            });
                        }
                    }
                }
            } else if let Some((doc, body)) = self.remote.uploader.queued.take() {
                let ssh = self.remote.ssh.clone();
                self.remote.uploader.upload(&ssh, doc, body);
            }
        }
        if self.remote.is_busy() {
            // Keep polling while answers are due, in case no waker is attached (tests).
            tx.wake_after(Duration::from_millis(50));
        }
        changed
    }
}

// MARK: View

/// The SSH prompt, the waiting sheet and alerts, topmost last.
pub fn view(s: &App, p: &Palette) -> Vec<El> {
    let w = s.window.0;
    let mut out = Vec::new();
    if s.remote.prompting {
        out.push(ssh_prompt(s, p, w));
    }
    if let Some(leaving) = &s.remote.leaving {
        let body = col()
            .w_full()
            .items_center()
            .gap(8.0)
            .child(text(format!("Saving to {}…", leaving.host)).key("leave-wait-title").font(ui_font(13.0, true)).color(p.text).align(Align::Center).wrap().w_full())
            .child(row().w_full().mt(12.0).justify_end().child(button(p, "leave-wait-keep", "Keep Editing", true).on_press_tx(|s: &mut App, tx| s.stop_leaving(tx))));
        out.push(sheet(p, w, 300.0, None, "leave-wait", body).modal_tx(false, |s: &mut App, tx| s.stop_leaving(tx)));
    }
    if let Some(alert) = &s.remote.alert {
        out.push(alert_sheet(p, w, alert));
    }
    out
}

fn ssh_prompt(s: &App, p: &Palette, w: f32) -> El {
    let field = text_input(&s.remote.field, ui_font(13.0, false))
        .key("ssh-field")
        .label("SSH location")
        .single_line()
        .placeholder("user@host:~/notes/draft.md", p.quiet)
        .color(p.text)
        .caret(p.caret, p.selection)
        .w_full()
        .on_edit(|s: &mut App| &mut s.remote.field)
        .on_submit(|s: &mut App, tx| s.ssh_prompt_open(tx));
    let body = col()
        .w_full()
        .items_center()
        .gap(8.0)
        .child(text("Open over SSH").font(ui_font(13.0, true)).color(p.text).align(Align::Center).w_full())
        .child(
            text("Enter a file to edit or a folder to browse, like me@server:~/notes or ssh://me@server:2222/srv/notes/draft.md. Your SSH config and keys are used; password prompts aren't supported.")
                .key("ssh-text")
                .font(ui_font(11.0, false))
                .color(p.text)
                .align(Align::Center)
                .wrap()
                .w_full(),
        )
        .child(field_box(p, field).mt(4.0).size(340.0, 24.0))
        .child(
            row()
                .w_full()
                .mt(12.0)
                .gap(8.0)
                .justify_end()
                .child(button(p, "ssh-cancel", "Cancel", false).on_press_tx(|s: &mut App, tx| s.ssh_prompt_cancel(tx)))
                .child(button(p, "ssh-open", "Open", true).default_button().on_press_tx(|s: &mut App, tx| s.ssh_prompt_open(tx))),
        );
    sheet(p, w, 380.0, None, "ssh-prompt", body).modal_tx(false, |s: &mut App, tx| s.ssh_prompt_cancel(tx))
}

fn alert_sheet(p: &Palette, w: f32, alert: &Alert) -> El {
    let mut body = col().w_full().items_center().gap(8.0).child(text(alert.title.clone()).key("alert-title").font(ui_font(13.0, true)).color(p.text).align(Align::Center).wrap().w_full());
    if !alert.text.is_empty() {
        body = body.child(text(alert.text.clone()).key("alert-text").font(ui_font(11.0, false)).color(p.text).align(Align::Center).wrap().w_full());
    }
    let buttons = match alert.kind {
        AlertKind::Message => row().w_full().justify_end().child(button(p, "alert-ok", "OK", true).default_button().on_press_tx(|s: &mut App, tx| s.alert_button(false, tx))),
        AlertKind::UnsentEdits(_) => row()
            .w_full()
            .gap(8.0)
            .justify_end()
            .child(button(p, "alert-continue", "Continue", false).on_press_tx(|s: &mut App, tx| s.alert_button(true, tx)))
            .child(button(p, "alert-keep", "Keep Editing", true).default_button().on_press_tx(|s: &mut App, tx| s.alert_button(false, tx))),
    };
    body = body.child(buttons.mt(12.0));
    sheet(p, w, 300.0, None, "alert", body).modal_tx(false, |s: &mut App, tx| s.alert_button(false, tx))
}

// MARK: Sheet pieces (private to this branch; to be unified with `ui.rs` at merge)

/// Where a sheet's top sits: under the 28 pt title bar on macOS, at the top elsewhere.
pub const SHEET_TOP: f32 = if cfg!(target_os = "macos") { 28.0 } else { 0.0 };

fn clear() -> Color {
    rgba(0, 0, 0, 0.0)
}

/// A sheet `width` wide, centred at the top of a window `window_w` wide. With `height` the
/// content is laid out absolutely inside it; without, it is padded by 20 and sized to fit.
pub fn sheet(p: &Palette, window_w: f32, width: f32, height: Option<f32>, key: &str, content: El) -> El {
    let mut e = el()
        .key(key)
        .absolute(((window_w - width) / 2.0).round(), SHEET_TOP)
        .w(width)
        .fill(p.canvas)
        .border(1.0, p.hairline)
        .radius(10.0)
        .box_shadow(Shadow { x: 0.0, y: 8.0, blur: 30.0, spread: 0.0, color: rgba(0, 0, 0, if p.dark { 0.5 } else { 0.2 }), inset: false })
        .role("dialog");
    e = match height {
        Some(h) => e.h(h).child(content),
        None => e.padding(20.0).child(content),
    };
    e
}

/// A push button; the default one is filled.
pub fn button(p: &Palette, key: &str, label: &str, default: bool) -> El {
    let chars = label.chars().count() as f32;
    let width = ((chars * 13.0 * crate::theme::CHARACTER_WIDTH_EM).ceil() + 24.0).max(70.0);
    let (fill, ink) = if default { (p.text, p.canvas) } else { (p.card, p.text) };
    el().key(key)
        .size(width, 21.0)
        .radius(6.0)
        .fill(fill)
        .border(1.0, if default { clear() } else { p.hairline })
        .hover(p.text.alpha(0.06))
        .center()
        .role("button")
        .label(label.to_string())
        .focusable(true)
        .child(text(label.to_string()).font(ui_font(13.0, false)).color(ink))
}

/// The bezel around a single-line text field.
pub fn field_box(p: &Palette, field: El) -> El {
    el().fill(p.card).border(1.0, p.hairline).radius(5.0).px(6.0).items_center().child(field)
}
