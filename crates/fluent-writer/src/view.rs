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
        .child(draft(s, &p, cx).absolute(f.editor_x(), s.draft_top()).size(w, (h - s.draft_top()).max(0.0)))
        .child(crate::version_preview::view(s, &p))
        .child(crate::find::view(s, &p, &f))
        .child(top_bar)
        .child(crate::footer::view(s, &p, opacity, fade))
        .children(crate::sidebar::view(s, &p, &f))
        .child(crate::assist_view::view(s, &p, &f))
        .children(menu_titles)
        .children(crate::chrome::view(s, &p, &chrome, opacity, fade))
        .children(menus)
        .child(crate::comment_composer::view(s, &p))
        .children(crate::open_picker::view(s, &p))
        .children(crate::remote_editing::view(s, &p))
        .child(crate::connect_agent::view(s, &p));
    if let Some(alert) = crate::ui::alert_view(s, &p) {
        root = root.child(alert);
    }
    root
}

fn draft(s: &App, p: &Palette, cx: &mut Cx<App>) -> El {
    let c = s.column();
    let (vw, vh) = s.visible();
    let m = &s.metrics;
    let mut styled = styler::style(&s.edit.text, &s.lines, m, p, &|line| s.images.space_after(line, s.image_gap()));
    if let Some(focus) = s.focus_range() {
        styled.spans.extend(styler::dimming(s.edit.text.len(), &focus, p));
    }
    styled.spans.extend(s.draft_highlights(p));
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
        .text_highlights(crate::find::highlights(s, p))
        .spellcheck(s.prefs.spell_check())
        .w_full()
        .min_h(vh)
        .pl(c.inset_x)
        .pr((vw - c.inset_x - c.width).max(0.0))
        .pt(c.inset_y)
        .pb(c.inset_y + c.bottom_overscroll)
        .on_edit(|s: &mut App| &mut s.edit)
        .on_input_change(|s: &mut App, tx| s.text_did_change(tx))
        .text_highlights(s.image_line_marks())
        .on_input_key(crate::keys::draft_key)
        .on_input_paste(|s: &mut App, content, tx| {
            // Text wins when there is any, so pasting from a web page still pastes its words.
            let png = content.image().filter(|_| content.text().is_empty()).map(|i| i.png().to_vec());
            png.is_some_and(|png| s.insert_images(&[], Some(&png), None, tx))
        })
        .on_drop_files(|s: &mut App, files, tx| {
            // The images go in where they were dropped.
            let at = tx.drop_point().and_then(|p| p.text_offset);
            s.insert_images(files, None, at, tx);
        })
        .accepts_extensions(&writer_core::markdown_images::IMAGE_EXTENSIONS)
        .on_secondary_press_local(|s: &mut App, at, tx| {
            s.menus.context_offset = at.text_offset;
            // Right-clicking a commented passage makes that comment the active one.
            if let Some(id) = at.text_offset.and_then(|o| s.notes.comment_at(o)).map(|c| c.id.clone()) {
                s.notes.set_active_comment(Some(id));
            }
            s.open_context_menu(at.window(), tx)
        })
        .on_submit(|s: &mut App, tx| {
            s.edit.insert("\n");
            s.text_did_change(tx);
        })
        .on_layout(|s: &mut App, r| s.field_rect = r);
    let page = el().w_full().child(field).children(crate::images::view(s, p, cx));
    // The draft's menu offers Check Spelling While Typing, as the text view's Spelling submenu did.
    col().key(SCROLL).scroll(ScrollSpec { stick_bottom: false }).on_spellcheck(s.prefs.spell_check(), |s: &mut App, on, _| s.prefs.set_spell_check(on)).child(page)
}
