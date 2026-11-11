//! The find bar above the draft, standing in for the bar `NSTextView` showed with
//! `usesFindBar` and incremental searching on: a search field with the match count, previous
//! and next, Done, an optional replace row (Replace, All) and the options AppKit's bar keeps
//! in its magnifying-glass menu (ignore case, wrap around, contains / starts with / full word).
//! Every match is highlighted in the draft and the current one is drawn distinct and scrolled
//! into view. The bar is drawn in the app's own look: IBM Plex Mono and `Palette`.

use std::ops::Range;

use rune::{Align, El, Key, KeyEvent, TextEdit, TextHighlight, Tx, col, el, row, text, text_input};

use crate::app::{App, DRAFT, Frame, TOP_BAR_HEIGHT};
use crate::theme::{Palette, ui_font};

/// The keys of the bar's fields, of the current match's highlight, and of the highlight over
/// the selection that Jump to Selection centres.
pub const FIND_FIELD: &str = "find-field";
pub const REPLACE_FIELD: &str = "replace-field";
pub const CURRENT: &str = "find-current";
pub const JUMP: &str = "find-jump";

/// One row of the bar.
pub const ROW_HEIGHT: f32 = 36.0;

/// How a match has to sit against the words around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Contains,
    StartsWith,
    FullWord,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Contains, Mode::StartsWith, Mode::FullWord];

    pub fn label(self) -> &'static str {
        match self {
            Mode::Contains => "Contains",
            Mode::StartsWith => "Starts With",
            Mode::FullWord => "Full Word",
        }
    }
}

/// The search as the options set it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub ignore_case: bool,
    pub wrap: bool,
    pub mode: Mode,
}

impl Default for Options {
    /// AppKit's defaults: ignore case, wrap around, contains.
    fn default() -> Options {
        Options { ignore_case: true, wrap: true, mode: Mode::Contains }
    }
}

pub struct Find {
    pub visible: bool,
    /// The replace row shows (Find and Replace…).
    pub replacing: bool,
    /// The options row shows.
    pub options_open: bool,
    pub query: TextEdit,
    pub replacement: TextEdit,
    pub options: Options,
    /// Replaces the match count until the search or the text changes ("3 replaced").
    pub notice: Option<String>,
}

impl Default for Find {
    fn default() -> Find {
        Find {
            visible: false,
            replacing: false,
            options_open: false,
            query: TextEdit::new(""),
            replacement: TextEdit::new(""),
            options: Options::default(),
            notice: None,
        }
    }
}

impl Find {
    /// The height the bar takes above the draft: nothing while it is hidden.
    pub fn height(&self) -> f32 {
        if !self.visible {
            return 0.0;
        }
        ROW_HEIGHT * (1 + self.replacing as usize + self.options_open as usize) as f32
    }
}

// MARK: Matching

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn same(a: char, b: char, ignore_case: bool) -> bool {
    a == b || (ignore_case && a.to_lowercase().eq(b.to_lowercase()))
}

/// Where `query` matches `text` under `options`, in order and not overlapping.
pub fn matches(text: &str, query: &str, options: Options) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    if query.is_empty() {
        return found;
    }
    let mut from = 0;
    while from < text.len() {
        let Some(end) = match_at(text, from, query, options.ignore_case) else {
            from += text[from..].chars().next().map_or(1, char::len_utf8);
            continue;
        };
        let before_ok = text[..from].chars().next_back().is_none_or(|c| !is_word(c));
        let after_ok = text[end..].chars().next().is_none_or(|c| !is_word(c));
        let fits = match options.mode {
            Mode::Contains => true,
            Mode::StartsWith => before_ok,
            Mode::FullWord => before_ok && after_ok,
        };
        if fits {
            found.push(from..end);
            from = end;
        } else {
            from += text[from..].chars().next().map_or(1, char::len_utf8);
        }
    }
    found
}

/// The end of `query` matched at byte `at` of `text`, if it matches there.
fn match_at(text: &str, at: usize, query: &str, ignore_case: bool) -> Option<usize> {
    let mut chars = text[at..].char_indices();
    let mut end = at;
    for q in query.chars() {
        let (i, c) = chars.next()?;
        if !same(c, q, ignore_case) {
            return None;
        }
        end = at + i + c.len_utf8();
    }
    Some(end)
}

/// The match to go to from the selection `sel`: the first starting at or after `from` going
/// forward, the last ending at or before it going back, wrapping round when `wrap`.
fn pick(found: &[Range<usize>], from: usize, forward: bool, wrap: bool) -> Option<Range<usize>> {
    let hit = if forward { found.iter().find(|m| m.start >= from) } else { found.iter().rev().find(|m| m.end <= from) };
    match hit {
        Some(m) => Some(m.clone()),
        None if wrap => if forward { found.first() } else { found.last() }.cloned(),
        None => None,
    }
}

fn selection(s: &App) -> Range<usize> {
    let (a, b) = s.edit.range();
    a.min(b)..a.max(b)
}

impl App {
    /// Every match of the bar's search in the draft.
    pub fn find_matches(&self) -> Vec<Range<usize>> {
        matches(&self.edit.text, &self.find.query.text, self.find.options)
    }
}

// MARK: Commands

/// Find… (and Find and Replace… with `replace`): shows the bar and puts the keyboard in its
/// search field with the search selected, as AppKit's bar does. The search is kept from the
/// last time, as AppKit keeps it on the find pasteboard.
pub fn show(s: &mut App, replace: bool, tx: &mut Tx) {
    s.find.visible = true;
    s.find.replacing = replace;
    let len = s.find.query.text.len();
    s.find.query.set_selection(0, len);
    tx.focus(FIND_FIELD, false);
}

/// Done and Escape: hides the bar and gives the keyboard back to the draft, which keeps the
/// current match selected.
pub fn close(s: &mut App, tx: &mut Tx) {
    s.find.visible = false;
    s.find.notice = None;
    tx.focus(DRAFT, false);
}

/// Selects `m` in the draft and scrolls it into view.
fn select(s: &mut App, m: Range<usize>, tx: &mut Tx) {
    s.edit.set_selection(m.start, m.end);
    s.edit.break_history_group();
    tx.scroll_into_view(CURRENT);
}

/// Find Next and Find Previous, from the selection. With no search they show the bar.
pub fn step(s: &mut App, forward: bool, tx: &mut Tx) {
    if s.find.query.text.is_empty() {
        show(s, s.find.replacing, tx);
        return;
    }
    s.find.notice = None;
    let found = s.find_matches();
    let sel = selection(s);
    let from = if forward { sel.end } else { sel.start };
    match pick(&found, from, forward, s.find.options.wrap) {
        Some(m) => select(s, m, tx),
        None => s.find.notice = Some("Not found".into()),
    }
}

/// Use Selection for Find: the selected text becomes the search. The bar stays as it is.
pub fn use_selection(s: &mut App) {
    let picked = s.edit.selected().to_string();
    if picked.is_empty() {
        return;
    }
    s.find.query.replace_text(picked);
    s.find.notice = None;
}

/// Jump to Selection: scrolls the selection to the middle of the editor.
pub fn jump_to_selection(tx: &mut Tx) {
    tx.scroll_center(JUMP);
}

/// The search changed: find from where the current selection starts, as incremental search
/// does, so each typed letter keeps the match in place while it still matches.
pub fn query_changed(s: &mut App, tx: &mut Tx) {
    s.find.notice = None;
    if s.find.query.text.is_empty() {
        return;
    }
    let found = s.find_matches();
    let from = selection(s).start;
    if let Some(m) = pick(&found, from, true, true) {
        select(s, m, tx);
    }
}

/// Replace: replaces the selected match, then goes to the next one. When the selection isn't
/// a match it only finds the next. One undo step.
pub fn replace(s: &mut App, tx: &mut Tx) {
    let found = s.find_matches();
    let sel = selection(s);
    if !found.contains(&sel) {
        step(s, true, tx);
        return;
    }
    let with = s.find.replacement.text.clone();
    let end = sel.start + with.len();
    s.apply(&writer_core::TextEdit::new(sel.clone(), with, end..end), tx);
    let found = s.find_matches();
    if let Some(m) = pick(&found, end, true, s.find.options.wrap) {
        select(s, m, tx);
    }
}

/// Replace All: every match in the draft, as one undo step. The caret goes after the last
/// replacement and the bar says how many it replaced.
pub fn replace_all(s: &mut App, tx: &mut Tx) {
    let found = s.find_matches();
    let (Some(first), Some(last)) = (found.first().cloned(), found.last().cloned()) else {
        s.find.notice = Some("Not found".into());
        return;
    };
    let with = &s.find.replacement.text;
    let mut middle = String::new();
    let mut at = first.start;
    for m in &found {
        middle.push_str(&s.edit.text[at..m.start]);
        middle.push_str(with);
        at = m.end;
    }
    let end = first.start + middle.len();
    let edit = writer_core::TextEdit::new(first.start..last.end, middle, end..end);
    s.apply(&edit, tx);
    s.find.notice = Some(format!("{} replaced", found.len()));
}

/// Return and Shift-Return go to the next and previous match, Escape closes the bar.
pub fn field_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let plain = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt;
    match ev.key {
        Key::Escape if plain => close(s, tx),
        Key::Enter if plain => step(s, !ev.mods.shift, tx),
        _ => return false,
    }
    true
}

/// Return in the replace field replaces the match and goes on to the next.
fn replace_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let plain = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt && !ev.mods.shift;
    match ev.key {
        Key::Escape if plain => close(s, tx),
        Key::Enter if plain => replace(s, tx),
        _ => return false,
    }
    true
}

// MARK: Drawing

/// The highlights the draft draws: every match, the current one (the selected match) distinct
/// and keyed for scrolling, and an invisible one over the selection for Jump to Selection.
pub fn highlights(s: &App, p: &Palette) -> Vec<TextHighlight> {
    let mut out = Vec::new();
    let sel = selection(s);
    if s.find.visible || !s.find.query.text.is_empty() {
        for m in s.find_matches() {
            if m == sel {
                out.push(TextHighlight::new(m, p.find_current).ring(p.caret, 1.0).key(CURRENT));
            } else if s.find.visible {
                out.push(TextHighlight::new(m, p.find_match));
            }
        }
    }
    let text = &s.edit.text;
    // An empty selection centres the character beside the caret, on the caret's line.
    let jump = if !sel.is_empty() {
        Some(sel)
    } else if let Some(c) = text[sel.start..].chars().next().filter(|c| *c != '\n') {
        Some(sel.start..sel.start + c.len_utf8())
    } else {
        text[..sel.start].chars().next_back().filter(|c| *c != '\n').map(|c| sel.start - c.len_utf8()..sel.start)
    };
    if let Some(r) = jump {
        out.push(TextHighlight::new(r, p.canvas.alpha(0.0)).key(JUMP));
    }
    out
}

fn label_for(s: &App, found: &[Range<usize>]) -> String {
    if let Some(n) = &s.find.notice {
        return n.clone();
    }
    if s.find.query.text.is_empty() {
        return String::new();
    }
    let sel = selection(s);
    match (found.len(), found.iter().position(|m| *m == sel)) {
        (0, _) => "Not found".into(),
        (n, Some(i)) => format!("{} of {n}", i + 1),
        (1, None) => "1 match".into(),
        (n, None) => format!("{n} matches"),
    }
}

/// A text button. Pressing it leaves the keyboard in the bar's field `home`, as AppKit's bar
/// buttons never take it, unless the action sends it somewhere else.
fn button(key: &'static str, title: &str, p: &Palette, enabled: bool, home: &'static str, on: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    let b = el()
        .key(key)
        .role("button")
        .label(title.trim())
        .h(24.0)
        .px(8.0)
        .radius(5.0)
        .items_center()
        .child(text(title).font(ui_font(12.0, false)).color(if enabled { p.icon } else { p.quiet.alpha(0.6) }));
    let press = move |s: &mut App, tx: &mut Tx| {
        on(s, tx);
        tx.return_focus(home, false);
    };
    if enabled { b.hover(p.text.alpha(0.06)).press(p.text.alpha(0.1)).on_press_tx(press) } else { b }
}

fn check(key: &'static str, title: &str, on: bool, p: &Palette, press: impl Fn(&mut App) + 'static) -> El {
    let press = move |s: &mut App, tx: &mut Tx| {
        press(s);
        tx.return_focus(FIND_FIELD, false);
    };
    let mark = el().w(12.0).h(12.0).radius(3.0).border(1.0, if on { p.caret } else { p.quiet }).fill(if on { p.caret } else { p.canvas });
    row()
        .key(key)
        .role("checkbox")
        .checked(on)
        .label(title)
        .h(24.0)
        .px(6.0)
        .gap(6.0)
        .radius(5.0)
        .items_center()
        .hover(p.text.alpha(0.06))
        .on_press_tx(press)
        .child(mark)
        .child(text(title).font(ui_font(12.0, false)).color(p.icon))
}

/// Room for the mode button, wide enough for "Replace", so both fields start together.
const MODE_WIDTH: f32 = 72.0;

/// The buttons right of a field, in a box as wide on both rows so the fields end together.
fn trailing() -> El {
    row().w(176.0).gap(4.0).items_center().justify_end()
}

fn field(input: El, p: &Palette, trailing: Option<El>) -> El {
    let mut f = row().h(26.0).px(8.0).gap(8.0).radius(6.0).items_center().grow(1.0).fill(p.card).border(1.0, p.hairline).child(input.grow(1.0));
    if let Some(t) = trailing {
        f = f.child(t);
    }
    f
}

/// The bar, laid over the editor's top edge below the top bar. Nothing while hidden.
pub fn view(s: &App, p: &Palette, f: &Frame) -> El {
    let h = s.find.height();
    let mut bar = col().key("find-bar").absolute(f.editor_x(), TOP_BAR_HEIGHT).size(f.editor_w, h).fill(p.canvas).border_sides([0.0, 0.0, 1.0, 0.0], p.hairline);
    if !s.find.visible {
        return bar;
    }
    // Escape closes the bar from its buttons too.
    bar = bar.on_key(|s: &mut App, ev: &KeyEvent, tx: &mut Tx| ev.key == Key::Escape && {
        close(s, tx);
        true
    });
    let found = s.find_matches();
    let any = !found.is_empty();
    let font = ui_font(13.0, false);
    let count = text(label_for(s, &found)).key("find-count").font(ui_font(12.0, false)).color(p.quiet).align(Align::End).label("Matches");
    let query = text_input(&s.find.query, font)
        .key(FIND_FIELD)
        .label("Find")
        .single_line()
        .placeholder("Find", p.quiet)
        .color(p.text)
        .caret(p.caret, p.selection)
        .on_edit(|s: &mut App| &mut s.find.query)
        .on_input_change(query_changed)
        .on_input_key(field_key);
    // The mode button names the bar's mode, as AppKit's Find / Replace pop-up does; a press
    // switches it.
    let mode = if s.find.replacing { "Replace" } else { "Find" };
    let find_row = row()
        .key("find-row")
        .h(ROW_HEIGHT)
        .px(12.0)
        .gap(4.0)
        .items_center()
        .child(button("find-mode", mode, p, true, FIND_FIELD, |s, tx| {
            s.find.replacing = !s.find.replacing;
            tx.focus(if s.find.replacing { REPLACE_FIELD } else { FIND_FIELD }, false);
        })
        .w(MODE_WIDTH)
        .help(if s.find.replacing { "Find only" } else { "Find and replace" }))
        .child(field(query, p, Some(count)))
        .child(
            trailing()
                .child(button("find-previous", "‹", p, any, FIND_FIELD, |s, tx| step(s, false, tx)).label("Previous"))
                .child(button("find-next", "›", p, any, FIND_FIELD, |s, tx| step(s, true, tx)).label("Next"))
                .child(button("find-options", "Options", p, true, FIND_FIELD, |s, _| s.find.options_open = !s.find.options_open))
                .child(button("find-done", "Done", p, true, FIND_FIELD, close)),
        );
    bar = bar.child(find_row);
    if s.find.replacing {
        let with = text_input(&s.find.replacement, font)
            .key(REPLACE_FIELD)
            .label("Replace")
            .single_line()
            .placeholder("Replace", p.quiet)
            .color(p.text)
            .caret(p.caret, p.selection)
            .on_edit(|s: &mut App| &mut s.find.replacement)
            .on_input_change(|s: &mut App, _| s.find.notice = None)
            .on_input_key(replace_key);
        bar = bar.child(
            row()
                .key("replace-row")
                .h(ROW_HEIGHT)
                .px(12.0)
                .gap(4.0)
                .items_center()
                .child(el().w(MODE_WIDTH))
                .child(field(with, p, None))
                .child(
                    trailing()
                        .child(button("replace-one", "Replace", p, any, REPLACE_FIELD, replace))
                        .child(button("replace-all", "All", p, any, REPLACE_FIELD, replace_all).label("Replace All")),
                ),
        );
    }
    if s.find.options_open {
        let o = s.find.options;
        let mut options = row()
            .key("find-options-row")
            .h(ROW_HEIGHT)
            .px(12.0)
            .gap(2.0)
            .items_center()
            .child(check("find-ignore-case", "Ignore Case", o.ignore_case, p, |s| s.find.options.ignore_case = !s.find.options.ignore_case))
            .child(check("find-wrap", "Wrap Around", o.wrap, p, |s| s.find.options.wrap = !s.find.options.wrap))
            .child(el().w(1.0).h(16.0).ml(6.0).mr(6.0).fill(p.hairline));
        for mode in Mode::ALL {
            let key = match mode {
                Mode::Contains => "find-contains",
                Mode::StartsWith => "find-starts-with",
                Mode::FullWord => "find-full-word",
            };
            options = options.child(check(key, mode.label(), o.mode == mode, p, move |s| s.find.options.mode = mode).role("radio"));
        }
        bar = bar.child(options);
    }
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(ignore_case: bool, mode: Mode) -> Options {
        Options { ignore_case, wrap: true, mode }
    }

    #[test]
    fn matching_follows_the_options() {
        let t = "The cat scattered; the Cat's catalogue.";
        assert_eq!(matches(t, "cat", o(true, Mode::Contains)).len(), 4);
        assert_eq!(matches(t, "cat", o(false, Mode::Contains)).len(), 3);
        assert_eq!(matches(t, "cat", o(true, Mode::StartsWith)), vec![4..7, 23..26, 29..32]);
        assert_eq!(matches(t, "cat", o(true, Mode::FullWord)), vec![4..7, 23..26]);
        assert_eq!(matches("aaaa", "aa", o(true, Mode::Contains)), vec![0..2, 2..4]);
        assert_eq!(matches("Ünïcode ünïcode", "ÜNÏ", o(true, Mode::Contains)), vec![0..5, 10..15]);
        assert!(matches("abc", "", o(true, Mode::Contains)).is_empty());
    }

    #[test]
    fn picking_wraps_only_when_asked() {
        let found = vec![2..4, 10..12];
        assert_eq!(pick(&found, 5, true, false), Some(10..12));
        assert_eq!(pick(&found, 12, true, false), None);
        assert_eq!(pick(&found, 12, true, true), Some(2..4));
        assert_eq!(pick(&found, 2, false, true), Some(10..12));
        assert_eq!(pick(&found, 10, false, false), Some(2..4));
    }
}
