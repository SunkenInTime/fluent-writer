//! Opening, Save As, and leaving a draft: the checks before a new draft, another file, closing
//! the window or quitting (the Swift `confirmLeavingDocument`, `saveAs`, `open` and
//! `browseForFile`, and the app delegate's quit).

use std::path::{Component, Path, PathBuf};

use rune::{CloseRequest, FileDialogOptions, Tx};
use writer_core::draft_store::FILE_EXTENSIONS;
use writer_core::{atomic_file, title_deriver};

use crate::app::{App, DRAFT};
use crate::ui::Alert;

/// What runs once the open draft may be left.
pub type Then = Box<dyn FnOnce(&mut App, &mut Tx)>;

#[derive(Default)]
pub struct Documents {
    /// Waits on an alert of the leave check: the "couldn't be saved" one (runs after Continue,
    /// or after Save As succeeds) or the "aren't on <host> yet" one (runs after Continue).
    pub(crate) pending: Option<Then>,
    /// The leave check passed for a quit already, so closing the window doesn't ask again.
    pub quitting: bool,
}

impl App {
    /// The leave check before the draft is replaced (New, Open, the Open picker, a sidebar
    /// click, open over SSH, Close, Quit), then `then`. The Swift `confirmLeavingDocument`:
    ///
    /// 1. Save. If that works, a remote file's unsent edits are uploaded first
    ///    ([`App::confirm_remote_uploaded`]: the "Saving to <host>…" sheet, and a warning with
    ///    Keep Editing / Continue if the upload fails), then `then` runs.
    /// 2. If the save fails, ask: Keep Editing drops `then`, Save As… runs it once the copy is
    ///    written, Continue runs it anyway. Neither uploads, as in the Swift.
    pub fn leave_document(&mut self, tx: &mut Tx, then: impl FnOnce(&mut App, &mut Tx) + 'static) {
        if self.flush() {
            self.confirm_remote_uploaded(Box::new(then), tx);
            return;
        }
        let message = match &self.save_state {
            crate::app::SaveState::Failed(m) => m.clone(),
            _ => String::new(),
        };
        self.documents.pending = Some(Box::new(then));
        self.show_alert(
            Alert::new("This draft couldn't be saved.")
                .informative(format!("{message}\n\nA recovery copy is kept and will be offered next time."))
                .warning()
                .button("Keep Editing")
                .button("Save As…")
                .button("Continue")
                .on_answer(|s, button, tx| {
                    let then = s.documents.pending.take();
                    match button {
                        1 => s.save_as(tx, then),
                        2 => {
                            if let Some(then) = then {
                                then(s, tx);
                            }
                        }
                        _ => {}
                    }
                }),
        );
    }

    /// File › New: an empty, never-saved draft just takes the keyboard; anything else is left
    /// for a new empty draft.
    pub fn new_draft(&mut self, tx: &mut Tx) {
        if self.draft.is_empty_untitled() {
            self.focus_draft(tx);
            return;
        }
        self.leave_document(tx, |s, _| {
            let doc = s.library.new_draft();
            s.load(doc);
        });
    }

    pub fn focus_draft(&mut self, tx: &mut Tx) {
        self.edit.focused = true;
        tx.focus(DRAFT, false);
    }

    /// File › Open Other File…: an open dialog for Markdown and text files, starting in the
    /// drafts folder.
    pub fn browse_for_file(&mut self, tx: &mut Tx) {
        let options = FileDialogOptions { directory: Some(self.library.drafts_directory().to_path_buf()), ..Default::default() }.filter("Text", &FILE_EXTENSIONS);
        tx.open_file(options, |s: &mut App, answer, tx| match answer {
            Ok(Some(path)) => s.open(&path, tx),
            Ok(None) => {}
            Err(e) => s.present_error(e),
        });
    }

    /// Opens the file at `path` in place of the draft. The file already open just takes the
    /// keyboard; a file that can't be read says why.
    pub fn open(&mut self, path: &Path, tx: &mut Tx) {
        let path = standardize(path);
        if self.draft.path().is_some_and(|p| standardize(p) == path) {
            self.focus_draft(tx);
            return;
        }
        self.leave_document(tx, move |s, tx| match s.library.open(&path) {
            Ok(doc) => {
                s.load(doc);
                s.focus_draft(tx);
                // The system's recent-documents list (the Dock menu) has no Rune API yet.
            }
            Err(e) => s.present_error(e),
        });
    }

    /// File › Save As…: a save dialog for a `.md` file named after the title, starting in the
    /// drafts folder. The text is written there and that file replaces the draft, keeping the
    /// selection; the old file stays where it was. `then` runs only when the copy was written.
    pub fn save_as(&mut self, tx: &mut Tx, then: Option<Then>) {
        let name = format!("{}.md", title_deriver::file_name_stem(&self.draft.title));
        let options = FileDialogOptions { directory: Some(self.library.drafts_directory().to_path_buf()), file_name: Some(name), ..Default::default() }.filter("Markdown", &["md"]);
        tx.save_file(options, move |s: &mut App, answer, tx| match answer {
            Ok(Some(path)) => {
                if s.write_copy(&path) {
                    s.focus_draft(tx);
                    if let Some(then) = then {
                        then(s, tx);
                    }
                }
            }
            Ok(None) => {}
            Err(e) => s.present_error(e),
        });
    }

    /// Writes the text to `path` and opens it in place of the draft. False after an error,
    /// which it shows.
    fn write_copy(&mut self, path: &Path) -> bool {
        let mut path = path.to_path_buf();
        // The Mac's panel only names `.md` files; the Windows and Linux dialogs can return a name
        // without it.
        if path.extension().is_none_or(|e| e != "md") {
            let mut name = path.file_name().unwrap_or_default().to_os_string();
            name.push(".md");
            path.set_file_name(name);
        }
        self.sync_document_from_view();
        let selection = self.draft.selection.clone();
        let result = atomic_file::write(self.draft.body.as_bytes(), &path).map_err(writer_core::Error::from).and_then(|_| self.library.open(&path));
        match result {
            Ok(mut doc) => {
                let len = doc.body.len();
                doc.selection = selection.start.min(len)..selection.end.min(len);
                self.library.discard_recovery(self.draft.id());
                self.load(doc);
                true
            }
            Err(e) => {
                self.present_error(e);
                false
            }
        }
    }

    /// File › Close, File › Quit, and the window's close button: the leave check once, then the
    /// app ends (it has one window). The Swift asked twice when the save failed, once for the
    /// window and again for the quit; here a quit that passed the check doesn't ask again.
    pub fn quit(&mut self, tx: &mut Tx) {
        if self.documents.quitting {
            tx.quit();
            return;
        }
        // A running Assist request stops first, even when the check then keeps the app open.
        // `main.swift:38-42`
        self.assist_stop();
        self.leave_document(tx, |s, tx| {
            s.documents.quitting = true;
            s.app_will_quit();
            tx.quit();
        });
    }

    /// The last things before the app ends: the window's frame, the caret, and the draft's
    /// notes, with the live-state file deleted (`applicationWillTerminate`).
    pub fn app_will_quit(&mut self) {
        self.save_window_frame();
        self.library.remember_selection(&self.draft);
        self.notes.app_will_terminate();
    }

    /// Rune's `on_close_request`: the user asked to close the window (its close button, Alt+F4,
    /// ⌘W nothing took) or, on macOS, to quit from the Dock or the app menu. The leave check
    /// runs first, as `windowShouldClose` ran it (`EditorController.swift:841-843`): when the
    /// draft is saved it quits (`tx.quit`); when the save failed the window stays up, as it
    /// was, with the alert, and Continue or a successful Save As… quits from there.
    pub fn close_requested(&mut self, _why: CloseRequest, tx: &mut Tx) -> bool {
        if self.documents.quitting {
            return true;
        }
        self.quit(tx);
        false
    }

    /// Rune's `on_close`: the window is closing (after the leave check let it, or because the
    /// app closed it). The app quits with it; a check that already passed doesn't ask again.
    pub fn window_closing(&mut self, tx: &mut Tx) {
        self.quit(tx);
    }

    /// Rune's `on_quit`: the system is quitting the app (macOS Quit from the Dock, logging out,
    /// a Windows shutdown). It can't be refused here, so save what can be saved.
    pub fn app_quitting(&mut self, _tx: &mut Tx) {
        if !self.documents.quitting {
            self.documents.quitting = true;
            self.flush();
            self.app_will_quit();
        }
    }
}

/// `path` made absolute with `.` and `..` resolved, without touching the disk, as Foundation's
/// `standardizedFileURL` does.
pub fn standardize(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir().map(|d| d.join(path)).unwrap_or_else(|_| path.to_path_buf()) };
    let mut out = PathBuf::new();
    for c in absolute.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
