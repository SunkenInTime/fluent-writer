//! The controls over the top of the window: the sidebar button, the four toolbar buttons at the
//! top-right corner and where the title sits between them. A port of `ChromeControls.swift`.
//!
//! The Swift buttons draw SF Symbols at 13 pt; here each symbol is an SVG body drawn in the
//! same 16 pt box with the same stroke weight, so they read the same at that size.

use rune::{Anim, Curve, Easing, El, Icon, Tx, el, icon, text};

use crate::app::{App, Frame};
use crate::commands::Command;
use crate::theme::{CHARACTER_WIDTH_EM, Palette, ui_font};

/// A button's slot: 28 × 24 points.
pub const SLOT: (f32, f32) = (28.0, 24.0);

pub const SIDEBAR_LEFT: Icon = Icon {
    name: "sidebar.left",
    view: 16.0,
    body: r#"<rect x="1.5" y="2.5" width="13" height="11" rx="2.6" stroke="currentColor" stroke-width="1.1" fill="none"/><path d="M6 2.5V13.5" stroke="currentColor" stroke-width="1.1"/><path d="M3.2 5.2H4.4M3.2 7.2H4.4M3.2 9.2H4.4" stroke="currentColor" stroke-width="1" stroke-linecap="round"/>"#,
};

pub const PLUS_BUBBLE: Icon = Icon {
    name: "plus.bubble",
    view: 16.0,
    body: concat!(
        r#"<path d="M3.6 2.5H12.4Q14.5 2.5 14.5 4.6V9.4Q14.5 11.5 12.4 11.5H7.6L4.6 14V11.5H3.6Q1.5 11.5 1.5 9.4V4.6Q1.5 2.5 3.6 2.5Z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" fill="none"/>"#,
        r#"<path d="M8 4.6V9.4M5.6 7H10.4" stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>"#
    ),
};

pub const TEXT_BUBBLE: Icon = Icon {
    name: "text.bubble",
    view: 16.0,
    body: concat!(
        r#"<path d="M3.6 2.5H12.4Q14.5 2.5 14.5 4.6V9.4Q14.5 11.5 12.4 11.5H7.6L4.6 14V11.5H3.6Q1.5 11.5 1.5 9.4V4.6Q1.5 2.5 3.6 2.5Z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" fill="none"/>"#,
        r#"<path d="M4.6 5.6H11.4M4.6 8.4H9.4" stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>"#
    ),
};

pub const CLOCK_ARROW_CIRCLEPATH: Icon = Icon {
    name: "clock.arrow.circlepath",
    view: 16.0,
    body: concat!(
        r#"<path d="M3 8.2A5.6 5.6 0 1 1 4.3 11.8" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" fill="none"/>"#,
        r#"<path d="M1.5 6.7L3 8.4L4.6 6.7" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
        r#"<path d="M8.6 5.2V8.4L10.6 9.7" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#
    ),
};

pub const SPARKLES: Icon = Icon {
    name: "sparkles",
    view: 16.0,
    body: concat!(
        r#"<path d="M6.5 3.6Q6.9 8.6 11.4 9Q6.9 9.4 6.5 14.4Q6.1 9.4 1.6 9Q6.1 8.6 6.5 3.6Z" stroke="currentColor" stroke-width="1" stroke-linejoin="round" fill="none"/>"#,
        r#"<path d="M12 1.4Q12.2 3.6 14.4 3.8Q12.2 4 12 6.2Q11.8 4 9.6 3.8Q11.8 3.6 12 1.4Z" fill="currentColor"/>"#,
        r#"<path d="M12.6 10.6Q12.75 12 14.1 12.15Q12.75 12.3 12.6 13.7Q12.45 12.3 11.1 12.15Q12.45 12 12.6 10.6Z" fill="currentColor"/>"#
    ),
};

/// The Assist panel's tabs, which three of the toolbar buttons open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelTab {
    Ask,
    Comments,
    Versions,
}

/// One toolbar button: what it shows and does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Sidebar,
    AddComment,
    Comments,
    Versions,
    Assist,
}

impl Button {
    pub const TOOLBAR: [Button; 4] = [Button::AddComment, Button::Comments, Button::Versions, Button::Assist];

    pub fn icon(self) -> &'static Icon {
        match self {
            Button::Sidebar => &SIDEBAR_LEFT,
            Button::AddComment => &PLUS_BUBBLE,
            Button::Comments => &TEXT_BUBBLE,
            Button::Versions => &CLOCK_ARROW_CIRCLEPATH,
            Button::Assist => &SPARKLES,
        }
    }

    /// The accessibility label.
    pub fn label(self) -> &'static str {
        match self {
            Button::Sidebar => "Toggle sidebar",
            Button::AddComment => "Add comment",
            Button::Comments => "Comments",
            Button::Versions => "Versions",
            Button::Assist => "Assist",
        }
    }

    /// The tooltip. The Swift wrote the Mac's shortcuts into it; elsewhere it names that
    /// platform's chord for the same command.
    pub fn tip(self) -> String {
        use crate::commands::Keys;
        let keys = |mac: &str, other: Keys| if cfg!(target_os = "macos") { mac.to_string() } else { other.label() };
        match self {
            Button::Sidebar => format!("Show or hide the sidebar ({}). Point at the left edge to peek.", keys("⌃⌘S", Keys("", "alt+shift+s"))),
            Button::AddComment => format!("Comment on the selection or this line ({})", keys("⌥⌘M", Keys("", "mod+alt+m"))),
            Button::Comments => format!("Comments ({})", keys("⇧⌘M", Keys("", "mod+shift+m"))),
            Button::Versions => format!("Versions ({})", keys("⌥⌘V", Keys("", "mod+alt+v"))),
            Button::Assist => format!("Assist ({})", keys("⌥⌘J", Keys("", "mod+alt+j"))),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Button::Sidebar => "sidebar-button",
            Button::AddComment => "add-comment-button",
            Button::Comments => "comments-button",
            Button::Versions => "versions-button",
            Button::Assist => "assist-button",
        }
    }
}

/// Where the chrome sits, in window points `[x, y, w, h]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub sidebar: [f32; 4],
    /// Add Comment, Comments, Versions, Assist.
    pub toolbar: [[f32; 4]; 4],
    /// The title field, or none when it would be narrower than 60 points.
    pub title: Option<[f32; 4]>,
}

/// What the window's own controls take at the top: the trailing edge of the macOS traffic
/// lights and their vertical centre, and the width of the Windows caption buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub traffic_lights: Option<(f32, f32)>,
    pub caption_buttons: f32,
    /// The right edge of anything else at the left of the top bar (the in-window menu bar).
    pub leading_extra: f32,
}

/// The Swift `layoutSidebarButton` and `layoutToolbar`.
pub fn layout(f: &Frame, insets: Insets) -> Layout {
    // Next to the traffic lights, centred on them; at the left edge when there are none (full
    // screen, and on Windows and Linux).
    let sidebar = match insets.traffic_lights {
        Some((right, mid_y)) => [right + 12.0, (mid_y - SLOT.1 / 2.0).round(), SLOT.0, SLOT.1],
        None => [12.0, 8.0, SLOT.0, SLOT.1],
    };
    let mid_y = sidebar[1] + SLOT.1 / 2.0;
    // Right to left: Assist, Versions, Comments, then Add Comment a little apart.
    let mut x = f.width - insets.caption_buttons - 12.0;
    let mut toolbar = [[0.0; 4]; 4];
    for (i, slot) in [3usize, 2, 1, 0].into_iter().enumerate() {
        x -= SLOT.0;
        toolbar[slot] = [x, (mid_y - SLOT.1 / 2.0).round(), SLOT.0, SLOT.1];
        x -= if i == 2 { 10.0 } else { 2.0 };
    }
    // Keep the centred title clear of whatever part of the buttons sits over the top bar.
    let (bar_x, bar_w) = (f.editor_x(), f.editor_w);
    let overlap = (bar_x + bar_w - (x + 8.0)).max(0.0);
    let left_reserve = (sidebar[0] + sidebar[2] + 12.0 - bar_x).max(0.0).max(insets.leading_extra + 12.0 - bar_x);
    let reserve = overlap.max(left_reserve).max(24.0);
    let title_w = (bar_w - 2.0 * reserve).clamp(0.0, 420.0);
    let title = (title_w >= 60.0).then(|| [bar_x + (bar_w - title_w) / 2.0, (mid_y - 9.0).round(), title_w, 18.0]);
    Layout { sidebar, toolbar, title }
}

/// The insets this window has, read from Rune.
pub fn insets(cx: &rune::Cx<App>, menu_bar_right: f32) -> Insets {
    let lights = if cfg!(target_os = "macos") {
        let right = cx.titlebar_inset();
        // Rune reports only how far the buttons reach; AppKit centres them at the height of
        // its default placement, which Rune's `Chrome` describes.
        let mid = rune::Chrome::DEFAULT.traffic_light_rects().map_or(13.0, |r| r[2][1] + r[2][3] / 2.0);
        (right > 0.0).then_some((right, mid))
    } else {
        None
    };
    Insets { traffic_lights: lights, caption_buttons: cx.chrome_insets().trailing, leading_extra: menu_bar_right }
}

/// How the chrome fades: in over 0.25 s, out over 0.6 s, at once with Reduce Motion. AppKit's
/// animation context eases in and out by default.
pub fn fade(visible: bool, reduce_motion: bool) -> Anim {
    if reduce_motion {
        return Anim::INSTANT;
    }
    Anim::new(Curve::tween(if visible { 0.25 } else { 0.6 }, Easing::cubic(0.25, 0.1, 0.25, 1.0)))
}

/// A quiet borderless icon button: grey at rest, a soft rounded plate on hover, tinted while
/// its panel is open, with an optional count in its corner.
#[allow(clippy::too_many_arguments)]
pub fn chrome_button(b: Button, rect: [f32; 4], active: bool, count: usize, window_key: bool, p: &Palette, opacity: f32, fade: Anim) -> El {
    let plate = if active { p.sidebar_selection.alpha(0.9) } else { rune::Color::TRANSPARENT };
    let mut e = el()
        .key(b.key())
        .role("button")
        .label(b.label())
        .help(b.tip())
        .absolute(rect[0], rect[1])
        .size(rect[2], rect[3])
        .radius(6.0)
        .fill(plate)
        .center()
        .hover_group()
        .opacity(opacity)
        .opacity_motion(fade)
        .on_press_tx(move |s: &mut App, tx| s.press_chrome_button(b, tx));
    // Hover shows only while the window is key, as the Swift tracking area asked.
    if window_key && !active {
        e = e.hover(p.sidebar_selection.alpha(0.7));
    }
    if count > 0 {
        e = e.a11y_value(count.to_string());
    }
    let tint = if active { p.caret } else { p.icon };
    let mut glyph = icon(b.icon(), 16.0).key(format!("{}-icon", b.key())).color(tint);
    if window_key && !active {
        glyph = glyph.group_color(p.text);
    }
    e = e.child(glyph);
    if count > 0 {
        let label = if count > 99 { "99+".to_string() } else { count.to_string() };
        let w = (label.chars().count() as f32 * 9.0 * CHARACTER_WIDTH_EM).ceil() + 2.0;
        let w = w.max(12.0);
        e = e.child(
            text(label)
                .key(format!("{}-badge", b.key()))
                .font(ui_font(9.0, true))
                .color(if active { p.caret } else { p.text })
                .align(rune::Align::Center)
                .absolute(rect[2] - w - 1.0, 0.0)
                .size(w, 11.0)
                .a11y_hidden(),
        );
    }
    e
}

impl App {
    /// Runs a chrome button's action.
    pub fn press_chrome_button(&mut self, b: Button, tx: &mut Tx) {
        match b {
            Button::Sidebar => self.run(Command::ToggleSidebar, tx),
            Button::AddComment => self.run(Command::AddComment, tx),
            Button::Comments => self.toggle_panel(PanelTab::Comments, tx),
            Button::Versions => self.toggle_panel(PanelTab::Versions, tx),
            Button::Assist => self.toggle_panel(PanelTab::Ask, tx),
        }
    }

    /// The Swift `togglePanel`: close the Assist panel if it is open on `tab`, else open it on
    /// `tab` keeping the provider. The Assist button opens the panel even for an empty draft,
    /// where Show Assist is disabled.
    pub fn toggle_panel(&mut self, tab: PanelTab, tx: &mut Tx) {
        let tab = match tab {
            PanelTab::Ask => crate::notes::AssistTab::Ask,
            PanelTab::Comments => crate::notes::AssistTab::Comments,
            PanelTab::Versions => crate::notes::AssistTab::Versions,
        };
        self.assist_toggle_tab(tab, tx);
    }

    /// Which Assist tab is open, if the panel is.
    pub fn open_panel_tab(&self) -> Option<PanelTab> {
        self.assist_visible.then_some(match self.notes.assist_tab {
            crate::notes::AssistTab::Ask => PanelTab::Ask,
            crate::notes::AssistTab::Comments => PanelTab::Comments,
            crate::notes::AssistTab::Versions => PanelTab::Versions,
        })
    }

    /// Open comments, detached ones included, for the Comments badge.
    pub fn open_comment_count(&self) -> usize {
        self.notes.open_count()
    }

    /// Whether a version preview is showing.
    pub fn previewing_version(&self) -> bool {
        self.preview.is_active()
    }

    /// Whether `b` shows as active: tinted, on its plate.
    pub fn chrome_button_active(&self, b: Button) -> bool {
        let tab = self.open_panel_tab();
        match b {
            Button::Sidebar => self.sidebar_visible,
            Button::AddComment => false,
            Button::Comments => tab == Some(PanelTab::Comments),
            Button::Versions => tab == Some(PanelTab::Versions) || self.previewing_version(),
            Button::Assist => tab == Some(PanelTab::Ask),
        }
    }

    /// The count in `b`'s corner. The Versions badge is always 0, as in the Swift.
    pub fn chrome_button_count(&self, b: Button) -> usize {
        match b {
            Button::Comments => self.open_comment_count(),
            _ => 0,
        }
    }
}

/// The sidebar button and the toolbar, over everything else in the window.
pub fn view(s: &App, p: &Palette, l: &Layout, opacity: f32, fade: Anim) -> Vec<El> {
    let mut out = vec![chrome_button(Button::Sidebar, l.sidebar, s.chrome_button_active(Button::Sidebar), 0, s.win.key, p, opacity, fade)];
    for (i, b) in Button::TOOLBAR.into_iter().enumerate() {
        out.push(chrome_button(b, l.toolbar[i], s.chrome_button_active(b), s.chrome_button_count(b), s.win.key, p, opacity, fade));
    }
    out
}

