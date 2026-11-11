//! The sidebar: recent files and the contents of an opened folder (local or over SSH), pinned
//! beside the text or peeking over it from the window's left edge. Port of `Sidebar.swift`, the
//! sidebar parts of `RemoteEditing.swift`, and the peek in `ChromeControls.swift`. The sidebar
//! button is the chrome's (`chrome.rs`); it runs View › Show Sidebar, [`App::toggle_sidebar`].

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rune::{Align, Anim, Color, Curve, Easing, El, FileDialogOptions, Icon, Key, KeyEvent, Latest, ScrollSpec, Shadow, Transition, Tx, Waker, col, el, icon, text};
use writer_core::draft_store::FILE_EXTENSIONS;
use writer_core::{RemoteEntry, RemoteLocation, SshTransport};

use crate::app::{App, Frame, SIDEBAR_WIDTH};
use crate::documents::standardize;
use crate::theme::{Palette, rgba, ui_font};

/// Room left at the top of the panel for the window controls, the sidebar button and (off
/// macOS) the in-window menu bar, which all sit within the top bar's 40 points
/// (`SidebarController.topInset`).
pub const TOP_INSET: f32 = crate::app::TOP_BAR_HEIGHT;
/// The key of the file list, which takes the keyboard when clicked.
pub const LIST: &str = "sidebar-list";

// Peek timings and distances (`ChromeControls.swift:171-246`).
const PEEK_EDGE: f32 = 4.0;
const PEEK_CONFIRM: f32 = 10.0;
const PEEK_DELAY: f64 = 0.12;
const PEEK_LEAVE_DISTANCE: f32 = SIDEBAR_WIDTH + 36.0;
const PEEK_LEAVE_DELAY: f64 = 0.25;
const PEEK_IN: f32 = 0.18;
const PEEK_OUT: f32 = 0.16;

/// Type-select keeps adding to what was typed while keys come this close together.
const TYPE_SELECT_GAP: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Recent,
    Folder,
}

impl Mode {
    fn raw(self) -> &'static str {
        match self {
            Mode::Recent => "recent",
            Mode::Folder => "folder",
        }
    }
}

/// A folder the sidebar lists: on this machine or over SSH.
#[derive(Clone, Debug, PartialEq)]
pub enum FolderRoot {
    Local(PathBuf),
    Remote(RemoteLocation),
}

impl FolderRoot {
    pub fn name(&self) -> String {
        match self {
            FolderRoot::Local(path) => path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string_lossy().into_owned()),
            FolderRoot::Remote(loc) if loc.name() == "~" => format!("{} ~", loc.host_name()),
            FolderRoot::Remote(loc) => loc.name(),
        }
    }

    /// The header's tooltip: the full path with `~` for home, or the remote location.
    pub fn detail(&self) -> String {
        match self {
            FolderRoot::Local(path) => abbreviate_home(path),
            FolderRoot::Remote(loc) => loc.display(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Directory,
    Parent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub kind: Kind,
    pub title: String,
    pub subtitle: Option<String>,
    pub path: Option<PathBuf>,
    pub remote: Option<RemoteLocation>,
}

impl Item {
    fn height(&self) -> f32 {
        if self.subtitle.is_some() { 42.0 } else { 28.0 }
    }
}

/// Where a peeking sidebar is in its slide.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Peek {
    Hidden,
    /// Mounted off the left edge; the next frame slides it in.
    Entering,
    Open,
    /// Sliding out; unmounts at this time.
    Leaving(f64),
}

type Listing = (u64, Result<Vec<RemoteEntry>, String>);

pub struct Sidebar {
    pub mode: Mode,
    /// The opened folder and the subfolders navigated into, root first.
    pub folder_stack: Vec<FolderRoot>,
    pub items: Vec<Item>,
    /// The empty, loading or error message over the list.
    pub message: Option<String>,
    pub selected: Option<usize>,
    pub peek: Peek,
    generation: u64,
    listing: Latest<Listing>,
    peek_at: Option<f64>,
    unpeek_at: Option<f64>,
    seen_pointer: (f32, f32),
    seen_selection: (usize, usize),
    typed: String,
    typed_at: f64,
}

impl Sidebar {
    pub fn new(prefs: &crate::prefs::Prefs) -> Sidebar {
        let root = match (prefs.folder_local(), prefs.folder_remote()) {
            (Some(path), _) => Some(FolderRoot::Local(PathBuf::from(path))),
            (None, Some(remote)) => RemoteLocation::parse(remote).map(FolderRoot::Remote),
            _ => None,
        };
        Sidebar {
            mode: if prefs.sidebar_mode() == "folder" { Mode::Folder } else { Mode::Recent },
            folder_stack: root.into_iter().collect(),
            items: Vec::new(),
            message: None,
            selected: None,
            peek: Peek::Hidden,
            generation: 0,
            listing: Latest::default(),
            peek_at: None,
            unpeek_at: None,
            seen_pointer: (-1.0, -1.0),
            seen_selection: (0, 0),
            typed: String::new(),
            typed_at: f64::NEG_INFINITY,
        }
    }

    pub fn set_waker(&self, waker: Waker) {
        self.listing.set_waker(waker);
    }

    fn header_shown(&self) -> bool {
        self.mode == Mode::Folder && !self.folder_stack.is_empty()
    }

    pub fn header(&self) -> String {
        match self.folder_stack.as_slice() {
            [] => String::new(),
            [only] => only.name(),
            stack => stack.iter().map(FolderRoot::name).collect::<Vec<_>>().join(" › "),
        }
    }

    fn parent_item(&self) -> Vec<Item> {
        if self.folder_stack.len() > 1 {
            vec![Item { kind: Kind::Parent, title: "..".into(), subtitle: None, path: None, remote: None }]
        } else {
            Vec::new()
        }
    }

    fn apply(&mut self, result: Result<Vec<RemoteEntry>, String>) {
        let Some(dir) = self.folder_stack.last().cloned() else { return };
        match result {
            Err(message) => {
                self.items = self.parent_item();
                self.message = Some(message);
            }
            Ok(entries) => {
                let mut visible: Vec<RemoteEntry> = entries
                    .into_iter()
                    .filter(|e| e.is_directory || FILE_EXTENSIONS.contains(&extension(&e.name).as_str()))
                    .collect();
                visible.sort_by(|a, b| if a.is_directory != b.is_directory { b.is_directory.cmp(&a.is_directory) } else { finder_order(&a.name, &b.name) });
                let mut items = self.parent_item();
                let empty = visible.is_empty();
                items.extend(visible.into_iter().map(|e| {
                    let kind = if e.is_directory { Kind::Directory } else { Kind::File };
                    match &dir {
                        FolderRoot::Local(path) => Item { kind, title: e.name.clone(), subtitle: None, path: Some(path.join(&e.name)), remote: None },
                        FolderRoot::Remote(loc) => Item { kind, title: e.name.clone(), subtitle: None, path: None, remote: Some(loc.appending(&e.name)) },
                    }
                }));
                self.items = items;
                self.message = empty.then(|| "No Markdown or text files here.".to_string());
            }
        }
        self.selected = None;
    }
}

// MARK: Listing helpers

/// The lowercased extension of a file name, as `pathExtension.lowercased()`.
fn extension(name: &str) -> String {
    Path::new(name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Finder's name order (`localizedStandardCompare`): case-insensitive, with runs of digits
/// compared by value, so "Draft 2" comes before "Draft 10".
pub fn finder_order(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (m, n) = (take(&mut x), take(&mut y));
                let (mt, nt) = (m.trim_start_matches('0'), n.trim_start_matches('0'));
                let order = mt.len().cmp(&nt.len()).then_with(|| mt.cmp(nt));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(c), Some(d)) => {
                let order = c.to_lowercase().cmp(d.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                x.next();
                y.next();
            }
        }
    }
}

/// A folder's entries on this machine, skipping hidden names. Errors read as Foundation's.
pub fn local_entries(dir: &Path) -> Result<Vec<RemoteEntry>, String> {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let read = std::fs::read_dir(dir).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => format!("The folder “{name}” doesn’t exist."),
        std::io::ErrorKind::PermissionDenied => format!("The folder “{name}” couldn’t be opened because you don’t have permission to view it."),
        _ => format!("The folder “{name}” couldn’t be opened."),
    })?;
    Ok(read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            Some(RemoteEntry::new(name, e.file_type().is_ok_and(|t| t.is_dir())))
        })
        .collect())
}

/// `path` with `~` for the home folder (`abbreviatingWithTildeInPath`).
pub fn abbreviate_home(path: &Path) -> String {
    if let Some(home) = std::env::home_dir()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        return if rest.as_os_str().is_empty() { "~".into() } else { format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()) };
    }
    path.display().to_string()
}

// MARK: The app's side

impl App {
    /// Lists the sidebar again from scratch (`SidebarController.reload`).
    pub fn reload_sidebar(&mut self) {
        self.sidebar.generation += 1;
        match self.sidebar.mode {
            Mode::Recent => self.show_recent(),
            Mode::Folder => self.show_folder(),
        }
    }

    /// Re-marks the open file; the Recent list is listed again since its order changed.
    pub fn sidebar_document_changed(&mut self) {
        if self.sidebar.mode == Mode::Recent {
            self.show_recent();
        } else {
            self.sidebar.selected = None;
        }
    }

    fn show_recent(&mut self) {
        let items: Vec<Item> = self
            .library
            .recent_drafts(100)
            .into_iter()
            .map(|s| {
                let subtitle = match &s.remote {
                    Some(remote) => remote.display(),
                    None => abbreviate_home(s.path.parent().unwrap_or(Path::new(""))),
                };
                Item { kind: Kind::File, title: s.title, subtitle: Some(subtitle), path: Some(s.path), remote: s.remote }
            })
            .collect();
        self.sidebar.message = items.is_empty().then(|| "Files you open show up here.".to_string());
        self.sidebar.items = items;
        self.sidebar.selected = None;
    }

    fn show_folder(&mut self) {
        let sb = &mut self.sidebar;
        let Some(dir) = sb.folder_stack.last().cloned() else {
            sb.items.clear();
            sb.message = Some("Open a folder to see its files here.".into());
            sb.selected = None;
            return;
        };
        sb.items = sb.parent_item();
        sb.selected = None;
        let generation = sb.generation;
        match dir {
            FolderRoot::Local(path) => {
                sb.message = None;
                sb.listing.request(path, move |path| (generation, local_entries(&path)));
            }
            FolderRoot::Remote(loc) => {
                sb.message = Some(format!("Loading {}…", loc.host_name()));
                let ssh: SshTransport = self.remote.ssh.clone();
                sb.listing.request(loc, move |loc| (generation, ssh.list(&loc).map_err(|e| e.to_string())));
            }
        }
    }

    /// Shows `root` in Folder mode and remembers it.
    pub fn set_sidebar_folder(&mut self, root: FolderRoot) {
        match &root {
            FolderRoot::Local(path) => self.prefs.set_folder(Some(path.to_string_lossy().into_owned()), None),
            FolderRoot::Remote(loc) => self.prefs.set_folder(None, Some(loc.display())),
        }
        self.sidebar.folder_stack = vec![root];
        self.set_sidebar_mode(Mode::Folder);
    }

    pub fn set_sidebar_mode(&mut self, mode: Mode) {
        self.sidebar.mode = mode;
        self.prefs.set_sidebar_mode(mode.raw());
        self.reload_sidebar();
    }

    /// View › Show/Hide Sidebar and the sidebar button. A peeking sidebar is pinned instead.
    pub fn toggle_sidebar(&mut self, tx: &mut Tx) {
        if self.sidebar_peeking {
            self.pin_peeking_sidebar();
            return;
        }
        self.sidebar_visible = !self.sidebar_visible;
        if self.sidebar_visible {
            self.reload_sidebar();
        }
        self.prefs.set_show_sidebar(self.sidebar_visible);
        self.show_chrome(true, tx);
    }

    /// Pins the sidebar if needed and switches it to `mode`.
    pub fn show_sidebar(&mut self, mode: Mode, tx: &mut Tx) {
        if !self.sidebar_visible {
            self.toggle_sidebar(tx);
        }
        self.set_sidebar_mode(mode);
    }

    /// File › Open Folder…: a folder dialog starting in the current local root.
    pub fn open_folder_dialog(&mut self, tx: &mut Tx) {
        let directory = match self.sidebar.folder_stack.first() {
            Some(FolderRoot::Local(path)) => Some(path.clone()),
            _ => None,
        };
        // Rune's dialog options carry a title only: the Swift panel's button ("Open Folder") and
        // message can't be set, so the title carries the message.
        let options = FileDialogOptions { title: "Choose a folder to list its Markdown and text files in the sidebar.".into(), directory, ..Default::default() };
        tx.open_folder(options, |s: &mut App, result, tx| {
            if let Ok(Some(path)) = result {
                if !s.sidebar_visible {
                    s.toggle_sidebar(tx);
                }
                s.set_sidebar_folder(FolderRoot::Local(standardize(&path)));
            }
        });
    }

    fn is_current(&self, item: &Item) -> bool {
        if item.kind != Kind::File {
            return false;
        }
        if let Some(remote) = &item.remote {
            return self.draft.remote() == Some(remote);
        }
        item.path.is_some() && item.path.as_deref() == self.draft.path()
    }

    /// A click on a row, or Return on the selected one.
    pub fn activate_sidebar_item(&mut self, index: usize, tx: &mut Tx) {
        let Some(item) = self.sidebar.items.get(index).cloned() else { return };
        match item.kind {
            Kind::Parent => {
                self.sidebar.folder_stack.pop();
                self.reload_sidebar();
            }
            Kind::Directory => {
                if let Some(path) = item.path {
                    self.sidebar.folder_stack.push(FolderRoot::Local(path));
                }
                if let Some(remote) = item.remote {
                    self.sidebar.folder_stack.push(FolderRoot::Remote(remote));
                }
                self.reload_sidebar();
            }
            Kind::File => {
                self.sidebar.selected = None;
                if let Some(remote) = item.remote {
                    self.open_remote(remote, tx);
                } else if let Some(path) = item.path {
                    self.open(&path, tx);
                }
            }
        }
    }

    // MARK: Peek

    fn begin_peek(&mut self, tx: &mut Tx) {
        if self.sidebar_visible || self.sidebar_peeking {
            return;
        }
        self.sidebar_peeking = true;
        self.sidebar.peek = Peek::Entering;
        self.reload_sidebar();
        self.show_chrome(false, tx);
        tx.wake_after(Duration::ZERO);
    }

    pub fn end_peek(&mut self, tx: &mut Tx) {
        if !self.sidebar_peeking || matches!(self.sidebar.peek, Peek::Leaving(_)) {
            return;
        }
        self.sidebar.unpeek_at = None;
        self.sidebar.peek = Peek::Leaving(self.now + PEEK_OUT as f64);
        tx.wake_after(Duration::from_secs_f32(PEEK_OUT));
    }

    fn pin_peeking_sidebar(&mut self) {
        if !self.sidebar_peeking {
            return;
        }
        self.sidebar.peek_at = None;
        self.sidebar.unpeek_at = None;
        self.sidebar_peeking = false;
        self.sidebar.peek = Peek::Hidden;
        self.sidebar_visible = true;
        self.prefs.set_show_sidebar(true);
    }

    /// The pointer moved: start or cancel the peek timers (`trackSidebarPeek`).
    fn track_peek(&mut self, tx: &mut Tx) {
        if !self.win.key {
            return;
        }
        let (x, y) = self.pointer;
        if self.sidebar_peeking {
            if x > PEEK_LEAVE_DISTANCE {
                if self.sidebar.unpeek_at.is_none() {
                    self.sidebar.unpeek_at = Some(self.now + PEEK_LEAVE_DELAY);
                    tx.wake_after(Duration::from_secs_f64(PEEK_LEAVE_DELAY));
                }
            } else {
                self.sidebar.unpeek_at = None;
            }
            return;
        }
        if self.sidebar_visible || x > PEEK_EDGE || y <= 0.0 || y >= self.window.1 || x < 0.0 {
            if !self.sidebar_visible {
                self.sidebar.peek_at = None;
            }
            return;
        }
        if self.sidebar.peek_at.is_none() {
            self.sidebar.peek_at = Some(self.now + PEEK_DELAY);
            tx.wake_after(Duration::from_secs_f64(PEEK_DELAY));
        }
    }

    /// The sidebar's share of `App::poll`: listings that landed, peek timers, and a click in
    /// the text closing a peek. Returns whether anything the view reads changed.
    pub fn poll_sidebar(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some((generation, result)) = self.sidebar.listing.take() {
            if generation == self.sidebar.generation && self.sidebar.mode == Mode::Folder {
                self.sidebar.apply(result);
            }
            changed = true;
        }
        let now = self.now;
        // The frame before drew the panel off the left edge; slide it in from there.
        if self.sidebar.peek == Peek::Entering {
            self.sidebar.peek = Peek::Open;
            changed = true;
        }
        let selection = self.edit.range();
        // A click in the text moves the caret without a key: that closes a peek.
        if self.sidebar_peeking && selection != self.sidebar.seen_selection && !self.typed_since_poll && self.pointer.0 > SIDEBAR_WIDTH {
            self.end_peek(tx);
            changed = true;
        }
        self.sidebar.seen_selection = selection;
        if self.pointer != self.sidebar.seen_pointer {
            self.sidebar.seen_pointer = self.pointer;
            self.track_peek(tx);
        }
        if self.sidebar.peek_at.is_some_and(|t| t <= now) {
            self.sidebar.peek_at = None;
            if self.pointer.0 <= PEEK_CONFIRM && self.pointer.0 >= 0.0 {
                self.begin_peek(tx);
                changed = true;
            }
        }
        if self.sidebar.unpeek_at.is_some_and(|t| t <= now) {
            self.sidebar.unpeek_at = None;
            self.end_peek(tx);
            changed = true;
        }
        if let Peek::Leaving(at) = self.sidebar.peek
            && at <= now
        {
            self.sidebar.peek = Peek::Hidden;
            if self.sidebar_peeking {
                self.sidebar_peeking = false;
                self.show_chrome(true, tx);
            }
            changed = true;
        }
        changed
    }

    // MARK: Keyboard

    fn sidebar_select(&mut self, index: usize, tx: &mut Tx) {
        self.sidebar.selected = Some(index);
        tx.scroll_into_view(row_key(index));
    }

    /// Keys for the focused file list: arrows move, Return opens, letters type-select.
    pub fn sidebar_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        let count = self.sidebar.items.len();
        let plain = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt;
        match &ev.key {
            Key::Down | Key::Up if plain && count > 0 => {
                let next = match (self.sidebar.selected, &ev.key) {
                    (None, _) => 0,
                    (Some(i), Key::Down) => (i + 1).min(count - 1),
                    (Some(i), _) => i.saturating_sub(1),
                };
                self.sidebar_select(next, tx);
                true
            }
            Key::Enter if plain => match self.sidebar.selected {
                Some(i) => {
                    self.activate_sidebar_item(i, tx);
                    true
                }
                None => false,
            },
            Key::Text(t) if plain && !t.trim().is_empty() => {
                if self.now - self.sidebar.typed_at > TYPE_SELECT_GAP {
                    self.sidebar.typed.clear();
                }
                self.sidebar.typed_at = self.now;
                self.sidebar.typed.push_str(&t.to_lowercase());
                let typed = self.sidebar.typed.clone();
                if let Some(i) = self.sidebar.items.iter().position(|item| item.title.to_lowercase().starts_with(&typed)) {
                    self.sidebar_select(i, tx);
                }
                true
            }
            _ => false,
        }
    }
}

// MARK: Icons (vector stand-ins for the SF Symbols the Swift app used)

/// An outlined 16-unit icon, in the weight of an 11 pt regular symbol.
macro_rules! glyph {
    ($name:ident, $id:literal, $body:literal) => {
        pub static $name: Icon = Icon { name: $id, view: 16.0, body: concat!(r#"<g fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round">"#, $body, "</g>") };
    };
}

glyph!(DOC_TEXT, "fw-doc-text", r#"<path d="M4 1.75h5.25L12.5 5v9.25H4z"/><path d="M9.25 1.75V5h3.25"/><path d="M6 8h4.5M6 10.25h4.5M6 12.5h3"/>"#);
glyph!(FOLDER, "fw-folder", r#"<path d="M1.75 4.25a1 1 0 0 1 1-1h3.4l1.4 1.5h5.7a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1H2.75a1 1 0 0 1-1-1z"/><path d="M1.75 6.5h12.5"/>"#);
glyph!(NETWORK, "fw-network", r#"<circle cx="8" cy="8" r="6.25"/><ellipse cx="8" cy="8" rx="2.6" ry="6.25"/><path d="M1.75 8h12.5M2.7 4.9h10.6M2.7 11.1h10.6"/>"#);
glyph!(PARENT, "fw-arrow-turn-left-up", r#"<path d="M13.5 12.5H6.5a2 2 0 0 1-2-2V3"/><path d="M1.75 5.75L4.5 3l2.75 2.75"/>"#);
glyph!(REFRESH, "fw-arrow-clockwise", r#"<path d="M12.9 8.6A5 5 0 1 1 11.2 4.2"/><path d="M11.8 1.6l-.4 2.8-2.9-.3"/>"#);

// MARK: View

pub fn row_key(i: usize) -> String {
    format!("sidebar-row-{i}")
}

fn shadow_black(alpha: f32) -> Color {
    rgba(0, 0, 0, alpha)
}

/// The panel, when pinned or peeking.
pub fn view(s: &App, p: &Palette, f: &Frame) -> Option<El> {
    let sb = &s.sidebar;
    let peeking = s.sidebar_peeking;
    if !peeking && !s.sidebar_visible {
        return None;
    }
    let pw = if peeking { SIDEBAR_WIDTH } else { f.side_w };
    let h = f.height;
    let w = (pw - 1.0).max(0.0);
    // Off macOS the window's top 40 points are its menu bar, whose titles run past a pinned
    // sidebar's edge: the panel's fill and hairline start under it, so the band reads as one
    // bar. On macOS the panel runs to the top, as in the Swift.
    let band = if crate::menubar::SHOWN { TOP_INSET } else { 0.0 };

    // AppKit adds and removes the panel and its rows at once, with no fade: the panel has no
    // entrance, and everything inside it changes at once.
    let mut panel = el().key("sidebar-content").absolute(0.0, 0.0).size(pw, h).transition(Transition::none()).instant_changes();
    // The 1 pt hairline on the right edge.
    panel = panel.child(el().absolute(w, band).size(1.0, (h - band).max(0.0)).fill(p.hairline));
    panel = panel.child(modes(s, p, w));

    let show_header = sb.header_shown();
    let header_y = TOP_INSET + 34.0;
    if show_header {
        let detail = sb.folder_stack.last().map(FolderRoot::detail).unwrap_or_default();
        panel = panel.child(text(sb.header()).key("sidebar-header").font(ui_font(11.0, false)).color(p.quiet).truncate_middle().absolute(16.0, header_y).size((w - 52.0).max(0.0), 16.0).help(detail));
        panel = panel.child(
            el().key("sidebar-refresh")
                .absolute(w - 32.0, header_y - 2.0)
                .size(20.0, 20.0)
                .center()
                .role("button")
                .label("Refresh folder")
                .help("Refresh")
                .on_press(|s: &mut App| s.reload_sidebar())
                .child(icon(&REFRESH, 13.0).color(p.quiet)),
        );
    }
    let list_y = if show_header { header_y + 22.0 } else { TOP_INSET + 34.0 };

    let rows = sb.items.iter().enumerate().map(|(i, item)| row(s, p, item, i, w));
    panel = panel.child(
        col()
            .key(LIST)
            .absolute(0.0, list_y)
            .size(w, (h - list_y).max(0.0))
            .scroll(ScrollSpec { stick_bottom: false })
            .focusable(true)
            .no_focus_ring()
            .role("list")
            .label("Files")
            .on_key(|s: &mut App, ev, tx| s.sidebar_key(ev, tx))
            .child(col().w_full().children(rows)),
    );
    if let Some(message) = &sb.message {
        panel = panel.child(text(message.clone()).key("sidebar-message").font(ui_font(12.0, false)).color(p.quiet).align(Align::Center).wrap().absolute(20.0, list_y + 24.0).size((w - 40.0).max(0.0), 60.0));
    }
    if sb.mode == Mode::Folder && sb.folder_stack.is_empty() {
        let label = "Open Folder…";
        let bw = crate::ui::small_button_width(label);
        panel = panel.child(crate::ui::small_button("sidebar-open-folder", label, p, |s: &mut App, tx| s.open_folder_dialog(tx)).absolute(((w - bw) / 2.0).round(), list_y + 90.0));
    }

    let mut fill = el().key("sidebar-fill").absolute(0.0, band).size(pw, (h - band).max(0.0)).fill(p.sidebar);
    if peeking {
        fill = fill.box_shadow(Shadow { x: 4.0, y: 0.0, blur: 36.0, spread: 0.0, color: shadow_black(0.22), inset: false });
    }
    let mut outer = el().key("sidebar").absolute(0.0, 0.0).size(pw, h).label("Sidebar").transition(Transition::none()).child(fill).child(panel);
    if peeking {
        let open = sb.peek == Peek::Open;
        let (offset, anim) = if open {
            (0.0, Anim::from(Curve::tween(PEEK_IN, Easing::EASE_OUT)))
        } else {
            (-SIDEBAR_WIDTH, Anim::from(Curve::tween(PEEK_OUT, Easing::EASE_IN)))
        };
        outer = outer.offset(offset, 0.0).offset_motion(anim);
    }
    Some(outer)
}

/// The Recent | Folder switch, drawn as AppKit's small segmented control.
fn modes(s: &App, p: &Palette, w: f32) -> El {
    let width = (w - 28.0).max(0.0);
    let seg = (width - 2.0) / 2.0;
    let segment = |label: &'static str, mode: Mode, i: usize| {
        let on = s.sidebar.mode == mode;
        el().key(if i == 0 { "sidebar-mode-recent" } else { "sidebar-mode-folder" })
            .absolute(1.0 + seg * i as f32, 1.0)
            .size(seg, 20.0)
            .radius(5.0)
            .fill(if on { p.sidebar_selection } else { rgba(0, 0, 0, 0.0) })
            .center()
            .role("radio")
            .checked(on)
            .label(label)
            .on_press(move |s: &mut App| s.set_sidebar_mode(mode))
            .child(text(label).font(ui_font(11.0, false)).color(if on { p.text } else { p.text.alpha(0.8) }))
    };
    el().key("sidebar-modes")
        .absolute(14.0, TOP_INSET + 2.0)
        .size(width, 22.0)
        .radius(6.0)
        .fill(p.sidebar_current)
        .role("radiogroup")
        .label("Sidebar view")
        .child(segment("Recent", Mode::Recent, 0))
        .child(segment("Folder", Mode::Folder, 1))
}

fn row(s: &App, p: &Palette, item: &Item, i: usize, w: f32) -> El {
    let h = item.height();
    let current = s.is_current(item);
    let selected = s.sidebar.selected == Some(i);
    let glyph = match item.kind {
        Kind::Parent => &PARENT,
        Kind::Directory => &FOLDER,
        Kind::File if item.remote.is_some() => &NETWORK,
        Kind::File => &DOC_TEXT,
    };
    let label = match &item.subtitle {
        Some(sub) => format!("{}, {}", item.title, sub),
        None => item.title.clone(),
    };
    let mut r = el().key(row_key(i)).w(w).h(h).role("listitem").label(label).on_press_tx(move |s: &mut App, tx| {
        s.sidebar.selected = Some(i);
        s.activate_sidebar_item(i, tx);
    });
    if selected || current {
        let fill = if selected { p.sidebar_selection } else { p.sidebar_current };
        r = r.child(el().absolute(8.0, 1.0).size((w - 16.0).max(0.0), h - 2.0).radius(6.0).fill(fill));
    }
    let title_color = if current { p.text } else { p.text.alpha(0.8) };
    // The Swift cell view isn't flipped: its frames count up from the row's bottom. These are
    // the same frames measured from the top.
    let (icon_y, title_y) = if item.subtitle.is_some() { (6.0, 4.0) } else { (7.0, 6.0) };
    r = r.child(icon(glyph, 14.0).color(p.quiet).absolute(18.0, icon_y));
    r = r.child(text(item.title.clone()).font(ui_font(13.0, false)).color(title_color).truncate().absolute(38.0, title_y).size((w - 56.0).max(0.0), 17.0));
    if let Some(sub) = &item.subtitle {
        r = r.child(text(sub.clone()).font(ui_font(11.0, false)).color(p.quiet).truncate_middle().absolute(38.0, 23.0).size((w - 56.0).max(0.0), 15.0));
    }
    r
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::commands::Command;
    use crate::tests::settle;
    use rune::{Env, KeyEvent, Mods, Runtime};
    use writer_core::DraftLibrary;

    /// The app on a library whose drafts folder holds `files`, with the first one open.
    pub(crate) fn app_with(files: &[(&str, &str)]) -> (Runtime<App>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let drafts = dir.path().join("drafts");
        std::fs::create_dir_all(&drafts).unwrap();
        for (i, (name, body)) in files.iter().enumerate() {
            let path = drafts.join(name);
            std::fs::write(&path, body).unwrap();
            // Distinct, ordered modification times: the first file is the newest.
            let t = std::time::SystemTime::now() - Duration::from_secs(60 * (i as u64 + 1));
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(t).unwrap();
        }
        let mut library = DraftLibrary::new(&drafts, dir.path().join("support")).unwrap();
        let doc = match files.first() {
            Some((name, _)) => library.open(&drafts.join(name)).unwrap(),
            None => library.new_draft(),
        };
        let app = App::new(library, crate::prefs::Prefs::in_memory(), doc);
        let mut rt = Runtime::new(app, crate::view::view, (1060.0, 760.0), Env { system_dark: false, scale: 1.0 });
        rt.poll = Some(|s, clock, tx| s.poll(clock, tx));
        settle(&mut rt, 0.2);
        (rt, dir)
    }

    /// Runs frames until `done` holds, sleeping between them so background threads can answer.
    pub(crate) fn wait_for(rt: &mut Runtime<App>, what: &str, done: impl Fn(&App) -> bool) {
        for _ in 0..1500 {
            rt.frame(1.0 / 60.0);
            if done(&rt.state) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for {what}");
    }

    pub(crate) fn click(rt: &mut Runtime<App>, key: &str) {
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key}"));
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        rt.frame(1.0 / 60.0);
    }

    pub(crate) fn run(rt: &mut Runtime<App>, c: Command) {
        rt.update(|s, tx| s.run(c, tx));
        rt.frame(1.0 / 60.0);
    }

    fn titles(s: &App) -> Vec<String> {
        s.sidebar.items.iter().map(|i| i.title.clone()).collect()
    }

    fn key(rt: &mut Runtime<App>, key: Key) {
        rt.key(KeyEvent { key, mods: Mods::default() });
        rt.frame(1.0 / 60.0);
    }

    #[test]
    fn show_sidebar_pins_it_lists_recent_files_and_retitles_the_command() {
        let (mut rt, _dir) = app_with(&[("One.md", "One\n\nfirst"), ("Two.md", "Two\n\nsecond")]);
        assert!(!rt.state.sidebar_visible);
        assert_eq!(rt.state.command_title(Command::ToggleSidebar, "Show Sidebar"), "Show Sidebar");
        assert!(rt.find("sidebar").is_none());
        run(&mut rt, Command::ToggleSidebar);
        let s = &rt.state;
        assert!(s.sidebar_visible && s.prefs.show_sidebar());
        assert_eq!(s.frame().side_w, 260.0);
        assert_eq!(s.command_title(Command::ToggleSidebar, "Show Sidebar"), "Hide Sidebar");
        assert_eq!(titles(s), ["One", "Two"]);
        assert_eq!(s.sidebar.items[0].subtitle.as_deref(), Some(abbreviate_home(&_dir.path().join("drafts")).as_str()));
        // The open file is marked, nothing is selected.
        assert!(s.is_current(&s.sidebar.items[0]) && !s.is_current(&s.sidebar.items[1]));
        assert_eq!(s.sidebar.selected, None);
        let row = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("sidebar-row-0")).unwrap();
        assert_eq!(row.rect[3], 42.0);
        run(&mut rt, Command::ToggleSidebar);
        assert!(!rt.state.sidebar_visible && !rt.state.prefs.show_sidebar());
        assert_eq!(rt.state.frame().side_w, 0.0);
    }

    #[test]
    fn the_pinned_sidebar_gives_way_in_a_narrow_window() {
        let (mut rt, _dir) = app_with(&[("One.md", "One")]);
        run(&mut rt, Command::ToggleSidebar);
        rt.state.window = (500.0, 400.0);
        assert_eq!(rt.state.frame().side_w, 180.0);
    }

    #[test]
    fn recent_is_empty_until_files_are_opened() {
        let (mut rt, _dir) = app_with(&[]);
        run(&mut rt, Command::RecentFiles);
        assert!(rt.state.sidebar_visible);
        assert_eq!(rt.state.sidebar.mode, Mode::Recent);
        assert_eq!(rt.state.sidebar.message.as_deref(), Some("Files you open show up here."));
        crate::tests::type_text(&mut rt, "Hello");
        settle(&mut rt, 1.0);
        // The first save puts the draft in the list.
        assert_eq!(titles(&rt.state), ["Hello"]);
        assert_eq!(rt.state.sidebar.message, None);
    }

    #[test]
    fn clicking_a_recent_file_opens_it() {
        let (mut rt, dir) = app_with(&[("One.md", "One"), ("Two.md", "Two body")]);
        run(&mut rt, Command::ToggleSidebar);
        click(&mut rt, "sidebar-row-1");
        assert_eq!(rt.state.edit.text, "Two body");
        assert_eq!(rt.state.draft.path(), Some(dir.path().join("drafts/Two.md").as_path()));
        // The list follows: the file just opened comes first and is marked.
        assert_eq!(titles(&rt.state), ["Two", "One"]);
        assert!(rt.state.is_current(&rt.state.sidebar.items[0]));
        assert_eq!(rt.state.sidebar.selected, None);
    }

    #[test]
    fn folder_mode_lists_sorts_goes_in_and_up() {
        let (mut rt, dir) = app_with(&[("One.md", "One")]);
        let folder = dir.path().join("folder");
        for (name, body) in [("Draft 10.txt", ""), ("Draft 2.MD", ""), ("notes.markdown", ""), ("image.png", ""), (".hidden.md", "")] {
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join(name), body).unwrap();
        }
        std::fs::create_dir_all(folder.join("Sub/deeper")).unwrap();
        std::fs::write(folder.join("Sub/inside.md"), "inside").unwrap();
        run(&mut rt, Command::ToggleSidebar);
        rt.update(|s, _| s.set_sidebar_folder(FolderRoot::Local(folder.clone())));
        wait_for(&mut rt, "the listing", |s| !s.sidebar.items.is_empty());
        let s = &rt.state;
        assert_eq!(titles(s), ["Sub", "Draft 2.MD", "Draft 10.txt", "notes.markdown"]);
        assert_eq!(s.sidebar.header(), "folder");
        assert_eq!(s.prefs.folder_local(), Some(folder.to_string_lossy().as_ref()));
        assert_eq!(s.prefs.sidebar_mode(), "folder");
        // Rows without a subtitle are 28 tall; the list starts under the header.
        let rect = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k)).unwrap().rect;
        assert_eq!(rect(&rt, "sidebar-row-0")[3], 28.0);
        assert_eq!(rect(&rt, LIST)[1], 96.0);
        assert_eq!(rect(&rt, "sidebar-header"), [16.0, 74.0, 207.0, 16.0]);
        assert_eq!(rect(&rt, "sidebar-refresh"), [227.0, 72.0, 20.0, 20.0]);

        click(&mut rt, "sidebar-row-0");
        wait_for(&mut rt, "the subfolder", |s| s.sidebar.items.len() == 3);
        assert_eq!(titles(&rt.state), ["..", "deeper", "inside.md"]);
        assert_eq!(rt.state.sidebar.header(), "folder › Sub");
        assert_eq!(rt.state.sidebar.items[0].subtitle, None);
        // Only the root is remembered.
        assert_eq!(rt.state.prefs.folder_local(), Some(folder.to_string_lossy().as_ref()));

        click(&mut rt, "sidebar-row-1");
        wait_for(&mut rt, "the empty folder", |s| s.sidebar.message.is_some());
        assert_eq!(titles(&rt.state), [".."]);
        assert_eq!(rt.state.sidebar.message.as_deref(), Some("No Markdown or text files here."));

        click(&mut rt, "sidebar-row-0");
        wait_for(&mut rt, "going up", |s| s.sidebar.items.len() == 3);
        click(&mut rt, "sidebar-row-2");
        assert_eq!(rt.state.edit.text, "inside");

        // A folder that went away shows the error, keeping "..".
        std::fs::remove_dir_all(folder.join("Sub/deeper")).unwrap();
        rt.update(|s, _| {
            s.sidebar.folder_stack.push(FolderRoot::Local(folder.join("Sub/deeper")));
            s.reload_sidebar();
        });
        wait_for(&mut rt, "the error", |s| s.sidebar.message.is_some());
        assert_eq!(titles(&rt.state), [".."]);
        assert_eq!(rt.state.sidebar.message.as_deref(), Some("The folder “deeper” doesn’t exist."));
    }

    #[test]
    fn folder_mode_without_a_folder_offers_open_folder() {
        let (mut rt, dir) = app_with(&[("One.md", "One")]);
        run(&mut rt, Command::ToggleSidebar);
        click(&mut rt, "sidebar-mode-folder");
        assert_eq!(rt.state.sidebar.mode, Mode::Folder);
        assert_eq!(rt.state.sidebar.message.as_deref(), Some("Open a folder to see its files here."));
        assert!(rt.find("sidebar-header").is_none());
        click(&mut rt, "sidebar-open-folder");
        let (kind, options) = rt.file_dialog().expect("a folder dialog");
        assert_eq!(kind, rune::FileDialogKind::Folder);
        assert_eq!(options.directory, None);
        rt.resolve_file_dialog(Some(dir.path().join("drafts"))).unwrap();
        wait_for(&mut rt, "the folder", |s| s.sidebar.items.len() == 1);
        assert_eq!(titles(&rt.state), ["One.md"]);
        // The next dialog starts in the folder now shown.
        run(&mut rt, Command::OpenFolder);
        assert_eq!(rt.file_dialog().unwrap().1.directory.as_deref(), Some(dir.path().join("drafts").as_path()));
        rt.resolve_file_dialog(None).unwrap();
    }

    #[test]
    fn open_folder_pins_a_hidden_sidebar() {
        let (mut rt, dir) = app_with(&[("One.md", "One")]);
        run(&mut rt, Command::OpenFolder);
        rt.resolve_file_dialog(Some(dir.path().join("drafts/../drafts"))).unwrap();
        wait_for(&mut rt, "the folder", |s| s.sidebar.items.len() == 1);
        assert!(rt.state.sidebar_visible);
        assert_eq!(rt.state.sidebar.folder_stack, [FolderRoot::Local(dir.path().join("drafts"))]);
    }

    #[test]
    fn keyboard_moves_type_selects_and_opens() {
        let (mut rt, _dir) = app_with(&[("Apple.md", "a"), ("Banana.md", "b"), ("Cherry.md", "c")]);
        run(&mut rt, Command::ToggleSidebar);
        rt.update(|_, tx| tx.focus(LIST, false));
        rt.frame(1.0 / 60.0);
        key(&mut rt, Key::Down);
        assert_eq!(rt.state.sidebar.selected, Some(0));
        key(&mut rt, Key::Down);
        key(&mut rt, Key::Down);
        key(&mut rt, Key::Down);
        assert_eq!(rt.state.sidebar.selected, Some(2));
        key(&mut rt, Key::Up);
        assert_eq!(rt.state.sidebar.selected, Some(1));
        key(&mut rt, Key::Text("c".into()));
        assert_eq!(rt.state.sidebar.selected, Some(2));
        key(&mut rt, Key::Text("h".into()));
        assert_eq!(rt.state.sidebar.selected, Some(2));
        settle(&mut rt, 1.2);
        key(&mut rt, Key::Text("b".into()));
        assert_eq!(rt.state.sidebar.selected, Some(1));
        key(&mut rt, Key::Enter);
        assert_eq!(rt.state.edit.text, "b");
        assert_eq!(rt.state.sidebar.selected, None);
    }

    #[test]
    fn pointer_at_the_left_edge_peeks_and_moving_away_closes() {
        let (mut rt, _dir) = app_with(&[("One.md", "One")]);
        rt.pointer_move(400.0, 300.0);
        settle(&mut rt, 0.1);
        rt.pointer_move(3.0, 300.0);
        settle(&mut rt, 0.08);
        assert!(!rt.state.sidebar_peeking, "not before 0.12 s");
        settle(&mut rt, 0.08);
        assert!(rt.state.sidebar_peeking);
        // Peeking doesn't take room from the editor.
        assert_eq!(rt.state.frame().side_w, 0.0);
        assert!(!rt.state.sidebar_visible);
        assert_eq!(titles(&rt.state), ["One"]);
        // Within 296 pt it stays; a move back cancels the close.
        rt.pointer_move(290.0, 300.0);
        settle(&mut rt, 0.5);
        assert!(rt.state.sidebar_peeking);
        rt.pointer_move(300.0, 300.0);
        settle(&mut rt, 0.15);
        rt.pointer_move(100.0, 300.0);
        settle(&mut rt, 0.5);
        assert!(rt.state.sidebar_peeking, "moving back cancels the close");
        rt.pointer_move(500.0, 300.0);
        settle(&mut rt, 0.2);
        assert!(rt.state.sidebar_peeking, "closes 0.25 s after leaving");
        settle(&mut rt, 0.1);
        assert!(matches!(rt.state.sidebar.peek, Peek::Leaving(_)));
        settle(&mut rt, 0.2);
        assert!(!rt.state.sidebar_peeking);
        assert_eq!(rt.state.sidebar.peek, Peek::Hidden);
        assert!(rt.find("sidebar").is_none());
    }

    #[test]
    fn a_brief_touch_of_the_edge_doesnt_peek() {
        let (mut rt, _dir) = app_with(&[("One.md", "One")]);
        rt.pointer_move(400.0, 300.0);
        settle(&mut rt, 0.1);
        rt.pointer_move(2.0, 300.0);
        rt.frame(1.0 / 60.0);
        rt.pointer_move(40.0, 300.0);
        settle(&mut rt, 0.5);
        assert!(!rt.state.sidebar_peeking);
        // Nor does the edge of a window that isn't key.
        rt.window_focus(false);
        rt.pointer_move(2.0, 300.0);
        settle(&mut rt, 0.5);
        assert!(!rt.state.sidebar_peeking);
    }

    #[test]
    fn toggling_while_peeking_pins_and_a_click_in_the_text_closes() {
        let (mut rt, _dir) = app_with(&[("One.md", "One two three four five six seven eight nine ten")]);
        rt.pointer_move(400.0, 300.0);
        settle(&mut rt, 0.1);
        rt.pointer_move(2.0, 300.0);
        settle(&mut rt, 0.3);
        assert!(rt.state.sidebar_peeking);
        click(&mut rt, crate::chrome::Button::Sidebar.key());
        assert!(!rt.state.sidebar_peeking && rt.state.sidebar_visible && rt.state.prefs.show_sidebar());
        assert_eq!(rt.state.frame().side_w, 260.0);

        run(&mut rt, Command::ToggleSidebar);
        rt.pointer_move(400.0, 300.0);
        settle(&mut rt, 0.1);
        rt.pointer_move(2.0, 300.0);
        settle(&mut rt, 0.3);
        assert!(rt.state.sidebar_peeking);
        // A click in the text (inside 296 pt, so not the moving-away rule) moves the caret:
        // the peek slides away.
        let at = (280.0, rt.state.field_rect[1] + rt.state.column().inset_y + 10.0);
        rt.pointer_move(at.0, at.1);
        settle(&mut rt, 0.5);
        assert!(rt.state.sidebar_peeking);
        let before = rt.state.edit.range();
        rt.pointer_down(at.0, at.1);
        rt.pointer_up(at.0, at.1);
        settle(&mut rt, 0.4);
        assert_ne!(rt.state.edit.range(), before, "the click placed the caret");
        assert!(!rt.state.sidebar_peeking);
    }

    #[test]
    fn opening_from_the_sidebar_or_the_picker_runs_the_leave_check() {
        let (mut rt, dir) = app_with(&[("One.md", "One"), ("Two.md", "Two body"), ("Three.md", "Three body")]);
        run(&mut rt, Command::ToggleSidebar);
        let drafts = dir.path().join("drafts");
        let set_readonly = |on: bool| {
            let mut perms = std::fs::metadata(&drafts).unwrap().permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(on);
            std::fs::set_permissions(&drafts, perms).unwrap();
        };
        set_readonly(true);
        crate::tests::type_text(&mut rt, " more");
        settle(&mut rt, 1.0);
        assert!(matches!(rt.state.save_state, crate::app::SaveState::Failed(_)));

        // A sidebar click asks first; Keep Editing stays.
        click(&mut rt, "sidebar-row-1");
        assert_eq!(rt.state.alert().map(|a| a.message.as_str()), Some("This draft couldn't be saved."));
        click(&mut rt, "alert-button-0");
        assert_eq!(rt.state.edit.text, "One more");

        // The picker's choice asks too; Continue opens it.
        crate::tests::command(&mut rt, 'o', false, false);
        rt.update(|s, tx| {
            let three = s.picker.rows().position(|d| d.title == "Three");
            s.picker.selected = three;
            s.picker_choose_selected(tx);
        });
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.alert().map(|a| a.buttons.clone()), Some(vec!["Keep Editing".into(), "Save As…".into(), "Continue".into()]));
        click(&mut rt, "alert-button-2");
        assert_eq!(rt.state.edit.text, "Three body");
        set_readonly(false);
    }

    #[test]
    fn the_sidebar_and_its_peek_keep_the_chrome_up_as_the_bars_do() {
        let (mut rt, _dir) = app_with(&[("One.md", "One")]);
        let opacity = |rt: &Runtime<App>, k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k) && !e.exiting).unwrap().opacity;
        run(&mut rt, Command::ToggleSidebar);
        assert!(rt.state.edit.focused);
        // Over the pinned sidebar the chrome outlasts its 3 s, as over the top bar.
        rt.pointer_move(100.0, 400.0);
        settle(&mut rt, 3.5);
        assert!(rt.state.chrome_visible);
        // Over the text it goes; the sidebar button fades with it, the panel doesn't.
        rt.pointer_move(600.0, 400.0);
        settle(&mut rt, 3.8);
        assert!(!rt.state.chrome_visible);
        assert!(opacity(&rt, crate::chrome::Button::Sidebar.key()) < 0.01);
        assert_eq!(opacity(&rt, "sidebar"), 1.0);

        // A peek brings the chrome up and keeps it while the pointer is over the panel.
        run(&mut rt, Command::ToggleSidebar);
        crate::tests::type_text(&mut rt, "x");
        assert!(!rt.state.chrome_visible);
        rt.pointer_move(2.0, 400.0);
        settle(&mut rt, 0.3);
        assert!(rt.state.sidebar_peeking && rt.state.chrome_visible);
        rt.pointer_move(120.0, 400.0);
        settle(&mut rt, 3.5);
        assert!(rt.state.chrome_visible, "kept over the peeking panel");
        // Moving away closes the peek; the 3 s auto-hide starts again after it slides out.
        rt.pointer_move(600.0, 400.0);
        settle(&mut rt, 0.5);
        assert!(!rt.state.sidebar_peeking && rt.state.chrome_visible);
        settle(&mut rt, 3.2);
        assert!(!rt.state.chrome_visible);
    }

    #[test]
    fn window_coming_back_relists_a_pinned_sidebar() {
        let (mut rt, dir) = app_with(&[("One.md", "One")]);
        run(&mut rt, Command::ToggleSidebar);
        std::fs::write(dir.path().join("drafts/New.md"), "new").unwrap();
        rt.window_focus(false);
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.sidebar.items.len(), 1);
        rt.window_focus(true);
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.sidebar.items.len(), 2);
    }

    #[test]
    fn mode_and_folder_come_back_from_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let support = dir.path().join("support");
        std::fs::create_dir_all(&support).unwrap();
        std::fs::write(support.join("preferences.json"), r#"{"showSidebar": true, "sidebarMode": "folder", "folderRemote": "ssh://me@box:2222/srv/notes"}"#).unwrap();
        let prefs = crate::prefs::Prefs::load(&support);
        let sb = Sidebar::new(&prefs);
        assert_eq!(sb.mode, Mode::Folder);
        assert_eq!(sb.folder_stack, [FolderRoot::Remote(RemoteLocation::new("me@box", Some(2222), "/srv/notes"))]);
        assert!(prefs.show_sidebar());
    }

    #[test]
    fn finder_order_compares_numbers_by_value_and_ignores_case() {
        let mut names = vec!["Draft 10.md", "draft 2.md", "Apple.md", "Draft 1.md"];
        names.sort_by(|a, b| finder_order(a, b));
        assert_eq!(names, ["Apple.md", "Draft 1.md", "draft 2.md", "Draft 10.md"]);
    }

    #[test]
    fn folder_names() {
        let home = FolderRoot::Remote(RemoteLocation::parse("me@box:").unwrap());
        assert_eq!(home.name(), "box ~");
        let notes = FolderRoot::Remote(RemoteLocation::parse("box:/srv/notes").unwrap());
        assert_eq!(notes.name(), "notes");
        assert_eq!(notes.detail(), "box:/srv/notes");
        assert_eq!(FolderRoot::Local(PathBuf::from("/tmp/a")).name(), "a");
    }
}
