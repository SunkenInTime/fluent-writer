//! Editing over SSH: the Open over SSH prompt, finding out whether a location is a file or a
//! folder, opening remote files (falling back to the local copy when offline), uploading saves,
//! and making sure edits reached the server before leaving a file. Port of `RemoteEditing.swift`
//! and the remote parts of `EditorController.swift`.
//!
//! Every ssh call runs off the UI thread. Answers come back through `rune::Latest` (reads and
//! checks, newest wins) or the uploader's own worker, and are applied in `App::poll`.
//!
//! Leaving a remote draft runs `App::leave_document` (`documents.rs`), whose save check hands
//! over to [`App::confirm_remote_uploaded`] here, as the Swift `confirmLeavingDocument` calls
//! `confirmRemoteUploaded`. Alerts and sheets are `ui.rs`'s.

use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rune::{El, Latest, TextEdit, Tx, Waker, col, row, text_input};
use writer_core::{DraftDocument, RemoteKind, RemoteLocation, SshTransport};

use crate::app::App;
use crate::documents::Then;
use crate::sidebar::{FolderRoot, Mode};
use crate::theme::{Palette, ui_font};
use crate::ui::{self, Alert};

#[derive(Clone, Debug, PartialEq)]
pub enum RemoteStatus {
    Idle,
    Uploading,
    Uploaded,
    Offline,
    Failed(String),
}

/// Leaving waits for an upload to finish.
pub struct Leaving {
    pub host: String,
    pub then: Then,
}

// MARK: Uploader

struct Job {
    ssh: SshTransport,
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

    fn send(&mut self, job: Job) {
        if self.jobs.is_none() {
            let (send, receive) = channel::<Job>();
            let (done, waker) = (self.done.clone(), self.waker.clone());
            std::thread::Builder::new()
                .name("fluent-writer-upload".into())
                .spawn(move || {
                    for job in receive {
                        let result = job.ssh.write(job.body.as_bytes(), &job.loc).map_err(|e| e.to_string());
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
        self.send(Job { ssh: ssh.clone(), now: false, doc, body, loc });
    }

    /// Uploads `body` once any running upload is done, dropping what was queued.
    fn upload_now(&mut self, ssh: &SshTransport, doc: DraftDocument, body: String) {
        let Some(loc) = doc.remote().cloned() else { return };
        self.queued = None;
        self.send(Job { ssh: ssh.clone(), now: true, doc, body, loc });
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
}

impl Default for Remote {
    fn default() -> Self {
        Remote::new()
    }
}

// MARK: The app's side

/// The Open over SSH prompt's card width: its 340-point field and the alert's padding.
const PROMPT_WIDTH: f32 = 380.0;

impl App {
    /// The second half of the leave check, once the save succeeded (`confirmRemoteUploaded`):
    /// for a remote file with edits that haven't reached the server, uploads them, then runs
    /// `then`. The upload runs off the UI thread behind a "Saving to <host>…" sheet; if it fails
    /// the writer is warned and chooses (Keep Editing / Continue). Anything else runs `then` now.
    pub fn confirm_remote_uploaded(&mut self, then: Then, tx: &mut Tx) {
        if let Some(loc) = self.draft.remote().cloned()
            && self.draft.remote_upload_pending()
        {
            let body = self.draft.saved_body().to_string();
            self.remote.uploader.upload_now(&self.remote.ssh, self.draft.clone(), body);
            self.remote.leaving = Some(Leaving { host: loc.host_name().to_string(), then });
            crate::documents::bring_window_back();
            return;
        }
        then(self, tx);
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

    /// The footer's status for a saved remote file (see [`crate::footer::RemoteStatus`]), or
    /// None for a local one.
    pub fn remote_status(&self) -> Option<crate::footer::RemoteStatus> {
        let loc = self.draft.remote()?;
        let host = loc.host_name();
        let (text, warning) = match &self.remote.status {
            RemoteStatus::Idle | RemoteStatus::Uploaded if self.draft.remote_upload_pending() => (format!("Saved locally · not yet on {host}"), false),
            RemoteStatus::Idle | RemoteStatus::Uploaded => (format!("Saved to {host}"), false),
            RemoteStatus::Uploading => (format!("Saving to {host}…"), false),
            RemoteStatus::Offline => ("Offline · editing the local copy".into(), false),
            RemoteStatus::Failed(message) => (format!("Not on {host} yet — {message}"), true),
        };
        let tooltip = match &self.remote.status {
            RemoteStatus::Failed(message) => message.clone(),
            _ => loc.display(),
        };
        Some(crate::footer::RemoteStatus { text, warning, tooltip })
    }

    // MARK: Open over SSH

    /// File › Open over SSH…: an alert with a field for the location, prefilled with the last
    /// one and selected.
    pub fn open_over_ssh(&mut self) {
        let last = self.prefs.last_ssh_location().to_string();
        let mut field = TextEdit::new(last.clone());
        field.set_selection(0, last.len());
        field.focused = true;
        self.remote.field = field;
        self.remote.prompting = true;
        self.show_alert(
            Alert::new("Open over SSH")
                .informative("Enter a file to edit or a folder to browse, like me@server:~/notes or ssh://me@server:2222/srv/notes/draft.md. Your SSH config and keys are used; password prompts aren't supported.")
                .button("Open")
                .button("Cancel")
                .width(PROMPT_WIDTH)
                .accessory(ssh_field)
                .on_answer(|s, button, tx| if button == 0 { s.ssh_prompt_open(tx) } else { s.ssh_prompt_cancel(tx) }),
        );
    }

    pub fn ssh_prompt_cancel(&mut self, tx: &mut Tx) {
        self.remote.prompting = false;
        self.focus_draft(tx);
    }

    pub fn ssh_prompt_open(&mut self, tx: &mut Tx) {
        self.remote.prompting = false;
        let input = self.remote.field.text.clone();
        let Some(loc) = RemoteLocation::parse(&input) else {
            self.show_alert(Alert::new("That doesn't look like an SSH location.").informative("Use host:path, user@host:path, or ssh://user@host:port/path."));
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

    /// Opens a remote file in place of the current draft, after the leave check.
    pub fn open_remote(&mut self, loc: RemoteLocation, tx: &mut Tx) {
        if self.draft.remote() == Some(&loc) {
            self.focus_draft(tx);
            return;
        }
        self.leave_document(tx, move |s, tx| s.read_remote(loc, tx));
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
                    self.focus_draft(tx);
                    if kept {
                        self.flash_message(format!("Kept your edits that hadn't reached {}", loc.host_name()), tx);
                    }
                }
                Err(e) => self.present_error(e),
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
                        self.focus_draft(tx);
                        self.remote.status = RemoteStatus::Offline;
                    }
                    Err(e) => self.present_error(e),
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

    /// The upload before leaving failed: warn, and leave only on Continue.
    fn warn_unsent_edits(&mut self, leaving: Leaving, error: String) {
        self.documents.pending = Some(leaving.then);
        self.show_alert(
            Alert::new(format!("Your latest edits aren't on {} yet.", leaving.host))
                .informative(format!("{error}\n\nThey're saved on this Mac and will be sent the next time you open this file."))
                .warning()
                .button("Keep Editing")
                .button("Continue")
                .on_answer(|s, button, tx| {
                    let then = s.documents.pending.take();
                    match (button, then) {
                        (1, Some(then)) => then(s, tx),
                        _ => s.focus_draft(tx),
                    }
                }),
        );
        crate::documents::bring_window_back();
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
                        None => (leaving.then)(self, tx),
                        Some(error) => self.warn_unsent_edits(leaving, error),
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

/// The Open over SSH prompt's field, the alert's accessory: 340 × 24, as the Swift's.
fn ssh_field(s: &App, p: &Palette) -> El {
    let field = text_input(&s.remote.field, ui_font(13.0, false))
        .key("ssh-field")
        .label("SSH location")
        .single_line()
        .placeholder("user@host:~/notes/draft.md", p.quiet)
        .color(p.text)
        .caret(p.caret, p.selection)
        .w_full()
        .on_edit(|s: &mut App| &mut s.remote.field)
        .on_submit(|s: &mut App, tx| s.answer_alert(0, tx));
    ui::field_box(p, field).size(340.0, 24.0)
}

/// The sheet that waits for the upload before leaving a remote draft.
pub fn view(s: &App, p: &Palette) -> Option<El> {
    let leaving = s.remote.leaving.as_ref()?;
    let body = col()
        .key("leave-wait-body")
        .gap(4.0)
        .child(ui::message_text("leave-wait-title", &format!("Saving to {}…", leaving.host), p))
        .child(row().gap(8.0).mt(8.0).justify_end().child(ui::button("leave-wait-keep", "Keep Editing", true, p, |s: &mut App, tx| s.stop_leaving(tx))));
    Some(ui::sheet("leave-wait", p, s.window, 300.0, body, |s: &mut App, tx| s.stop_leaving(tx)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::Command;
    use crate::sidebar::tests::{app_with, click, run, wait_for};
    use crate::tests::{settle, type_text};
    use rune::{Key, Mods, Runtime};

    /// The test server: `FLUENT_WRITER_SSH_TEST_DIR=host:/folder` it may write in, and
    /// `FLUENT_WRITER_SSH_TEST_SSH` / `FLUENT_WRITER_SSH_TEST_SSH_DOWN`, programs that run ssh
    /// against it and against a closed port. Without them these tests do nothing.
    struct Server {
        host: String,
        dir: String,
        ssh: String,
        down: String,
    }

    fn server(name: &str) -> Option<Server> {
        let (Ok(spec), Ok(ssh), Ok(down)) = (std::env::var("FLUENT_WRITER_SSH_TEST_DIR"), std::env::var("FLUENT_WRITER_SSH_TEST_SSH"), std::env::var("FLUENT_WRITER_SSH_TEST_SSH_DOWN")) else {
            eprintln!("skipped: set FLUENT_WRITER_SSH_TEST_DIR, FLUENT_WRITER_SSH_TEST_SSH and FLUENT_WRITER_SSH_TEST_SSH_DOWN");
            return None;
        };
        let (host, base) = spec.split_once(':').expect("host:/folder");
        let s = Server { host: host.into(), dir: format!("{base}/{name}-{}", std::process::id()), ssh, down };
        s.sh(&format!("rm -rf '{0}' && mkdir -p '{0}/notes/sub'", s.dir));
        Some(s)
    }

    impl Server {
        fn sh(&self, command: &str) -> String {
            let out = std::process::Command::new(&self.ssh).args(["-o", "BatchMode=yes", &self.host, command]).output().unwrap();
            assert!(out.status.success(), "{command}: {}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8(out.stdout).unwrap()
        }
        fn put(&self, path: &str, body: &str) {
            SshTransport::new(&self.ssh, 10).write(body.as_bytes(), &self.loc(path)).unwrap();
        }
        fn get(&self, path: &str) -> String {
            self.sh(&format!("cat '{}/{path}'", self.dir))
        }
        fn loc(&self, path: &str) -> RemoteLocation {
            RemoteLocation::new(self.host.clone(), None, format!("{}/{path}", self.dir))
        }
        fn up(&self) -> SshTransport {
            SshTransport::new(&self.ssh, 10)
        }
        fn down(&self) -> SshTransport {
            SshTransport::new(&self.down, 10)
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            let _ = std::process::Command::new(&self.ssh).args(["-o", "BatchMode=yes", &self.host, &format!("rm -rf '{}'", self.dir)]).output();
        }
    }

    fn status(rt: &Runtime<App>) -> String {
        rt.state.status().0
    }

    /// The alert showing, taken off the screen.
    fn take_alert(rt: &mut Runtime<App>) -> crate::ui::Alert {
        assert!(!rt.state.alerts.is_empty(), "an alert");
        rt.state.alerts.remove(0)
    }

    /// Types `text` into the SSH prompt and presses Return.
    fn ssh_prompt(rt: &mut Runtime<App>, text: &str) {
        run(rt, Command::OpenOverSsh);
        assert!(rt.state.remote.prompting);
        rt.update(|s, _| s.remote.field.replace_text(text));
        crate::tests::press(rt, Key::Enter, Mods::default());
        rt.frame(1.0 / 60.0);
    }

    #[test]
    fn the_prompt_checks_the_location_and_remembers_it() {
        let (mut rt, _dir) = app_with(&[("One.md", "One")]);
        ssh_prompt(&mut rt, "no colon here");
        assert!(!rt.state.remote.prompting);
        let alert = rt.state.alert().cloned().expect("an alert");
        assert_eq!(alert.message, "That doesn't look like an SSH location.");
        assert_eq!(alert.informative, "Use host:path, user@host:path, or ssh://user@host:port/path.");
        assert_eq!(rt.state.prefs.last_ssh_location(), "");
        click(&mut rt, "alert-button-0");
        assert!(rt.state.alert().is_none());
        for bad in ["-oProxyCommand=x:/a", "ho st:/a", "h;rm:/a", ":/a"] {
            ssh_prompt(&mut rt, bad);
            assert_eq!(take_alert(&mut rt).message, "That doesn't look like an SSH location.", "{bad} is refused");
        }
        // A valid location is remembered and comes back selected next time.
        rt.update(|s, _| s.remote.ssh = SshTransport::new("/nonexistent/ssh", 10));
        ssh_prompt(&mut rt, "me@box:~/notes");
        assert_eq!(rt.state.transient.as_deref(), Some("Connecting to box…"));
        assert_eq!(rt.state.prefs.last_ssh_location(), "me@box:~/notes");
        wait_for(&mut rt, "the failed check", |s| s.alert().is_some());
        assert!(take_alert(&mut rt).message.starts_with("Couldn't start ssh: "));
        run(&mut rt, Command::OpenOverSsh);
        assert_eq!(rt.state.remote.field.text, "me@box:~/notes");
        assert_eq!(rt.state.remote.field.range(), (0, 14));
        let el = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("ssh-field")).unwrap();
        assert_eq!(el.label.as_deref(), Some("SSH location"));
        // The prompt is an alert: Open is the default, Cancel beside it takes Escape.
        let prompt = rt.state.alert().cloned().expect("the prompt");
        assert_eq!(prompt.message, "Open over SSH");
        assert_eq!(prompt.buttons, ["Open", "Cancel"]);
        assert_eq!(prompt.cancel, Some(1));
        click(&mut rt, "alert-button-1");
        assert!(!rt.state.remote.prompting);
        assert!(rt.state.alert().is_none());
    }

    #[test]
    fn ssh_folder_file_save_offline_and_leaving() {
        let Some(srv) = server("round-trip") else { return };
        srv.put("notes/draft.md", "# Remote draft\n\nWritten on the server.");
        srv.put("notes/other.txt", "other");
        let (mut rt, dir) = app_with(&[("Local.md", "Local")]);
        rt.update(|s, _| s.remote.ssh = srv.up());
        let host = srv.host.clone();

        // A folder: the sidebar pins and lists it over ssh.
        ssh_prompt(&mut rt, &format!("{host}:{}/notes", srv.dir));
        assert_eq!(rt.state.transient.as_deref(), Some(format!("Connecting to {host}…").as_str()));
        wait_for(&mut rt, "the folder check", |s| !s.sidebar.folder_stack.is_empty());
        assert!(rt.state.sidebar_visible);
        assert_eq!(rt.state.sidebar.message.as_deref(), Some(format!("Loading {host}…").as_str()));
        wait_for(&mut rt, "the listing", |s| s.sidebar.message.is_none());
        let titles: Vec<String> = rt.state.sidebar.items.iter().map(|i| i.title.clone()).collect();
        assert_eq!(titles, ["sub", "draft.md", "other.txt"]);
        assert_eq!(rt.state.sidebar.header(), "notes");
        assert_eq!(rt.state.prefs.folder_remote(), Some(srv.loc("notes").display().as_str()));

        // A file in it: read over ssh into the local copy.
        click(&mut rt, "sidebar-row-1");
        assert_eq!(rt.state.transient.as_deref(), Some(format!("Opening draft.md from {host}…").as_str()));
        wait_for(&mut rt, "the remote file", |s| s.draft.remote().is_some());
        let loc = srv.loc("notes/draft.md");
        assert_eq!(rt.state.draft.remote(), Some(&loc));
        assert_eq!(rt.state.edit.text, "# Remote draft\n\nWritten on the server.");
        assert_eq!(rt.state.draft.title, "draft.md");
        let mirror = rt.state.library.mirror_path(&loc);
        assert!(mirror.starts_with(dir.path().join("support/Remote").join(&host).join("root")));
        rt.state.transient = None;
        assert_eq!(status(&rt), format!("Saved to {host}"));
        assert_eq!(rt.state.remote_status().map(|r| r.tooltip), Some(loc.display()));
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.status_tooltip, Some(loc.display()), "the footer's tooltip is the remote location");
        assert!(!rt.state.command_enabled(Command::Reveal));

        // Saving writes the copy, then uploads.
        rt.update(|s, tx| {
            s.edit.set_selection(s.edit.text.len(), s.edit.text.len());
            s.focus_draft(tx);
        });
        rt.frame(1.0 / 60.0);
        type_text(&mut rt, " Edited here.");
        settle(&mut rt, 0.85);
        assert_eq!(status(&rt), format!("Saving to {host}…"));
        assert_eq!(std::fs::read_to_string(&mirror).unwrap(), "# Remote draft\n\nWritten on the server. Edited here.");
        wait_for(&mut rt, "the upload", |s| s.remote.status == RemoteStatus::Uploaded);
        assert_eq!(status(&rt), format!("Saved to {host}"));
        assert_eq!(srv.get("notes/draft.md"), "# Remote draft\n\nWritten on the server. Edited here.");
        assert!(!rt.state.draft.remote_upload_pending());

        // Saves while an upload runs: only the newest waits, and it is what lands.
        for word in [" one", " two", " three"] {
            rt.update(|s, tx| {
                let end = s.edit.text.len();
                s.edit.set_selection(end, end);
                s.edit.insert(word);
                s.text_did_change(tx);
                s.flush();
            });
        }
        assert!(rt.state.remote.uploader.in_flight);
        assert!(rt.state.remote.uploader.queued.as_ref().is_some_and(|(_, body)| body.ends_with("one two three")));
        wait_for(&mut rt, "the queued upload", |s| !s.remote.uploader.busy() && s.remote.status == RemoteStatus::Uploaded);
        assert!(srv.get("notes/draft.md").ends_with("Edited here. one two three"));

        // The server goes away: saves stay local and the footer says so.
        rt.update(|s, _| s.remote.ssh = srv.down());
        type_text(&mut rt, "!");
        wait_for(&mut rt, "the failed upload", |s| matches!(s.remote.status, RemoteStatus::Failed(_)));
        let (text, warning) = rt.state.status();
        assert!(text.starts_with(&format!("Not on {host} yet — {host}: ssh: connect to host 127.0.0.1 port 22230")), "{text}");
        assert!(warning && rt.state.chrome_visible);
        assert!(rt.state.draft.remote_upload_pending());
        rt.state.remote.status = RemoteStatus::Idle;
        assert_eq!(status(&rt), format!("Saved locally · not yet on {host}"));

        // Leaving with edits not on the server: a waiting sheet, then the warning.
        run(&mut rt, Command::NewDraft);
        assert!(rt.state.remote.leaving.is_some());
        assert!(rt.find("leave-wait").is_some());
        wait_for(&mut rt, "the leave warning", |s| s.alert().is_some());
        assert!(rt.find("leave-wait").is_none());
        let alert = rt.state.alert().cloned().unwrap();
        assert_eq!(alert.message, format!("Your latest edits aren't on {host} yet."));
        assert!(alert.informative.ends_with("\n\nThey're saved on this Mac and will be sent the next time you open this file."));
        assert_eq!(alert.buttons, ["Keep Editing", "Continue"]);
        assert!(alert.warning);
        click(&mut rt, "alert-button-0");
        assert_eq!(rt.state.draft.remote(), Some(&loc), "Keep Editing stays");
        run(&mut rt, Command::NewDraft);
        wait_for(&mut rt, "the leave warning", |s| s.alert().is_some());
        click(&mut rt, "alert-button-1");
        assert_eq!(rt.state.draft.remote(), None, "Continue leaves");
        assert!(rt.state.edit.text.is_empty());

        // Offline, a file opened before opens from its local copy.
        run(&mut rt, Command::RecentFiles);
        let row = rt.state.sidebar.items.iter().position(|i| i.remote.as_ref() == Some(&loc)).expect("the remote file is in Recent");
        assert_eq!(rt.state.sidebar.items[row].subtitle.as_deref(), Some(loc.display().as_str()));
        click(&mut rt, &crate::sidebar::row_key(row));
        wait_for(&mut rt, "the offline open", |s| s.draft.remote().is_some());
        rt.state.transient = None;
        assert_eq!(status(&rt), "Offline · editing the local copy");
        assert!(rt.state.edit.text.ends_with("three!"));

        // Back online, opening it again keeps the edits that never arrived and sends them.
        run(&mut rt, Command::NewDraft);
        wait_for(&mut rt, "leaving", |s| s.alert().is_some() || s.draft.remote().is_none());
        if rt.state.alert().is_some() {
            click(&mut rt, "alert-button-1");
        }
        srv.put("notes/draft.md", "changed on the server meanwhile");
        rt.update(|s, tx| {
            s.remote.ssh = srv.up();
            s.open_remote(loc.clone(), tx);
        });
        wait_for(&mut rt, "the reopen", |s| s.draft.remote().is_some());
        assert_eq!(rt.state.transient.as_deref(), Some(format!("Kept your edits that hadn't reached {host}").as_str()));
        assert!(rt.state.edit.text.ends_with("three!"));
        wait_for(&mut rt, "the catch-up upload", |s| s.remote.status == RemoteStatus::Uploaded);
        assert!(srv.get("notes/draft.md").ends_with("three!"));

        // Leaving right after an edit waits for the upload, then goes on by itself.
        type_text(&mut rt, "?");
        run(&mut rt, Command::NewDraft);
        assert!(rt.state.remote.leaving.is_some());
        wait_for(&mut rt, "leaving", |s| s.draft.remote().is_none());
        assert!(rt.state.alert().is_none());
        assert!(srv.get("notes/draft.md").ends_with("three!?"));

        // The Open picker fetches the server's copy of a remote file, as the sidebar does.
        srv.put("notes/draft.md", "newest from the server");
        run(&mut rt, Command::OpenDraftPicker);
        let i = rt.state.picker.rows().position(|d| d.remote.as_ref() == Some(&loc)).expect("listed");
        rt.update(|s, tx| {
            s.picker.selected = Some(i);
            s.picker_choose_selected(tx);
        });
        wait_for(&mut rt, "the picker's open", |s| s.draft.remote().is_some());
        assert_eq!(rt.state.edit.text, "newest from the server");

        // Opening the open remote file again only focuses it.
        rt.state.transient = None;
        rt.update(|s, tx| s.open_remote(loc.clone(), tx));
        assert_eq!(rt.state.transient, None);
    }

    #[test]
    fn ssh_failures_read_as_the_swift_wrote_them() {
        let Some(srv) = server("failures") else { return };
        let (mut rt, _dir) = app_with(&[("Local.md", "Local")]);
        rt.update(|s, _| s.remote.ssh = srv.up());
        let missing = srv.loc("nothing-here.md");
        ssh_prompt(&mut rt, &missing.display());
        wait_for(&mut rt, "the alert", |s| s.alert().is_some());
        assert_eq!(take_alert(&mut rt).message, format!("Nothing at {}.", missing.display()));

        ssh_prompt(&mut rt, "fwstranger:/tmp");
        wait_for(&mut rt, "the alert", |s| s.alert().is_some());
        assert_eq!(take_alert(&mut rt).message, "fwstranger isn't a known host yet. Run `ssh fwstranger` in Terminal once to trust it.");

        ssh_prompt(&mut rt, "fwbadkey:/tmp");
        wait_for(&mut rt, "the alert", |s| s.alert().is_some());
        assert_eq!(take_alert(&mut rt).message, "fwbadkey refused the login. Fluent Writer can't type passwords, so use an SSH key or ssh-agent (ssh-copy-id fwbadkey).");

        // A remote folder that can't be listed shows the error and keeps "..".
        srv.put("notes/sub/a.md", "a");
        rt.update(|s, _| s.set_sidebar_folder(crate::sidebar::FolderRoot::Remote(srv.loc("notes"))));
        rt.update(|s, tx| {
            if !s.sidebar_visible {
                s.toggle_sidebar(tx);
            }
        });
        wait_for(&mut rt, "the listing", |s| s.sidebar.message.is_none() && !s.sidebar.items.is_empty());
        srv.sh(&format!("rm -rf '{}/notes/sub'", srv.dir));
        click(&mut rt, "sidebar-row-0");
        wait_for(&mut rt, "the failed listing", |s| s.sidebar.message.as_deref().is_some_and(|m| !m.starts_with("Loading")));
        let message = rt.state.sidebar.message.clone().unwrap();
        assert!(message.starts_with(&format!("{}: ", srv.host)) && message.contains("sub"), "{message}");
        assert_eq!(rt.state.sidebar.items.len(), 1);

        // A listing that lands after the sidebar moved on is dropped.
        rt.update(|s, _| s.set_sidebar_folder(crate::sidebar::FolderRoot::Remote(srv.loc("notes"))));
        rt.update(|s, _| s.set_sidebar_mode(crate::sidebar::Mode::Recent));
        for _ in 0..60 {
            rt.frame(1.0 / 60.0);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let titles: Vec<String> = rt.state.sidebar.items.iter().map(|i| i.title.clone()).collect();
        assert_eq!(titles, ["Local"]);

        // A never-opened file that can't be read is an error, not an offline copy.
        rt.update(|s, tx| {
            s.remote.ssh = srv.down();
            s.open_remote(srv.loc("notes/new.md"), tx);
        });
        wait_for(&mut rt, "the alert", |s| s.alert().is_some());
        assert!(take_alert(&mut rt).message.contains("Connection refused"));
        assert_eq!(rt.state.draft.remote(), None);

        // The home folder is named after the host.
        rt.update(|s, _| s.remote.ssh = srv.up());
        ssh_prompt(&mut rt, &format!("{}:", srv.host));
        wait_for(&mut rt, "the home folder", |s| s.sidebar.folder_stack.first().is_some_and(|r| matches!(r, crate::sidebar::FolderRoot::Remote(l) if l.path == "~")));
        assert_eq!(rt.state.sidebar.header(), format!("{} ~", srv.host));
    }
}
