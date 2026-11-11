//! The menus off macOS, where Rune has no native menu bar: a menu bar drawn in the top bar from
//! `commands::MENUS`, and the editor's context menu where Rune has no native pop-up menu.
//!
//! The titles sit to the right of the sidebar button and fade with the rest of the chrome. When
//! the window is too narrow for all six, they fold into one button that lists them as submenus.
//! The keyboard works as Windows' menus do: Alt alone (or F10) highlights the first title, Alt
//! with a title's underlined letter opens it, arrows move between titles and rows, Return picks,
//! Escape backs out.
//!
//! macOS gets its native menu bar instead; `SHOWN` switches this one off there.

use rune::{Anim, Color, El, Icon, Key, KeyEvent, Modifier, PopupItem, PopupMenu, PointerPress, Tx, el, icon, row, text};

use crate::app::App;
use crate::chrome::{self, Layout};
use crate::commands::{Command, Entry, Keys, MENUS};
use crate::theme::{CHARACTER_WIDTH_EM, Palette, ui_font};

/// Whether the window draws its own menu bar.
pub const SHOWN: bool = !cfg!(target_os = "macos");

const FONT: f32 = 12.0;
const TITLE_PAD: f32 = 8.0;
const ROW_H: f32 = 24.0;
const SEPARATOR_H: f32 = 9.0;
const MENU_PAD: f32 = 4.0;
const MIN_MENU_W: f32 = 180.0;

/// Each title's access key, as Windows underlines it: File, Edit, fOrmat, View, Assist, Window.
const MNEMONICS: [(char, usize); 6] = [('f', 0), ('e', 0), ('o', 1), ('v', 0), ('a', 0), ('w', 0)];

const CHECK: Icon = Icon { name: "menu.check", view: 16.0, body: r#"<path d="M3.5 8.4L6.6 11.3L12.5 4.8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"# };
const CHEVRON: Icon = Icon { name: "menu.chevron", view: 16.0, body: r#"<path d="M6.5 4L10.5 8L6.5 12" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"# };
const LINES: Icon = Icon { name: "line.3.horizontal", view: 16.0, body: r#"<path d="M2.5 4.5H13.5M2.5 8H13.5M2.5 11.5H13.5" stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>"# };

/// What an open menu lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// One of `MENUS`.
    Menu(usize),
    /// The folded menu bar: every menu as a submenu.
    All,
    /// The editor's context menu, drawn by the app.
    Context,
}

#[derive(Debug, Default)]
pub struct MenuBar {
    pub open: Option<Source>,
    /// The highlighted row of the open menu.
    pub row: Option<usize>,
    /// The open submenu: its row in the open menu, and its highlighted row.
    pub sub: Option<(usize, Option<usize>)>,
    /// The keyboard is in the submenu.
    pub in_sub: bool,
    /// Alt was pressed alone: the titles take the keyboard, `hot` highlighted.
    pub armed: bool,
    pub hot: usize,
    /// Alt went down with nothing else yet.
    alt_alone: bool,
    /// Where the context menu opened, in window points.
    pub context_at: (f32, f32),
    /// The titles are folded into one button (set by the view's layout, read by the keys).
    pub folded: bool,
}

/// A menu row as drawn.
#[derive(Clone, Debug)]
pub struct Row {
    pub what: What,
    pub title: String,
    pub shortcut: String,
    pub enabled: bool,
    pub checked: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum What {
    Command(Command),
    Submenu(&'static [Entry]),
    Separator,
    /// A context menu item, by the id the native menu uses.
    Pick(&'static str),
}

impl Row {
    fn separator() -> Row {
        Row { what: What::Separator, title: String::new(), shortcut: String::new(), enabled: false, checked: false }
    }
    fn selectable(&self) -> bool {
        self.enabled && !matches!(self.what, What::Separator)
    }
}

/// The items macOS keeps in the app menu, which the in-window File menu ends with.
const APP_ITEMS: &[Entry] = &[Entry::Separator, Entry::Item(Command::About, "About", Keys("", "")), Entry::Item(Command::Quit, "Quit", Keys("mod+q", "mod+q"))];

impl App {
    /// The rows of `entries` as they stand now: titles, shortcuts, enabled and checked.
    pub fn menu_rows(&self, entries: &[Entry]) -> Vec<Row> {
        entries
            .iter()
            .map(|e| match *e {
                Entry::Item(c, title, keys) => {
                    let title = match c {
                        Command::About => format!("About {}", crate::app_name()),
                        Command::Quit => format!("Quit {}", crate::app_name()),
                        _ => self.command_title(c, title),
                    };
                    Row { what: What::Command(c), title, shortcut: keys.label(), enabled: self.command_enabled(c), checked: self.command_checked(c) }
                }
                Entry::Submenu(title, inner) => Row { what: What::Submenu(inner), title: title.into(), shortcut: String::new(), enabled: true, checked: false },
                Entry::Separator => Row::separator(),
            })
            .collect()
    }

    pub fn source_rows(&self, source: Source) -> Vec<Row> {
        match source {
            Source::Menu(i) => {
                let mut rows = self.menu_rows(MENUS[i].1);
                if i == 0 {
                    rows.extend(self.menu_rows(APP_ITEMS));
                }
                rows
            }
            Source::All => MENUS.iter().map(|(t, e)| Row { what: What::Submenu(e), title: (*t).into(), shortcut: String::new(), enabled: true, checked: false }).collect(),
            Source::Context => self.context_rows(),
        }
    }

    /// The editor's context menu: the comment item, then the text items.
    pub fn context_rows(&self) -> Vec<Row> {
        let sel = self.has_selection();
        let item = |id: &'static str, title: &str, enabled: bool, checked: bool| Row { what: What::Pick(id), title: title.into(), shortcut: String::new(), enabled, checked };
        let mut rows = vec![item("comment", if sel { "Comment on Selection…" } else { "Comment on This Line…" }, true, false)];
        // The comments port adds "Show Comment" here when the click is on a commented passage.
        rows.push(Row::separator());
        rows.push(item("cut", "Cut", sel, false));
        rows.push(item("copy", "Copy", sel, false));
        rows.push(item("paste", "Paste", true, false));
        rows.push(Row::separator());
        rows.push(item("spell", "Check Spelling While Typing", true, self.prefs.spell_check()));
        rows
    }

    /// The context menu as a native pop-up menu.
    fn context_popup(&self) -> PopupMenu {
        let mut menu = PopupMenu::new();
        for r in self.context_rows() {
            menu = match r.what {
                What::Pick(id) => menu.item(PopupItem::new(id, r.title).enabled(r.enabled).checked(r.checked)),
                _ => menu.separator(),
            };
        }
        menu
    }

    /// A right-click in the text.
    pub fn open_context_menu(&mut self, press: PointerPress, tx: &mut Tx) {
        if rune::popup_menu_supported() {
            tx.popup_menu(self.context_popup(), press, |s: &mut App, picked, tx| {
                if let Some(id) = picked {
                    s.context_pick(id, tx);
                }
            });
        } else {
            self.menus.context_at = (press.x, press.y);
            self.open_menu(Source::Context, false, tx);
        }
    }

    pub fn context_pick(&mut self, id: &str, tx: &mut Tx) {
        match id {
            "comment" => self.run(Command::AddComment, tx),
            "cut" => self.run(Command::Cut, tx),
            "copy" => self.run(Command::Copy, tx),
            "paste" => self.run(Command::Paste, tx),
            "spell" => self.run(Command::ToggleSpellCheck, tx),
            _ => {}
        }
    }

    /// Opens `source`, its first row highlighted when the keyboard opened it.
    pub fn open_menu(&mut self, source: Source, by_keys: bool, tx: &mut Tx) {
        let rows = self.source_rows(source);
        self.menus.open = Some(source);
        self.menus.sub = None;
        self.menus.in_sub = false;
        self.menus.row = if by_keys { next(&rows, None, 1) } else { None };
        if let Source::Menu(i) = source {
            self.menus.hot = i;
        }
        self.show_chrome(false, tx);
    }

    pub fn close_menus(&mut self, tx: &mut Tx) {
        let was = self.menus.open.is_some() || self.menus.armed;
        self.menus.open = None;
        self.menus.sub = None;
        self.menus.in_sub = false;
        self.menus.row = None;
        self.menus.armed = false;
        if was {
            self.show_chrome(true, tx);
        }
    }

    /// Picks row `i` of the open menu (or of its submenu when `in_sub`).
    pub fn pick_row(&mut self, i: usize, in_sub: bool, tx: &mut Tx) {
        let Some(source) = self.menus.open else { return };
        let rows = self.source_rows(source);
        let rows = match (in_sub, self.menus.sub) {
            (true, Some((r, _))) => match rows.get(r).map(|r| r.what) {
                Some(What::Submenu(inner)) => self.menu_rows(inner),
                _ => return,
            },
            _ => rows,
        };
        let Some(row) = rows.get(i).filter(|r| r.selectable()) else { return };
        match row.what {
            What::Submenu(inner) => {
                let first = next(&self.menu_rows(inner), None, 1);
                self.menus.sub = Some((i, first));
                self.menus.row = Some(i);
                self.menus.in_sub = true;
            }
            What::Command(c) => {
                self.close_menus(tx);
                self.run(c, tx);
            }
            What::Pick(id) => {
                self.close_menus(tx);
                self.context_pick(id, tx);
            }
            What::Separator => {}
        }
    }

    /// The pointer is over row `i` (of the submenu when `in_sub`).
    fn hover_row(&mut self, i: usize, in_sub: bool) {
        if in_sub {
            if let Some((r, _)) = self.menus.sub {
                self.menus.sub = Some((r, Some(i)));
                self.menus.in_sub = true;
            }
            return;
        }
        self.menus.row = Some(i);
        self.menus.in_sub = false;
        let Some(source) = self.menus.open else { return };
        let rows = self.source_rows(source);
        self.menus.sub = match rows.get(i).map(|r| r.what) {
            Some(What::Submenu(_)) => Some((i, None)),
            _ => None,
        };
    }
}

/// The next selectable row after `from` in direction `step` (±1), wrapping as menus do.
fn next(rows: &[Row], from: Option<usize>, step: i32) -> Option<usize> {
    let n = rows.len() as i32;
    if n == 0 {
        return None;
    }
    let mut i = from.map_or(if step > 0 { -1 } else { n }, |f| f as i32);
    for _ in 0..n {
        i = (i + step).rem_euclid(n);
        if rows[i as usize].selectable() {
            return Some(i as usize);
        }
    }
    None
}

fn is_alt(m: Modifier) -> bool {
    matches!(m, Modifier::AltLeft | Modifier::AltRight)
}

/// Keys for the menus, before anything else sees them. True when the menus took the key.
pub fn key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if !SHOWN {
        return false;
    }
    let m = &mut s.menus;
    if let Key::Modifier { modifier, pressed } = ev.key {
        if is_alt(modifier) {
            if pressed {
                m.alt_alone = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.shift;
            } else if std::mem::take(&mut m.alt_alone) {
                if m.open.is_some() || m.armed {
                    s.close_menus(tx);
                } else {
                    m.armed = true;
                    m.hot = 0;
                    s.show_chrome(false, tx);
                }
            }
        } else {
            m.alt_alone = false;
        }
        return false;
    }
    m.alt_alone = false;
    let plain = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt;
    let letter = match &ev.key {
        Key::Text(t) => t.chars().next().map(|c| c.to_ascii_lowercase()),
        Key::Command(c) => Some(c.to_ascii_lowercase()),
        _ => None,
    };
    let letter = tx.key_base().map(|c| c.to_ascii_lowercase()).or(letter);
    let mnemonic = |c: Option<char>| c.and_then(|c| MNEMONICS.iter().position(|(m, _)| *m == c));

    // Alt with a title's letter opens that menu.
    if ev.mods.alt && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.shift && let Some(i) = mnemonic(letter) {
        let source = if s.menus.folded { Source::All } else { Source::Menu(i) };
        s.menus.armed = false;
        s.open_menu(source, true, tx);
        if s.menus.folded {
            s.pick_row(i, false, tx);
        }
        return true;
    }
    if ev.key == Key::Function(10) && plain && !ev.mods.shift {
        if s.menus.open.is_some() || s.menus.armed {
            s.close_menus(tx);
        } else {
            s.menus.armed = true;
            s.menus.hot = 0;
            s.show_chrome(false, tx);
        }
        return true;
    }

    let Some(source) = s.menus.open else {
        if !s.menus.armed {
            return false;
        }
        let count = if s.menus.folded { 1 } else { MENUS.len() };
        match ev.key {
            Key::Left => s.menus.hot = (s.menus.hot + count - 1) % count,
            Key::Right => s.menus.hot = (s.menus.hot + 1) % count,
            Key::Down | Key::Enter => {
                let source = if s.menus.folded { Source::All } else { Source::Menu(s.menus.hot) };
                s.menus.armed = false;
                s.open_menu(source, true, tx);
            }
            Key::Escape => s.close_menus(tx),
            _ => match mnemonic(letter).filter(|_| plain) {
                Some(i) => {
                    s.menus.armed = false;
                    let source = if s.menus.folded { Source::All } else { Source::Menu(i) };
                    s.open_menu(source, true, tx);
                }
                None => {
                    s.close_menus(tx);
                    return false;
                }
            },
        }
        return true;
    };

    let rows = s.source_rows(source);
    let sub_rows = s.menus.sub.and_then(|(r, _)| match rows.get(r).map(|r| r.what) {
        Some(What::Submenu(inner)) => Some(s.menu_rows(inner)),
        _ => None,
    });
    let in_sub = s.menus.in_sub && sub_rows.is_some();
    match ev.key {
        Key::Up | Key::Down => {
            let step = if ev.key == Key::Up { -1 } else { 1 };
            if in_sub {
                let (r, at) = s.menus.sub.expect("in a submenu");
                s.menus.sub = Some((r, next(sub_rows.as_deref().unwrap_or(&[]), at, step)));
            } else {
                s.menus.row = next(&rows, s.menus.row, step);
                s.menus.sub = None;
            }
        }
        Key::Right => {
            let on_sub = !in_sub && s.menus.row.and_then(|r| rows.get(r)).is_some_and(|r| matches!(r.what, What::Submenu(_)));
            if on_sub {
                s.pick_row(s.menus.row.expect("a row"), false, tx);
            } else if let Source::Menu(i) = source {
                s.open_menu(Source::Menu((i + 1) % MENUS.len()), true, tx);
            }
        }
        Key::Left => {
            if in_sub {
                s.menus.in_sub = false;
                s.menus.sub = None;
            } else if let Source::Menu(i) = source {
                s.open_menu(Source::Menu((i + MENUS.len() - 1) % MENUS.len()), true, tx);
            }
        }
        Key::Enter => {
            if in_sub {
                if let Some((_, Some(i))) = s.menus.sub {
                    s.pick_row(i, true, tx);
                }
            } else if let Some(i) = s.menus.row {
                s.pick_row(i, false, tx);
            }
        }
        Key::Escape => {
            if in_sub {
                s.menus.in_sub = false;
                s.menus.sub = None;
            } else {
                s.close_menus(tx);
            }
        }
        _ => {}
    }
    // An open menu holds the keyboard, as a menu's tracking loop does.
    true
}

fn text_w(t: &str) -> f32 {
    (t.chars().count() as f32 * FONT * CHARACTER_WIDTH_EM).ceil()
}

fn title_w(t: &str) -> f32 {
    text_w(t) + TITLE_PAD * 2.0
}

/// Where the titles go: their x positions, or one folded button, after the sidebar button.
pub struct BarLayout {
    pub titles: Vec<[f32; 4]>,
    pub folded: Option<[f32; 4]>,
    /// The bar's right edge, which the centred title keeps clear of.
    pub right: f32,
}

pub fn bar_layout(sidebar: [f32; 4], toolbar_left: f32) -> BarLayout {
    if !SHOWN {
        return BarLayout { titles: Vec::new(), folded: None, right: 0.0 };
    }
    let (y, h) = (sidebar[1], sidebar[3]);
    let mut x = sidebar[0] + sidebar[2] + 6.0;
    let mut titles = Vec::new();
    for (t, _) in MENUS {
        let w = title_w(t);
        titles.push([x, y, w, h]);
        x += w;
    }
    if x + 12.0 <= toolbar_left - 12.0 {
        return BarLayout { titles, folded: None, right: x };
    }
    let b = [sidebar[0] + sidebar[2] + 6.0, y, chrome::SLOT.0, h];
    BarLayout { titles: Vec::new(), folded: Some(b), right: b[0] + b[2] }
}

/// The menu bar's titles in the top bar, and any open menu over the window.
pub fn view(s: &App, p: &Palette, bar: &BarLayout, opacity: f32, fade: Anim) -> (Vec<El>, Vec<El>) {
    let mut titles = Vec::new();
    let mut layer = Vec::new();
    let m = &s.menus;
    let show_letters = m.armed || m.open.is_some_and(|o| o != Source::Context);
    for (i, (t, _)) in MENUS.iter().enumerate() {
        let Some(r) = bar.titles.get(i).copied() else { break };
        let open = m.open == Some(Source::Menu(i));
        let hot = open || (m.armed && m.hot == i);
        let mut e = row()
            .key(format!("menu-title-{i}"))
            .role("menuitem")
            .label(*t)
            .expanded(open)
            .absolute(r[0], r[1])
            .size(r[2], r[3])
            .px(TITLE_PAD)
            .center_y()
            .radius(6.0)
            .fill(if hot { p.sidebar_selection.alpha(0.9) } else { Color::TRANSPARENT })
            .hover_group()
            .opacity(opacity)
            .opacity_motion(fade.clone())
            .child(text(*t).font(ui_font(FONT, false)).color(if hot { p.text } else { p.icon }).group_color(p.text))
            .on_press_tx(move |s: &mut App, tx| {
                if s.menus.open == Some(Source::Menu(i)) {
                    s.close_menus(tx);
                } else {
                    s.menus.armed = false;
                    s.open_menu(Source::Menu(i), false, tx);
                }
            })
            .on_hover_change(move |s: &mut App, inside, tx| {
                // With a menu open, pointing at another title opens that one instead.
                if inside && matches!(s.menus.open, Some(Source::Menu(j)) if j != i) {
                    s.open_menu(Source::Menu(i), false, tx);
                }
            });
        if !hot && s.win.key {
            e = e.hover(p.sidebar_selection.alpha(0.7));
        }
        if show_letters {
            let (_, at) = MNEMONICS[i];
            let cw = FONT * CHARACTER_WIDTH_EM;
            e = e.child(el().absolute(TITLE_PAD + at as f32 * cw, r[3] / 2.0 + 7.0).size(cw, 1.0).fill(p.text).a11y_hidden());
        }
        titles.push(e);
    }
    if let Some(r) = bar.folded {
        let open = m.open == Some(Source::All);
        let mut e = el()
            .key("menu-button")
            .role("button")
            .label("Menu")
            .help("Menu (Alt)")
            .expanded(open)
            .absolute(r[0], r[1])
            .size(r[2], r[3])
            .radius(6.0)
            .center()
            .fill(if open || m.armed { p.sidebar_selection.alpha(0.9) } else { Color::TRANSPARENT })
            .hover_group()
            .opacity(opacity)
            .opacity_motion(fade.clone())
            .child(icon(&LINES, 16.0).color(if open { p.caret } else { p.icon }).group_color(p.text))
            .on_press_tx(|s: &mut App, tx| {
                if s.menus.open == Some(Source::All) {
                    s.close_menus(tx);
                } else {
                    s.open_menu(Source::All, false, tx);
                }
            });
        if !open && s.win.key {
            e = e.hover(p.sidebar_selection.alpha(0.7));
        }
        titles.push(e);
    }

    let Some(source) = m.open else { return (titles, layer) };
    let at = match source {
        Source::Menu(i) => bar.titles.get(i).map(|r| (r[0], r[1] + r[3] + 4.0)),
        Source::All => bar.folded.map(|r| (r[0], r[1] + r[3] + 4.0)),
        Source::Context => Some(m.context_at),
    };
    let Some(at) = at else { return (titles, layer) };
    // Presses outside the menus close them, as clicking away from a menu does.
    layer.push(el().key("menu-scrim").absolute(0.0, 0.0).size(s.window.0, s.window.1).on_press_tx(|s: &mut App, tx| s.close_menus(tx)));
    let rows = s.source_rows(source);
    let (menu, rect, row_ys) = dropdown(s, p, "menu", &rows, at, m.row, m.sub.map(|(r, _)| r), false);
    layer.push(menu);
    if let Some((r, highlight)) = m.sub
        && let Some(What::Submenu(inner)) = rows.get(r).map(|r| r.what)
    {
        let sub_rows = s.menu_rows(inner);
        let w = menu_width(&sub_rows);
        let mut x = rect[0] + rect[2] - 2.0;
        if x + w > s.window.0 - 4.0 {
            x = (rect[0] - w + 2.0).max(4.0);
        }
        let (sub, _, _) = dropdown(s, p, "submenu", &sub_rows, (x, row_ys[r] - MENU_PAD), highlight, None, true);
        layer.push(sub);
    }
    (titles, layer)
}

fn menu_width(rows: &[Row]) -> f32 {
    let widest = rows
        .iter()
        .map(|r| {
            let shortcut = if r.shortcut.is_empty() { 0.0 } else { text_w(&r.shortcut) + 24.0 };
            let chevron = if matches!(r.what, What::Submenu(_)) { 16.0 } else { 0.0 };
            20.0 + text_w(&r.title) + shortcut + chevron + 10.0
        })
        .fold(0.0, f32::max);
    (widest + MENU_PAD * 2.0).max(MIN_MENU_W)
}

/// One menu's card at `at` (window points, kept inside the window): its element, its rect, and
/// each row's y.
#[allow(clippy::too_many_arguments)]
fn dropdown(s: &App, p: &Palette, key: &str, rows: &[Row], at: (f32, f32), highlight: Option<usize>, open_sub: Option<usize>, is_sub: bool) -> (El, [f32; 4], Vec<f32>) {
    let w = menu_width(rows);
    let h: f32 = rows.iter().map(|r| if matches!(r.what, What::Separator) { SEPARATOR_H } else { ROW_H }).sum::<f32>() + MENU_PAD * 2.0;
    let x = at.0.min(s.window.0 - w - 4.0).max(4.0);
    let y = if at.1 + h > s.window.1 - 4.0 { (s.window.1 - 4.0 - h).max(4.0) } else { at.1 };
    let mut card = rune::col()
        .key(key)
        .role("menu")
        .absolute(x, y)
        .w(w)
        .padding(MENU_PAD)
        .radius(8.0)
        .fill(p.card)
        .border(1.0, p.hairline)
        .shadow(6.0, 24.0, Color { r: 0.0, g: 0.0, b: 0.0, a: if p.dark { 0.45 } else { 0.16 } })
        .instant_changes();
    let mut ys = Vec::new();
    let mut cy = y + MENU_PAD;
    for (i, r) in rows.iter().enumerate() {
        ys.push(cy);
        if matches!(r.what, What::Separator) {
            card = card.child(el().key(format!("{key}-sep-{i}")).h(SEPARATOR_H).py(4.0).child(el().h(1.0).w_full().fill(p.hairline)).a11y_hidden());
            cy += SEPARATOR_H;
            continue;
        }
        cy += ROW_H;
        let lit = highlight == Some(i) || open_sub == Some(i);
        let mut e = row()
            .key(format!("{key}-row-{i}"))
            .role(if r.checked { "menuitemcheckbox" } else { "menuitem" })
            .label(r.title.as_str())
            .checked(r.checked)
            .disabled(!r.enabled)
            .h(ROW_H)
            .px(6.0)
            .gap(4.0)
            .center_y()
            .radius(5.0)
            .fill(if lit && r.enabled { p.sidebar_selection } else { Color::TRANSPARENT })
            .child(icon(&CHECK, 14.0).color(p.text).opacity(if r.checked { 1.0 } else { 0.0 }))
            .child(text(r.title.as_str()).font(ui_font(FONT, false)).color(p.text).grow(1.0).truncate());
        if !r.shortcut.is_empty() {
            e = e.shortcut_hint(r.shortcut.as_str()).child(text(r.shortcut.as_str()).font(ui_font(FONT, false)).color(p.quiet).a11y_hidden());
        }
        if matches!(r.what, What::Submenu(_)) {
            e = e.expanded(open_sub == Some(i)).child(icon(&CHEVRON, 12.0).color(p.quiet));
        }
        if r.enabled {
            e = e
                .on_press_tx(move |s: &mut App, tx| s.pick_row(i, is_sub, tx))
                .on_hover_change(move |s: &mut App, inside, _| {
                    if inside {
                        s.hover_row(i, is_sub);
                    }
                });
        } else {
            e = e.layer_opacity(0.4);
        }
        card = card.child(e);
    }
    (card, [x, y, w, h], ys)
}

/// The menu bar's place: after the sidebar button, clear of the toolbar.
pub fn layout(chrome: &Layout) -> BarLayout {
    bar_layout(chrome.sidebar, chrome.toolbar[0][0])
}
