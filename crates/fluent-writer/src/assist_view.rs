//! Drawing the Assist panel: its frame, the provider menu, Close, the three tabs and the Ask
//! tab (Swift `AssistController.swift` `build` and `layout`). The Comments and Versions tabs are
//! in `assist_lists.rs`.

use rune::{Align, El, Icon, Key, KeyEvent, PopupMenu, ScrollSpec, Shadow, Tx, col, el, icon, row, text, text_input};
use rune::tok::TextStyle;
use writer_agents::{AgentAction, AgentProvider};

use crate::app::{App, Frame};
use crate::assist::INSTRUCTION;
use crate::notes::AssistTab;
use crate::theme::{CHARACTER_WIDTH_EM, Palette, rgba, ui_font};

/// Panel padding. `AssistController.swift:294`
pub const PAD: f32 = 22.0;
/// Where the tab content starts. `AssistController.swift:299`
pub const CONTENT_TOP: f32 = 106.0;
pub const PROVIDER_KEY: &str = "assist-provider";

/// IBM Plex Mono at `size` on AppKit's line height for it (ascent plus descent, 1.3 em), so
/// wrapped labels take the height the Swift labels did.
pub fn font(size: f32, medium: bool) -> TextStyle {
    ui_font(size, medium).lh(size * 1.3)
}

/// Width of `s` in IBM Plex Mono at `size`.
pub fn text_width(s: &str, size: f32) -> f32 {
    s.chars().count() as f32 * size * CHARACTER_WIDTH_EM
}

pub static CHEVRONS: Icon = Icon {
    name: "fw-chevron-up-down",
    view: 12.0,
    body: r#"<path d="M3.5 4.75L6 2.25L8.5 4.75M3.5 7.25L6 9.75L8.5 7.25" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
static CHECK: Icon = Icon {
    name: "fw-menu-check",
    view: 12.0,
    body: r#"<path d="M2.5 6.5L5 9L9.5 3" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

/// A push button in the look of the Swift app's: a light grey plate, IBM Plex Mono. `h` is the
/// bezel's height.
pub fn push_button(key: &str, title: &str, size: f32, w: f32, h: f32, enabled: bool, p: &Palette) -> El {
    let plate = if p.dark { rgba(255, 255, 255, 0.12) } else { rgba(0, 0, 0, 0.075) };
    let b = el()
        .key(key)
        .size(w, h)
        .radius(5.0)
        .fill(plate)
        .role("button")
        .label(title)
        .disabled(!enabled)
        .preserve_focus_on_press(true)
        .child(
            text(title)
                .font(ui_font(size, false))
                .color(if enabled { p.text } else { p.quiet })
                .align(Align::Center)
                .truncate()
                .absolute(0.0, ((h - size * 1.3) / 2.0).round())
                .w(w),
        );
    if enabled { b.press(p.text.alpha(0.08)) } else { b.opacity(0.6) }
}

/// The panel at the window's right edge, or nothing while it is hidden.
pub fn view(s: &App, p: &Palette, f: &Frame) -> El {
    if !s.assist_visible || f.panel_w <= 0.0 {
        return el().key("assist-panel").absolute(0.0, 0.0).size(0.0, 0.0);
    }
    let (w, h) = (f.panel_w, f.height);
    let tab = s.notes.assist_tab;
    let content_h = (h - CONTENT_TOP).max(0.0);
    let content = match tab {
        AssistTab::Ask => ask(s, p, w, content_h),
        AssistTab::Comments => crate::assist_lists::comments(s, p, w, content_h),
        AssistTab::Versions => crate::assist_lists::versions(s, p, w, content_h),
    };
    let mut panel = el()
        .key("assist-panel")
        .absolute(f.panel_x(), 0.0)
        .size(w, h)
        .fill(p.canvas)
        .role("group")
        .label("Assist")
        .child(el().key("assist-hairline").absolute(0.0, 0.0).size(1.0, h).fill(p.hairline))
        .child(content.key("assist-content").absolute(0.0, CONTENT_TOP).size(w, content_h))
        .child(provider_button(s, p))
        .child(
            el().key("assist-close")
                .absolute(w - PAD - 50.0, 42.0)
                .size(50.0, 20.0)
                .role("button")
                .label("Close assist")
                .preserve_focus_on_press(true)
                .on_press_tx(|s: &mut App, tx| s.assist_close(tx))
                .child(text("Close").font(font(12.0, false)).color(p.quiet).align(Align::Center).absolute(0.0, 2.0).w(50.0)),
        )
        .child(tabs(s, p, w));
    if s.assist.provider_menu_open {
        panel = panel.child(provider_menu(s, p));
    }
    panel
}

/// The borderless provider pop-up at (18, 40), 120×24. `AssistController.swift:185-191`
fn provider_button(s: &App, p: &Palette) -> El {
    let enabled = !s.assist.running;
    let color = if enabled { p.text } else { p.quiet };
    el().key(PROVIDER_KEY)
        .absolute(PAD - 4.0, 40.0)
        .size(120.0, 24.0)
        .role("combobox")
        .label("Provider")
        .a11y_value(s.assist.provider.display_name())
        .disabled(!enabled)
        .preserve_focus_on_press(true)
        .child(text(s.assist.provider.display_name()).font(font(12.0, false)).color(color).absolute(8.0, 4.0).w(90.0))
        .child(icon(&CHEVRONS, 12.0).color(color).absolute(120.0 - 20.0, 6.0))
        .on_press_tx(|s: &mut App, tx| {
            if s.assist.running {
                return;
            }
            if rune::popup_menu_supported() {
                let menu = PopupMenu::picker(AgentProvider::ALL.map(|p| (p.raw_value(), p.display_name())), s.assist.provider.raw_value());
                tx.popup_menu(menu, PROVIDER_KEY, |s: &mut App, picked, _| {
                    if let Some(p) = picked.and_then(|id| id.parse().ok()) {
                        s.assist_set_provider(p);
                    }
                });
            } else {
                s.assist.provider_menu_open = !s.assist.provider_menu_open;
            }
        })
}

/// The provider menu, drawn in the app where the platform has no pop-up menus (Linux): the
/// current provider ticked and placed over the button, as AppKit's pop-up opens.
fn provider_menu(s: &App, p: &Palette) -> El {
    let index = AgentProvider::ALL.iter().position(|x| *x == s.assist.provider).unwrap_or(0) as f32;
    let mut menu = col()
        .key("assist-provider-menu")
        .absolute(PAD - 4.0 - 4.0, 40.0 - 4.0 - index * 22.0)
        .w(128.0)
        .py(4.0)
        .radius(6.0)
        .fill(p.card)
        .border(1.0, p.hairline)
        .box_shadow(Shadow { x: 0.0, y: 4.0, blur: 16.0, spread: 0.0, color: rgba(0, 0, 0, if p.dark { 0.5 } else { 0.18 }), inset: false })
        .role("menu")
        .label("Provider")
        .modal_tx(true, |s: &mut App, _| s.assist.provider_menu_open = false);
    for provider in AgentProvider::ALL {
        let on = provider == s.assist.provider;
        let mut item = row()
            .key(match provider {
                AgentProvider::Codex => "assist-provider-codex",
                AgentProvider::Claude => "assist-provider-claude",
            })
            .h(22.0)
            .mx(4.0)
            .radius(4.0)
            .items_center()
            .role("menuitemradio")
            .checked(on)
            .label(provider.display_name())
            .hover(p.caret.alpha(0.18))
            .on_press(move |s: &mut App| s.assist_set_provider(provider));
        item = item.child(el().w(18.0).h(12.0).ml(2.0).child(if on { icon(&CHECK, 12.0).color(p.text) } else { el() }));
        menu = menu.child(item.child(text(provider.display_name()).font(font(12.0, false)).color(p.text)));
    }
    menu
}

/// "Ask" | "Comments" | "Versions" with counts, at (22, 72), the inner width, 22 tall.
/// `AssistController.swift:201-207`, `AssistController.swift:472-477`
fn tabs(s: &App, p: &Palette, w: f32) -> El {
    let inner = w - PAD * 2.0;
    let open = s.notes.open_count();
    let versions = s.notes.versions().len();
    let titles = [
        (AssistTab::Ask, "assist-tab-ask", "Ask".to_string()),
        (AssistTab::Comments, "assist-tab-comments", if open > 0 { format!("Comments {open}") } else { "Comments".into() }),
        (AssistTab::Versions, "assist-tab-versions", if versions > 0 { format!("Versions {versions}") } else { "Versions".into() }),
    ];
    let seg_w = ((inner - 4.0) / 3.0).floor();
    let track = if p.dark { rgba(255, 255, 255, 0.08) } else { rgba(0, 0, 0, 0.06) };
    let selected = if p.dark { rgba(255, 255, 255, 0.2) } else { p.card };
    let mut bar = row()
        .key("assist-tabs")
        .absolute(PAD, 72.0)
        .size(inner, 22.0)
        .padding(2.0)
        .radius(6.0)
        .fill(track)
        .role("tablist")
        .label("Assist section")
        .focus_group();
    for (i, (tab, key, title)) in titles.into_iter().enumerate() {
        let on = s.notes.assist_tab == tab;
        let mut seg = el()
            .key(key)
            .size(if i == 2 { inner - 4.0 - seg_w * 2.0 } else { seg_w }, 18.0)
            .radius(4.5)
            .role("tab")
            .checked(on)
            .label(title.clone())
            .preserve_focus_on_press(true)
            .on_press(move |s: &mut App| s.assist_select(tab))
            .child(text(&title).font(font(11.0, false)).color(p.text).align(Align::Center).truncate().w_full().mt(2.0));
        if on {
            seg = seg.fill(selected).box_shadow(Shadow { x: 0.0, y: 0.5, blur: 1.5, spread: 0.0, color: rgba(0, 0, 0, if p.dark { 0.4 } else { 0.18 }), inset: false });
        }
        bar = bar.child(seg);
    }
    bar
}

/// The width AppKit gives a recessed button sized to fit `title` at 12 pt, plus the 8 pt the
/// panel adds. `AssistController.swift:323-324`
pub fn quick_action_width(title: &str) -> f32 {
    (text_width(title, 12.0) + 24.0).ceil() + 8.0
}

/// The Ask tab. `AssistController.swift:313-346`
fn ask(s: &App, p: &Palette, w: f32, h: f32) -> El {
    let a = &s.assist;
    let inner = w - PAD * 2.0;
    let running = a.running;

    let mut actions = row().key("assist-actions").w(inner).flex_wrap().gaps(6.0, 6.0).mt(14.0);
    for action in AgentAction::QUICK_ACTIONS {
        let key = match action {
            AgentAction::Critique => "assist-critique",
            AgentAction::Clarify => "assist-clarify",
            AgentAction::Shorten => "assist-shorten",
            _ => "assist-stronger-opening",
        };
        let b = push_button(key, action.label(), 12.0, quick_action_width(action.label()), 22.0, !running, p)
            .label(format!("{} with the selected provider", action.label()));
        actions = actions.child(if running { b } else { b.on_press_tx(move |s: &mut App, tx| s.assist_send(action, tx)) });
    }

    let field = text_input(&a.instruction, font(13.0, false))
        .key(INSTRUCTION)
        .label("Instruction")
        .placeholder("Ask anything, or say what to change or add", p.quiet)
        .color(p.text)
        .caret(p.caret, p.selection)
        .w_full()
        .min_h(44.0)
        .pl(6.0)
        .pr(6.0)
        .pt(4.0)
        .pb(4.0)
        .on_edit(|s: &mut App| &mut s.assist.instruction)
        .on_input_key(instruction_key)
        .on_submit(|s: &mut App, tx| s.assist_send_instruction(tx));
    let field_box = col()
        .key("assist-instruction-box")
        .size(inner - 74.0, 44.0)
        .radius(6.0)
        .fill(p.card)
        .border(1.0, if p.dark { rgba(255, 255, 255, 0.14) } else { rgba(0, 0, 0, 0.13) })
        .scroll(ScrollSpec { stick_bottom: false })
        .child(field);
    let send_title = if running { "Stop" } else { "Send" };
    let send = el()
        .key("assist-send-frame")
        .size(66.0, 28.0)
        .mt(8.0)
        .child(
            push_button("assist-send", send_title, 12.0, 66.0, 22.0, true, p)
                .absolute(0.0, 3.0)
                .label(if running { "Stop request" } else { "Send instruction" })
                .on_press_tx(|s: &mut App, tx| s.assist_send_or_stop(tx)),
        );

    let mut c = col()
        .key("assist-ask")
        .px(PAD)
        .pb(20.0)
        .size(w, h)
        .child(text(&a.scope).key("assist-scope").font(font(12.0, false)).color(p.text).truncate().h(18.0).label(&a.scope_label))
        .child(
            text("Nothing is sent until you press Send. The agent gets this draft, your selection and comments, with no files, commands or web access.")
                .key("assist-privacy")
                .font(font(11.0, false))
                .color(p.quiet)
                .wrap()
                .mt(4.0),
        )
        .child(actions)
        .child(row().key("assist-instruction-row").w(inner).gap(8.0).items_start().mt(12.0).child(field_box).child(send));
    let mut gap = 10.0;
    if !a.status.is_empty() {
        c = c.child(text(&a.status).key("assist-status").font(font(12.0, false)).color(if a.status_warning { p.warning } else { p.quiet }).wrap().mt(gap));
        gap = 8.0;
    }
    if s.assist_preview_target().is_some() {
        let title = "Preview Version";
        let bw = crate::version_preview::fitted_button_width(title, 11.0) + 10.0;
        c = c.child(
            el().key("assist-preview-frame").size(bw, 24.0).mt(gap).child(
                crate::version_preview::small_button("assist-preview", title, 11.0, false, bw, true, p).absolute(0.0, 1.0).on_press_tx(|s: &mut App, tx| {
                    if let Some(id) = s.assist_preview_target().map(str::to_string) {
                        s.preview_version(&id, tx);
                    }
                }),
            ),
        );
        gap = 10.0;
    }
    let header_shown = !a.feedback.is_empty() || running;
    c.child(text("Feedback").key("assist-feedback-header").font(font(11.0, false)).color(p.quiet).h(16.0).mt(gap).opacity(if header_shown { 1.0 } else { 0.0 }))
        .child(
            col().key("assist-feedback-scroll").mt(2.0).grow(1.0).min_h(0.0).w(inner).scroll(ScrollSpec { stick_bottom: false }).child(
                text(&a.feedback)
                    .key("assist-feedback")
                    .font(ui_font(13.0, false).lh((13.0 * 1.3 * 1.35_f32).round()))
                    .color(p.text)
                    .wrap()
                    .selectable(true)
                    .selection_color(p.selection)
                    .label("Feedback")
                    .w(inner)
                    .pt(4.0)
                    .pb(4.0),
            ),
        )
}

/// Keys in the instruction field: Return sends (Option-Return types a line break, as in
/// AppKit's field editor; Shift-Return sends as plain Return does there); Esc stops a running
/// request, otherwise closes the panel. `AssistController.swift:693-703`
fn instruction_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let m = ev.mods;
    let plain = !m.cmd && !m.ctrl && !m.alt;
    match ev.key {
        Key::Escape if plain && !m.shift => {
            if s.assist.running { s.assist_stop() } else { s.assist_close(tx) }
            true
        }
        Key::Enter if plain && m.shift => {
            s.assist_send_instruction(tx);
            true
        }
        _ => false,
    }
}

