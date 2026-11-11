//! The app's state and the editor's behaviour: loading and saving drafts, restyling on edits,
//! focus modes, scrolling, the footer's status and counts, and the chrome that fades while typing.

use std::ops::Range;
use std::time::Duration;

use rune::{TextEdit, Tx};
use writer_core::{DraftDocument, DraftLibrary, FocusUnit, MarkdownBlockKind, MarkdownLine, markdown, text_stats, text_units, title_deriver};

use crate::prefs::Prefs;
use crate::styler::Metrics;

pub const TOP_BAR_HEIGHT: f32 = 40.0;
pub const FOOTER_HEIGHT: f32 = 34.0;
/// The key of the draft's text field, and of the scroll viewport around it.
pub const DRAFT: &str = "draft";
pub const SCROLL: &str = "draft-scroll";

#[derive(Clone, Debug, PartialEq)]
pub enum SaveState {
    Saved,
    Pending,
    Failed(String),
    Recovered,
}

/// Where the column sits in the editor area, as the Swift `updateTextLayout` computes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    /// Width of the text field: the prose column plus the hanging margin.
    pub width: f32,
    pub inset_x: f32,
    pub inset_y: f32,
    /// Extra scrollable space below the last line so it can rise to the reading position.
    pub bottom_overscroll: f32,
}

/// How the window divides, left to right: sidebar, editor, Assist panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub width: f32,
    pub height: f32,
    /// Width the pinned sidebar takes from the editor (0 when hidden or only peeking).
    pub side_w: f32,
    pub editor_w: f32,
    pub panel_w: f32,
}

impl Frame {
    pub fn editor_x(&self) -> f32 {
        self.side_w
    }
    pub fn panel_x(&self) -> f32 {
        self.side_w + self.editor_w
    }
}

pub const SIDEBAR_WIDTH: f32 = 260.0;
pub const ASSIST_WIDTH: f32 = 380.0;

pub struct App {
    /// The sidebar is pinned open (not just peeking over the text).
    pub sidebar_visible: bool,
    /// The sidebar is showing over the text because the pointer is at the left edge.
    pub sidebar_peeking: bool,
    pub assist_visible: bool,
    pub library: DraftLibrary,
    pub draft: DraftDocument,
    pub edit: TextEdit,
    pub find: crate::find::Find,
    pub editing: crate::editing::Editing,
    pub prefs: Prefs,
    pub lines: Vec<MarkdownLine>,
    pub focus: FocusUnit,
    pub metrics: Metrics,
    pub save_state: SaveState,
    /// A message that replaces the save status for a moment ("Saved", "Sentence focus").
    pub transient: Option<String>,
    pub counts: String,
    pub chrome_visible: bool,
    /// The window's size in points.
    pub window: (f32, f32),
    /// The draft field's rect in window points, after scrolling. The field is the whole page:
    /// its padding holds the column in place.
    pub field_rect: [f32; 4],
    pub pointer: (f32, f32),

    /// The app clock at the last poll, in seconds.
    pub now: f64,
    autosave_at: Option<f64>,
    counts_at: Option<f64>,
    selection_at: Option<f64>,
    chrome_hide_at: Option<f64>,
    transient_until: Option<f64>,
    last_selection: (usize, usize),
    /// The caret line in window points as last drawn, when the draft has the keyboard.
    pub last_caret: Option<[f32; 4]>,
    /// Bring the caret back to its reading position once the next frame has drawn it:
    /// `Some(animated)`.
    recenter: Option<bool>,
    /// Frames to wait before acting on `recenter`, so the caret rect is the edited one.
    recenter_delay: u8,
    /// A draft was just loaded: start its scroll view from the top before finding the caret.
    scroll_reset: bool,
    typed_since_poll: bool,
}

fn after(tx: &mut Tx, now: f64, seconds: f64) -> Option<f64> {
    tx.wake_after(Duration::from_secs_f64(seconds));
    Some(now + seconds)
}

fn due(at: &mut Option<f64>, now: f64) -> bool {
    if at.is_some_and(|t| t <= now) {
        *at = None;
        return true;
    }
    false
}

impl App {
    pub fn new(library: DraftLibrary, prefs: Prefs, document: DraftDocument) -> App {
        let mut app = App {
            sidebar_visible: prefs.show_sidebar(),
            sidebar_peeking: false,
            assist_visible: false,
            library,
            draft: document.clone(),
            edit: TextEdit::new(""),
            find: Default::default(),
            editing: Default::default(),
            focus: prefs.focus(),
            metrics: Metrics::new(prefs.text_size()),
            prefs,
            lines: Vec::new(),
            save_state: SaveState::Saved,
            transient: None,
            counts: String::new(),
            chrome_visible: true,
            window: (1060.0, 760.0),
            field_rect: [0.0; 4],
            pointer: (-1.0, -1.0),
            now: 0.0,
            autosave_at: None,
            counts_at: None,
            selection_at: None,
            chrome_hide_at: None,
            transient_until: None,
            last_selection: (0, 0),
            last_caret: None,
            recenter: None,
            recenter_delay: 0,
            scroll_reset: false,
            typed_since_poll: false,
        };
        app.load(document);
        app
    }

    // MARK: Layout

    pub fn frame(&self) -> Frame {
        let (width, height) = self.window;
        // The panel gives way on narrow windows so the draft keeps a readable column.
        let panel_w = if self.assist_visible { ASSIST_WIDTH.min((width * 0.5).round().max(260.0)) } else { 0.0 };
        let side_w = if self.sidebar_visible && !self.sidebar_peeking { SIDEBAR_WIDTH.min((width - panel_w - 320.0).max(0.0)) } else { 0.0 };
        Frame { width, height, side_w, editor_w: width - panel_w - side_w, panel_w }
    }

    /// The editor area's size: its share of the window less the top bar. The footer lies over
    /// its bottom.
    pub fn visible(&self) -> (f32, f32) {
        (self.frame().editor_w, (self.window.1 - self.draft_top()).max(0.0))
    }

    /// Where the draft's scroll view starts: under the top bar, and under the find bar while it shows.
    pub fn draft_top(&self) -> f32 {
        TOP_BAR_HEIGHT + self.find.height()
    }

    pub fn column(&self) -> Column {
        let (vw, vh) = self.visible();
        let m = &self.metrics;
        let hang = m.hang_width();
        let min_pad = crate::theme::MINIMUM_SIDE_PADDING.max(hang + 12.0);
        let ideal = (m.character_width * crate::theme::COLUMN_CHARACTERS).round();
        let prose = ideal.min(vw - min_pad * 2.0).max(160.0);
        let inset_x = (((vw - prose) / 2.0) - hang).round().max(4.0);
        let (inset_y, bottom_overscroll) = if self.focus.centers_caret() {
            ((vh / 2.0 - m.line_height / 2.0).round().max(40.0), 0.0)
        } else {
            (56.0, (vh * 0.35).round())
        };
        Column { width: prose + hang, inset_x, inset_y, bottom_overscroll }
    }

    // MARK: Documents

    pub fn load(&mut self, doc: DraftDocument) {
        self.draft = doc;
        let mut edit = TextEdit::new(self.draft.body.clone());
        let len = self.draft.body.len();
        let sel = &self.draft.selection;
        let (a, b) = (sel.start.min(len), sel.end.min(len));
        edit.set_selection(a, b);
        edit.focused = true;
        self.edit = edit;
        self.last_selection = (a, b);
        self.reparse();
        self.autosave_at = None;
        if self.draft.recovered_at().is_some() {
            self.save_state = SaveState::Recovered;
            self.autosave_at = Some(self.now + 0.2);
        } else {
            self.save_state = SaveState::Saved;
            if self.draft.remote_upload_pending() {
                self.autosave_at = Some(self.now + 0.2);
            }
        }
        self.update_counts();
        self.recenter = Some(false);
        self.recenter_delay = 2;
        self.scroll_reset = true;
    }

    fn sync_document_from_view(&mut self) {
        if self.draft.body != self.edit.text {
            self.draft.body = self.edit.text.clone();
        }
        let (a, b) = self.edit.range();
        self.draft.selection = a.min(b)..a.max(b);
    }

    /// Saves pending text. Returns false when the draft couldn't be written.
    pub fn flush(&mut self) -> bool {
        self.autosave_at = None;
        self.sync_document_from_view();
        match self.library.save(&mut self.draft) {
            Ok(_) => {
                self.library.remember_selection(&self.draft);
                self.save_state = SaveState::Saved;
                true
            }
            Err(e) => {
                self.save_state = SaveState::Failed(e.to_string());
                self.chrome_visible = true;
                self.chrome_hide_at = None;
                false
            }
        }
    }

    // MARK: Status and counts

    /// The footer's status text and whether it is a warning.
    pub fn status(&self) -> (String, bool) {
        if let Some(t) = &self.transient {
            return (t.clone(), false);
        }
        match &self.save_state {
            SaveState::Saved => (if self.draft.path().is_none() { String::new() } else { "Saved".into() }, false),
            SaveState::Pending => ("Edited".into(), false),
            SaveState::Recovered => ("Recovered unsaved changes".into(), false),
            SaveState::Failed(m) => (format!("Not saved — {m}"), true),
        }
    }

    pub fn flash_message(&mut self, message: impl Into<String>, tx: &mut Tx) {
        self.transient = Some(message.into());
        self.show_chrome(true, tx);
        self.transient_until = after(tx, self.now, 2.2);
    }

    pub fn update_counts(&mut self) {
        self.counts_at = None;
        let total = text_stats::counts_for_markdown(&self.edit.text);
        let (a, b) = self.edit.range();
        let selection = (a != b).then(|| text_stats::counts_for_markdown(&self.edit.text[a.min(b)..a.max(b)]));
        self.counts = text_stats::format(total, selection);
    }

    fn schedule_counts(&mut self, tx: &mut Tx) {
        if self.counts_at.is_none() {
            self.counts_at = after(tx, self.now, 0.15);
        }
    }

    /// The title shown in the top bar: empty for an untouched new draft.
    pub fn shown_title(&self) -> &str {
        if self.draft.title == title_deriver::UNTITLED && self.draft.body.is_empty() { "" } else { &self.draft.title }
    }

    // MARK: Parsing

    fn reparse(&mut self) {
        self.lines = markdown::parse(&self.edit.text);
    }

    pub fn fence_count(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == MarkdownBlockKind::CodeFence).count()
    }

    // MARK: Focus

    pub fn set_focus(&mut self, unit: FocusUnit, tx: &mut Tx) {
        self.focus = unit;
        self.prefs.set_focus(unit);
        if unit.dims_surroundings() {
            self.prefs.set_last_dimming_focus(unit);
        }
        if unit.centers_caret() {
            self.recenter = Some(true);
            self.recenter_delay = 1;
        }
        let message = if unit == FocusUnit::Off { "Focus off".to_string() } else { format!("{} focus", focus_name(unit)) };
        self.flash_message(message, tx);
    }

    /// Range kept bright in sentence and paragraph focus, covering the whole selection.
    pub fn focus_range(&self) -> Option<Range<usize>> {
        if !self.focus.dims_surroundings() {
            return None;
        }
        let text = &self.edit.text;
        let (a, b) = self.edit.range();
        let (lo, hi) = (a.min(b), a.max(b));
        let first = text_units::focus_range(self.focus, text, lo)?;
        if lo == hi {
            return Some(first);
        }
        match text_units::focus_range(self.focus, text, hi) {
            Some(second) => Some(first.start.min(second.start)..first.end.max(second.end)),
            None => Some(first),
        }
    }

    pub fn set_text_size(&mut self, size: f32, tx: &mut Tx) {
        self.prefs.set_text_size(size);
        self.metrics = Metrics::new(size);
        self.recenter = Some(false);
        self.recenter_delay = 1;
        self.flash_message(format!("Text size {} pt", size as i32), tx);
    }

    pub fn new_draft(&mut self, tx: &mut Tx) {
        if self.draft.is_empty_untitled() {
            tx.focus(DRAFT, false);
            return;
        }
        if !self.flush() {
            return;
        }
        let doc = self.library.new_draft();
        self.load(doc);
    }

    // MARK: Scrolling

    /// The scroll viewport's offset, worked out from where the field was laid out.
    fn scroll_offset(&self) -> f32 {
        self.draft_top() - self.field_rect[1]
    }

    fn max_scroll(&self) -> f32 {
        let (_, vh) = self.visible();
        (self.field_rect[3] - vh).max(0.0)
    }

    fn scroll_to(&self, y: f32, animated: bool, tx: &mut Tx) {
        if (self.scroll_offset() - y).abs() < 0.5 {
            return;
        }
        if animated {
            tx.scroll_to_with(SCROLL, 0.0, y, rune::Spring::new(0.14, 0.0));
        } else {
            tx.scroll_to_y(SCROLL, y);
        }
    }

    /// Keeps the caret line at the vertical middle of the editor area.
    fn center_caret(&self, caret: [f32; 4], animated: bool, tx: &mut Tx) {
        let (_, vh) = self.visible();
        let mid = caret[1] + caret[3] / 2.0 - self.draft_top() + self.scroll_offset();
        let target = (mid - vh / 2.0).round().clamp(0.0, self.max_scroll());
        self.scroll_to(target, animated, tx);
    }

    /// Outside typewriter modes, keeps the caret out of the bottom edge without recentering.
    fn ensure_caret_readable(&self, caret: [f32; 4], animated: bool, tx: &mut Tx) {
        let (_, vh) = self.visible();
        let lh = self.metrics.line_height;
        let off = self.scroll_offset();
        let top = self.draft_top();
        let (min_y, max_y) = (caret[1] - top + off, caret[1] + caret[3] - top + off);
        let bottom_limit = off + vh - (FOOTER_HEIGHT + lh * 2.0).max(vh * 0.22);
        let top_limit = off + lh;
        let target = if max_y > bottom_limit {
            Some(off + (max_y - bottom_limit))
        } else if min_y < top_limit {
            Some((min_y - lh).max(0.0))
        } else {
            None
        };
        if let Some(t) = target {
            self.scroll_to(t.min(self.max_scroll()), animated, tx);
        }
    }

    // MARK: Chrome

    pub fn user_is_typing(&mut self) {
        self.typed_since_poll = true;
        if matches!(self.save_state, SaveState::Failed(_)) {
            return;
        }
        self.chrome_hide_at = None;
        self.chrome_visible = false;
    }

    pub fn show_chrome(&mut self, auto_hide: bool, tx: &mut Tx) {
        self.chrome_visible = true;
        self.chrome_hide_at = if auto_hide { after(tx, self.now, 3.0) } else { None };
    }

    fn pointer_over_chrome(&self) -> bool {
        let (x, y) = self.pointer;
        let f = self.frame();
        let over_sidebar = (self.sidebar_visible || self.sidebar_peeking) && x < SIDEBAR_WIDTH.max(f.side_w);
        let in_editor = (f.editor_x()..f.panel_x()).contains(&x);
        over_sidebar || (in_editor && ((0.0..TOP_BAR_HEIGHT).contains(&y) || (self.window.1 - FOOTER_HEIGHT..=self.window.1).contains(&y)))
    }

    // MARK: Editing

    /// Runs after every change to the draft's text.
    pub fn text_did_change(&mut self, tx: &mut Tx) {
        self.reparse();
        self.draft.body = self.edit.text.clone();
        let (a, b) = self.edit.range();
        self.draft.selection = a.min(b)..a.max(b);
        if !self.draft.has_manual_title {
            self.draft.refresh_derived_title();
        }
        if !matches!(self.save_state, SaveState::Failed(_)) {
            self.save_state = SaveState::Pending;
        }
        self.autosave_at = after(tx, self.now, 0.8);
        self.schedule_counts(tx);
        self.recenter = Some(self.focus.centers_caret());
        self.recenter_delay = 1;
        tx.wake_after(Duration::ZERO);
    }

    fn selection_did_change(&mut self, tx: &mut Tx) {
        let (a, b) = self.edit.range();
        self.draft.selection = a.min(b)..a.max(b);
        self.schedule_counts(tx);
        if self.typed_since_poll && self.focus.centers_caret() && self.recenter.is_none() {
            self.recenter = Some(true);
            self.recenter_delay = 1;
            tx.wake_after(Duration::ZERO);
        }
        self.selection_at = after(tx, self.now, 1.0);
    }

    /// Applies an edit as one undo step and places the selection after it. Undo puts back the
    /// text and the selection from before; redo the text and `selection_after`.
    pub fn apply(&mut self, edit: &writer_core::TextEdit, tx: &mut Tx) {
        self.edit.undo_group(|e| {
            let len = e.text.len();
            e.set_selection(edit.range.start.min(len), edit.range.end.min(len));
            e.insert(&edit.replacement);
            let len = e.text.len();
            let s = &edit.selection_after;
            e.set_selection(s.start.min(len), s.end.min(len));
        });
        self.text_did_change(tx);
    }

    // MARK: Clock

    /// Runs before every frame. Returns whether anything the view reads changed.
    pub fn poll(&mut self, clock: f64, tx: &mut Tx) -> bool {
        self.now = clock;
        let mut changed = false;
        let range = self.edit.range();
        if range != self.last_selection {
            self.last_selection = range;
            self.selection_did_change(tx);
            changed = true;
        }
        self.typed_since_poll = false;
        if due(&mut self.autosave_at, clock) {
            self.flush();
            changed = true;
        }
        if due(&mut self.counts_at, clock) {
            self.update_counts();
            changed = true;
        }
        if due(&mut self.selection_at, clock) {
            self.library.remember_selection(&self.draft);
        }
        if due(&mut self.transient_until, clock) {
            self.transient = None;
            changed = true;
        }
        if due(&mut self.chrome_hide_at, clock) && !matches!(self.save_state, SaveState::Failed(_)) && !self.pointer_over_chrome() && self.edit.focused {
            self.chrome_visible = false;
            changed = true;
        }
        if std::mem::take(&mut self.scroll_reset) {
            tx.scroll_to_y(SCROLL, 0.0);
            tx.wake_after(Duration::ZERO);
        }
        if let Some(animated) = self.recenter {
            if self.recenter_delay > 0 {
                self.recenter_delay -= 1;
                tx.wake_after(Duration::ZERO);
            } else if let Some(caret) = tx.caret(DRAFT) {
                self.recenter = None;
                if self.focus.centers_caret() {
                    self.center_caret(caret, animated, tx);
                } else {
                    self.ensure_caret_readable(caret, false, tx);
                }
            } else {
                self.recenter = None;
            }
        }
        self.last_caret = tx.caret(DRAFT);
        changed
    }
}

pub fn focus_name(unit: FocusUnit) -> &'static str {
    match unit {
        FocusUnit::Off => "Off",
        FocusUnit::Sentence => "Sentence",
        FocusUnit::Paragraph => "Paragraph",
        FocusUnit::Typewriter => "Typewriter",
    }
}
