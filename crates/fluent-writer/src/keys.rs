//! Keys the draft's text field handles before its editor does.

use rune::{Key, KeyEvent, Tx};
use writer_core::text_editing;

use crate::app::App;

/// Return continues lists, Tab and Shift-Tab indent them. Every key hides the chrome.
pub fn draft_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if crate::menubar::key(s, ev, tx) {
        return true;
    }
    if matches!(ev.key, Key::Modifier { .. }) {
        return false;
    }
    if let Some(command) = crate::commands::command_for(ev, tx) {
        s.run(command, tx);
        return true;
    }
    if ev.key == Key::Escape && s.clear_active_comment() {
        return true;
    }
    if ev.key == Key::Escape && s.assist_visible {
        s.assist_close(tx);
        return true;
    }
    s.user_is_typing();
    if crate::editing::draft_key(s, ev, tx) || crate::substitutions::on_typed(s, ev, tx) {
        return true;
    }
    let (a, b) = s.edit.range();
    let sel = a.min(b)..a.max(b);
    let plain = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt;
    // A continued list is a "Typing" step; Tab and Shift-Tab are "Indent" and "Outdent".
    // `EditorController.swift:738-752`
    let edit = match ev.key {
        Key::Enter if plain && !ev.mods.shift => text_editing::newline_edit(&s.edit.text, sel).map(|e| (e, crate::editing::TYPING)),
        Key::Tab if plain => text_editing::indent_edit(&s.edit.text, sel, ev.mods.shift).map(|e| (e, if ev.mods.shift { "Outdent" } else { "Indent" })),
        _ => None,
    };
    match edit {
        Some((edit, name)) => {
            s.apply(&edit, name, tx);
            true
        }
        None => false,
    }
}

/// Keys that reach the window: shortcuts while nothing has focus, or that a field passed on.
pub fn window_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if crate::menubar::key(s, ev, tx) {
        return true;
    }
    if s.preview.is_active() {
        return crate::version_preview::preview_key(s, ev, tx);
    }
    if ev.key == Key::Escape && s.find.visible {
        crate::find::close(s, tx);
        return true;
    }
    match crate::commands::command_for(ev, tx) {
        Some(command) => {
            s.run(command, tx);
            true
        }
        None => false,
    }
}
