//! The title in the top bar and renaming the draft from it (the Swift title field in
//! `EditorController.swift` and `renameDraft` in `Commands.swift`).
//!
//! At rest the title is a label that truncates at the tail; a click, or File › Rename…, swaps in
//! a text field. Return, Tab or clicking away renames; Escape puts the title back.

use rune::{Align, Anim, El, Key, KeyEvent, TextEdit, Tx, text, text_input};
use writer_core::title_deriver;

use crate::app::{App, DRAFT};
use crate::theme::{Palette, ui_font};

pub const FIELD: &str = "title-field";

#[derive(Default)]
pub struct TitleField {
    pub edit: TextEdit,
    /// The field has the keyboard.
    pub editing: bool,
}

impl App {
    /// The title shown in the top bar: empty (the placeholder shows) for an untouched new draft.
    pub fn shown_title(&self) -> &str {
        if self.draft.title == title_deriver::UNTITLED && self.draft.body.is_empty() { "" } else { &self.draft.title }
    }

    /// Starts editing the title. File › Rename… selects all of it; a click puts the caret at
    /// the end. Either way the chrome stays up until the edit ends.
    pub fn begin_title_edit(&mut self, select_all: bool, tx: &mut Tx) {
        self.show_chrome(false, tx);
        if self.title.editing {
            if select_all {
                let len = self.title.edit.text.len();
                self.title.edit.set_selection(0, len);
            }
            return;
        }
        let mut edit = TextEdit::new(self.shown_title().to_string());
        let len = edit.text.len();
        edit.set_selection(if select_all { 0 } else { len }, len);
        edit.focused = true;
        self.title = TitleField { edit, editing: true };
        tx.focus(FIELD, false);
    }

    /// Ends the edit: renames when the title changed, then hands the keyboard back to the text.
    pub fn end_title_edit(&mut self, tx: &mut Tx) {
        if !self.title.editing {
            return;
        }
        self.title.editing = false;
        self.title.edit.focused = false;
        let new_title = self.title.edit.text.clone();
        if new_title != self.draft.title {
            self.sync_document_from_view();
            // A never-saved draft with text gets its file first, so the rename names a file.
            if self.draft.path().is_none() && !self.draft.body.is_empty() {
                self.flush();
            }
            if let Err(e) = self.library.rename(&mut self.draft, &new_title) {
                self.present_error(e);
            }
            // The comments and versions port moves the draft's notes to its new path here.
        }
        self.edit.focused = true;
        tx.focus(DRAFT, false);
        self.show_chrome(true, tx);
    }

    /// Escape: the title goes back to what it was and the keyboard to the text, without renaming.
    pub fn cancel_title_edit(&mut self, tx: &mut Tx) {
        let title = self.draft.title.clone();
        self.title.edit.replace_text(title);
        self.end_title_edit(tx);
    }

    /// Clicking away from the field ends the edit as Return does.
    pub fn poll_title(&mut self, tx: &mut Tx) -> bool {
        if self.title.editing && !self.title.edit.focused {
            self.end_title_edit(tx);
            return true;
        }
        false
    }
}

fn title_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    match ev.key {
        Key::Escape => {
            s.cancel_title_edit(tx);
            true
        }
        Key::Tab => {
            s.end_title_edit(tx);
            true
        }
        _ => false,
    }
}

/// The title label or field at `rect` (window points), fading with the chrome.
pub fn view(s: &App, p: &Palette, rect: [f32; 4], opacity: f32, fade: Anim) -> El {
    let font = ui_font(13.0, false);
    let color = if s.draft.has_manual_title { p.text.alpha(0.75) } else { p.quiet };
    let e = if s.title.editing {
        text_input(&s.title.edit, font)
            .key(FIELD)
            .label("Draft title")
            .single_line()
            .align(Align::Center)
            .color(color)
            .caret(p.caret, p.selection)
            .placeholder(title_deriver::UNTITLED, p.dimmed)
            .on_edit(|s: &mut App| &mut s.title.edit)
            .on_input_key(title_key)
            .on_submit(|s: &mut App, tx| s.end_title_edit(tx))
    } else {
        let shown = s.shown_title();
        let (t, c) = if shown.is_empty() { (title_deriver::UNTITLED, p.dimmed) } else { (shown, color) };
        text(t)
            .key("title")
            .role("textbox")
            .label("Draft title")
            .a11y_value(shown)
            .help("Click to rename")
            .font(font)
            .color(c)
            .align(Align::Center)
            .truncate()
            .on_press_tx(|s: &mut App, tx| s.begin_title_edit(false, tx))
    };
    e.absolute(rect[0], rect[1]).size(rect[2], rect[3]).opacity(opacity).opacity_motion(fade)
}
