//! Modal alerts and sheets, drawn in the window in the app's type and colours.
//!
//! An [`Alert`] is AppKit's `NSAlert`: a message, informative text under it, one to three
//! buttons and a style. Show one with [`App::show_alert`]; its `on_answer` runs with the index of
//! the button pressed, 0 being the first. The first button is the default: Return presses it.
//! Alerts queue, one showing at a time, as sheets on one window do.
//!
//! ```ignore
//! s.show_alert(
//!     Alert::new("This draft couldn't be saved.")
//!         .informative("Disk full")
//!         .warning()
//!         .button("Keep Editing")
//!         .button("Continue")
//!         .on_answer(|s, button, tx| if button == 1 { s.new_draft(tx) }),
//! );
//! s.present_error("Couldn't open “a.md”: No such file or directory");
//! ```
//!
//! [`sheet`] is the container an alert draws in: a card hanging from the top of the window over
//! a backdrop that takes every click, with keyboard focus held inside it. Other modal content
//! (the SSH and connect sheets) can use it directly.

use rune::{Color, El, Tx, col, el, row, text};

use crate::app::{App, TOP_BAR_HEIGHT};
use crate::theme::{Palette, ui_font};

/// What an alert's buttons do.
pub type Answer = fn(&mut App, usize, &mut Tx);

/// A modal alert: a message, informative text, buttons and a style.
#[derive(Clone)]
pub struct Alert {
    pub message: String,
    pub informative: String,
    /// One to three titles, the default first. Empty means a single "OK".
    pub buttons: Vec<String>,
    /// AppKit's warning style. On macOS it looks the same as the informational style, so here
    /// it only tells assistive technology this is a warning.
    pub warning: bool,
    /// The button Escape presses, if any. AppKit gives Escape to a button titled "Cancel" only.
    pub cancel: Option<usize>,
    pub on_answer: Answer,
}

impl Alert {
    pub fn new(message: impl Into<String>) -> Alert {
        Alert { message: message.into(), informative: String::new(), buttons: Vec::new(), warning: false, cancel: None, on_answer: |_, _, _| {} }
    }

    /// The alert AppKit builds for an error (`NSAlert(error:)`): its description and an OK button.
    pub fn error(message: impl Into<String>) -> Alert {
        Alert::new(message)
    }

    pub fn informative(mut self, text: impl Into<String>) -> Alert {
        self.informative = text.into();
        self
    }

    /// Adds a button after the ones already there. The first one added is the default.
    pub fn button(mut self, title: &str) -> Alert {
        debug_assert!(self.buttons.len() < 3, "an alert has at most three buttons");
        if title == "Cancel" {
            self.cancel = Some(self.buttons.len());
        }
        self.buttons.push(title.into());
        self
    }

    pub fn warning(mut self) -> Alert {
        self.warning = true;
        self
    }

    pub fn on_answer(mut self, f: Answer) -> Alert {
        self.on_answer = f;
        self
    }

    fn titles(&self) -> Vec<String> {
        if self.buttons.is_empty() { vec!["OK".into()] } else { self.buttons.clone() }
    }
}

impl App {
    /// Shows `alert` now, or after the alerts already showing are answered.
    pub fn show_alert(&mut self, alert: Alert) {
        self.alerts.push(alert);
    }

    /// Shows an error as AppKit's `presentError` does: the message and an OK button.
    pub fn present_error(&mut self, message: impl std::fmt::Display) {
        self.show_alert(Alert::error(message.to_string()));
    }

    /// Answers the alert that is showing with button `index`.
    pub fn answer_alert(&mut self, index: usize, tx: &mut Tx) {
        if self.alerts.is_empty() {
            return;
        }
        let alert = self.alerts.remove(0);
        // Focus goes back to the draft unless the answer moves it somewhere else.
        self.edit.focused = true;
        (alert.on_answer)(self, index, tx);
    }

    /// The alert on screen, if any.
    pub fn alert(&self) -> Option<&Alert> {
        self.alerts.first()
    }
}

pub const ALERT_WIDTH: f32 = 420.0;

/// A button in an alert or sheet: the default one inverted, the rest on a soft plate.
pub fn button<S: 'static>(key: &str, title: &str, default: bool, p: &Palette, press: impl Fn(&mut S, &mut Tx) + 'static) -> El {
    let (fill, ink) = if default { (p.text, p.canvas) } else { (p.sidebar_selection, p.text) };
    let mut b = row()
        .key(key)
        .role("button")
        .label(title)
        .h(26.0)
        .px(14.0)
        .center()
        .radius(6.0)
        .fill(fill)
        .hover(hover_tint(p))
        .focusable(true)
        .focus_ring(2.0, 1.0, p.caret)
        .child(text(title).font(ui_font(12.0, true)).color(ink))
        .on_press_tx(press);
    if default {
        b = b.default_button();
    }
    b
}

fn hover_tint(p: &Palette) -> Color {
    if p.dark { Color { r: 1.0, g: 1.0, b: 1.0, a: 0.08 } } else { Color { r: 0.0, g: 0.0, b: 0.0, a: 0.06 } }
}

/// A sheet: `content` on a card `width` points wide, hanging from the top of a window
/// `window_w` points wide. The window behind takes no clicks or keys while it shows; Escape
/// runs `escape`.
pub fn sheet<S: 'static>(key: &str, p: &Palette, window: (f32, f32), width: f32, content: El, escape: impl Fn(&mut S, &mut Tx) + 'static) -> El {
    let width = width.min(window.0 - 24.0).max(0.0);
    let card = col()
        .key(format!("{key}-card"))
        .role("dialog")
        .absolute(((window.0 - width) / 2.0).round(), TOP_BAR_HEIGHT)
        .w(width)
        .padding(20.0)
        .gap(12.0)
        .radius(10.0)
        .fill(p.card)
        .border(1.0, p.hairline)
        .shadow(8.0, 32.0, Color { r: 0.0, g: 0.0, b: 0.0, a: if p.dark { 0.5 } else { 0.18 } })
        .child(content)
        .modal_tx(false, escape);
    // The backdrop takes the clicks the window behind would get, as a sheet's window does.
    el().key(key).absolute(0.0, 0.0).size(window.0, window.1).on_press(|_: &mut S| {}).child(card)
}

/// The alert that is showing, as a sheet.
pub fn alert_view(s: &App, p: &Palette) -> Option<El> {
    let alert = s.alert()?;
    let titles = alert.titles();
    let mut body = col().key("alert-text").gap(8.0).child(
        text(alert.message.as_str()).key("alert-message").font(ui_font(13.0, true)).color(p.text).wrap(),
    );
    if !alert.informative.is_empty() {
        body = body.child(text(alert.informative.as_str()).key("alert-informative").font(ui_font(12.0, false)).color(p.text.alpha(0.75)).wrap());
    }
    // AppKit lays the buttons out from the right: the default rightmost, the third leftmost.
    let mut buttons = row().key("alert-buttons").gap(8.0).mt(8.0).justify_end();
    if titles.len() == 3 {
        buttons = buttons.child(button("alert-button-2", &titles[2], false, p, |s: &mut App, tx| s.answer_alert(2, tx))).child(el().grow(1.0));
    }
    if titles.len() >= 2 {
        buttons = buttons.child(button("alert-button-1", &titles[1], false, p, |s: &mut App, tx| s.answer_alert(1, tx)));
    }
    buttons = buttons.child(button("alert-button-0", &titles[0], true, p, |s: &mut App, tx| s.answer_alert(0, tx)));
    let content = col().key("alert").role(if alert.warning { "alertdialog" } else { "dialog" }).label(alert.message.as_str()).gap(4.0).child(body).child(buttons);
    let cancel = alert.cancel;
    Some(sheet("alert-sheet", p, s.window, ALERT_WIDTH, content, move |s: &mut App, tx| {
        if let Some(i) = cancel {
            s.answer_alert(i, tx);
        }
    }))
}

