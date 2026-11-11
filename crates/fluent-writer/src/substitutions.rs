//! Edit › Substitutions: Smart Quotes, Smart Dashes and Text Replacement, which `NSTextView`
//! applied as you typed. The Swift app turned quotes and dashes off and text replacement on
//! (`EditorController.configureTextView`); here the three are settings in `Prefs`.
//!
//! Text replacement uses the system's list, which only macOS has: the shortcuts people set in
//! System Settings › Keyboard › Text Replacements, mirrored into the global defaults under
//! `NSUserDictionaryReplacementItems`. Windows and Linux have no such list, so there the
//! setting has nothing to apply.

use std::sync::OnceLock;

use rune::{Key, KeyEvent, Tx};

use crate::app::App;

static SYSTEM: OnceLock<Vec<(String, String)>> = OnceLock::new();

/// Starts reading the system's text replacements on a thread, at launch. Until it finishes
/// (and on Windows and Linux) the list is empty.
pub fn load_system_replacements() {
    #[cfg(target_os = "macos")]
    std::thread::spawn(|| {
        let out = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("/usr/bin/defaults export -g - | /usr/bin/plutil -extract NSUserDictionaryReplacementItems json -o - -")
            .output();
        let list = out.ok().filter(|o| o.status.success()).map(|o| parse_replacements(&o.stdout)).unwrap_or_default();
        let _ = SYSTEM.set(list);
    });
}

/// The system's replacements as (shortcut, phrase), longest shortcut first.
pub fn system_replacements() -> &'static [(String, String)] {
    SYSTEM.get().map_or(&[], Vec::as_slice)
}

/// Reads `NSUserDictionaryReplacementItems` as JSON: `[{"on": 1, "replace": "omw", "with": "On my way!"}]`.
/// Items turned off are left out.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn parse_replacements(json: &[u8]) -> Vec<(String, String)> {
    #[derive(serde::Deserialize)]
    struct Item {
        replace: Option<String>,
        with: Option<String>,
        on: Option<serde_json::Value>,
    }
    let items: Vec<Item> = serde_json::from_slice(json).unwrap_or_default();
    let mut list: Vec<(String, String)> = items
        .into_iter()
        .filter(|i| i.on.as_ref().is_none_or(|v| v.as_bool().unwrap_or(true) && v.as_i64() != Some(0)))
        .filter_map(|i| Some((i.replace?, i.with?)))
        .filter(|(r, _)| !r.is_empty())
        .collect();
    list.sort_by_key(|(r, _)| std::cmp::Reverse(r.len()));
    list
}

/// The curly quote AppKit types for a straight `typed` quote at the end of `before`: an
/// opening one at the start, after white space or an opening bracket or quote; a closing one
/// (which is also the apostrophe) anywhere else.
pub fn smart_quote(before: &str, typed: char) -> Option<char> {
    let opening = before.chars().next_back().is_none_or(|c| c.is_whitespace() || "([{<“‘«—–".contains(c));
    match typed {
        '"' => Some(if opening { '“' } else { '”' }),
        '\'' => Some(if opening { '‘' } else { '’' }),
        _ => None,
    }
}

/// The double hyphen ending `before` that becomes an em dash once something other than a
/// hyphen is typed after it: its byte range. Three or more hyphens are left alone.
pub fn dash_before(before: &str) -> Option<std::ops::Range<usize>> {
    let start = before.len().checked_sub(2)?;
    (before.ends_with("--") && !before[..start].ends_with('-')).then_some(start..before.len())
}

/// Characters that end a word and so trigger a text replacement.
fn is_delimiter(c: char) -> bool {
    c.is_whitespace() || ".,;:!?)]}\"'”’".contains(c)
}

/// The replacement whose shortcut ends `before` and starts a word there: its byte range and
/// phrase.
pub fn replacement_before<'a>(before: &str, list: &'a [(String, String)]) -> Option<(std::ops::Range<usize>, &'a str)> {
    list.iter().find_map(|(shortcut, phrase)| {
        let start = before.len().checked_sub(shortcut.len())?;
        if !before.is_char_boundary(start) || &before[start..] != shortcut {
            return None;
        }
        let starts_word = before[..start].chars().next_back().is_none_or(|c| c.is_whitespace() || !shortcut.starts_with(char::is_alphanumeric) || !c.is_alphanumeric());
        starts_word.then_some((start..before.len(), phrase.as_str()))
    })
}

/// Applies the substitutions that are on to a character about to be typed into the draft.
/// Returns true when it typed the character itself.
pub fn on_typed(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let Key::Text(t) = &ev.key else { return false };
    if ev.mods.cmd || ev.mods.ctrl || s.edit.is_composing() {
        return false;
    }
    let mut chars = t.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else { return false };
    let (a, b) = s.edit.range();
    let caret = a.min(b);
    let mut changed = false;
    // Replacements and dashes act on finished text, so only with nothing selected.
    if a == b {
        let before = &s.edit.text[..caret];
        let swap = if s.prefs.text_replacement() && is_delimiter(c) {
            replacement_before(before, system_replacements()).map(|(r, w)| (r, w.to_string()))
        } else {
            None
        };
        let swap = swap.or_else(|| (s.prefs.smart_dashes() && c != '-').then(|| dash_before(before)).flatten().map(|r| (r, "—".to_string())));
        if let Some((range, with)) = swap {
            let end = range.start + with.len();
            s.apply(&writer_core::TextEdit::new(range, with, end..end), crate::editing::TYPING, tx);
            changed = true;
        }
    }
    let quote = if s.prefs.smart_quotes() { smart_quote(&s.edit.text[..s.edit.range().0], c) } else { None };
    if !changed && quote.is_none() {
        return false;
    }
    let typed = quote.map_or_else(|| t.clone(), String::from);
    s.edit.key(&KeyEvent { key: Key::Text(typed), mods: ev.mods }, None);
    s.text_did_change(tx);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_open_after_space_and_close_after_letters() {
        assert_eq!(smart_quote("", '"'), Some('“'));
        assert_eq!(smart_quote("He said ", '"'), Some('“'));
        assert_eq!(smart_quote("He said “hi", '"'), Some('”'));
        assert_eq!(smart_quote("don", '\''), Some('’'));
        assert_eq!(smart_quote("(", '\''), Some('‘'));
        assert_eq!(smart_quote("a", 'x'), None);
    }

    #[test]
    fn two_hyphens_make_a_dash_three_do_not() {
        assert_eq!(dash_before("wait--"), Some(4..6));
        assert_eq!(dash_before("wait---"), None);
        assert_eq!(dash_before("wait-"), None);
    }

    #[test]
    fn replacements_parse_and_match_whole_shortcuts() {
        let json = r#"[{"on":1,"replace":"omw","with":"On my way!"},{"on":0,"replace":"off","with":"no"},{"replace":";shrug","with":"¯\\_(ツ)_/¯"}]"#;
        let list = parse_replacements(json.as_bytes());
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].0, ";shrug");
        assert_eq!(replacement_before("I'm omw", &list), Some((4..7, "On my way!")));
        assert_eq!(replacement_before("roomw", &list), None);
        assert_eq!(replacement_before("ok;shrug", &list).map(|(r, _)| r), Some(2..8));
    }
}
