//! Keys the draft's text field handles before its editor does.

use rune::{Key, KeyEvent, Tx};
use writer_core::text_editing;

use crate::app::App;

/// Return continues lists, Tab and Shift-Tab indent them. Every key hides the chrome.
pub fn draft_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    s.user_is_typing();
    let (a, b) = s.edit.range();
    let sel = a.min(b)..a.max(b);
    let plain = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt;
    let edit = match ev.key {
        Key::Enter if plain && !ev.mods.shift => text_editing::newline_edit(&s.edit.text, sel),
        Key::Tab if plain => text_editing::indent_edit(&s.edit.text, sel, ev.mods.shift),
        _ => None,
    };
    match edit {
        Some(edit) => {
            s.apply(&edit, tx);
            true
        }
        None => false,
    }
}
