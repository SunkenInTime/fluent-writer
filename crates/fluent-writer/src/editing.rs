//! Keys a Mac text view handles that Rune's editor doesn't, or handles another way, built on
//! the draft's key handler. The rest of the standard keys (arrows, ⌥/⌘ arrows, Shift to
//! extend, ⌃B ⌃F ⌃N ⌃P ⌃D ⌃H, word and line deletion, Page Up and Down) are Rune's own.
//!
//! - Escape leaves the keyboard in the draft, as `NSTextView` does; Rune's editor would let go
//!   of it.
//! - macOS `StandardKeyBinding`: ⌃A and ⌃E go to the paragraph's start and end (Rune goes to
//!   the line's), ⌃K deletes to the paragraph's end into a kill buffer that ⌃Y pastes back,
//!   ⌃T transposes, ⌃O opens a line, ⌃L centres the selection, and Home and End scroll to the
//!   top and bottom without moving the caret (Shift-Home and Shift-End select, in Rune).
//! - ⌥↑ and ⌥↓ on macOS, Ctrl+↑ and Ctrl+↓ elsewhere, go by paragraph.

use rune::{Key, KeyEvent, TextEdit, Tx, WordMotion};

use crate::app::{App, SCROLL};

/// A text field's editor that moves and deletes by word as AppKit's text views do: ⌥←/→ and
/// ⌥⌫ (Ctrl off macOS) stop at Unicode word boundaries, so "one, two" stops after "one"
/// rather than after the comma. Every field in the app starts from this.
pub fn text_edit(text: impl Into<String>) -> TextEdit {
    TextEdit::new(text.into()).word_motion(WordMotion::Unicode)
}

/// The undo name AppKit gives the edits its key bindings make (⌃K, ⌃Y, ⌃T, ⌃O, a list
/// continued on Return): "Undo Typing". `EditorController.swift:740`
pub const TYPING: &str = "Typing";

/// What the editing keys keep between presses.
#[derive(Default)]
pub struct Editing {
    /// The text ⌃K last deleted, for ⌃Y.
    pub kill: String,
}

const MAC: bool = cfg!(target_os = "macos");

fn para_start(text: &str, i: usize) -> usize {
    text[..i].rfind('\n').map_or(0, |n| n + 1)
}

fn para_end(text: &str, i: usize) -> usize {
    text[i..].find('\n').map_or(text.len(), |n| i + n)
}

fn char_after(text: &str, i: usize) -> usize {
    text[i..].chars().next().map_or(i, |c| i + c.len_utf8())
}

fn char_before(text: &str, i: usize) -> usize {
    text[..i].chars().next_back().map_or(i, |c| i - c.len_utf8())
}

/// Where ⌥↑ (`forward` false) or ⌥↓ takes the caret from `i`: the start of its paragraph, or
/// of the one before when it is already there; the end of its paragraph, or of the next.
pub fn paragraph_motion(text: &str, i: usize, forward: bool) -> usize {
    if forward {
        para_end(text, if text[i..].starts_with('\n') { i + 1 } else { i })
    } else {
        let start = para_start(text, i);
        if start == i && i > 0 { para_start(text, i - 1) } else { start }
    }
}

/// ⌃T: the edit that swaps the characters around the caret (the two before it at a
/// paragraph's end), leaving the caret after them.
pub fn transpose(text: &str, i: usize) -> Option<writer_core::TextEdit> {
    let at_end = i == text.len() || text[i..].starts_with('\n');
    let (a, mid, b) = if at_end {
        let mid = char_before(text, i);
        (char_before(text, mid), mid, i)
    } else {
        (char_before(text, i), i, char_after(text, i))
    };
    if a == mid || mid == b || text[a..b].contains('\n') {
        return None;
    }
    let swapped = format!("{}{}", &text[mid..b], &text[a..mid]);
    Some(writer_core::TextEdit::new(a..b, swapped, b..b))
}

/// The character a macOS Control-letter chord names, if this is one.
fn control_letter(ev: &KeyEvent) -> Option<char> {
    let m = ev.mods;
    if !MAC || !m.ctrl || m.cmd || m.alt {
        return None;
    }
    let Key::Text(t) = &ev.key else { return None };
    let mut chars = t.chars();
    let c = chars.next()?.to_ascii_lowercase();
    chars.next().is_none().then_some(c)
}

fn move_to(s: &mut App, to: usize, extend: bool) {
    let anchor = if extend { s.edit.anchor } else { to };
    s.edit.set_selection(anchor, to);
    s.edit.break_history_group();
}

pub fn draft_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let m = ev.mods;
    let none = !m.cmd && !m.ctrl && !m.alt && !m.shift;
    if ev.key == Key::Escape && none {
        return true;
    }
    let (a, b) = s.edit.range();
    let focus = s.edit.focus.min(s.edit.text.len());
    if let Some(c) = control_letter(ev) {
        let text = &s.edit.text;
        match c {
            'a' => move_to(s, para_start(text, focus), m.shift),
            'e' => move_to(s, para_end(text, focus), m.shift),
            'k' => {
                let range = if a != b {
                    a..b
                } else if text[a..].starts_with('\n') {
                    a..a + 1
                } else {
                    a..para_end(text, a)
                };
                if range.is_empty() {
                    return true;
                }
                s.editing.kill = text[range.clone()].to_string();
                s.apply(&writer_core::TextEdit::new(range.clone(), "", range.start..range.start), TYPING, tx);
            }
            'y' => {
                if !s.editing.kill.is_empty() {
                    let kill = s.editing.kill.clone();
                    let end = a + kill.len();
                    s.apply(&writer_core::TextEdit::new(a..b, kill, end..end), TYPING, tx);
                }
            }
            't' => {
                if a == b && let Some(edit) = transpose(text, a) {
                    s.apply(&edit, TYPING, tx);
                }
            }
            'o' => s.apply(&writer_core::TextEdit::new(a..b, "\n", a..a), TYPING, tx),
            'l' => crate::find::jump_to_selection(tx),
            _ => return false,
        }
        return true;
    }
    let by_paragraph = if MAC { m.alt && !m.cmd && !m.ctrl } else { m.ctrl && !m.alt };
    if by_paragraph && matches!(ev.key, Key::Up | Key::Down) {
        let forward = ev.key == Key::Down;
        let from = if m.shift || a == b { focus } else if forward { b } else { a };
        let to = paragraph_motion(&s.edit.text, from, forward);
        move_to(s, to, m.shift);
        return true;
    }
    if MAC && none && matches!(ev.key, Key::Home | Key::End) {
        tx.scroll_to_y(SCROLL, if ev.key == Key::Home { 0.0 } else { f32::MAX });
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraph_motion_goes_to_the_edges_then_on() {
        let t = "one two\nthree\n\nfour";
        assert_eq!(paragraph_motion(t, 5, false), 0);
        assert_eq!(paragraph_motion(t, 8, false), 0);
        assert_eq!(paragraph_motion(t, 10, false), 8);
        assert_eq!(paragraph_motion(t, 2, true), 7);
        assert_eq!(paragraph_motion(t, 7, true), 13);
        assert_eq!(paragraph_motion(t, 13, true), 14);
        assert_eq!(paragraph_motion(t, 14, true), 19);
    }

    #[test]
    fn transpose_swaps_around_the_caret_or_before_a_line_end() {
        let apply = |t: &str, i| transpose(t, i).map(|e| (e.applied(t), e.selection_after.start));
        assert_eq!(apply("abcd", 2), Some(("acbd".into(), 3)));
        assert_eq!(apply("abcd", 4), Some(("abdc".into(), 4)));
        assert_eq!(apply("ab\ncd", 2), Some(("ba\ncd".into(), 2)));
        assert_eq!(apply("a", 1), None);
        assert_eq!(apply("abc", 0), None);
        assert_eq!(apply("a\nb", 2), None);
    }
}
