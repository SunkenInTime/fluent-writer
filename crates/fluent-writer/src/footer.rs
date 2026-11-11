//! The footer: save status on the left, the focus mode in the middle when there's room, counts
//! on the right (the Swift `layoutFooter`, `updateStatus` and `flashMessage`).

use rune::{Align, Anim, El, LiveRegion, el, text};
use writer_core::FocusUnit;

use crate::app::{App, FOOTER_HEIGHT, SaveState, focus_name};
use crate::theme::{CHARACTER_WIDTH_EM, Palette, ui_font};

/// The status line for a file edited over SSH. The SSH port fills this in; `None` falls back
/// to the local statuses. Its strings are, with `<host>` the host without `user@`:
/// "Saved to <host>", "Saved locally · not yet on <host>", "Saving to <host>…",
/// "Offline · editing the local copy", and, as a warning with the message as its tooltip and
/// the chrome kept up, "Not on <host> yet — <message>". Its tooltip is the remote location.
pub struct RemoteStatus {
    pub text: String,
    pub warning: bool,
    pub tooltip: String,
}

impl App {
    /// The footer's status text and whether it is a warning.
    pub fn status(&self) -> (String, bool) {
        if let Some(t) = &self.transient {
            return (t.clone(), false);
        }
        if let (SaveState::Saved, Some(remote)) = (&self.save_state, self.remote_status()) {
            return (remote.text, remote.warning);
        }
        match &self.save_state {
            SaveState::Saved => (if self.draft.path().is_none() { String::new() } else { "Saved".into() }, false),
            SaveState::Pending => ("Edited".into(), false),
            SaveState::Recovered => ("Recovered unsaved changes".into(), false),
            SaveState::Failed(m) => (format!("Not saved — {m}"), true),
        }
    }

    /// See [`RemoteStatus`].
    pub fn remote_status(&self) -> Option<RemoteStatus> {
        None
    }

    /// Updates the status tooltip as the Swift `updateStatus` did: the file's path once saved,
    /// the message after a failure, the remote location for remote files. "Edited", "Recovered
    /// unsaved changes" and flash messages leave the last one in place, as the Swift did.
    pub fn refresh_status_tooltip(&mut self) {
        if self.transient.is_some() {
            return;
        }
        let tip = match &self.save_state {
            SaveState::Saved => match self.remote_status() {
                Some(r) => Some(r.tooltip),
                None => self.draft.path().map(|p| p.display().to_string()),
            },
            SaveState::Failed(m) => Some(m.clone()),
            SaveState::Pending | SaveState::Recovered => return,
        };
        self.status_tooltip = tip;
    }
}

/// A label's width as AppKit measured it: its text's width plus 4, or 0 when empty.
fn width(t: &str) -> f32 {
    if t.is_empty() { 0.0 } else { (t.chars().count() as f32 * 12.0 * CHARACTER_WIDTH_EM).ceil() + 4.0 }
}

pub fn view(s: &App, p: &Palette, opacity: f32, fade: Anim) -> El {
    let f = s.frame();
    let (w, h) = (f.editor_w, f.height);
    let font = ui_font(12.0, false);
    let pad = 20.0;
    let (status, warning) = s.status();
    let counts = if s.prefs.show_counts() { s.counts.as_str() } else { "" };
    let focus = if s.focus == FocusUnit::Off { "" } else { focus_name(s.focus) };

    let counts_w = width(counts).min(w * 0.45);
    let status_max = (w - pad * 2.0 - counts_w - if counts_w > 0.0 { 16.0 } else { 0.0 }).max(0.0);
    let status_w = width(&status).min(status_max);
    let focus_w = width(focus);
    let focus_x = (w - focus_w) / 2.0;
    let focus_fits = focus_x > pad + status_w + 12.0 && focus_x + focus_w < w - pad - counts_w - 12.0;

    let label = |key: &str, t: &str, x: f32, width: f32, align: Align| {
        text(t).key(key).font(font).color(p.quiet).align(align).truncate().absolute(x, 9.0).size(width, 16.0).opacity(opacity).opacity_motion(fade)
    };
    let mut bar = el().key("footer").absolute(f.editor_x(), h - FOOTER_HEIGHT).size(w, FOOTER_HEIGHT).fill(p.canvas);
    if !status.is_empty() {
        let mut e = label("status", &status, pad, status_w, Align::Start).color(if warning { p.warning } else { p.quiet }).label("Save status").a11y_value(status.as_str());
        if let Some(tip) = &s.status_tooltip {
            e = e.help(tip.as_str());
        }
        bar = bar.child(e);
    }
    // Each flash is announced to assistive technology, as the Swift's medium-priority
    // announcement was; the save statuses themselves are not.
    bar = bar.child(
        text(s.transient.as_deref().unwrap_or(""))
            .key("announcement")
            .font(font)
            .live_region(LiveRegion::Polite)
            .absolute(0.0, 0.0)
            .size(0.0, 0.0)
            .opacity(0.0),
    );
    if focus_fits && !focus.is_empty() {
        bar = bar.child(label("focus", focus, focus_x, focus_w, Align::Center));
    }
    if !counts.is_empty() {
        bar = bar.child(label("counts", counts, w - pad - counts_w, counts_w, Align::End).label("Counts").a11y_value(counts));
    }
    bar
}
