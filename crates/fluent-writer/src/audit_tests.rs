//! Headless checks from the parity audit: behaviours PARITY.md lists that no feature's own
//! tests pinned down, each against the Swift line it cites.

use rune::{Key, Mods, Runtime};
use writer_core::{DraftVersion, FocusUnit, VersionKind};

use crate::app::{App, SaveState};
use crate::commands::Command;
use crate::notes::AssistTab;
use crate::tests::{harness, press, settle, type_text};

fn run(rt: &mut Runtime<App>, c: Command) {
    rt.update(|s, tx| s.run(c, tx));
    settle(rt, 0.05);
}

/// The accessibility node with `key`, as `--tree` writes it.
fn node(rt: &Runtime<App>, key: &str) -> serde_json::Value {
    fn find(v: &serde_json::Value, key: &str) -> Option<serde_json::Value> {
        if v.get("key").and_then(|k| k.as_str()) == Some(key) {
            return Some(v.clone());
        }
        v.get("children")?.as_array()?.iter().find_map(|c| find(c, key))
    }
    find(&rt.accessibility_json(), key).unwrap_or_else(|| panic!("no accessibility node {key}"))
}

/// The value of the accessibility node named `name`.
fn label_value(rt: &Runtime<App>, name: &str) -> Option<String> {
    let tree = rt.accessibility_tree();
    tree.nodes.iter().find(|(_, n)| n.label() == Some(name)).and_then(|(_, n)| n.value().map(str::to_owned))
}

fn str_of<'a>(v: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    v.get(field).and_then(|f| f.as_str())
}

/// A harness on a saved draft in the drafts folder.
fn saved(body: &str) -> (Runtime<App>, tempfile::TempDir) {
    let (mut rt, dir) = harness("");
    let path = dir.path().join("drafts/Saved.md");
    std::fs::write(&path, body).unwrap();
    rt.update(|s, tx| s.open(&path, tx));
    settle(&mut rt, 0.1);
    (rt, dir)
}

// MARK: 1. Window and chrome

/// `EditorController.swift:253`
#[test]
fn the_assist_panel_runs_down_the_right_edge() {
    let (mut rt, _dir) = harness("Text");
    rt.update(|s, tx| s.assist_show(None, Some(AssistTab::Comments), tx));
    settle(&mut rt, 1.0);
    let panel = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("assist-panel")).expect("the panel is up");
    assert_eq!(panel.target, [1060.0 - 380.0, 0.0, 380.0, 760.0]);
}

/// `ChromeControls.swift:147-157`: wider to 720 pt, but not in full screen.
#[test]
fn opening_assist_widens_a_narrow_window_except_in_full_screen() {
    let (mut rt, _dir) = harness("Text");
    rt.set_viewport((600.0, 700.0), 1.0);
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.panel_room_needed(), Some(720.0));
    rt.update(|s, tx| s.show_sidebar(crate::sidebar::Mode::Recent, tx));
    assert_eq!(rt.state.panel_room_needed(), Some(980.0), "with the sidebar pinned");
    rt.update(|s, _| s.win.full_screen = true);
    assert_eq!(rt.state.panel_room_needed(), None, "full screen keeps its size");
    rt.update(|s, _| s.win.full_screen = false);
    rt.set_viewport((1060.0, 700.0), 1.0);
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.panel_room_needed(), None, "wide enough");
}

/// The toolbar's Add Comment isn't a validated menu item: on an empty draft it beeps.
/// `ChromeControls.swift:83-84`, `CommentComposer.swift:140`
#[test]
fn the_add_comment_button_beeps_on_an_empty_draft() {
    let (mut rt, _dir) = harness("");
    rt.update(|s, tx| s.press_chrome_button(crate::chrome::Button::AddComment, tx));
    assert_eq!(rt.beeps(), 1);
    assert!(rt.state.composer.open.is_none());
}

// MARK: 2. Editor surface

/// `Theme.swift:60-64`, `EditorController.swift:283-311`, `MarkdownStyler.swift:17-42`
#[test]
fn the_column_and_caret_follow_the_swift_metrics() {
    let (mut rt, _dir) = harness("Text");
    let s = &rt.state;
    assert_eq!(crate::theme::DEFAULT_TEXT_SIZE, 19.0);
    assert_eq!(crate::theme::TEXT_SIZES, [14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 22.0, 24.0, 26.0, 28.0, 32.0]);
    let m = s.metrics;
    assert_eq!(m.line_height, 29.0, "round(19 × 1.55)");
    assert!((m.character_width - 11.4).abs() < 1e-4, "the width of 0 in IBM Plex Mono");
    assert!((m.hang_width() - 45.6).abs() < 1e-4, "4 characters");
    assert_eq!((m.caret_width(), m.caret_height()), (2.0, 28.0), "max(2, round(19 / 9)) and round(natural × 1.12)");
    // 1060 × 760: the scroll view is 720 tall under the top bar, running under the footer.
    let c = s.column();
    assert_eq!(c.width - m.hang_width(), 775.0, "round(68 × 11.4)");
    assert_eq!((c.inset_x, c.inset_y, c.bottom_overscroll), (97.0, 56.0, 252.0));
    let ([x, y, w, h], _) = rt.presented(crate::app::DRAFT).unwrap();
    assert_eq!((x, y, w), (0.0, 40.0, 1060.0));
    assert!(h >= 720.0, "at least as tall as the visible area");
    // A narrow window: the column shrinks to keep the padding, never under 160.
    rt.set_viewport((500.0, 700.0), 1.0);
    settle(&mut rt, 0.1);
    let c = rt.state.column();
    assert!((c.width - m.hang_width() - (500.0 - 2.0 * 57.6)).abs() < 1e-3);
    rt.set_viewport((420.0, 320.0), 1.0);
    settle(&mut rt, 0.1);
    assert!(rt.state.column().width - m.hang_width() >= 160.0);
    // Focus modes: the first and last lines can reach the middle.
    rt.set_viewport((1060.0, 760.0), 1.0);
    rt.update(|s, tx| s.set_focus(FocusUnit::Paragraph, tx));
    settle(&mut rt, 0.1);
    let c = rt.state.column();
    assert_eq!((c.inset_y, c.bottom_overscroll), (346.0, 0.0), "max(40, round(720 / 2 − 29 / 2))");
}

/// `Commands.swift:129-148`, `Commands.swift:179-180`; ⌘= as well as ⇧⌘= (oddity 23, fixed).
#[test]
fn text_sizes_step_through_the_list_keep_the_selection_and_flash() {
    let (mut rt, _dir) = harness("Some words here");
    rt.update(|s, _| s.edit.set_selection(5, 10));
    run(&mut rt, Command::BiggerText);
    assert_eq!(rt.state.metrics.font_size, 20.0);
    assert_eq!(rt.state.status().0, "Text size 20 pt");
    assert_eq!(rt.state.edit.range(), (5, 10), "the selection stays");
    assert_eq!(rt.state.prefs.text_size(), 20.0, "saved");
    run(&mut rt, Command::SmallerText);
    run(&mut rt, Command::SmallerText);
    assert_eq!(rt.state.metrics.font_size, 18.0);
    run(&mut rt, Command::DefaultTextSize);
    assert_eq!(rt.state.metrics.font_size, 19.0);
    rt.update(|s, tx| s.set_text_size(32.0, tx));
    assert!(!rt.state.command_enabled(Command::BiggerText), "disabled at 32");
    rt.update(|s, tx| s.set_text_size(14.0, tx));
    assert!(!rt.state.command_enabled(Command::SmallerText), "disabled at 14");
    // Both ⌘= and ⇧⌘= (the "+" key equivalent) answer Bigger Text.
    let mac = cfg!(target_os = "macos");
    for shift in [false, true] {
        let key = if shift { Key::Command('+') } else { Key::Command('=') };
        press(&mut rt, key, Mods { cmd: true, ctrl: !mac, logo: mac, shift, alt: false });
    }
    assert_eq!(rt.state.metrics.font_size, 16.0);
}

/// Smart quotes, smart dashes and text replacement reset at each launch, as AppKit's toggles
/// did. `EditorController.swift:164-167`
#[test]
fn substitution_toggles_last_until_the_app_quits() {
    let dir = tempfile::tempdir().unwrap();
    let mut prefs = crate::prefs::Prefs::load(dir.path());
    assert!(!prefs.smart_quotes() && !prefs.smart_dashes() && prefs.text_replacement());
    prefs.set_smart_quotes(true);
    prefs.set_smart_dashes(true);
    prefs.set_text_replacement(false);
    prefs.set_show_counts(false);
    assert!(prefs.smart_quotes() && prefs.smart_dashes() && !prefs.text_replacement(), "on for this run");
    let again = crate::prefs::Prefs::load(dir.path());
    assert!(!again.smart_quotes() && !again.smart_dashes() && again.text_replacement(), "back to the launch defaults");
    assert!(!again.show_counts(), "real preferences are kept");
}

// MARK: 4. Focus modes

/// `EditorController.swift:553-602`, `Commands.swift:120-127`, `Commands.swift:171-175`
#[test]
fn focus_modes_dim_flash_and_toggle() {
    let body = "One sentence here. Another one follows.\nA second paragraph.";
    let (mut rt, _dir) = harness(body);
    rt.update(|s, _| s.edit.set_caret(22));
    assert_eq!(rt.state.focus_range(), None, "off dims nothing");
    assert_eq!(rt.state.command_title(Command::ToggleFocus, "Toggle Focus"), "Turn Sentence Focus On");
    run(&mut rt, Command::ToggleFocus);
    assert_eq!(rt.state.focus, FocusUnit::Sentence);
    assert_eq!(rt.state.status().0, "Sentence focus");
    assert_eq!(rt.state.focus_range(), Some(19..39), "the sentence with the caret");
    assert_eq!(rt.state.command_title(Command::ToggleFocus, "Toggle Focus"), "Turn Focus Off");
    assert!(rt.state.command_checked(Command::FocusSentence) && !rt.state.command_checked(Command::FocusOff));
    run(&mut rt, Command::FocusParagraph);
    assert_eq!(rt.state.focus_range(), Some(0..39), "the hard line");
    assert_eq!(rt.state.status().0, "Paragraph focus");
    // A selection across both paragraphs keeps both bright.
    rt.update(|s, _| s.edit.set_selection(5, 45));
    assert_eq!(rt.state.focus_range(), Some(0..body.len()));
    run(&mut rt, Command::FocusTypewriter);
    assert_eq!(rt.state.focus_range(), None, "typewriter dims nothing");
    assert_eq!(rt.state.status().0, "Typewriter focus");
    assert_eq!(rt.state.command_title(Command::ToggleFocus, "Toggle Focus"), "Turn Paragraph Focus On", "names the last dimming mode");
    run(&mut rt, Command::ToggleFocus);
    assert_eq!(rt.state.focus, FocusUnit::Paragraph, "from typewriter, back to the last dimming mode");
    run(&mut rt, Command::ToggleFocus);
    assert_eq!(rt.state.focus, FocusUnit::Off);
    assert_eq!(rt.state.status().0, "Focus off");
    assert_eq!((rt.state.prefs.focus(), rt.state.prefs.last_dimming_focus()), (FocusUnit::Off, FocusUnit::Paragraph), "both saved");
}

/// Sentence focus keeps the caret line in the middle too. `WriterCore/TextUnits.swift:11`
#[test]
fn sentence_focus_centres_the_caret_line() {
    let body = "A paragraph of prose.\n\n".repeat(60);
    let (mut rt, _dir) = harness(&body);
    rt.update(|s, tx| s.set_focus(FocusUnit::Sentence, tx));
    settle(&mut rt, 1.0);
    rt.update(|s, _| s.edit.set_caret(30 * 23));
    press(&mut rt, Key::Right, Mods::default());
    settle(&mut rt, 1.0);
    let caret = rt.state.last_caret.expect("the caret");
    let middle = 40.0 + 720.0 / 2.0;
    assert!((caret[1] + caret[3] / 2.0 - middle).abs() <= rt.state.metrics.line_height / 2.0, "an arrow key re-centres");
}

/// With focus off the view scrolls only to keep the caret clear of the bottom:
/// `max(34 + 2 × lineHeight, 22%)`. `EditorController.swift:616-628`
#[test]
fn with_focus_off_typing_keeps_the_caret_off_the_bottom_edge() {
    let body = "Line of text.\n".repeat(80);
    let (mut rt, _dir) = harness(&body);
    settle(&mut rt, 0.5);
    type_text(&mut rt, "x");
    settle(&mut rt, 0.5);
    let caret = rt.state.last_caret.expect("the caret");
    let limit = 760.0 - (34.0f32 + 2.0 * 29.0).max(720.0 * 0.22);
    assert!(caret[1] + caret[3] <= limit + 1.0, "caret bottom {} under the limit {limit}", caret[1] + caret[3]);
    assert!(caret[1] + caret[3] >= limit - 30.0, "and only just: it isn't centred");
}

// MARK: 5. Saving

/// `EditorController.swift:428-433`, `EditorController.swift:471-477`, `Commands.swift:51-54`
#[test]
fn autosave_waits_0_8_s_and_save_flashes_saved() {
    let (mut rt, dir) = harness("");
    assert_eq!(rt.state.status().0, "", "a never-saved draft has no status");
    run(&mut rt, Command::Save);
    assert!(rt.state.transient.is_none(), "Save does nothing for an empty new draft");
    type_text(&mut rt, "Draft");
    assert_eq!(rt.state.status().0, "Edited");
    settle(&mut rt, 0.7);
    assert_eq!(rt.state.status().0, "Edited", "not yet");
    settle(&mut rt, 0.2);
    assert_eq!(rt.state.status().0, "Saved");
    let path = dir.path().join("drafts/Draft.md");
    assert!(path.exists());
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.status_tooltip.as_deref(), Some(path.to_str().unwrap()), "the tooltip is the path");
    type_text(&mut rt, "!");
    run(&mut rt, Command::Save);
    assert_eq!(rt.state.status().0, "Saved");
    assert_eq!(rt.state.transient.as_deref(), Some("Saved"), "a flash");
    // The title followed the first line, so the file was renamed with it.
    assert_eq!(std::fs::read_to_string(dir.path().join("drafts/Draft!.md")).unwrap(), "Draft!");
}

/// While a save has failed, typing leaves "Not saved — …" up. `EditorController.swift:706`
#[test]
fn a_failed_save_keeps_its_status_while_typing() {
    let (mut rt, dir) = harness("");
    crate::tests::set_folder_locked(&dir.path().join("drafts"), true);
    type_text(&mut rt, "Lost");
    settle(&mut rt, 1.0);
    assert!(matches!(rt.state.save_state, SaveState::Failed(_)));
    type_text(&mut rt, " more");
    let (status, warning) = rt.state.status();
    assert!(status.starts_with("Not saved — Couldn't save “Lost.md”: "), "{status}");
    assert!(warning);
    assert!(rt.state.library.recovery_snapshots().iter().any(|s| s.body.starts_with("Lost")), "a recovery copy is kept");
    crate::tests::set_folder_locked(&dir.path().join("drafts"), false);
}

/// The caret is remembered in state.json a second after it stops moving.
/// `EditorController.swift:727-731`
#[test]
fn the_caret_is_remembered_a_second_after_it_stops() {
    let (mut rt, dir) = saved("Hello there world");
    let path = dir.path().join("drafts/Saved.md");
    rt.update(|s, _| s.edit.set_selection(6, 11));
    settle(&mut rt, 0.5);
    let record = |rt: &Runtime<App>| rt.state.library.record_for_path(&path).map(|r| (r.selection_location, r.selection_length));
    assert_ne!(record(&rt), Some((6, 5)), "not yet");
    settle(&mut rt, 0.6);
    assert_eq!(record(&rt), Some((6, 5)));
}

/// `Commands.swift:182`
#[test]
fn show_in_finder_needs_a_local_file_or_text() {
    let (rt, _dir) = harness("");
    assert!(!rt.state.command_enabled(Command::Reveal));
    let (rt, _dir) = harness("Text");
    assert!(rt.state.command_enabled(Command::Reveal), "it saves first to make the file");
    let (rt, _dir) = saved("");
    assert!(rt.state.command_enabled(Command::Reveal), "an empty file still has a file");
}

/// Quitting stops a running Assist request before the leave check, even when the check then
/// keeps the app open. `main.swift:38-42`
#[test]
fn quitting_stops_assist_first() {
    let (mut rt, dir) = harness("");
    crate::tests::set_folder_locked(&dir.path().join("drafts"), true);
    type_text(&mut rt, "Words");
    settle(&mut rt, 1.0);
    rt.update(|s, _| s.assist.running = true);
    run(&mut rt, Command::Quit);
    assert!(!rt.state.assist.running);
    assert_eq!(rt.state.assist.status, "Stopped. Nothing was changed.");
    assert_eq!(rt.state.alerts.first().map(|a| a.message.as_str()), Some("This draft couldn't be saved."));
    crate::tests::set_folder_locked(&dir.path().join("drafts"), false);
}

// MARK: 6. Title

/// Comments and versions move with a renamed file. `NotesController.swift:53-63`
#[test]
fn renaming_moves_the_notes_with_the_file() {
    let (mut rt, dir) = saved("Some words");
    rt.update(|s, _| {
        s.add_comment(0..4, "Why?");
    });
    settle(&mut rt, 1.0);
    rt.update(|s, tx| s.begin_title_edit(true, tx));
    rt.update(|s, _| s.title.edit.replace_text("Renamed"));
    rt.update(|s, tx| s.end_title_edit(tx));
    settle(&mut rt, 1.0);
    rt.state.notes.wait_for_disk();
    let new = dir.path().join("drafts/Renamed.md");
    assert!(new.exists());
    let store = writer_core::NotesStore::new(dir.path().join("support"));
    let notes = store.load(new.to_str().unwrap(), "Some words");
    assert_eq!(notes.comments.len(), 1, "the comment went with the file");
}

// MARK: 7. Footer

/// `EditorController.swift:491-501`, `EditorController.swift:520-523`, `Commands.swift:156-160`
#[test]
fn counts_wait_0_15_s_flashes_last_2_2_s_and_show_counts_toggles() {
    let (mut rt, _dir) = harness("");
    type_text(&mut rt, "one");
    let before = rt.state.counts.clone();
    rt.update(|s, tx| {
        s.edit.insert(" two");
        s.text_did_change(tx);
    });
    rt.frame(0.1);
    assert_eq!(rt.state.counts, before, "not within 0.15 s");
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.counts, "2 words  ·  7 characters");
    rt.update(|s, _| s.edit.set_selection(0, 3));
    settle(&mut rt, 0.2);
    assert_eq!(rt.state.counts, "1 of 2 words  ·  3 of 7 characters");

    rt.update(|s, tx| s.flash_message("Hello", tx));
    settle(&mut rt, 2.1);
    assert_eq!(rt.state.status().0, "Hello");
    settle(&mut rt, 0.2);
    assert_ne!(rt.state.status().0, "Hello", "gone after 2.2 s");

    assert!(rt.state.command_checked(Command::ToggleCounts));
    run(&mut rt, Command::ToggleCounts);
    assert!(!rt.state.prefs.show_counts() && !rt.state.command_checked(Command::ToggleCounts));
    assert!(rt.find("counts").is_none());
}

// MARK: 8. Menus

/// The menu bar against `MainMenu.swift`, item by item. The Mac's app menu is the system's;
/// the Edit menu's Spelling and Grammar items Rune can't back are left out (GAPS.md).
#[test]
fn menus_match_the_swift_menu_bar() {
    use crate::commands::{Entry, MENUS};
    fn flat(entries: &[Entry], out: &mut Vec<String>) {
        for e in entries {
            match e {
                Entry::Item(_, title, keys) => out.push(format!("{title} {}", keys.0)),
                Entry::Separator => out.push("—".into()),
                Entry::Submenu(title, inner) => {
                    out.push(format!("{title} ›"));
                    flat(inner, out);
                    out.push("‹".into());
                }
            }
        }
    }
    let menu = |name: &str| {
        let mut out = Vec::new();
        flat(MENUS.iter().find(|(n, _)| *n == name).unwrap().1, &mut out);
        out.join(" | ")
    };
    assert_eq!(MENUS.iter().map(|(n, _)| *n).collect::<Vec<_>>(), ["File", "Edit", "Format", "View", "Assist", "Window"]);
    assert_eq!(
        menu("File"),
        "New mod+n | Open… mod+o | Open Other File… mod+shift+o | Open Folder… mod+alt+o | Open over SSH… mod+ctrl+o | — | Save mod+s | Save As… mod+shift+s | Rename… mod+shift+r | Show in Finder mod+alt+r | — | Copy for Twitter mod+shift+c | — | Close mod+w"
    );
    assert_eq!(
        menu("Edit"),
        "Undo mod+z | Redo mod+shift+z | — | Cut mod+x | Copy mod+c | Paste mod+v | Delete  | Select All mod+a | — | Find › | Find… mod+f | Find and Replace… mod+alt+f | Find Next mod+g | Find Previous mod+shift+g | Use Selection for Find mod+e | Jump to Selection  | ‹ | Spelling and Grammar › | Check Spelling While Typing  | ‹ | Substitutions › | Smart Quotes  | Smart Dashes  | Text Replacement  | ‹"
    );
    assert_eq!(
        menu("Format"),
        "Bold mod+b | Italic mod+i | Strikethrough mod+shift+u | Link mod+k | Image… mod+shift+i | — | Heading 1 mod+1 | Heading 2 mod+2 | Heading 3 mod+3 | Body mod+alt+0 | — | Bulleted List mod+l | Numbered List mod+alt+l | Blockquote mod+' | — | Indent mod+] | Outdent mod+["
    );
    assert_eq!(
        menu("View"),
        "Show Sidebar mod+ctrl+s | Recent Files mod+shift+e | — | Toggle Focus mod+d | Focus › | Off mod+ctrl+0 | Sentence mod+ctrl+1 | Paragraph mod+ctrl+2 | Typewriter mod+ctrl+3 | ‹ | — | Bigger Text mod+plus | Smaller Text mod+minus | Default Text Size mod+0 | — | Appearance › | Match System  | Light  | Dark  | ‹ | Show Counts mod+alt+c | — | Enter Full Screen mod+ctrl+f"
    );
    assert_eq!(
        menu("Assist"),
        "Ask Codex… mod+j | Ask Claude… mod+shift+j | Show Assist mod+alt+j | — | Add Comment… mod+alt+m | Show Comments mod+shift+m | — | Show Versions mod+alt+v | Older Version mod+alt+left | Newer Version mod+alt+right | Use This Version mod+alt+enter | — | Connect an Outside Agent… "
    );
    assert_eq!(menu("Window"), "Minimize mod+m | Zoom ");
}

/// `Commands.swift:169-194`
#[test]
fn menu_items_follow_the_swift_validation() {
    let (mut rt, _dir) = harness("");
    let s = &rt.state;
    for c in [Command::CopyForTwitter, Command::ShowAssist, Command::AskCodex, Command::AskClaude, Command::AddComment, Command::OlderVersion, Command::NewerVersion, Command::UseThisVersion] {
        assert!(!s.command_enabled(c), "{c:?} is disabled on an empty draft with no versions");
    }
    for c in [Command::NewDraft, Command::OpenDraftPicker, Command::OpenOtherFile, Command::OpenFolder, Command::OpenOverSsh, Command::Save, Command::SaveAs, Command::Rename, Command::Close, Command::ShowComments, Command::ShowVersions, Command::ConnectOutsideAgent] {
        assert!(s.command_enabled(c), "{c:?} is always enabled");
    }
    for c in [Command::Bold, Command::Italic, Command::Strikethrough, Command::Link, Command::Image, Command::Heading1, Command::Heading2, Command::Heading3, Command::Body, Command::BulletList, Command::NumberedList, Command::Quote, Command::Indent, Command::Outdent] {
        assert!(s.command_enabled(c), "Format › {c:?} is always enabled");
    }
    type_text(&mut rt, "Some text");
    assert_eq!(rt.state.command_title(Command::CopyForTwitter, "Copy for Twitter"), "Copy for Twitter");
    rt.update(|s, _| s.edit.set_selection(0, 4));
    assert_eq!(rt.state.command_title(Command::CopyForTwitter, "Copy for Twitter"), "Copy Selection for Twitter");
    assert!(rt.state.command_enabled(Command::CopyForTwitter) && rt.state.command_enabled(Command::AddComment));
    let v = DraftVersion::new(VersionKind::Proposal, "V", "Codex", "Other text");
    let id = v.id.clone();
    rt.update(|s, _| s.add_version(v));
    assert!(rt.state.command_enabled(Command::OlderVersion), "any version");
    assert!(!rt.state.command_enabled(Command::NewerVersion), "only while previewing");
    rt.update(|s, tx| s.preview_version(&id, tx));
    assert!(rt.state.command_enabled(Command::NewerVersion) && rt.state.command_enabled(Command::UseThisVersion));
}

// MARK: 9. Copy for Twitter

/// `Commands.swift:70-82`
#[test]
fn copy_for_twitter_copies_plain_text_and_counts_characters() {
    let (mut rt, _dir) = harness("# Title\n\n**Bold** words");
    let clip = rune::clipboard::memory();
    rt.use_clipboard(&clip);
    run(&mut rt, Command::CopyForTwitter);
    settle(&mut rt, 0.05);
    assert_eq!(clip.text().as_deref().map(String::as_str), Some("Title\n\nBold words"));
    assert_eq!(rt.state.status().0, "Copied draft as plain text · 17 characters");
    rt.update(|s, _| s.edit.set_selection(9, 17));
    run(&mut rt, Command::CopyForTwitter);
    settle(&mut rt, 0.05);
    assert_eq!(clip.text().as_deref().map(String::as_str), Some("Bold"));
    assert_eq!(rt.state.status().0, "Copied selection as plain text · 4 characters");
}

// MARK: 13. Images

fn with_image_cache(rt: &mut Runtime<App>) {
    rt.update(|s, _| {
        s.images.setup(std::sync::Arc::new(|| {}));
        s.refresh_images();
    });
    settle(rt, 0.1);
}

fn placeholder(rt: &Runtime<App>, i: usize) -> String {
    match &rt.state.images.blocks[i].content {
        crate::images::Content::Placeholder { message, .. } => message.clone(),
        crate::images::Content::Image(..) => "an image".into(),
    }
}

/// `ImageBlocks.swift:228-246`, `WriterCore/MarkdownImages.swift:11-44`
#[test]
fn image_lines_say_why_they_show_no_image() {
    let body = "![a](missing.png)\n\n![b](javascript:alert)\n\n![c](broken.png)\n\n![d](<ok.png> \"Title\")\n\n```\n![e](ok.png)\n```\n\ntext ![f](ok.png)";
    let (mut rt, dir) = saved(body);
    std::fs::write(dir.path().join("drafts/broken.png"), b"not a png").unwrap();
    image::RgbaImage::from_pixel(30, 10, image::Rgba([0, 0, 0, 255])).save(dir.path().join("drafts/ok.png")).unwrap();
    with_image_cache(&mut rt);
    assert_eq!(rt.state.images.blocks.len(), 4, "code blocks and lines with other text get none");
    assert_eq!(placeholder(&rt, 0), "Image not found · missing.png");
    assert_eq!(placeholder(&rt, 1), "Can't read image address “javascript:alert”");
    assert_eq!(placeholder(&rt, 2), "Can't open broken.png");
    assert_eq!(placeholder(&rt, 3), "an image", "angle brackets and a title are fine");
    assert_eq!(rt.state.images.blocks[3].size, (30.0, 10.0), "its own size: narrower than the column");
    let img = node(&rt, "image-3");
    assert_eq!((str_of(&img, "role"), str_of(&img, "name")), (Some("image"), Some("d")));
    let missing = node(&rt, "image-0");
    assert_eq!(str_of(&missing, "name"), Some("a"));
}

/// Image lines read as captions, in `quiet`. `ImageBlocks.swift:168-169`
#[test]
fn image_lines_are_quiet_and_their_previews_fade_outside_the_focus() {
    let (mut rt, dir) = saved("Intro.\n![x](ok.png)\nOutro.");
    image::RgbaImage::from_pixel(30, 10, image::Rgba([0, 0, 0, 255])).save(dir.path().join("drafts/ok.png")).unwrap();
    with_image_cache(&mut rt);
    rt.update(|s, tx| s.set_focus(FocusUnit::Paragraph, tx));
    rt.update(|s, _| s.edit.set_caret(0));
    settle(&mut rt, 0.3);
    let info = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("image-0")).unwrap();
    assert!((info.opacity - 0.28).abs() < 0.01, "outside the bright paragraph: {}", info.opacity);
    rt.update(|s, _| s.edit.set_caret(9));
    settle(&mut rt, 0.3);
    let info = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("image-0")).unwrap();
    assert!((info.opacity - 1.0).abs() < 0.01);
}

/// Pasted image files go in as images even with their names as text beside them; text alone
/// pastes as text. `ImageBlocks.swift:300-310`
#[test]
fn pasting_image_files_inserts_them() {
    let (mut rt, dir) = saved("Start\n");
    let file = dir.path().join("Holiday-photo_1.png");
    image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 255])).save(&file).unwrap();
    rt.update(|s, _| s.edit.set_caret(6));
    let content = rune::ClipboardContent::new("Holiday-photo_1.png").with_files([file.clone()]);
    rt.update(|s, tx| assert!(s.paste_images(&content, tx)));
    assert_eq!(rt.state.edit.text, "Start\n![Holiday photo 1](images/Holiday-photo_1.png)");
    let text_only = rune::ClipboardContent::new("words");
    rt.update(|s, tx| assert!(!s.paste_images(&text_only, tx), "text pastes as text"));
}

/// `ImageBlocks.swift:323-326`, `ImageBlocks.swift:347-357`
#[test]
fn an_image_already_beside_the_draft_is_linked_and_blank_lines_keep_it_on_its_own_line() {
    let (mut rt, dir) = saved("Before after");
    let file = dir.path().join("drafts/pics/a b.png");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 255])).save(&file).unwrap();
    rt.update(|s, tx| assert!(s.insert_images(std::slice::from_ref(&file), None, Some(6), tx)));
    // The space after the drop point isn't a line end, so a blank line goes in before it.
    assert_eq!(rt.state.edit.text, "Before\n\n![a b](pics/a%20b.png)\n\n after");
    assert_eq!(rt.state.edit.range(), (6 + 2 + 22 + 2, 6 + 2 + 22 + 2), "the caret goes after the insertion");
    assert_eq!(rt.state.status().0, "Image added · linked where it already is");
    assert!(!dir.path().join("drafts/images").exists(), "nothing copied");
}

/// `ImageBlocks.swift:312-315`, `ImageBlocks.swift:229`
#[test]
fn a_remote_draft_takes_no_images() {
    let (mut rt, dir) = harness("");
    let loc = writer_core::RemoteLocation::parse("me@host:~/notes/a.md").unwrap();
    rt.update(|s, _| {
        let doc = s.library.open_remote(&loc, b"![x](a.png)").unwrap();
        s.load(doc);
    });
    with_image_cache(&mut rt);
    assert_eq!(placeholder(&rt, 0), "Images aren't shown for files opened over SSH");
    let file = dir.path().join("p.png");
    image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 255])).save(&file).unwrap();
    rt.update(|s, tx| assert!(s.insert_images(std::slice::from_ref(&file), None, None, tx)));
    assert_eq!(rt.state.status().0, "Images can't be added to files opened over SSH yet");
    assert_eq!(rt.state.edit.text, "![x](a.png)");
}

// MARK: 16. Version preview in narrow windows

/// `VersionPreview.swift:132-152`
#[test]
fn the_preview_bar_gives_way_in_narrow_windows() {
    let (mut rt, _dir) = harness("The dog sat on the mat.");
    let v = DraftVersion::new(VersionKind::Proposal, "A label", "Codex", "The cat sat on the mat.");
    let id = v.id.clone();
    rt.update(|s, _| s.add_version(v));
    rt.update(|s, tx| s.preview_version(&id, tx));
    settle(&mut rt, 0.1);
    assert!(rt.find("version-back").is_some());
    assert_eq!(node(&rt, "version-use").get("name").and_then(|n| n.as_str()), Some("Use This Version"));
    rt.set_viewport((600.0, 700.0), 1.0);
    settle(&mut rt, 0.1);
    assert!(rt.find("version-back").is_none(), "under 620 pt Back to Draft goes");
    assert_eq!(str_of(&node(&rt, "version-use"), "name"), Some("Use This Version"));
    assert!(rt.find("version-title").is_some());
    rt.set_viewport((450.0, 700.0), 1.0);
    settle(&mut rt, 0.1);
    assert_eq!(str_of(&node(&rt, "version-use"), "name"), Some("Use"), "under 460 pt it shortens");
    // 450 wide: the labels get 450 − 16 − 44 − 8 − (34 + 34 + 20 + 12) − 12 = 270 pt.
    assert!(rt.find("version-title").is_some());
    rt.set_viewport((420.0, 700.0), 1.0);
    settle(&mut rt, 0.1);
    let help = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("version-bar")).and_then(|e| e.help);
    let label_w = 420.0 - 16.0 - 44.0 - 112.0;
    assert!(label_w >= 140.0 || help.as_deref() == Some("A label\nFrom Codex · just now · +1 −1 words"));
}

// MARK: 19. Accessibility

/// `EditorController.swift:180-181`, `EditorController.swift:215`, `EditorController.swift:229-230`,
/// `ChromeControls.swift:63`
#[test]
fn the_editor_and_footer_name_themselves() {
    let (mut rt, _dir) = saved("Words here");
    let draft = node(&rt, crate::app::DRAFT);
    assert_eq!(str_of(&draft, "name"), Some("Draft"));
    assert_eq!(str_of(&draft, "description"), Some("The draft text. Use the Format menu or Markdown to style text."));
    assert_eq!(str_of(&node(&rt, "title"), "name"), Some("Draft title"));
    // `--tree` leaves a label's value out (it is the label's text); the tree itself has it.
    assert_eq!(label_value(&rt, "Save status").as_deref(), Some("Saved"));
    assert_eq!(label_value(&rt, "Counts").as_deref(), Some("2 words  ·  10 characters"));
    rt.update(|s, _| {
        s.add_comment(0..5, "Hm");
    });
    settle(&mut rt, 0.1);
    let b = node(&rt, "comments-button");
    assert_eq!((str_of(&b, "name"), str_of(&b, "value")), (Some("Comments"), Some("1")));
    assert_eq!(node(&rt, "versions-button").get("value"), None, "no count, no value");
}

/// `CommentComposer.swift:57`, `VersionPreview.swift:78-122`, `VersionPreview.swift:297-303`
#[test]
fn the_composer_and_the_preview_name_themselves() {
    let (mut rt, _dir) = harness("The dog sat.");
    run(&mut rt, Command::AddComment);
    settle(&mut rt, 0.1);
    assert_eq!(str_of(&node(&rt, crate::comment_composer::COMPOSER_FIELD), "name"), Some("Comment"));
    press(&mut rt, Key::Escape, Mods::default());
    let v = DraftVersion::new(VersionKind::Proposal, "Cat", "Codex", "The cat sat.");
    let id = v.id.clone();
    rt.update(|s, _| s.add_version(v));
    rt.update(|s, tx| s.preview_version(&id, tx));
    settle(&mut rt, 0.1);
    let bar = node(&rt, "version-bar");
    assert_eq!((str_of(&bar, "role"), str_of(&bar, "name")), (Some("toolbar"), Some("Version preview")));
    let diff = node(&rt, crate::version_preview::PREVIEW);
    assert_eq!(str_of(&diff, "name"), Some("Version preview"));
    // The Swift's value was the version's own text; Rune gives a text field's value as its
    // text, so it reads the diff (GAPS.md).
    assert_eq!(str_of(&diff, "value"), Some("The dog cat sat."));
    assert_eq!(str_of(&diff, "description"), Some("Inserted text is green, removed text is struck through."));
    assert_eq!(str_of(&node(&rt, "version-position"), "name"), Some("Version 1 of 1"));
    assert_eq!(str_of(&node(&rt, "version-older"), "name"), Some("Older version (←)"));
    assert_eq!(str_of(&node(&rt, "version-newer"), "name"), Some("Newer version (→)"));
}

// MARK: 20. Dev variant

/// `WriterCore/AppVariant.swift:9-13`, `script/build-app.sh:17-21`
#[test]
fn the_variant_comes_from_the_bundle_or_the_build_folder() {
    use writer_core::AppVariant;
    let dir = tempfile::tempdir().unwrap();
    let bundle = |name: &str, variant: Option<&str>| {
        let macos = dir.path().join(format!("{name}.app/Contents/MacOS"));
        std::fs::create_dir_all(&macos).unwrap();
        let key = variant.map_or(String::new(), |v| format!("<key>FWVariant</key><string>{v}</string>"));
        let plist = format!(r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleName</key><string>{name}</string>{key}</dict></plist>"#);
        std::fs::write(macos.parent().unwrap().join("Info.plist"), plist).unwrap();
        macos.join("FluentWriter")
    };
    assert_eq!(crate::variant_for_exe(Some(&bundle("Dev", Some("dev")))), AppVariant::Dev);
    assert_eq!(crate::variant_for_exe(Some(&bundle("Std", Some("standard")))), AppVariant::Standard);
    assert_eq!(crate::variant_for_exe(Some(&bundle("Plain", None))), AppVariant::Standard);
    let target = dir.path().join("build-out");
    std::fs::create_dir_all(target.join("debug")).unwrap();
    std::fs::write(target.join("CACHEDIR.TAG"), "Signature: 8a477f597d28d172789f06886806bc55").unwrap();
    assert_eq!(crate::variant_for_exe(Some(&target.join("debug/fluent-writer"))), AppVariant::Dev, "Cargo's target folder, wherever it is");
    assert_eq!(crate::bundle_id(AppVariant::Standard), "com.sunkenintime.fluentwriter");
    assert_eq!(crate::bundle_id(AppVariant::Dev), "com.sunkenintime.fluentwriter.dev");
    assert_eq!(AppVariant::Dev.app_name(), "Fluent Writer Dev");
}

/// Reduce Motion: the chrome fades at once and the peek slides out at once.
/// `EditorController.swift:683`, `ChromeControls.swift:220-240`
#[test]
fn reduce_motion_makes_the_chrome_and_the_peek_instant() {
    let (mut rt, _dir) = harness("Hello");
    rt.set_system_reduced_motion(true);
    settle(&mut rt, 0.1);
    type_text(&mut rt, "x");
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    let title = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("title")).unwrap();
    assert!(title.opacity < 0.5, "faded within two frames, not 0.6 s: {}", title.opacity);
    rt.pointer_move(400.0, 300.0);
    settle(&mut rt, 0.1);
    rt.pointer_move(2.0, 300.0);
    settle(&mut rt, 0.3);
    assert!(rt.state.sidebar_peeking);
    rt.update(|s, tx| s.end_peek(tx));
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("sidebar").is_none(), "gone at once");
}

/// `AssistController.swift:182-251`, `:452-466`, `:550`, `:601`, `:849`
#[test]
fn the_assist_panel_and_its_cards_name_themselves() {
    let (mut rt, _dir) = harness("One two three.\nFour five.");
    rt.update(|s, tx| s.assist_show(None, Some(AssistTab::Ask), tx));
    settle(&mut rt, 0.2);
    let panel = node(&rt, "assist-panel");
    assert_eq!((str_of(&panel, "role"), str_of(&panel, "name")), (Some("group"), Some("Assist")));
    assert_eq!(str_of(&node(&rt, "assist-close"), "name"), Some("Close assist"));
    assert_eq!(str_of(&node(&rt, crate::assist::INSTRUCTION), "name"), Some("Instruction"));
    assert_eq!(str_of(&node(&rt, "assist-tabs"), "name"), Some("Assist section"));
    assert_eq!(str_of(&node(&rt, "assist-scope"), "name"), Some("Will send the full draft, 5 words, to Codex"));
    rt.update(|s, _| s.edit.set_selection(0, 7));
    rt.update(|s, _| s.assist_refresh_scope());
    settle(&mut rt, 0.1);
    assert_eq!(str_of(&node(&rt, "assist-scope"), "name"), Some("Will send the draft, 5 words, focused on 2 selected words, to Codex"));
    let tree = rt.accessibility_tree();
    let names: Vec<&str> = tree.nodes.iter().filter_map(|(_, n)| n.label()).collect();
    for want in ["Provider", "Critique with the selected provider", "Stronger opening with the selected provider", "Send instruction"] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    // Cards.
    let mut id = String::new();
    rt.update(|s, _| id = s.add_comment(8..13, "Line\nbreak").unwrap());
    rt.update(|s, tx| s.assist_show(None, Some(AssistTab::Comments), tx));
    settle(&mut rt, 0.2);
    let card = node(&rt, &format!("comment-card-{id}"));
    assert_eq!((str_of(&card, "role"), str_of(&card, "name")), (Some("button"), Some("Comment on “three”: Line\nbreak")));
    assert_eq!(str_of(&node(&rt, &format!("comment-resolve-{id}")), "name"), Some("Resolve"));
    assert_eq!(str_of(&node(&rt, &format!("comment-edit-{id}")), "name"), Some("Edit comment"));
    assert_eq!(str_of(&node(&rt, &format!("comment-delete-{id}")), "name"), Some("Delete comment"));
    let v = DraftVersion::new(VersionKind::Proposal, "Tighter", "Claude", "One two.");
    let vid = v.id.clone();
    rt.update(|s, _| s.add_version(v));
    rt.update(|s, tx| s.assist_show(None, Some(AssistTab::Versions), tx));
    settle(&mut rt, 0.2);
    let card = node(&rt, &format!("version-card-{vid}"));
    assert_eq!((str_of(&card, "role"), str_of(&card, "name")), (Some("button"), Some("Tighter, Claude, just now")));
    assert_eq!(str_of(&node(&rt, &format!("version-delete-{vid}")), "name"), Some("Delete version"));
}

/// A preview opens scrolled to its first change, far down a long draft.
/// `VersionPreview.swift:180-181`, `:322-328`
#[test]
fn a_preview_opens_at_its_first_change_on_a_long_draft() {
    let body = "Line of the draft that stays the same.\n\n".repeat(80);
    let (mut rt, _dir) = harness(&body);
    let at = body.len() - 40 * 6;
    let changed = format!("{}Changed {}", &body[..at], &body[at..]);
    let v = DraftVersion::new(VersionKind::Proposal, "Late", "Codex", changed);
    let id = v.id.clone();
    rt.update(|s, _| s.add_version(v));
    rt.update(|s, tx| s.preview_version(&id, tx));
    settle(&mut rt, 1.0);
    let caret = rt.state.preview.rendered.first_change.unwrap();
    assert!(caret > body.len() / 2);
    let field = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(crate::version_preview::PREVIEW)).unwrap();
    assert!(field.rect[1] < -1000.0, "scrolled far down: the field's top is at {}", field.rect[1]);
    assert!(rt.state.preview.edit.focused, "the diff has the keyboard");
}

/// On quit the draft's notes are written and the live-state file goes.
/// `main.swift:44-46`, `NotesController.swift:254-257`
#[test]
fn quitting_writes_the_notes_and_removes_the_live_state() {
    let (mut rt, dir) = saved("Some words");
    rt.update(|s, _| {
        s.add_comment(0..4, "Why?");
    });
    settle(&mut rt, 0.6);
    rt.state.notes.wait_for_disk();
    let live = dir.path().join("support/Live.json");
    assert!(live.exists(), "written while running");
    run(&mut rt, Command::Quit);
    rt.state.notes.wait_for_disk();
    assert!(!live.exists(), "gone on quit");
    let store = writer_core::NotesStore::new(dir.path().join("support"));
    let path = dir.path().join("drafts/Saved.md");
    assert_eq!(store.load(path.to_str().unwrap(), "Some words").comments.len(), 1, "the comment was written");
}

/// Oddity 6, kept: Format commands edit the draft even from the title field or the preview.
/// `Commands.swift:86-116`
#[test]
fn format_commands_edit_the_draft_from_the_title_and_the_preview() {
    let (mut rt, _dir) = harness("word");
    rt.update(|s, _| s.edit.set_caret(2));
    rt.update(|s, tx| s.begin_title_edit(false, tx));
    settle(&mut rt, 0.1);
    crate::tests::command(&mut rt, 'b', false, false);
    assert_eq!(rt.state.edit.text, "**word**");
    let v = DraftVersion::new(VersionKind::Proposal, "V", "Codex", "other");
    let id = v.id.clone();
    rt.update(|s, _| s.add_version(v));
    rt.update(|s, tx| s.preview_version(&id, tx));
    settle(&mut rt, 0.1);
    crate::tests::command(&mut rt, 'i', false, false);
    assert_eq!(rt.state.edit.text, "***word***");
    assert!(rt.state.preview.is_active(), "the preview stays");
}
