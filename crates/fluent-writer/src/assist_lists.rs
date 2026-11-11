//! The Assist panel's Comments and Versions tabs: their buttons, headers and card lists (Swift
//! `AssistController.swift` `CardView`, `CardList`, `layoutComments`, `layoutVersions`,
//! `commentCard`, `versionCard`).

use rune::{Align, El, Icon, ScrollSpec, Tx, col, el, icon, row, text};
use writer_agents::AgentAction;
use writer_core::{Comment, DraftVersion, VersionKind};

use crate::app::App;
use crate::assist::plural;
use crate::assist_view::{PAD, font, push_button, text_width};
use crate::commands::Command;
use crate::notes::{relative_time, shortcut_label};
use crate::theme::{Palette, rgba};
use crate::version_preview::{fitted_button_width, small_button};

// SF Symbols the cards use, drawn as vector shapes in a 20-unit box.
static CHECKMARK_CIRCLE: Icon = Icon {
    name: "fw-checkmark-circle",
    view: 20.0,
    body: r#"<circle cx="10" cy="10" r="7.6" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M6.7 10.2L9 12.5L13.4 7.6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
static UTURN_CIRCLE: Icon = Icon {
    name: "fw-arrow-uturn-backward-circle",
    view: 20.0,
    body: r#"<circle cx="10" cy="10" r="7.6" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M8.6 6.6L6.4 8.7L8.6 10.8M6.6 8.7H11.2C12.6 8.7 13.6 9.8 13.6 11.1C13.6 12.4 12.6 13.5 11.2 13.5H9.4" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
static PENCIL: Icon = Icon {
    name: "fw-pencil",
    view: 20.0,
    body: r#"<path d="M13.2 3.9L16.1 6.8L7.4 15.5L3.9 16.1L4.5 12.6Z M11.6 5.5L14.5 8.4" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
static TRASH: Icon = Icon {
    name: "fw-trash",
    view: 20.0,
    body: r#"<path d="M3.8 5.6H16.2M8 5.6V4.2C8 3.7 8.4 3.3 8.9 3.3H11.1C11.6 3.3 12 3.7 12 4.2V5.6M5.3 5.6L6 15.6C6.1 16.3 6.6 16.8 7.3 16.8H12.7C13.4 16.8 13.9 16.3 14 15.6L14.7 5.6M8.4 8.4V14M11.6 8.4V14" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
static XMARK: Icon = Icon {
    name: "fw-xmark",
    view: 20.0,
    body: r#"<path d="M5 5L15 15M15 5L5 15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>"#,
};
static CHECKBOX_TICK: Icon = Icon {
    name: "fw-checkbox-tick",
    view: 12.0,
    body: r#"<path d="M3 6.3L5.1 8.4L9 3.8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

/// A borderless symbol button tinted `quiet`, labelled and tipped with `tip`.
/// `AssistController.swift:517-528`
fn icon_button(key: String, glyph: &'static Icon, tip: &str, size: f32, glyph_size: f32, p: &Palette, on: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    let inset = ((size - glyph_size) / 2.0).round();
    el().key(key)
        .size(size, size)
        .role("button")
        .label(tip)
        .help(tip)
        .preserve_focus_on_press(true)
        .on_press_tx(on)
        .child(icon(glyph, glyph_size).color(p.quiet).absolute(inset, inset))
}

/// A card: radius 8, `card`, `cardHover` under the pointer, `cardSelected` and a `caret` α0.5
/// edge when selected. `AssistController.swift:21-57`
fn card(key: String, selected: bool, p: &Palette) -> El {
    let hover = if p.dark { rgba(255, 255, 255, 0.027) } else { rgba(0, 0, 0, 0.012) };
    el().key(key)
        .w_full()
        .radius(8.0)
        .fill(if selected { p.card_selected } else { p.card })
        .border(1.0, if selected { p.caret.alpha(0.5) } else { p.hairline })
        .hover(if selected { rgba(0, 0, 0, 0.0) } else { hover })
        .role("button")
        .preserve_focus_on_press(true)
}

/// A wrapped label of the cards' kind: `lines` 0 shows every line.
fn label(s: &str, size: f32, medium: bool, color: rune::Color, lines: usize) -> El {
    let t = text(s).font(font(size, medium)).color(color);
    if lines > 0 { t.max_lines(lines) } else { t.wrap() }
}

/// The Comments tab. `AssistController.swift:348-364`, `AssistController.swift:484-495`
pub fn comments(s: &App, p: &Palette, w: f32, h: f32) -> El {
    let inner = w - PAD * 2.0;
    let open = s.notes.open_comments();
    let resolved = s.notes.resolved_comments();
    let mut shown: Vec<&Comment> = open.clone();
    if s.assist.show_resolved {
        shown.extend(resolved.iter().copied());
    }
    let keys = shortcut_label(Command::AddComment);
    let add_w = fitted_button_width("Add Comment", 11.0) + 10.0;
    let mut tab = el()
        .key("assist-comments")
        .size(w, h)
        .child(
            el().key("assist-add-comment-frame").absolute(PAD, 0.0).size(add_w, 24.0).child(
                small_button("assist-add-comment", "Add Comment", 11.0, false, add_w, true, p)
                    .absolute(0.0, 1.0)
                    .help(format!("Comment on the selection or the current line ({keys})"))
                    .on_press_tx(|s: &mut App, tx| s.add_comment_command(tx)),
            ),
        );
    if !resolved.is_empty() {
        let title = "Show resolved";
        let cw = 14.0 + 4.0 + text_width(title, 11.0).ceil() + 2.0;
        let on = s.assist.show_resolved;
        let mark = el()
            .size(12.0, 12.0)
            .mt(3.0)
            .radius(3.0)
            .fill(if on { p.caret } else { p.card })
            .border(1.0, if on { p.caret } else { p.quiet.alpha(0.6) })
            .child(if on { icon(&CHECKBOX_TICK, 12.0).color(rune::Color::WHITE) } else { el() });
        tab = tab.child(
            row()
                .key("assist-show-resolved")
                .absolute(w - PAD - cw, 3.0)
                .size(cw, 18.0)
                .gap(6.0)
                .role("checkbox")
                .checked(on)
                .label(title)
                .preserve_focus_on_press(true)
                .on_press(|s: &mut App| s.assist.show_resolved = !s.assist.show_resolved)
                .child(mark)
                .child(text(title).font(font(11.0, false)).color(p.text).mt(2.0)),
        );
    }
    let top = 32.0;
    let bottom = 52.0;
    let list_h = (h - top - bottom).max(0.0);
    let mut list = col().key("assist-comment-list").w(inner).gap(8.0);
    for c in &shown {
        list = list.child(comment_card(s, c, p));
    }
    tab = tab.child(col().key("assist-comment-scroll").absolute(PAD, top).size(inner, list_h).scroll(ScrollSpec { stick_bottom: false }).child(list));
    if shown.is_empty() {
        let empty = format!(
            "Select some text, or put the caret on a line, and press {keys} to leave a comment.\n\nWrite as many as you like, then send them all at once. The agent's fixes come back as a version you can preview."
        );
        tab = tab.child(text(empty).key("assist-comments-empty").font(font(12.0, false)).color(p.quiet).align(Align::Center).wrap().absolute(PAD + 8.0, top + 30.0).w(inner - 16.0));
    }
    let attached = s.notes.attached_open_comments().len();
    let send_title = if attached == 0 {
        "Send Comments".to_string()
    } else {
        format!("Send {} to {}", plural(attached, "Comment", "Comments"), s.assist.provider.display_name())
    };
    let enabled = s.assist_can_send_comments();
    let send = push_button("assist-send-comments", &send_title, 12.0, inner, 22.0, enabled, p);
    tab.child(
        el().key("assist-send-comments-frame")
            .absolute(PAD, h - bottom + 6.0)
            .size(inner, 30.0)
            .child(if enabled { send.on_press_tx(|s: &mut App, tx| s.assist_send(AgentAction::ResolveComments, tx)) } else { send }.absolute(0.0, 4.0)),
    )
}

/// One comment: quote, body, replies, and Resolve or Reopen, Edit and Delete at its top right.
/// `AssistController.swift:530-574`
fn comment_card(s: &App, c: &Comment, p: &Palette) -> El {
    let selected = s.notes.active_comment_id() == Some(c.id.as_str());
    let quote = c.quote.replace('\n', " ");
    let quote_line = if c.detached { format!("“{quote}” — no longer in the draft") } else { format!("“{quote}”") };
    let excerpt: String = quote.chars().take(60).collect();
    let id = c.id.clone();
    let quote_font = rune::tok::TextStyle { italic: true, ..font(11.0, false) };
    let mut body = col()
        .w_full()
        .pt(10.0)
        .px(12.0)
        .pb(10.0)
        .child(text(&quote_line).key(format!("comment-quote-{}", c.id)).font(quote_font).color(p.quiet).max_lines(2).mr(72.0))
        .child(label(&c.body, 12.0, false, p.text, 0).key(format!("comment-body-{}", c.id)).mt(4.0));
    for (i, r) in c.replies.iter().enumerate() {
        body = body.child(label(&format!("{}: {}", r.author, r.text), 11.0, false, p.quiet, 0).key(format!("comment-reply-{}-{i}", c.id)).mt(6.0));
    }
    let open = c.is_open();
    let (glyph, tip) = if open { (&CHECKMARK_CIRCLE, "Resolve") } else { (&UTURN_CIRCLE, "Reopen") };
    let (rid, eid, did) = (c.id.clone(), c.id.clone(), c.id.clone());
    let buttons = row()
        .absolute_right(12.0, 8.0)
        .gap(6.0)
        .child(icon_button(format!("comment-resolve-{}", c.id), glyph, tip, 18.0, 15.0, p, move |s, _| s.set_comment_resolved(&rid, open)))
        .child(icon_button(format!("comment-edit-{}", c.id), &PENCIL, "Edit comment", 18.0, 15.0, p, move |s, tx| s.edit_comment(&eid, tx)))
        .child(icon_button(format!("comment-delete-{}", c.id), &TRASH, "Delete comment", 18.0, 15.0, p, move |s, _| s.delete_comment(&did)));
    card(format!("comment-card-{}", c.id), selected, p)
        .label(format!("Comment on “{excerpt}”: {}", c.body))
        .opacity(if open { 1.0 } else { 0.6 })
        .on_press_tx(move |s: &mut App, tx| s.reveal_comment(&id, tx))
        .child(body)
        .child(buttons)
}

/// The Versions tab: the header, then "Your draft" and one card per version, newest first.
/// `AssistController.swift:366-373`, `AssistController.swift:497-504`
pub fn versions(s: &App, p: &Palette, w: f32, h: f32) -> El {
    let inner = w - PAD * 2.0;
    let previewing = s.preview.version_id();
    let mut list = col().key("assist-version-list").w(inner).gap(8.0);
    let mine = card("version-card-draft".into(), previewing.is_none(), p)
        .label("Your draft")
        .on_press_tx(|s: &mut App, tx| s.close_preview(tx))
        .child(
            col().w_full().pt(9.0).px(12.0).pb(10.0)
                .child(label("Your draft", 12.0, true, p.text, 0).mr(20.0))
                .child(label(if previewing.is_none() { "What you're editing now" } else { "Back to editing (→ or esc)" }, 11.0, false, p.quiet, 0).mt(3.0)),
        );
    list = list.child(mine);
    for v in s.notes.versions() {
        list = list.child(version_card(v, previewing == Some(v.id.as_str()), p));
    }
    col().key("assist-versions").px(PAD).size(w, h).pb(16.0)
        .child(
            text("Each agent reply becomes a version. Click one to preview it as a diff, then use it or go back. Using one keeps your current draft here too.")
                .key("assist-versions-header")
                .font(font(11.0, false))
                .color(p.quiet)
                .wrap(),
        )
        .child(col().key("assist-version-scroll").mt(12.0).w(inner).grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list))
}

/// `AssistController.swift:588-603`, `AssistController.swift:605-621`
fn version_card(v: &DraftVersion, selected: bool, p: &Palette) -> El {
    let mut bits = vec![if v.kind == VersionKind::Snapshot { "Saved draft".to_string() } else { v.source.clone() }, relative_time(v.created)];
    if v.used_at.is_some() {
        bits.push("used".into());
    }
    let mut body = col()
        .w_full()
        .pt(9.0)
        .px(12.0)
        .pb(10.0)
        .child(label(&v.label, 12.0, true, p.text, 2).key(format!("version-label-{}", v.id)).mr(20.0))
        .child(label(&bits.join(" · "), 11.0, false, p.quiet, 0).key(format!("version-detail-{}", v.id)).mt(3.0));
    if let Some(fb) = v.feedback.as_deref().filter(|f| !f.is_empty()) {
        body = body.child(label(fb, 11.0, false, p.quiet, 3).key(format!("version-feedback-{}", v.id)).mt(3.0));
    }
    let (id, did) = (v.id.clone(), v.id.clone());
    card(format!("version-card-{}", v.id), selected, p)
        .label(format!("{}, {}", v.label, bits.join(", ")))
        .on_press_tx(move |s: &mut App, tx| s.preview_version(&id, tx))
        .child(body)
        .child(icon_button(format!("version-delete-{}", v.id), &XMARK, "Delete version", 14.0, 10.0, p, move |s, tx| s.assist_delete_version(&did, tx)).absolute_right(12.0, 9.0))
}

trait AbsoluteRight {
    /// Pins an element `right` points in from its parent's right edge and `top` down.
    fn absolute_right(self, right: f32, top: f32) -> Self;
}

impl AbsoluteRight for El {
    fn absolute_right(self, right: f32, top: f32) -> El {
        self.pin(None, Some(top), Some(right), None)
    }
}

