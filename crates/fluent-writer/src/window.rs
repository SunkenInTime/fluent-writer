//! The window itself: whether it is key, its frame between launches, its title and full screen.
//!
//! AppKit saved the frame under the autosave name `FluentWriterMain` and centred the window
//! when there was none. Rune has no autosave, so the app keeps the content rect in its
//! preferences (screen points, from the primary display's top-left) with the scale it was on,
//! opens there, and centres the window on its first frame when nothing was saved.

use std::sync::OnceLock;

use rune::{DisplayTarget, OverlayPlacement, Tx};

use crate::app::App;
use crate::prefs::Prefs;

/// The running app's handle, for what a handler can't ask of its `Tx`: moving the window.
pub static HANDLE: OnceLock<rune::AppHandle<App>> = OnceLock::new();

#[derive(Debug, Default)]
pub struct Window {
    /// The window is the key window (has the keyboard).
    pub key: bool,
    /// Full screen, as far as the app knows: Rune doesn't report it, so this follows the
    /// app's own requests (macOS says it through the traffic lights' inset instead).
    pub full_screen: bool,
    /// The content rect in screen points as last reported, and the scale it draws at.
    pub frame: Option<([f32; 4], f32)>,
    /// The frame last written to the preferences.
    saved: Option<[f32; 4]>,
    /// Where the window was asked to open, until the first report of where it did.
    opened_at: Option<[f32; 4]>,
    /// The window title last sent to the system.
    pub title: Option<String>,
    /// The first frame centred the window (there was no saved frame), or needn't.
    pub placed: bool,
    /// The saved frame came from the Swift app, without a scale to place it in pixels: the
    /// window opens at its size and moves there in points once it reports where it is.
    place_on_open: bool,
}

impl Window {
    /// A window that opens key, as AppKit's `showWindow` makes it.
    pub fn new() -> Window {
        Window { key: true, ..Default::default() }
    }
}

/// What `main` sets up before the window exists: the saved frame, if any, as a content size and
/// an outer position in desktop pixels.
pub struct Opening {
    pub size: (f32, f32),
    pub options: rune::WindowOptions,
    pub frame: Option<[f32; 4]>,
    /// `frame` has no scale (carried over from the Swift app): place it once the window is up.
    pub place: bool,
}

pub fn opening(prefs: &Prefs) -> Opening {
    let default = Opening { size: (1060.0, 760.0), options: rune::WindowOptions::default(), frame: None, place: false };
    let Some(saved) = prefs.window_frame() else { return default };
    let [x, y, w, h, scale] = saved.map(|v| v as f32);
    if !(w >= 420.0 && h >= 320.0 && scale >= 0.0) {
        return default;
    }
    if scale == 0.0 {
        return Opening { size: (w, h), options: rune::WindowOptions::default(), frame: Some([x, y, w, h]), place: true };
    }
    // The saved rect is the content's; the window is placed by its outer corner, which the
    // title bar and borders on Windows and Linux push up and left of it.
    let (ox, oy) = prefs.window_decoration().map_or((0.0, 0.0), |o| (o[0] as f32, o[1] as f32));
    let position = (((x - ox) * scale).round() as i32, ((y - oy) * scale).round() as i32);
    Opening { size: (w, h), options: rune::WindowOptions { position: Some(position), ..Default::default() }, frame: Some([x, y, w, h]), place: false }
}

impl App {
    /// The window opened at `frame` (from [`opening`]); its first report checks it landed there.
    pub fn opened_at(&mut self, frame: Option<[f32; 4]>, place: bool) {
        self.win.opened_at = frame;
        self.win.placed = frame.is_some();
        self.win.place_on_open = place && frame.is_some();
    }

    /// The window became key or stopped being key (`windowDidBecomeKey`, `windowDidResignKey`).
    /// A pinned sidebar lists again when the window comes back.
    pub fn window_focus_changed(&mut self, focused: bool, tx: &mut Tx) {
        self.win.key = focused;
        if focused {
            self.show_chrome(true, tx);
            if self.sidebar_visible {
                self.reload_sidebar();
            }
        } else {
            self.flush();
            self.show_chrome(false, tx);
            self.save_window_frame();
        }
    }

    /// Where the window's content is on screen, in points, at `scale`.
    pub fn screen_frame_changed(&mut self, r: [f32; 4], scale: f32) {
        self.win.frame = Some((r, scale));
        // Image previews are decoded for the screen the window is on. `ImageBlocks.swift:234`
        if scale > 0.0 {
            self.scale = scale;
        }
        let Some(want) = self.win.opened_at.take() else { return };
        if std::mem::take(&mut self.win.place_on_open) {
            if let Some(handle) = HANDLE.get() {
                handle.place(DisplayTarget::Primary, OverlayPlacement::Rect { x: want[0], y: want[1], width: want[2], height: want[3] });
            }
            self.win.frame = Some((want, scale));
            return;
        }
        // Off macOS the window is placed by its outer corner. The first report says how far the
        // decorations put the content from it; move once to land exactly, and keep the offset
        // so the next launch opens in place.
        let (dx, dy) = (r[0] - want[0], r[1] - want[1]);
        if dx.abs() < 0.5 && dy.abs() < 0.5 || cfg!(target_os = "macos") {
            return;
        }
        let known = self.prefs.window_decoration().map_or((0.0, 0.0), |o| (o[0] as f32, o[1] as f32));
        self.prefs.set_window_decoration([(known.0 + dx) as f64, (known.1 + dy) as f64]);
        if let Some(handle) = HANDLE.get() {
            handle.place(DisplayTarget::Primary, OverlayPlacement::Rect { x: want[0] - dx, y: want[1] - dy, width: want[2], height: want[3] });
        }
        self.win.frame = Some((want, scale));
    }

    /// Writes the window's frame to the preferences when it changed.
    pub fn save_window_frame(&mut self) {
        let Some((r, scale)) = self.win.frame else { return };
        if self.win.saved == Some(r) || r[2] <= 0.0 || r[3] <= 0.0 {
            return;
        }
        self.win.saved = Some(r);
        self.prefs.set_window_frame([r[0] as f64, r[1] as f64, r[2] as f64, r[3] as f64, scale as f64]);
    }

    /// Before each frame: the title follows the draft, and a window with no saved frame is
    /// centred once.
    pub fn sync_window(&mut self, tx: &mut Tx) {
        if self.win.title.as_deref() != Some(self.draft.title.as_str()) {
            self.win.title = Some(self.draft.title.clone());
            tx.set_title(self.draft.title.clone());
        }
        if !self.win.placed {
            self.win.placed = true;
            tx.instant();
            tx.center_window();
        }
    }

    pub fn toggle_full_screen(&mut self, tx: &mut Tx) {
        self.win.full_screen = !self.win.full_screen;
        tx.toggle_full_screen();
    }
}
