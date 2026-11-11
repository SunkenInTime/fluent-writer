//! The Open picker (⌘O): a keyboard-first sheet listing recent drafts with a search field.
//! Port of `OpenPicker.swift` and `openDraftPicker` in `Commands.swift`.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rune::{Align, El, Icon, Key, KeyEvent, ScrollSpec, TextEdit, Tx, col, el, icon, row, text, text_input};
use writer_core::DraftSummary;

use crate::app::App;
use crate::commands::Command;
use crate::remote_editing::{button, field_box, sheet};
use crate::theme::{Palette, ui_font};

pub const WIDTH: f32 = 520.0;
pub const HEIGHT: f32 = 420.0;
const ROW_HEIGHT: f32 = 50.0;
const SEARCH_TOP: f32 = 16.0;
const SEARCH_HEIGHT: f32 = 22.0;
const BUTTON_HEIGHT: f32 = 21.0;
const LIST_TOP: f32 = SEARCH_TOP + SEARCH_HEIGHT + 10.0;
const LIST_BOTTOM: f32 = HEIGHT - 14.0 - BUTTON_HEIGHT - 12.0;
pub const LIST: &str = "picker-list";

#[derive(Clone, Debug, PartialEq)]
pub enum Choice {
    Draft(DraftSummary),
    Browse,
    New,
}

#[derive(Default)]
pub struct Picker {
    pub open: bool,
    pub query: TextEdit,
    all: Vec<DraftSummary>,
    /// Indexes into `all` that match the query.
    pub shown: Vec<usize>,
    pub selected: Option<usize>,
    current: Option<PathBuf>,
    /// What to do once the sheet has closed; runs on the next frame.
    pub chosen: Option<Choice>,
}

impl Picker {
    pub fn rows(&self) -> impl Iterator<Item = &DraftSummary> {
        self.shown.iter().map(|&i| &self.all[i])
    }

    fn filter(&mut self, tx: Option<&mut Tx>) {
        let q = self.query.text.trim_matches(' ').to_lowercase();
        self.shown = (0..self.all.len()).filter(|&i| q.is_empty() || self.all[i].title.to_lowercase().contains(&q) || self.all[i].preview.to_lowercase().contains(&q)).collect();
        if self.shown.is_empty() {
            self.selected = None;
            return;
        }
        let current_first = self.current.is_some() && self.all[self.shown[0]].path.as_path() == self.current.as_deref().unwrap();
        let row = if current_first && self.shown.len() > 1 && q.is_empty() { 1 } else { 0 };
        self.selected = Some(row);
        if let Some(tx) = tx {
            tx.scroll_into_view(row_key(row));
        }
    }

    pub fn empty_message(&self) -> &'static str {
        if self.all.is_empty() { "No drafts yet" } else { "No matching drafts" }
    }
}

pub fn row_key(i: usize) -> String {
    format!("picker-row-{i}")
}

/// RelativeDateTimeFormatter's short English style: "2 hr. ago", "in 5 min.".
pub fn relative_date(date: SystemTime, now: SystemTime) -> String {
    let (seconds, past) = match now.duration_since(date) {
        Ok(d) => (d.as_secs(), true),
        Err(e) => (e.duration().as_secs(), false),
    };
    const UNITS: [(u64, &str, &str); 7] = [
        (365 * 86_400, "yr.", "yr."),
        (30 * 86_400, "mo.", "mo."),
        (7 * 86_400, "wk.", "wk."),
        (86_400, "day", "days"),
        (3_600, "hr.", "hr."),
        (60, "min.", "min."),
        (1, "sec.", "sec."),
    ];
    let (n, one, many) = UNITS.iter().find(|(size, _, _)| seconds >= *size).map(|&(size, one, many)| (seconds / size, one, many)).unwrap_or((0, "sec.", "sec."));
    let unit = if n == 1 { one } else { many };
    if past { format!("{n} {unit} ago") } else { format!("in {n} {unit}") }
}

impl App {
    /// File › Open…: saves, then shows the picker.
    pub fn open_picker(&mut self) {
        self.flush();
        let mut query = TextEdit::new("");
        query.focused = true;
        let mut picker = Picker { open: true, query, all: self.library.recent_drafts(200), current: self.draft.path().map(PathBuf::from), ..Picker::default() };
        picker.filter(None);
        self.picker = picker;
    }

    pub fn picker_changed(&mut self, tx: &mut Tx) {
        self.picker.filter(Some(tx));
    }

    fn picker_move(&mut self, d: isize, tx: &mut Tx) {
        let count = self.picker.shown.len();
        if count == 0 {
            return;
        }
        let from = self.picker.selected.map_or(-1, |i| i as isize);
        let row = (from + d).clamp(0, count as isize - 1) as usize;
        self.picker.selected = Some(row);
        tx.scroll_into_view(row_key(row));
    }

    pub fn picker_finish(&mut self, choice: Option<Choice>, tx: &mut Tx) {
        self.picker.open = false;
        self.picker.query.focused = false;
        self.picker.chosen = choice;
        if self.picker.chosen.is_none() {
            self.focus_draft(tx);
        }
        tx.wake_after(Duration::ZERO);
    }

    pub fn picker_choose_selected(&mut self, tx: &mut Tx) {
        let Some(summary) = self.picker.selected.and_then(|i| self.picker.rows().nth(i)).cloned() else { return };
        self.picker_finish(Some(Choice::Draft(summary)), tx);
    }

    fn picker_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt {
            return false;
        }
        match ev.key {
            Key::Down => self.picker_move(1, tx),
            Key::Up => self.picker_move(-1, tx),
            Key::Enter => self.picker_choose_selected(tx),
            Key::Escape => self.picker_finish(None, tx),
            _ => return false,
        }
        true
    }

    /// Runs the picker's choice on the frame after it closed.
    pub fn poll_picker(&mut self, tx: &mut Tx) -> bool {
        let Some(choice) = self.picker.chosen.take() else { return false };
        match choice {
            // A remote file's row opens it over SSH, as the sidebar's Recent list does, rather
            // than its local copy (the Swift opened the mirror without fetching the server's).
            Choice::Draft(summary) => match summary.remote {
                Some(remote) => self.open_remote(remote, tx),
                None => self.open_document(&summary.path, tx),
            },
            Choice::Browse => self.run(Command::OpenOtherFile, tx),
            Choice::New => self.new_draft(tx),
        }
        true
    }
}

pub static MAGNIFIER: Icon = Icon {
    name: "fw-magnifyingglass",
    view: 16.0,
    body: r#"<g fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"><circle cx="7" cy="7" r="4.5"/><path d="M10.4 10.4l3.6 3.6"/></g>"#,
};

pub fn view(s: &App, p: &Palette) -> Option<El> {
    let pk = &s.picker;
    if !pk.open {
        return None;
    }
    let now = SystemTime::now();
    let inner_w = WIDTH - 16.0;
    let rows = pk.rows().enumerate().map(|(i, d)| {
        let when = if d.modified == UNIX_EPOCH { String::new() } else { relative_date(d.modified, now) };
        let detail = [when.as_str(), d.preview.as_str()].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join(" — ");
        let selected = pk.selected == Some(i);
        col()
            .key(row_key(i))
            .w(inner_w)
            .h(ROW_HEIGHT)
            .px(10.0)
            .gap(3.0)
            .justify_center()
            .fill(if selected { p.sidebar_selection } else { crate::theme::rgba(0, 0, 0, 0.0) })
            .role("listitem")
            .label(format!("{}, {}", d.title, when))
            .on_press(move |s: &mut App| s.picker.selected = Some(i))
            .on_double_press_tx(move |s: &mut App, tx| {
                s.picker.selected = Some(i);
                s.picker_choose_selected(tx);
            })
            .child(text(d.title.clone()).font(ui_font(13.0, true)).color(p.text).truncate().w_full())
            .child(text(detail).font(ui_font(11.0, false)).color(p.text.alpha(0.5)).truncate().w_full())
    });
    let list_h = LIST_BOTTOM - LIST_TOP;
    let field = text_input(&pk.query, ui_font(14.0, false))
        .key("picker-search")
        .label("Find a draft")
        .single_line()
        .placeholder("Find a draft", p.quiet)
        .color(p.text)
        .caret(p.caret, p.selection)
        .grow(1.0)
        .on_edit(|s: &mut App| &mut s.picker.query)
        .on_input_change(|s: &mut App, tx| s.picker_changed(tx))
        .on_input_key(|s: &mut App, ev, tx| s.picker_key(ev, tx));
    let search = field_box(p, row().w_full().items_center().gap(4.0).child(icon(&MAGNIFIER, 13.0).color(p.quiet)).child(field)).w_full().h(SEARCH_HEIGHT).radius(6.0);

    let mut content = el()
        .w_full()
        .h_full()
        .child(el().absolute(16.0, SEARCH_TOP).size(WIDTH - 32.0, SEARCH_HEIGHT).child(search))
        .child(col().key(LIST).absolute(8.0, LIST_TOP).size(inner_w, list_h).scroll(ScrollSpec { stick_bottom: false }).role("list").label("Recent drafts").child(col().w_full().children(rows)));
    if pk.shown.is_empty() {
        content = content.child(text(pk.empty_message()).key("picker-empty").font(ui_font(13.0, false)).color(p.quiet).align(Align::Center).absolute(8.0, LIST_TOP + list_h / 2.0 - 9.0).size(inner_w, 18.0));
    }
    let buttons = row()
        .absolute(16.0, HEIGHT - 14.0 - BUTTON_HEIGHT)
        .size(WIDTH - 32.0, BUTTON_HEIGHT)
        .gap(8.0)
        .child(button(p, "picker-browse", "Other File…", false).on_press_tx(|s: &mut App, tx| s.picker_finish(Some(Choice::Browse), tx)))
        .child(button(p, "picker-new", "New Draft", false).on_press_tx(|s: &mut App, tx| s.picker_finish(Some(Choice::New), tx)))
        .child(el().grow(1.0))
        .child(button(p, "picker-cancel", "Cancel", false).on_press_tx(|s: &mut App, tx| s.picker_finish(None, tx)))
        .child(button(p, "picker-open", "Open", true).default_button().on_press_tx(|s: &mut App, tx| s.picker_choose_selected(tx)));
    content = content.child(buttons);
    Some(sheet(p, s.window.0, WIDTH, Some(HEIGHT), "picker", content).modal_tx(false, |s: &mut App, tx| s.picker_finish(None, tx)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidebar::tests::{app_with, click, run};
    use crate::tests::{command, settle, type_text};
    use rune::{Mods, Runtime};

    fn titles(s: &App) -> Vec<String> {
        s.picker.rows().map(|d| d.title.clone()).collect()
    }

    fn press(rt: &mut Runtime<App>, key: Key) {
        crate::tests::press(rt, key, Mods::default());
        rt.frame(1.0 / 60.0);
    }

    const FILES: &[(&str, &str)] = &[("Morning.md", "Morning\n\nA walk by the river"), ("Letter.md", "Letter\n\nDear Sam"), ("List.md", "List\n\n- eggs")];

    #[test]
    fn opens_with_the_second_row_selected_and_the_search_focused() {
        let (mut rt, _dir) = app_with(FILES);
        type_text(&mut rt, "!");
        command(&mut rt, 'o', false, false);
        settle(&mut rt, 0.1);
        let s = &rt.state;
        assert!(s.picker.open);
        // Opening saved the draft first.
        assert_eq!(s.status().0, "Saved");
        assert_eq!(titles(s), ["Morning", "Letter", "List"]);
        assert_eq!(s.picker.selected, Some(1));
        assert!(s.picker.query.focused);
        let el = |k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k)).unwrap();
        let sheet = el("picker").rect;
        assert_eq!((sheet[2], sheet[3]), (520.0, 420.0));
        let list = el(LIST).rect;
        assert_eq!((list[0] - sheet[0], list[1] - sheet[1], list[2]), (8.0, 48.0, 504.0));
        assert_eq!(list[1] + list[3], sheet[1] + 420.0 - 14.0 - 21.0 - 12.0);
        assert_eq!(el("picker-row-0").rect[3], 50.0);
        let label = el("picker-row-0").label.unwrap();
        assert!(label.starts_with("Morning, ") && label.ends_with(" ago"), "{label}");
        let open = el("picker-open").rect;
        assert_eq!(open[0] + open[2], sheet[0] + 520.0 - 16.0);
        assert_eq!(open[1] + open[3], sheet[1] + 420.0 - 14.0);
    }

    #[test]
    fn typing_filters_and_arrows_move_the_selection() {
        let (mut rt, _dir) = app_with(FILES);
        command(&mut rt, 'o', false, false);
        type_text(&mut rt, " LI ");
        assert_eq!(titles(&rt.state), ["List"]);
        assert_eq!(rt.state.picker.selected, Some(0));
        // The preview counts too, without its first line.
        rt.update(|s, tx| {
            s.picker.query.replace_text("RIVER");
            s.picker_changed(tx);
        });
        assert_eq!(titles(&rt.state), ["Morning"]);
        // With an empty query and the open draft first, the second row is chosen again.
        rt.update(|s, tx| {
            s.picker.query.replace_text("");
            s.picker_changed(tx);
        });
        assert_eq!(rt.state.picker.selected, Some(1));
        press(&mut rt, Key::Down);
        press(&mut rt, Key::Down);
        assert_eq!(rt.state.picker.selected, Some(2));
        press(&mut rt, Key::Up);
        press(&mut rt, Key::Up);
        press(&mut rt, Key::Up);
        assert_eq!(rt.state.picker.selected, Some(0));
        type_text(&mut rt, "zzz");
        assert!(rt.state.picker.shown.is_empty());
        assert_eq!(rt.state.picker.empty_message(), "No matching drafts");
        assert!(rt.find("picker-empty").is_some());
        press(&mut rt, Key::Enter);
        assert!(rt.state.picker.open, "Return with nothing selected does nothing");
    }

    #[test]
    fn return_opens_after_the_sheet_closes_and_escape_cancels() {
        let (mut rt, dir) = app_with(FILES);
        command(&mut rt, 'o', false, false);
        press(&mut rt, Key::Escape);
        assert!(!rt.state.picker.open);
        assert_eq!(rt.state.edit.text, "Morning\n\nA walk by the river");
        command(&mut rt, 'o', false, false);
        crate::tests::press(&mut rt, Key::Enter, Mods::default());
        assert!(!rt.state.picker.open);
        assert_eq!(rt.state.draft.path(), Some(dir.path().join("drafts/Morning.md").as_path()), "runs on the next frame");
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.draft.path(), Some(dir.path().join("drafts/Letter.md").as_path()));
    }

    #[test]
    fn double_click_opens_and_new_draft_makes_one() {
        let (mut rt, dir) = app_with(FILES);
        command(&mut rt, 'o', false, false);
        let (x, y) = rt.find("picker-row-2").unwrap();
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.picker.selected, Some(2));
        assert!(rt.state.picker.open);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        settle(&mut rt, 0.1);
        assert_eq!(rt.state.draft.path(), Some(dir.path().join("drafts/List.md").as_path()));

        command(&mut rt, 'o', false, false);
        click(&mut rt, "picker-new");
        assert!(!rt.state.picker.open);
        assert_eq!(rt.state.draft.path(), None);
        assert!(rt.state.edit.text.is_empty());
    }

    #[test]
    fn with_no_drafts_it_says_so() {
        let (mut rt, _dir) = app_with(&[]);
        run(&mut rt, Command::OpenDraftPicker);
        assert_eq!(rt.state.picker.empty_message(), "No drafts yet");
        assert_eq!(rt.state.picker.selected, None);
    }

    #[test]
    fn relative_dates_read_like_the_short_style() {
        let now = UNIX_EPOCH + Duration::from_secs(10_000_000);
        let ago = |s: u64| relative_date(now - Duration::from_secs(s), now);
        assert_eq!(ago(2 * 3600 + 5), "2 hr. ago");
        assert_eq!(ago(5 * 60), "5 min. ago");
        assert_eq!(ago(86_400), "1 day ago");
        assert_eq!(ago(3 * 86_400), "3 days ago");
        assert_eq!(ago(0), "0 sec. ago");
        assert_eq!(relative_date(now + Duration::from_secs(120), now), "in 2 min.");
    }
}
