//! The window: the top bar, the draft in its scroll view, the footer, and the chrome over them.

use rune::{Cx, El, ScrollSpec, col, el, text_input};

use crate::app::{App, DRAFT, SCROLL, TOP_BAR_HEIGHT};
use crate::styler;
use crate::theme::{Palette, draft_font};

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let dark = s.prefs.appearance().is_dark(cx.system_dark());
    let p = Palette::new(dark);
    // The Windows caption buttons follow the app's appearance, not the system's.
    cx.caption_color(p.text);
    // Reading the caret here keeps Rune tracking it, which `App::poll` scrolls by.
    let _ = cx.caret(DRAFT);
    let f = s.frame();
    let (w, h) = (f.editor_w, f.height);
    let opacity = if s.chrome_visible { 1.0 } else { 0.0 };
    let fade = crate::chrome::fade(s.chrome_visible, cx.is_reduced_motion());

    // The chrome's places: the sidebar button and toolbar first, then the menu bar after the
    // sidebar button, then the title clear of both.
    let mut insets = crate::chrome::insets(cx, 0.0);
    let first = crate::chrome::layout(&f, insets);
    let bar = crate::menubar::layout(&first);
    insets.leading_extra = bar.right;
    let chrome = crate::chrome::layout(&f, insets);
    let folded = bar.folded.is_some();
    let scale = cx.env().scale;

    let mut top_bar = el().key("top-bar").absolute(f.editor_x(), 0.0).size(w, TOP_BAR_HEIGHT).fill(p.canvas).window_drag();
    if let Some(r) = chrome.title {
        top_bar = top_bar.child(crate::title::view(s, &p, [r[0] - f.editor_x(), r[1], r[2], r[3]], opacity, fade));
    }
    let (menu_titles, menus) = crate::menubar::view(s, &p, &bar, opacity, fade);

    let mut root = col()
        .key("window")
        .w_full()
        .h_full()
        .fill(p.canvas)
        .on_layout(move |s: &mut App, r| {
            s.window = (r[2], r[3]);
            s.menus.folded = folded;
        })
        .on_screen_layout(move |s: &mut App, r| s.screen_frame_changed(r, scale))
        .on_window_focus(|s: &mut App, focused, tx| s.window_focus_changed(focused, tx))
        .on_pointer_move_local(|s: &mut App, at, tx| s.pointer_moved(at.window_x, at.window_y, tx))
        .child(draft(s, &p).absolute(f.editor_x(), TOP_BAR_HEIGHT).size(w, (h - TOP_BAR_HEIGHT).max(0.0)))
        .child(top_bar)
        .child(crate::footer::view(s, &p, opacity, fade))
        .children(menu_titles)
        .children(crate::chrome::view(s, &p, &chrome, opacity, fade))
        .children(menus);
    if let Some(alert) = crate::ui::alert_view(s, &p) {
        root = root.child(alert);
    }
    root
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
        .description("The draft text. Use the Format menu or Markdown to style text.")
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
        .on_secondary_press(|s: &mut App, press, tx| s.open_context_menu(press, tx))
        .on_submit(|s: &mut App, tx| {
            s.edit.insert("\n");
            s.text_did_change(tx);
        })
        .on_layout(|s: &mut App, r| s.field_rect = r);
    col().key(SCROLL).scroll(ScrollSpec { stick_bottom: false }).child(field)
}
