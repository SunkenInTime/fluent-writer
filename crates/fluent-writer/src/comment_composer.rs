//! The popover for writing, or editing, a comment on a span of the draft (Swift
//! `CommentComposer.swift`): Add Comment… (⌥⌘M), the toolbar button, the Comments tab button and
//! the editor's context menu all open it.

use std::ops::Range;
use std::time::Duration;

use rune::{Color, El, Icon, Key, ScrollSpec, Shadow, TextEdit, Tx, col, el, icon, text, text_input};

use crate::app::{App, DRAFT};
use crate::commands::Command;
use crate::notes::shortcut_label;
use crate::theme::{Palette, rgba, ui_font};
use crate::version_preview::{fitted_button_width, small_button};

/// The comment field's key.
pub const COMPOSER_FIELD: &str = "comment-composer-field";
/// The popover's content size. `CommentComposer.swift:29-30`
pub const WIDTH: f32 = 320.0;
pub const HEIGHT: f32 = 150.0;
/// The popover's arrow: how far it reaches out of the content towards the text.
const ARROW: f32 = 11.0;
/// Frames to wait for the editor to draw the caret at the start of the target, which the
/// popover points at.
const ANCHOR_FRAMES: u8 = 2;

/// What saving the composer does.
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code, reason = "Edit comes from the Assist panel's comment cards")]
pub enum Target {
    /// A new comment on this byte range of the draft.
    New(Range<usize>),
    /// New text for the comment with this id.
    Edit(String),
}

#[derive(Clone, Debug)]
pub struct Open {
    /// The commented text as the quote line shows it.
    pub quote: String,
    pub range: Range<usize>,
    pub target: Target,
    /// The first character of the target, relative to the draft field (so it scrolls with it).
    pub anchor: [f32; 4],
}

#[derive(Clone, Debug)]
struct Pending {
    open: Open,
    initial: String,
    frames: u8,
}

#[derive(Default)]
pub struct Composer {
    pub open: Option<Open>,
    /// The comment field.
    pub edit: TextEdit,
    pending: Option<Pending>,
}

#[allow(dead_code, reason = "the Assist panel (next task) calls these")]
impl Composer {
    pub fn is_open(&self) -> bool {
        self.open.is_some() || self.pending.is_some()
    }
    /// Closes without saving or moving focus, as opening another draft does.
    pub fn dismiss(&mut self) {
        self.open = None;
        self.pending = None;
    }
    /// Whether the placeholder shows: a new comment with nothing typed yet.
    pub fn shows_placeholder(&self) -> bool {
        self.open.as_ref().is_some_and(|o| matches!(o.target, Target::New(_))) && self.edit.text.is_empty()
    }
}

/// The quote line: “text” with line breaks as spaces, cut to 90 characters and "…".
/// `CommentComposer.swift:32-33`
pub fn quote_line(quote: &str) -> String {
    let excerpt = quote.replace('\n', " ");
    if excerpt.chars().count() > 90 {
        format!("“{}…”", excerpt.chars().take(90).collect::<String>())
    } else {
        format!("“{excerpt}”")
    }
}

/// AppKit's `labelColor` and `secondaryLabelColor`, which the popover's text uses.
fn label_colors(p: &Palette) -> (Color, Color) {
    if p.dark { (rgba(255, 255, 255, 0.85), rgba(255, 255, 255, 0.55)) } else { (rgba(0, 0, 0, 0.85), rgba(0, 0, 0, 0.5)) }
}

impl App {
    /// What the draft draws behind its text: comment highlights, and while the composer has the
    /// keyboard, the draft's selection in AppKit's unfocused selection grey (the editor's window
    /// isn't key while its popover is).
    pub fn draft_highlights(&self, p: &Palette) -> Vec<rune::Span> {
        let mut spans = self.notes.highlights(self.edit.text.len(), p);
        let (a, b) = self.edit.range();
        if self.composer.open.is_some() && a != b {
            let grey = if p.dark { rgba(70, 70, 70, 1.0) } else { rgba(220, 220, 220, 1.0) };
            spans.push(rune::Span::new(a.min(b), a.max(b)).bg(grey, 0.0));
        }
        spans
    }

    /// Selects `range`, scrolls it into view and opens the composer pointing at its first
    /// character. `CommentComposer.swift:138-165`
    pub fn composer_present(&mut self, range: Range<usize>, quote: String, initial: String, target: Target, tx: &mut Tx) {
        if matches!(target, Target::New(_)) {
            self.show_chrome(false, tx);
        }
        // Put the caret on the first character for a frame: the popover points at it.
        let len = self.edit.text.len();
        self.edit.set_caret(range.start.min(len));
        self.edit.focused = true;
        tx.focus(DRAFT, false);
        self.reveal_caret();
        self.composer.open = None;
        self.composer.pending = Some(Pending { open: Open { quote, range, target, anchor: [0.0; 4] }, initial, frames: ANCHOR_FRAMES });
        tx.wake_after(Duration::ZERO);
    }

    /// Opens the popover once the editor has drawn the caret at the target's start.
    pub fn composer_poll(&mut self, tx: &mut Tx) -> bool {
        let Some(p) = self.composer.pending.as_mut() else { return false };
        let caret = tx.caret(DRAFT);
        if p.frames > 0 {
            p.frames -= 1;
            tx.wake_after(Duration::ZERO);
            return false;
        }
        let Some(mut pending) = self.composer.pending.take() else { return false };
        let field = self.field_rect;
        pending.open.anchor = match caret {
            Some(c) => [c[0] - field[0], c[1] - field[1], c[2], c[3]],
            // The draft didn't have the keyboard: point at the column's top-left.
            None => {
                let c = self.column();
                [c.inset_x + self.metrics.hang_width(), c.inset_y, 2.0, self.metrics.line_height]
            }
        };
        let r = pending.open.range.clone();
        let len = self.edit.text.len();
        self.edit.set_selection(r.start.min(len), r.end.min(len));
        let mut edit = TextEdit::new(pending.initial);
        edit.focused = true;
        let end = edit.text.len();
        edit.set_caret(end);
        self.composer.edit = edit;
        self.composer.open = Some(pending.open);
        tx.focus(COMPOSER_FIELD, false);
        true
    }

    /// Return: saves the trimmed text, or beeps and stays open when it's empty.
    /// `CommentComposer.swift:100-107`, `CommentComposer.swift:145-154`
    pub fn composer_save(&mut self, tx: &mut Tx) {
        let body = self.composer.edit.text.trim().to_string();
        if body.is_empty() {
            self.beep();
            return;
        }
        let Some(open) = self.composer.open.take() else { return };
        match open.target {
            Target::New(range) => {
                let id = self.add_comment(range.clone(), &body);
                let len = self.edit.text.len();
                self.edit.set_caret(range.end.min(len));
                self.edit.focused = true;
                tx.focus(DRAFT, false);
                if let Some(id) = id {
                    self.notes.set_active_comment(Some(id));
                }
                let n = self.notes.open_count();
                let keys = shortcut_label(Command::ShowComments);
                self.flash_message(format!("{n} open comment{} · {keys} to review and send", if n == 1 { "" } else { "s" }), tx);
            }
            Target::Edit(id) => {
                self.notes.update_comment(&id, &body, self.now);
                self.edit.focused = true;
                tx.focus(DRAFT, false);
            }
        }
    }

    /// Esc, or a click outside: closes without saving.
    pub fn composer_cancel(&mut self, tx: &mut Tx) {
        if self.composer.open.take().is_some() {
            self.edit.focused = true;
            tx.focus(DRAFT, false);
        }
    }
}

static ARROW_UP: Icon = Icon { name: "fw-popover-arrow-up", view: 22.0, body: r#"<path d="M0 11L11 0L22 11Z" fill="currentColor"/>"# };
static ARROW_DOWN: Icon = Icon { name: "fw-popover-arrow-down", view: 22.0, body: r#"<path d="M0 0L11 11L22 0Z" fill="currentColor"/>"# };

/// The popover, below the target's first character (above it when there's no room), or
/// nothing while closed. `CommentComposer.swift:27-98`
pub fn view(s: &App, p: &Palette) -> El {
    let Some(open) = &s.composer.open else {
        return el().key("comment-composer").absolute(0.0, 0.0).size(0.0, 0.0);
    };
    let (label, secondary) = label_colors(p);
    let (ww, wh) = s.window;
    // The first character's box: one character wide, one line tall, around the caret.
    let m = &s.metrics;
    let a = open.anchor;
    let line_top = s.field_rect[1] + a[1] - ((m.line_height - a[3]) / 2.0).round();
    let (char_x, char_w) = (s.field_rect[0] + a[0], m.character_width);
    let below = line_top + m.line_height + ARROW + HEIGHT <= wh - 8.0;
    let top = if below { line_top + m.line_height + ARROW } else { line_top - ARROW - HEIGHT };
    let mid = char_x + char_w / 2.0;
    let left = (mid - WIDTH / 2.0).clamp(8.0, (ww - 8.0 - WIDTH).max(8.0));

    let new = matches!(open.target, Target::New(_));
    let button = if new { "Comment" } else { "Save" };
    let button_w = fitted_button_width(button, 11.0);
    let button_x = WIDTH - 12.0 - button_w;
    let quote_font = rune::tok::TextStyle { italic: true, ..ui_font(11.0, false) };

    let field = text_input(&s.composer.edit, ui_font(13.0, false))
        .key(COMPOSER_FIELD)
        .label("Comment")
        .color(label)
        .caret(p.caret, p.selection)
        .w_full()
        .min_h(84.0)
        .pl(7.0)
        .pr(7.0)
        .pt(4.0)
        .pb(4.0)
        .on_edit(|s: &mut App| &mut s.composer.edit)
        .on_input_key(|s: &mut App, ev, tx| {
            if ev.key == Key::Escape {
                s.composer_cancel(tx);
                return true;
            }
            false
        })
        .on_submit(|s: &mut App, tx| s.composer_save(tx));

    let mut card = el()
        .key("comment-composer")
        .absolute(left, top)
        .size(WIDTH, HEIGHT)
        .radius(10.0)
        .fill(p.card)
        .border(1.0, p.hairline)
        .box_shadow(Shadow { x: 0.0, y: 6.0, blur: 24.0, spread: 0.0, color: rgba(0, 0, 0, if p.dark { 0.5 } else { 0.18 }), inset: false })
        .modal_tx(true, |s: &mut App, tx| s.composer_cancel(tx))
        .child(text(quote_line(&open.quote)).key("composer-quote").font(quote_font).color(secondary).truncate().absolute(14.0, 10.0).size(WIDTH - 28.0, 16.0));
    if s.composer.shows_placeholder() {
        // The label's frame is at x 16; its cell draws the text 2 pt in.
        card = card.child(text("What should change here?").font(ui_font(13.0, false)).color(secondary).absolute(18.0, 36.0).size(WIDTH - 34.0, 18.0));
    }
    card = card
        .child(col().key("composer-scroll").absolute(12.0, 32.0).size(WIDTH - 24.0, 84.0).scroll(ScrollSpec { stick_bottom: false }).child(field))
        .child(
            text("↩ save · esc cancel")
                .key("composer-hint")
                .font(ui_font(10.0, false))
                .color(secondary)
                .truncate()
                .help("Shift-Return adds a new line")
                .absolute(14.0, HEIGHT - 10.0 - 14.0)
                .size((button_x - 14.0 - 10.0).max(0.0), 14.0),
        )
        .child(small_button("composer-save", button, 11.0, false, button_w, true, p).absolute(button_x, HEIGHT - 6.0 - 22.0).on_press_tx(|s: &mut App, tx| s.composer_save(tx)));

    let (arrow, arrow_y) = if below { (&ARROW_UP, top - ARROW) } else { (&ARROW_DOWN, top + HEIGHT - 1.0) };
    el().key("comment-popover")
        .absolute(0.0, 0.0)
        .size(0.0, 0.0)
        .child(icon(arrow, 22.0).color(p.card).absolute(mid - 11.0, arrow_y))
        .child(card)
        .child(el().absolute(mid - 10.0, if below { top - 1.0 } else { top + HEIGHT - 1.0 }).size(20.0, 2.0).fill(p.card))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_line_is_cut_at_90_characters() {
        assert_eq!(quote_line("one\ntwo"), "“one two”");
        let long = "x".repeat(95);
        assert_eq!(quote_line(&long), format!("“{}…”", "x".repeat(90)));
    }
}
