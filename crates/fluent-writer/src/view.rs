//! The window: the top bar, the draft in its scroll view, and the footer.

use rune::{Align, Cx, El, ScrollSpec, Spring, col, el, text, text_input};
use writer_core::FocusUnit;

use crate::app::{App, DRAFT, FOOTER_HEIGHT, SCROLL, TOP_BAR_HEIGHT, focus_name};
use crate::styler;
use crate::theme::{Palette, draft_font, ui_font};

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let p = Palette::new(s.prefs.appearance().is_dark(cx.system_dark()));
    // Reading the caret here keeps Rune tracking it, which `App::poll` scrolls by.
    let _ = cx.caret(DRAFT);
    let f = s.frame();
    let (w, h) = (f.editor_w, f.height);
    let chrome = if s.chrome_visible { 1.0 } else { 0.0 };
    // Chrome comes back quickly and leaves slowly.
    let fade = Spring::new(if s.chrome_visible { 0.25 } else { 0.6 }, 0.0);

    let top_bar = el().key("top-bar").absolute(f.editor_x(), 0.0).size(w, TOP_BAR_HEIGHT).fill(p.canvas).child(
        text(s.shown_title())
            .key("title")
            .font(ui_font(13.0, false))
            .color(if s.draft.has_manual_title { p.text.alpha(0.75) } else { p.quiet })
            .align(Align::Center)
            .truncate()
            .absolute(80.0, 11.0)
            .w((w - 160.0).max(0.0))
            .opacity(chrome)
            .opacity_motion(fade),
    );

    col()
        .key("window")
        .w_full()
        .h_full()
        .fill(p.canvas)
        .on_layout(|s: &mut App, r| s.window = (r[2], r[3]))
        .on_pointer_move(|s: &mut App, at| s.pointer = (at.x, at.y))
        .child(draft(s, &p).absolute(f.editor_x(), TOP_BAR_HEIGHT).size(w, (h - TOP_BAR_HEIGHT).max(0.0)))
        .child(top_bar)
        .child(footer(s, &p, chrome, fade))
}

fn draft(s: &App, p: &Palette) -> El {
    let c = s.column();
    let (vw, vh) = s.visible();
    let m = &s.metrics;
    let mut styled = styler::style(&s.edit.text, &s.lines, m, p, &|_| 0.0);
    if let Some(focus) = s.focus_range() {
        styled.spans.extend(styler::dimming(s.edit.text.len(), &focus, p));
    }
    let field = text_input(&s.edit, draft_font(m.font_size))
        .key(DRAFT)
        .label("Draft")
        .color(p.text)
        .caret(p.caret, p.selection)
        .caret_size(m.caret_width(), m.caret_height())
        .spans(styled.spans)
        .paragraph_blocks(styled.blocks)
        .input_strikes(styled.strikes)
        .spellcheck(s.prefs.spell_check())
        .w_full()
        .min_h(vh)
        .pl(c.inset_x)
        .pr((vw - c.inset_x - c.width).max(0.0))
        .pt(c.inset_y)
        .pb(c.inset_y + c.bottom_overscroll)
        .on_edit(|s: &mut App| &mut s.edit)
        .on_input_change(|s: &mut App, tx| s.text_did_change(tx))
        .on_input_key(crate::keys::draft_key)
        .on_submit(|s: &mut App, tx| {
            s.edit.insert("\n");
            s.text_did_change(tx);
        })
        .on_layout(|s: &mut App, r| s.field_rect = r);
    col().key(SCROLL).scroll(ScrollSpec { stick_bottom: false }).child(field)
}

/// Status on the left, counts on the right, focus mode centred only when there's room for it.
fn footer(s: &App, p: &Palette, chrome: f32, fade: Spring) -> El {
    let f = s.frame();
    let (w, h) = (f.editor_w, f.height);
    let font = ui_font(12.0, false);
    let cw = 12.0 * crate::theme::CHARACTER_WIDTH_EM;
    let width = |t: &str| if t.is_empty() { 0.0 } else { (t.chars().count() as f32 * cw).ceil() + 4.0 };
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
        text(t).key(key).font(font).color(p.quiet).align(align).truncate().absolute(x, 9.0).size(width, 16.0).opacity(chrome).opacity_motion(fade)
    };
    let mut bar = el().key("footer").absolute(f.editor_x(), h - FOOTER_HEIGHT).size(w, FOOTER_HEIGHT).fill(p.canvas);
    if !status.is_empty() {
        bar = bar.child(label("status", &status, pad, status_w, Align::Start).color(if warning { p.warning } else { p.quiet }).label("Save status"));
    }
    if counts.is_empty() {
        return bar;
    }
    if focus_fits && !focus.is_empty() {
        bar = bar.child(label("focus", focus, focus_x, focus_w, Align::Center));
    }
    bar.child(label("counts", counts, w - pad - counts_w, counts_w, Align::End).label("Counts"))
}
