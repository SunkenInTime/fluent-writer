//! Colours, type sizes and column metrics. The values are the Swift app's `Theme`.

use rune::Color;
use rune::tok::{Family, TextStyle};

/// The appearance setting: follow the system, or force light or dark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    System,
    Light,
    Dark,
}

impl Appearance {
    pub const ALL: [Appearance; 3] = [Appearance::System, Appearance::Light, Appearance::Dark];

    /// The stored preference value.
    pub fn raw(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }

    pub fn from_raw(raw: &str) -> Option<Appearance> {
        Appearance::ALL.into_iter().find(|a| a.raw() == raw)
    }

    /// Whether the app draws dark, given the system's appearance.
    pub fn is_dark(self, system_dark: bool) -> bool {
        match self {
            Appearance::System => system_dark,
            Appearance::Light => false,
            Appearance::Dark => true,
        }
    }
}

/// An sRGB colour from 0–255 channels and 0–1 alpha.
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    let lin = |v: u8| {
        let c = v as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    Color { r: lin(r), g: lin(g), b: lin(b), a }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    rgba(r, g, b, 1.0)
}

/// Every colour the app draws with, for one appearance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub canvas: Color,
    pub text: Color,
    pub dimmed: Color,
    pub markup: Color,
    pub quiet: Color,
    pub caret: Color,
    pub selection: Color,
    pub hairline: Color,
    pub sidebar: Color,
    pub sidebar_selection: Color,
    pub sidebar_current: Color,
    pub code_background: Color,
    pub warning: Color,
    pub inserted: Color,
    pub inserted_background: Color,
    pub deleted: Color,
    pub deleted_background: Color,
    pub comment_highlight: Color,
    pub comment_highlight_active: Color,
    pub card: Color,
    pub card_hover: Color,
    pub card_selected: Color,
    pub icon: Color,
    pub banner: Color,
    /// Find bar matches in the draft, and the current match.
    pub find_match: Color,
    pub find_current: Color,
    /// AppKit's `unemphasizedSelectedTextBackgroundColor`: the grey a text view draws its
    /// selection in while it isn't first responder or its window isn't key.
    pub unfocused_selection: Color,
}

impl Palette {
    pub fn new(dark: bool) -> Palette {
        let pick = |light: Color, dark_c: Color| if dark { dark_c } else { light };
        Palette {
            dark,
            canvas: pick(rgb(247, 247, 247), rgb(26, 26, 26)),
            text: pick(rgb(25, 25, 25), rgb(218, 218, 216)),
            dimmed: pick(rgb(199, 196, 194), rgb(78, 78, 76)),
            markup: pick(rgb(172, 170, 168), rgb(112, 112, 110)),
            quiet: pick(rgb(150, 148, 146), rgb(120, 120, 118)),
            caret: pick(rgb(0, 190, 255), rgb(0, 175, 245)),
            selection: pick(rgb(178, 228, 250), rgb(22, 78, 104)),
            hairline: pick(rgb(228, 228, 226), rgb(44, 44, 44)),
            sidebar: pick(rgb(240, 240, 239), rgb(32, 32, 32)),
            sidebar_selection: pick(rgb(222, 222, 220), rgb(52, 52, 52)),
            sidebar_current: pick(rgb(230, 230, 228), rgb(44, 44, 44)),
            code_background: pick(rgb(236, 236, 234), rgb(38, 38, 38)),
            warning: pick(rgb(196, 54, 36), rgb(240, 110, 90)),
            inserted: pick(rgb(30, 130, 70), rgb(110, 200, 140)),
            inserted_background: pick(rgba(30, 160, 80, 0.13), rgba(90, 200, 120, 0.18)),
            deleted: pick(rgb(180, 50, 40), rgb(235, 120, 105)),
            deleted_background: pick(rgba(210, 60, 40, 0.11), rgba(230, 90, 70, 0.17)),
            comment_highlight: pick(rgba(255, 196, 0, 0.20), rgba(255, 196, 40, 0.16)),
            comment_highlight_active: pick(rgba(255, 180, 0, 0.42), rgba(255, 190, 40, 0.34)),
            card: pick(rgb(255, 255, 255), rgb(34, 34, 34)),
            card_hover: pick(rgb(252, 252, 251), rgb(40, 40, 40)),
            card_selected: pick(rgb(234, 246, 253), rgb(26, 54, 70)),
            icon: pick(rgb(120, 118, 116), rgb(172, 172, 170)),
            banner: pick(rgb(238, 238, 236), rgb(36, 36, 36)),
            find_match: pick(rgba(0, 190, 255, 0.16), rgba(0, 175, 245, 0.22)),
            find_current: pick(rgba(0, 190, 255, 0.34), rgba(0, 175, 245, 0.42)),
            unfocused_selection: pick(rgb(220, 220, 220), rgb(70, 70, 70)),
        }
    }
}

pub const DEFAULT_TEXT_SIZE: f32 = 19.0;
pub const TEXT_SIZES: [f32; 12] = [14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 22.0, 24.0, 26.0, 28.0, 32.0];
pub const LINE_HEIGHT_MULTIPLE: f32 = 1.55;
pub const COLUMN_CHARACTERS: f32 = 68.0;
pub const MINIMUM_SIDE_PADDING: f32 = 36.0;

/// IBM Plex Mono's advance is 600 units of a 1000-unit em, for every glyph.
pub const CHARACTER_WIDTH_EM: f32 = 0.6;

/// The embedded faces: IBM Plex Mono in the weights and styles the app draws.
pub const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/IBMPlexMono-Regular.otf"),
    include_bytes!("../assets/fonts/IBMPlexMono-Italic.otf"),
    include_bytes!("../assets/fonts/IBMPlexMono-Medium.otf"),
    include_bytes!("../assets/fonts/IBMPlexMono-SemiBold.otf"),
    include_bytes!("../assets/fonts/IBMPlexMono-Bold.otf"),
    include_bytes!("../assets/fonts/IBMPlexMono-BoldItalic.otf"),
];
pub const FONT_FAMILY: &str = "IBM Plex Mono";

/// The draft's text style at `size` points: one monospaced size on a fixed line height.
pub fn draft_font(size: f32) -> TextStyle {
    TextStyle::new(size, 400.0).family(Family::Mono).lh((size * LINE_HEIGHT_MULTIPLE).round())
}

/// The interface's text style: IBM Plex Mono, regular or medium.
pub fn ui_font(size: f32, medium: bool) -> TextStyle {
    TextStyle::new(size, if medium { 500.0 } else { 400.0 }).family(Family::Mono)
}
