//! Shows a version in place of the draft as a word diff against it (Swift `VersionPreview.swift`).
//! Versions line up oldest to newest with your draft at the end, so → walks toward the present
//! and ← back in time.

use std::ops::Range;
use std::time::Duration;

use rune::{Align, Color, El, Icon, Key, KeyEvent, LiveRegion, ScrollSpec, TextEdit, Tx, col, el, icon, text, text_input};
use writer_core::{DiffSegmentKind, DraftVersion, MarkdownLine, VersionKind, markdown, text_stats, word_diff};

use crate::app::{App, DRAFT, FOOTER_HEIGHT, TOP_BAR_HEIGHT};
use crate::notes::relative_time;
use crate::styler;
use crate::theme::{CHARACTER_WIDTH_EM, Palette, draft_font, ui_font};

/// The key of the diff's text field, and of the scroll view around it.
pub const PREVIEW: &str = "version-preview-text";
pub const PREVIEW_SCROLL: &str = "version-preview-scroll";
/// `VersionPreview.swift:47`
pub const BAR_HEIGHT: f32 = 44.0;
/// Top and bottom inset of the diff text. `VersionPreview.swift:157`
const TEXT_INSET_Y: f32 = 40.0;

pub static CHEVRON_LEFT: Icon = Icon {
    name: "fw-chevron-left",
    view: 20.0,
    body: r#"<path d="M12.5 4.5L7 10L12.5 15.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
pub static CHEVRON_RIGHT: Icon = Icon {
    name: "fw-chevron-right",
    view: 20.0,
    body: r#"<path d="M7.5 4.5L13 10L7.5 15.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

/// The diff as last rendered, and what the bar says about it.
#[derive(Clone, Debug, Default)]
pub struct Rendered {
    /// The draft and the version woven together.
    pub combined: String,
    pub lines: Vec<MarkdownLine>,
    /// Inserted and deleted parts of `combined`, without their line breaks.
    pub marks: Vec<(Range<usize>, DiffSegmentKind)>,
    pub first_change: Option<usize>,
    pub added: usize,
    pub removed: usize,
    /// 1 for the oldest version.
    pub position: usize,
    pub total: usize,
    pub label: String,
    pub detail: String,
    /// The version's own text, which the diff's accessibility value reads.
    pub body: String,
}

impl Rendered {
    /// The version is word for word the draft: Use is disabled.
    pub fn same_as_draft(&self) -> bool {
        self.marks.is_empty()
    }
}

#[derive(Default)]
pub struct Preview {
    version_id: Option<String>,
    /// The diff's text field. It is selectable but never edited.
    pub edit: TextEdit,
    pub rendered: Rendered,
    /// The notes and text revisions the diff was rendered from.
    seen: (u64, u64),
    /// Frames to wait before scrolling the diff to its first change.
    scroll_in: Option<u8>,
    /// The diff field's rect in window points, after scrolling.
    pub field_rect: [f32; 4],
}

impl Preview {
    pub fn is_active(&self) -> bool {
        self.version_id.is_some()
    }
    pub fn version_id(&self) -> Option<&str> {
        self.version_id.as_deref()
    }
    /// Forgets the preview without touching focus, as a document load does.
    pub fn close_quietly(&mut self) {
        self.version_id = None;
        self.scroll_in = None;
    }
}

/// The diff of `draft` against `v`, as `VersionPreview.render` builds it. `timeline` is the
/// versions oldest first. `VersionPreview.swift:242-304`
pub fn render(v: &DraftVersion, draft: &str, timeline: &[&DraftVersion]) -> Rendered {
    let mut r = Rendered::default();
    let mut previous = DiffSegmentKind::Equal;
    for seg in word_diff::diff(draft, &v.body) {
        // A space between a deletion and the insertion after it when neither side has one.
        if previous == DiffSegmentKind::Delete
            && seg.kind == DiffSegmentKind::Insert
            && r.combined.chars().next_back().is_some_and(|c| !c.is_whitespace())
            && seg.text.chars().next().is_some_and(|c| !c.is_whitespace())
        {
            r.combined.push(' ');
        }
        previous = seg.kind;
        let start = r.combined.len();
        r.combined.push_str(&seg.text);
        if seg.kind != DiffSegmentKind::Equal {
            for run in line_runs(start..r.combined.len(), &r.combined) {
                r.marks.push((run, seg.kind));
            }
            r.first_change.get_or_insert(start);
            let words = text_stats::counts_for_plain_text(&seg.text).words;
            if seg.kind == DiffSegmentKind::Insert {
                r.added += words;
            } else {
                r.removed += words;
            }
        }
    }
    r.lines = markdown::parse(&r.combined);
    let changed = r.first_change.is_some();
    r.position = timeline.iter().position(|t| t.id == v.id).unwrap_or(0) + 1;
    r.total = timeline.len();
    r.label = v.label.clone();
    let mut detail = vec![if v.kind == VersionKind::Snapshot { "Saved draft".to_string() } else { format!("From {}", v.source) }, relative_time(v.created)];
    detail.push(if changed { format!("+{} −{} words", r.added, r.removed) } else { "same as your draft".into() });
    if v.used_at.is_some() {
        detail.push("used before".into());
    }
    r.detail = detail.join(" · ");
    r.body = v.body.clone();
    r
}

/// The parts of `range` that aren't line breaks, so added or removed blank lines aren't
/// painted. `VersionPreview.swift:306-320`
pub fn line_runs(range: Range<usize>, text: &str) -> Vec<Range<usize>> {
    let mut runs = Vec::new();
    let mut start = None;
    for (i, b) in text.as_bytes()[range.clone()].iter().enumerate() {
        let at = range.start + i;
        if *b == b'\n' || *b == b'\r' {
            if let Some(s) = start.take() {
                runs.push(s..at);
            }
        } else if start.is_none() {
            start = Some(at);
        }
    }
    if let Some(s) = start {
        runs.push(s..range.end);
    }
    runs
}

#[allow(dead_code, reason = "the Assist panel (next task) calls these")]
impl App {
    /// Versions oldest first; the draft itself sits after the last one.
    fn timeline(&self) -> Vec<&DraftVersion> {
        self.notes.versions().iter().rev().collect()
    }

    /// The version being previewed, if any: what Assist sends and builds on while previewing.
    /// `AssistController.swift:705-708`
    pub fn previewed_version(&self) -> Option<&DraftVersion> {
        self.preview.version_id().and_then(|id| self.notes.version(id))
    }

    /// The text a request to Assist reads: the previewed version's, otherwise the draft's.
    pub fn assist_source_text(&self) -> &str {
        self.previewed_version().map_or(self.edit.text.as_str(), |v| v.body.as_str())
    }

    fn preview_render(&mut self, id: &str) -> bool {
        let Some(v) = self.notes.version(id) else { return false };
        let r = render(v, &self.edit.text, &self.timeline());
        let focused = self.preview.edit.focused;
        let mut edit = TextEdit::new(r.combined.clone());
        edit.focused = focused;
        edit.set_caret(0);
        self.preview.edit = edit;
        self.preview.rendered = r;
        self.preview.seen = (self.notes.revision(), self.notes.text_revision());
        true
    }

    /// Shows version `id` in place of the draft, scrolled to its first change.
    /// `VersionPreview.swift:169-189`
    pub fn preview_version(&mut self, id: &str, tx: &mut Tx) {
        if !self.preview_render(id) {
            return;
        }
        self.preview.version_id = Some(id.to_string());
        self.preview.edit.focused = true;
        tx.focus(PREVIEW, false);
        if let Some(first) = self.preview.rendered.first_change {
            // The caret marks the first change for scrolling; the diff draws no caret.
            self.preview.edit.set_caret(first);
        }
        self.preview.scroll_in = Some(2);
        tx.wake_after(Duration::ZERO);
        self.notes.schedule_live_state(self.now);
        self.notes.announcement =
            format!("Previewing {}. Left and right arrows switch versions, Return uses it, Escape goes back.", self.preview.rendered.label);
    }

    /// Back to the draft, focused. `VersionPreview.swift:191-202`
    pub fn close_preview(&mut self, tx: &mut Tx) {
        if !self.preview.is_active() {
            return;
        }
        self.preview.close_quietly();
        self.edit.focused = true;
        tx.focus(DRAFT, false);
        self.notes.schedule_live_state(self.now);
    }

    /// One step along the timeline: −1 older, +1 newer. `VersionPreview.swift:212-226`
    pub fn preview_step(&mut self, delta: isize, tx: &mut Tx) {
        let line: Vec<String> = self.timeline().iter().map(|v| v.id.clone()).collect();
        if line.is_empty() {
            return;
        }
        // Index `line.len()` stands for the draft.
        let current = self.preview.version_id().and_then(|id| line.iter().position(|v| v == id)).unwrap_or(line.len());
        let next = current as isize + delta;
        if next < 0 {
            self.beep();
            return;
        }
        if next as usize >= line.len() {
            if self.preview.is_active() {
                self.close_preview(tx);
            } else {
                self.beep();
            }
            return;
        }
        self.preview_version(&line[next as usize].clone(), tx);
    }

    /// Uses the previewed version. `VersionPreview.swift:228-231`
    pub fn preview_use(&mut self, tx: &mut Tx) {
        if let Some(id) = self.preview.version_id().map(str::to_string) {
            self.use_version(&id, tx);
        }
    }

    /// Re-renders when the draft or the versions changed underneath, closes when the version
    /// went, and scrolls a newly shown diff to its first change. `VersionPreview.swift:204-210`
    pub fn preview_poll(&mut self, tx: &mut Tx) -> bool {
        let Some(id) = self.preview.version_id().map(str::to_string) else { return false };
        let mut changed = false;
        if self.preview.seen != (self.notes.revision(), self.notes.text_revision()) {
            if !self.preview_render(&id) {
                self.close_preview(tx);
                return true;
            }
            changed = true;
        }
        if let Some(n) = self.preview.scroll_in {
            if n > 0 {
                self.preview.scroll_in = Some(n - 1);
                tx.wake_after(Duration::ZERO);
            } else {
                self.preview.scroll_in = None;
                if self.preview.rendered.first_change.is_none() {
                    tx.scroll_to_y(PREVIEW_SCROLL, 0.0);
                } else if let Some(caret) = tx.caret(PREVIEW) {
                    self.preview_reveal(caret, tx);
                }
            }
        }
        changed
    }

    /// Scrolls the diff just enough to show `caret`, as `scrollRangeToVisible` does.
    fn preview_reveal(&self, caret: [f32; 4], tx: &mut Tx) {
        let top = TOP_BAR_HEIGHT + BAR_HEIGHT;
        let height = self.preview_scroll_height();
        let offset = top - self.preview.field_rect[1];
        let (y0, y1) = (caret[1] - top + offset, caret[1] + caret[3] - top + offset);
        let target = if y0 < offset {
            y0
        } else if y1 > offset + height {
            y1 - height
        } else {
            return;
        };
        tx.scroll_to_y(PREVIEW_SCROLL, target.max(0.0).round());
    }

    fn preview_scroll_height(&self) -> f32 {
        (self.window.1 - TOP_BAR_HEIGHT - FOOTER_HEIGHT - BAR_HEIGHT).max(0.0)
    }
}

/// Keys in the diff: ← older, → newer, Esc back to the draft, Return uses it. Menu shortcuts
/// still work; the text can be selected and copied but not changed. `VersionPreview.swift:6-20`
pub fn preview_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let copy_or_select = matches!(ev.key, Key::Command('c') | Key::Command('a')) && !ev.mods.shift && !ev.mods.alt;
    if !copy_or_select && let Some(command) = crate::commands::command_for(ev, tx) {
        s.run(command, tx);
        return true;
    }
    match ev.key {
        Key::Left => s.preview_step(-1, tx),
        Key::Right => s.preview_step(1, tx),
        Key::Escape => s.close_preview(tx),
        Key::Enter => s.preview_use(tx),
        Key::Text(_) | Key::Backspace | Key::Delete | Key::Tab => s.beep(),
        Key::Command('x') | Key::Command('v') | Key::Command('z') => s.beep(),
        _ => return false,
    }
    true
}

/// The width AppKit gives a small rounded push button sized to fit `title`.
pub fn fitted_button_width(title: &str, size: f32) -> f32 {
    (title.chars().count() as f32 * size * CHARACTER_WIDTH_EM).ceil() + 20.0
}

/// A small rounded push button: `title` in IBM Plex Mono at `size`, `w`×22.
pub fn small_button(key: &str, title: &str, size: f32, medium: bool, w: f32, enabled: bool, p: &Palette) -> El {
    el().key(key)
        .size(w, 22.0)
        .radius(5.0)
        .fill(p.card)
        .border(1.0, p.hairline)
        .role("button")
        .label(title)
        .disabled(!enabled)
        .preserve_focus_on_press(true)
        .child(
            text(title)
                .font(ui_font(size, medium))
                .color(if enabled { p.text } else { p.quiet })
                .align(Align::Center)
                .truncate()
                .absolute(0.0, ((22.0 - size * 1.3) / 2.0).round())
                .w(w),
        )
}

fn chevron(key: &str, glyph: &'static Icon, tip: &str, x: f32, enabled: bool, p: &Palette) -> El {
    el().key(key)
        .absolute(x, 12.0)
        .size(20.0, 20.0)
        .role("button")
        .label(tip)
        .help(tip)
        .disabled(!enabled)
        .preserve_focus_on_press(true)
        .child(icon(glyph, 20.0).color(p.text).opacity(if enabled { 1.0 } else { 0.3 }))
}

/// The preview: a 44 pt bar over the read-only diff, filling the editor column above the
/// footer. Empty when no version is previewed. `VersionPreview.swift:127-162`
pub fn view(s: &App, p: &Palette) -> El {
    if !s.preview.is_active() {
        return el().key("version-preview").absolute(0.0, 0.0).size(0.0, 0.0);
    }
    let f = s.frame();
    let w = f.editor_w;
    let h = (f.height - TOP_BAR_HEIGHT - FOOTER_HEIGHT).max(0.0);
    let r = &s.preview.rendered;

    // Bar. Narrow windows drop "Back to Draft" (esc and → still work), then shorten "Use This Version".
    let use_title = if w < 460.0 { "Use" } else { "Use This Version" };
    let compact = w < 620.0;
    let use_w = fitted_button_width(use_title, 11.0) + 8.0;
    let back_w = fitted_button_width("Back to Draft", 11.0) + 4.0;
    let use_x = w - 16.0 - use_w;
    let back_x = use_x - 8.0 - back_w;
    let position = format!("{}/{}", r.position, r.total);
    let pos_w = 34f32.max((position.chars().count() as f32 * 11.0 * CHARACTER_WIDTH_EM).ceil() + 6.0);
    let label_x = 34.0 + pos_w + 20.0 + 12.0;
    let label_w = ((if compact { use_x } else { back_x }) - label_x - 12.0).max(0.0);

    let mut bar = el()
        .key("version-bar")
        .absolute(0.0, 0.0)
        .size(w, BAR_HEIGHT)
        .fill(p.banner)
        .role("toolbar")
        .label("Version preview")
        .child(el().absolute(0.0, BAR_HEIGHT - 1.0).size(w, 1.0).fill(p.hairline))
        .child(chevron("version-older", &CHEVRON_LEFT, "Older version (←)", 12.0, r.position > 1, p).on_press_tx(|s: &mut App, tx| s.preview_step(-1, tx)))
        .child(
            text(&position)
                .key("version-position")
                .font(ui_font(11.0, false))
                .color(p.quiet)
                .align(Align::Center)
                .absolute(34.0, 14.0)
                .size(pos_w, 16.0)
                .label(format!("Version {} of {}", r.position, r.total)),
        )
        .child(chevron("version-newer", &CHEVRON_RIGHT, "Newer version (→)", 34.0 + pos_w, true, p).on_press_tx(|s: &mut App, tx| s.preview_step(1, tx)))
        .child(
            small_button("version-use", use_title, 11.0, true, use_w, !r.same_as_draft(), p)
                .absolute(use_x, 11.0)
                .help("Replace your draft with this version (↩). Your current draft is kept in Versions.")
                .on_press_tx(|s: &mut App, tx| {
                    if !s.preview.rendered.same_as_draft() {
                        s.preview_use(tx)
                    }
                }),
        );
    if !compact {
        bar = bar.child(
            small_button("version-back", "Back to Draft", 11.0, false, back_w, true, p)
                .absolute(back_x, 11.0)
                .help("Keep your draft as it is (esc)")
                .on_press_tx(|s: &mut App, tx| s.close_preview(tx)),
        );
    }
    if label_w >= 48.0 {
        bar = bar
            .child(text(&r.label).key("version-title").font(ui_font(12.0, true)).color(p.text).truncate().absolute(label_x, 5.0).size(label_w, 16.0))
            .child(text(&r.detail).key("version-detail").font(ui_font(11.0, false)).color(p.quiet).truncate().absolute(label_x, 22.0).size(label_w, 15.0));
    }
    if label_w < 140.0 {
        bar = bar.help(format!("{}\n{}", r.label, r.detail));
    }

    el().key("version-preview")
        .absolute(f.editor_x(), TOP_BAR_HEIGHT)
        .size(w, h)
        .fill(p.canvas)
        .child(diff(s, p).absolute(0.0, BAR_HEIGHT).size(w, (h - BAR_HEIGHT).max(0.0)))
        .child(bar)
        .child(text(&s.notes.announcement).live_region(LiveRegion::Polite).absolute(0.0, 0.0).size(1.0, 1.0).opacity(0.0))
}

/// The diff text: the editor's column and Markdown styling, inserted words green, deleted ones
/// red and struck through, each on its tint. `VersionPreview.swift:154-161`, `VersionPreview.swift:270-287`
fn diff(s: &App, p: &Palette) -> El {
    let r = &s.preview.rendered;
    let m = &s.metrics;
    let c = s.column();
    let vw = s.frame().editor_w;
    let vh = s.preview_scroll_height();
    let mut styled = styler::style(&r.combined, &r.lines, m, p, &|_| 0.0);
    for (range, kind) in &r.marks {
        let span = rune::Span::new(range.start, range.end);
        match kind {
            DiffSegmentKind::Insert => styled.spans.push(span.color(p.inserted).bg(p.inserted_background, 0.0)),
            DiffSegmentKind::Delete => {
                styled.spans.push(span.color(p.deleted).bg(p.deleted_background, 0.0));
                styled.strikes.push((range.start, range.end, Some(p.deleted)));
            }
            DiffSegmentKind::Equal => {}
        }
    }
    let field = text_input(&s.preview.edit, draft_font(m.font_size))
        .key(PREVIEW)
        .label("Version preview")
        .a11y_value(&r.body)
        .description("Inserted text is green, removed text is struck through.")
        .read_only(true)
        .color(p.text)
        .caret(Color::TRANSPARENT, p.selection)
        .spans(styled.spans)
        .paragraph_blocks(styled.blocks)
        .input_strikes(styled.strikes)
        .w_full()
        .min_h(vh)
        .pl(c.inset_x)
        .pr((vw - c.inset_x - c.width).max(0.0))
        .pt(TEXT_INSET_Y)
        .pb(TEXT_INSET_Y + (vh * 0.35).round())
        .on_edit(|s: &mut App| &mut s.preview.edit)
        .on_input_key(preview_key)
        .on_input_paste(|s: &mut App, _, _| {
            s.beep();
            true
        })
        .on_input_change(|s: &mut App, _| {
            // Read-only: anything that got through is put back.
            if s.preview.edit.text != s.preview.rendered.combined {
                let sel = s.preview.edit.range();
                let mut edit = TextEdit::new(s.preview.rendered.combined.clone());
                edit.focused = true;
                let len = edit.text.len();
                edit.set_caret(sel.1.min(len));
                s.preview.edit = edit;
            }
        })
        .on_submit(|s: &mut App, tx| s.preview_use(tx))
        .on_layout(|s: &mut App, r| s.preview.field_rect = r);
    col().key(PREVIEW_SCROLL).scroll(ScrollSpec { stick_bottom: false }).child(field)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_runs_skip_line_breaks() {
        let t = "ab\n\ncd";
        assert_eq!(line_runs(0..t.len(), t), vec![0..2, 4..6]);
        assert_eq!(line_runs(2..4, t), Vec::<Range<usize>>::new());
    }

    #[test]
    fn a_space_separates_a_deletion_from_the_insertion_after_it() {
        let v = DraftVersion::new(VersionKind::Proposal, "Shorter", "Codex", "The cat sat.");
        let r = render(&v, "The dog sat.", &[&v]);
        assert_eq!(r.combined, "The dog cat sat.");
        assert_eq!(r.marks, vec![(4..7, DiffSegmentKind::Delete), (8..11, DiffSegmentKind::Insert)]);
        assert_eq!((r.added, r.removed), (1, 1));
        assert_eq!(r.first_change, Some(4));
        assert_eq!((r.position, r.total), (1, 1));
        assert_eq!(r.detail, "From Codex · just now · +1 −1 words");
    }
}
