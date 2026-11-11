//! Headless tests for the window's chrome and document handling: the toolbar and title layout,
//! chrome fading, renaming, the footer, alerts, leaving a draft, Open and Save As, the
//! in-window menus and the window frame.

use rune::{Key, KeyEvent, Modifier, Mods, Runtime};
use writer_core::{DraftLibrary, FocusUnit};

use crate::app::{App, Frame, SaveState};
use crate::chrome::{self, Insets};
use crate::commands::Command;
use crate::menubar::Source;
use crate::prefs::Prefs;
use crate::tests::{command, harness, press, settle, type_text};

fn click(rt: &mut Runtime<App>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key}"));
    // Moving there brings the chrome back first, as it does for a person.
    rt.pointer_move(x, y);
    settle(rt, 0.3);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt, 0.05);
}

fn info(rt: &Runtime<App>, key: &str) -> rune::ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element {key}"))
}

fn run(rt: &mut Runtime<App>, c: Command) {
    rt.update(|s, tx| s.run(c, tx));
    settle(rt, 0.05);
}

/// The app on a library whose drafts folder can't be written, so every save fails.
fn failing(body: &str) -> (Runtime<App>, tempfile::TempDir) {
    let (mut rt, dir) = harness("");
    let drafts = dir.path().join("drafts");
    let mut perms = std::fs::metadata(&drafts).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&drafts, perms).unwrap();
    type_text(&mut rt, body);
    settle(&mut rt, 1.0);
    assert!(matches!(rt.state.save_state, SaveState::Failed(_)), "the save should fail");
    (rt, dir)
}

fn writable(dir: &tempfile::TempDir) {
    let drafts = dir.path().join("drafts");
    let mut perms = std::fs::metadata(&drafts).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&drafts, perms).unwrap();
}

// MARK: Layout

#[test]
fn toolbar_and_title_sit_where_the_swift_put_them() {
    let f = Frame { width: 1060.0, height: 760.0, side_w: 0.0, editor_w: 1060.0, panel_w: 0.0 };
    // macOS: AppKit's traffic lights end at 67 pt and centre at 13 pt.
    let l = chrome::layout(&f, Insets { traffic_lights: Some((67.0, 13.0)), ..Default::default() });
    assert_eq!(l.sidebar, [79.0, 1.0, 28.0, 24.0], "12 pt right of the zoom button, centred on it");
    // Right to left: Assist 12 pt from the edge, 2 pt gaps, 10 pt before Add Comment.
    assert_eq!(l.toolbar[3], [1020.0, 1.0, 28.0, 24.0]);
    assert_eq!(l.toolbar[2], [990.0, 1.0, 28.0, 24.0]);
    assert_eq!(l.toolbar[1], [960.0, 1.0, 28.0, 24.0]);
    assert_eq!(l.toolbar[0], [922.0, 1.0, 28.0, 24.0]);
    // Title: min(420, width − 2 × reserve), reserve 132 for the buttons; 18 pt tall at midY − 9.
    assert_eq!(l.title, Some([320.0, 4.0, 420.0, 18.0]));

    // Full screen, Windows and Linux: the sidebar button at (12, 8).
    let l = chrome::layout(&f, Insets::default());
    assert_eq!(l.sidebar, [12.0, 8.0, 28.0, 24.0]);
    assert_eq!(l.toolbar[3][1], 8.0);
    assert_eq!(l.title.unwrap()[1], 11.0);

    // A narrow window: the title gives way, and hides under 60 pt.
    let f = Frame { width: 420.0, height: 320.0, side_w: 0.0, editor_w: 420.0, panel_w: 0.0 };
    let l = chrome::layout(&f, Insets::default());
    assert_eq!(l.title, Some([132.0, 11.0, 156.0, 18.0]));
    let f = Frame { width: 300.0, height: 320.0, side_w: 0.0, editor_w: 300.0, panel_w: 0.0 };
    assert_eq!(chrome::layout(&f, Insets::default()).title, None);

    // Windows' caption buttons push the toolbar left.
    let f = Frame { width: 1060.0, height: 760.0, side_w: 0.0, editor_w: 1060.0, panel_w: 0.0 };
    let l = chrome::layout(&f, Insets { caption_buttons: 138.0, ..Default::default() });
    assert_eq!(l.toolbar[3][0], 1060.0 - 138.0 - 12.0 - 28.0);
}

#[test]
fn toolbar_buttons_have_their_labels_tooltips_and_actions() {
    let (mut rt, _dir) = harness("Some text");
    let b = info(&rt, "add-comment-button");
    assert_eq!(b.label.as_deref(), Some("Add comment"));
    assert!(b.help.as_deref().unwrap().starts_with("Comment on the selection or this line ("));
    assert_eq!(info(&rt, "comments-button").label.as_deref(), Some("Comments"));
    assert_eq!(info(&rt, "versions-button").label.as_deref(), Some("Versions"));
    assert_eq!(info(&rt, "assist-button").label.as_deref(), Some("Assist"));
    let side = info(&rt, "sidebar-button");
    assert_eq!(side.label.as_deref(), Some("Toggle sidebar"));
    assert!(side.help.as_deref().unwrap().ends_with("Point at the left edge to peek."));
    click(&mut rt, "versions-button");
    assert!(rt.state.assist_visible && rt.state.notes.assist_tab == crate::notes::AssistTab::Versions);
}

// MARK: Chrome fade

#[test]
fn typing_hides_the_chrome_and_the_pointer_brings_it_back_for_three_seconds() {
    let (mut rt, _dir) = harness("Hello");
    assert!(rt.state.chrome_visible);
    type_text(&mut rt, "x");
    assert!(!rt.state.chrome_visible, "a key in the draft hides the chrome");
    settle(&mut rt, 0.7);
    assert!(info(&rt, "title").opacity < 0.01, "the title fades out within 0.6 s");
    assert_eq!(info(&rt, "top-bar").opacity, 1.0, "the bars themselves don't fade");

    rt.pointer_move(500.0, 400.0);
    settle(&mut rt, 0.3);
    assert!(rt.state.chrome_visible);
    assert!(info(&rt, "title").opacity > 0.99, "it fades in within 0.25 s");
    settle(&mut rt, 2.6);
    assert!(rt.state.chrome_visible, "still up before 3 s");
    settle(&mut rt, 0.2);
    assert!(!rt.state.chrome_visible, "and gone after 3 s");

    // Over the top bar or the footer it stays.
    rt.pointer_move(500.0, 20.0);
    settle(&mut rt, 3.5);
    assert!(rt.state.chrome_visible);
    rt.pointer_move(500.0, 750.0);
    settle(&mut rt, 3.5);
    assert!(rt.state.chrome_visible);
}

#[test]
fn a_failed_save_keeps_the_chrome_up() {
    let (mut rt, dir) = failing("Text");
    type_text(&mut rt, " more");
    assert!(rt.state.chrome_visible, "typing doesn't hide it while a save has failed");
    assert!(rt.state.status().0.starts_with("Not saved — Couldn't save “Text"), "{}", rt.state.status().0);
    assert!(rt.state.status().1, "in the warning colour");
    assert_eq!(rt.state.status_tooltip.as_deref(), Some(rt.state.status().0.trim_start_matches("Not saved — ")));
    writable(&dir);
}

#[test]
fn losing_the_window_saves_and_keeps_the_chrome_up() {
    let (mut rt, dir) = harness("");
    type_text(&mut rt, "Unsaved words");
    rt.window_focus(false);
    settle(&mut rt, 0.05);
    assert!(dir.path().join("drafts/Unsaved words.md").exists(), "saved when the window lost focus");
    assert!(rt.state.chrome_visible);
    settle(&mut rt, 4.0);
    assert!(rt.state.chrome_visible, "no auto-hide while the window isn't key");
    rt.window_focus(true);
    settle(&mut rt, 0.1);
    rt.update(|s, _| s.pointer = (500.0, 400.0));
    settle(&mut rt, 3.1);
    assert!(!rt.state.chrome_visible, "auto-hide again once key");
}

// MARK: Title and rename

#[test]
fn the_title_shows_the_placeholder_until_there_is_text() {
    let (mut rt, _dir) = harness("");
    assert_eq!(rt.state.shown_title(), "");
    assert_eq!(info(&rt, "title").text.as_deref(), Some("Untitled"));
    assert_eq!(info(&rt, "title").help.as_deref(), Some("Click to rename"));
    assert_eq!(info(&rt, "title").label.as_deref(), Some("Draft title"));
    type_text(&mut rt, "# A heading");
    assert_eq!(rt.state.shown_title(), "A heading", "the title follows the first line");
}

#[test]
fn rename_command_renames_the_file_and_returns_to_the_text() {
    let (mut rt, dir) = harness("");
    type_text(&mut rt, "First words");
    settle(&mut rt, 1.0);
    assert!(dir.path().join("drafts/First words.md").exists());
    command(&mut rt, 'r', true, false);
    settle(&mut rt, 0.05);
    assert!(rt.state.title.editing);
    assert_eq!(rt.state.title.edit.range(), (0, "First words".len()), "Rename selects all of the title");
    settle(&mut rt, 4.0);
    assert!(rt.state.chrome_visible, "the chrome stays up while the title is edited");
    type_text(&mut rt, "Chapter One");
    press(&mut rt, Key::Enter, Mods::default());
    settle(&mut rt, 0.05);
    assert!(!rt.state.title.editing);
    assert!(dir.path().join("drafts/Chapter One.md").exists(), "renamed on disk");
    assert!(!dir.path().join("drafts/First words.md").exists());
    assert!(rt.state.draft.has_manual_title);
    assert!(rt.state.edit.focused, "the keyboard is back in the text");
    rt.update(|s, _| s.pointer = (500.0, 400.0));
    settle(&mut rt, 3.1);
    assert!(!rt.state.chrome_visible, "auto-hide restarted");
}

#[test]
fn escape_puts_the_title_back_and_clicking_away_renames() {
    let (mut rt, dir) = harness("");
    type_text(&mut rt, "Notes");
    settle(&mut rt, 1.0);
    click(&mut rt, "title");
    assert!(rt.state.title.editing, "a click starts editing");
    type_text(&mut rt, " more");
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt, 0.05);
    assert!(!rt.state.title.editing);
    assert_eq!(rt.state.draft.title, "Notes");
    assert!(!rt.state.draft.has_manual_title);

    click(&mut rt, "title");
    type_text(&mut rt, " kept");
    // The keyboard goes back to the text: the edit ends and renames.
    rt.update(|s, tx| s.focus_draft(tx));
    settle(&mut rt, 0.1);
    assert!(!rt.state.title.editing);
    assert_eq!(rt.state.draft.title, "Notes kept");
    assert!(dir.path().join("drafts/Notes kept.md").exists());
}

#[test]
fn an_empty_title_goes_back_to_the_derived_one_and_a_new_draft_is_saved_first() {
    let (mut rt, dir) = harness("");
    type_text(&mut rt, "Body line");
    // Not yet saved: the rename saves it, then names the file.
    rt.update(|s, tx| s.begin_title_edit(true, tx));
    type_text(&mut rt, "Named");
    press(&mut rt, Key::Enter, Mods::default());
    assert!(dir.path().join("drafts/Named.md").exists());
    rt.update(|s, tx| s.begin_title_edit(true, tx));
    press(&mut rt, Key::Backspace, Mods::default());
    press(&mut rt, Key::Enter, Mods::default());
    assert!(!rt.state.draft.has_manual_title);
    assert_eq!(rt.state.draft.title, "Body line");
}

#[test]
fn a_rename_that_fails_says_why() {
    let (mut rt, dir) = harness("");
    type_text(&mut rt, "Stuck");
    settle(&mut rt, 1.0);
    let drafts = dir.path().join("drafts");
    let mut perms = std::fs::metadata(&drafts).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&drafts, perms).unwrap();
    rt.update(|s, tx| s.begin_title_edit(true, tx));
    type_text(&mut rt, "Moved");
    press(&mut rt, Key::Enter, Mods::default());
    let alert = rt.state.alert().expect("an alert");
    assert!(alert.message.starts_with("Couldn't rename “Stuck.md”: "), "{}", alert.message);
    writable(&dir);
}

// MARK: Footer

#[test]
fn a_flash_gets_its_own_width_and_returns_to_the_status() {
    let (mut rt, _dir) = harness("");
    // An empty status, then a long flash: the Swift kept the old (zero) width.
    run(&mut rt, Command::ToggleCounts);
    rt.update(|s, tx| s.flash_message("Copied draft as plain text · 1,234 characters", tx));
    settle(&mut rt, 0.05);
    let status = info(&rt, "status");
    assert_eq!(status.text.as_deref(), Some("Copied draft as plain text · 1,234 characters"));
    assert!(status.rect[2] >= 45.0 * 12.0 * 0.6, "wide enough for the message: {}", status.rect[2]);
    assert_eq!(info(&rt, "announcement").text.as_deref(), Some("Copied draft as plain text · 1,234 characters"));
    settle(&mut rt, 2.3);
    assert!(rt.state.transient.is_none(), "gone after 2.2 s");
}

#[test]
fn the_focus_label_shows_without_counts_and_tooltips_follow_the_swift() {
    let (mut rt, dir) = harness("");
    rt.update(|s, tx| s.set_focus(FocusUnit::Paragraph, tx));
    run(&mut rt, Command::ToggleCounts);
    settle(&mut rt, 2.5);
    assert!(rt.find("counts").is_none());
    let focus = info(&rt, "focus");
    assert_eq!(focus.text.as_deref(), Some("Paragraph"));
    assert!((focus.rect[0] + focus.rect[2] / 2.0 - 530.0).abs() < 1.0, "centred");
    run(&mut rt, Command::ToggleCounts);
    let counts = info(&rt, "counts");
    assert_eq!(counts.label.as_deref(), Some("Counts"));

    type_text(&mut rt, "Words");
    settle(&mut rt, 1.0);
    let path = dir.path().join("drafts/Words.md");
    assert_eq!(info(&rt, "status").help.as_deref(), Some(path.to_str().unwrap()), "Saved shows the path");
    type_text(&mut rt, "!");
    assert_eq!(rt.state.status().0, "Edited");
    assert_eq!(info(&rt, "status").help.as_deref(), Some(path.to_str().unwrap()), "Edited keeps the last tooltip, as the Swift did");
}

// MARK: Leaving a draft

#[test]
fn a_failed_save_asks_before_a_new_draft() {
    let (mut rt, dir) = failing("Precious");
    command(&mut rt, 'n', false, false);
    let alert = rt.state.alert().expect("asks");
    assert_eq!(alert.message, "This draft couldn't be saved.");
    assert!(alert.informative.ends_with("\n\nA recovery copy is kept and will be offered next time."));
    assert_eq!(alert.buttons, ["Keep Editing", "Save As…", "Continue"]);
    assert!(alert.warning);
    // Keep Editing (Return, the default) stays.
    press(&mut rt, Key::Enter, Mods::default());
    settle(&mut rt, 0.05);
    assert!(rt.state.alert().is_none());
    assert_eq!(rt.state.edit.text, "Precious");
    // Continue leaves.
    command(&mut rt, 'n', false, false);
    click(&mut rt, "alert-button-2");
    assert_eq!(rt.state.edit.text, "");
    assert!(std::fs::read_dir(dir.path().join("support/Recovery")).unwrap().count() > 0, "a recovery copy is kept");
    writable(&dir);
}

#[test]
fn closing_with_a_failed_save_asks_once() {
    let (mut rt, dir) = failing("Precious");
    command(&mut rt, 'w', false, false);
    assert_eq!(rt.state.alerts.len(), 1);
    click(&mut rt, "alert-button-2");
    assert!(rt.state.documents.quitting, "Continue quits");
    // Rune's close hook then runs for the closing window: it doesn't ask again.
    rt.update(|s, tx| s.window_closing(tx));
    assert!(rt.state.alerts.is_empty(), "the Swift showed the alert a second time here");
    writable(&dir);
}

#[test]
fn quitting_saves_and_remembers_the_caret() {
    let (mut rt, dir) = harness("");
    type_text(&mut rt, "Last words");
    rt.update(|s, _| s.edit.set_selection(2, 2));
    command(&mut rt, 'q', false, false);
    assert!(rt.state.documents.quitting);
    assert!(dir.path().join("drafts/Last words.md").exists());
    let mut library = DraftLibrary::new(dir.path().join("drafts"), dir.path().join("support")).unwrap();
    let doc = library.open(&dir.path().join("drafts/Last words.md")).unwrap();
    assert_eq!(doc.selection, 2..2);
}

// MARK: Open and Save As

#[test]
fn open_other_file_offers_text_files_from_the_drafts_folder() {
    let (mut rt, dir) = harness("");
    command(&mut rt, 'o', true, false);
    let (kind, options) = rt.file_dialog().expect("an open dialog");
    assert_eq!(kind, rune::FileDialogKind::Open);
    assert_eq!(options.directory.as_deref(), Some(dir.path().join("drafts").as_path()));
    assert_eq!(options.filters[0].1, ["md", "markdown", "txt", "text"]);
    let other = dir.path().join("other.txt");
    std::fs::write(&other, "Elsewhere").unwrap();
    rt.resolve_file_dialog(Some(other.clone())).unwrap();
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.edit.text, "Elsewhere");
    assert_eq!(rt.state.draft.title, "other");
    assert_eq!(rt.state.edit.range(), (9, 9), "with no record the caret goes to the end");

    // Opening the open file again just focuses the text; a missing one says why.
    rt.update(|s, tx| s.open(&other, tx));
    assert!(rt.state.alert().is_none());
    rt.update(|s, tx| s.open(&dir.path().join("gone.md"), tx));
    assert!(rt.state.alert().unwrap().message.starts_with("Couldn't open “gone.md”: "));
}

#[test]
fn save_as_writes_a_copy_and_opens_it_in_place() {
    let (mut rt, dir) = harness("");
    type_text(&mut rt, "Original text");
    settle(&mut rt, 1.0);
    rt.update(|s, _| s.edit.set_selection(3, 5));
    command(&mut rt, 's', true, false);
    let (kind, options) = rt.file_dialog().expect("a save dialog");
    assert_eq!(kind, rune::FileDialogKind::Save);
    assert_eq!(options.file_name.as_deref(), Some("Original text.md"));
    assert_eq!(options.filters[0].1, ["md"]);
    let copy = dir.path().join("Copy");
    rt.resolve_file_dialog(Some(copy)).unwrap();
    settle(&mut rt, 0.1);
    let copy = dir.path().join("Copy.md");
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), "Original text");
    assert_eq!(rt.state.draft.path(), Some(copy.as_path()));
    assert_eq!(rt.state.edit.range(), (3, 5), "the selection carries over");
    assert!(dir.path().join("drafts/Original text.md").exists(), "the old file stays");
}

#[test]
fn save_as_from_the_alert_leaves_only_once_the_copy_is_written() {
    let (mut rt, dir) = failing("Rescue me");
    command(&mut rt, 'n', false, false);
    click(&mut rt, "alert-button-1");
    assert!(rt.file_dialog().is_some());
    rt.resolve_file_dialog(Some(dir.path().join("rescued.md"))).unwrap();
    settle(&mut rt, 0.1);
    assert_eq!(std::fs::read_to_string(dir.path().join("rescued.md")).unwrap(), "Rescue me");
    assert_eq!(rt.state.edit.text, "", "then the new draft opens");
    writable(&dir);
}

#[test]
fn a_recovered_draft_says_so_and_saves_after_a_moment() {
    let dir = tempfile::tempdir().unwrap();
    let library = DraftLibrary::new(dir.path().join("drafts"), dir.path().join("support")).unwrap();
    let mut doc = library.new_draft();
    doc.body = "Recovered words".into();
    doc.refresh_derived_title();
    library.write_recovery(&doc).unwrap();
    let orphan = library.orphaned_recoveries().into_iter().next().expect("an orphan");
    let doc = library.restore(&orphan);
    let app = App::new(library, Prefs::in_memory(), doc);
    let mut rt = Runtime::new(app, crate::view::view, (1060.0, 760.0), rune::Env { system_dark: false, scale: 1.0 });
    rt.poll = Some(|s, clock, tx| s.poll(clock, tx));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.status().0, "Recovered unsaved changes");
    settle(&mut rt, 0.3);
    assert_eq!(rt.state.status().0, "Saved");
    assert!(dir.path().join("drafts/Recovered words.md").exists());
}

// MARK: Menus

#[test]
fn menu_titles_follow_state() {
    let (mut rt, _dir) = harness("");
    rt.update(|s, _| s.sidebar_visible = true);
    assert_eq!(rt.state.command_title(Command::ToggleSidebar, "Show Sidebar"), "Hide Sidebar");
    rt.update(|s, tx| s.toggle_full_screen(tx));
    assert_eq!(rt.state.command_title(Command::FullScreen, "Enter Full Screen"), "Exit Full Screen");
}

#[test]
fn the_in_window_menu_bar_opens_moves_and_runs_commands() {
    if !crate::menubar::SHOWN {
        return;
    }
    let (mut rt, _dir) = harness("Some text");
    rt.set_modifier_keys(true);
    // Alt alone arms the bar; arrows move; Down opens; Return runs.
    let alt = |pressed| KeyEvent { key: Key::Modifier { modifier: Modifier::AltLeft, pressed }, mods: Mods { alt: pressed, ..Mods::default() } };
    rt.key(alt(true));
    rt.key(alt(false));
    settle(&mut rt, 0.05);
    assert!(rt.state.menus.armed);
    assert!(rt.state.chrome_visible);
    press(&mut rt, Key::Right, Mods::default());
    press(&mut rt, Key::Right, Mods::default());
    press(&mut rt, Key::Right, Mods::default());
    press(&mut rt, Key::Down, Mods::default());
    assert_eq!(rt.state.menus.open, Some(Source::Menu(3)), "View");
    assert_eq!(rt.state.menus.row, Some(0));
    // Down to "Turn Sentence Focus On" (past Recent Files and the separator), Return.
    press(&mut rt, Key::Down, Mods::default());
    press(&mut rt, Key::Down, Mods::default());
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.menus.open, None);
    assert_eq!(rt.state.focus, FocusUnit::Sentence);
    assert_eq!(rt.state.edit.text, "Some text", "no key reached the draft");

    // Alt+O opens Format; Escape closes it.
    press(&mut rt, Key::Text("o".into()), Mods { alt: true, ..Mods::default() });
    assert_eq!(rt.state.menus.open, Some(Source::Menu(2)));
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.menus.open, None);

    // The pointer: click View, point at Focus, pick Typewriter from its submenu.
    click(&mut rt, "menu-title-3");
    assert_eq!(rt.state.menus.open, Some(Source::Menu(3)));
    let (x, y) = rt.find("menu-row-4").unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt, 0.05);
    assert!(rt.state.menus.sub.is_some(), "the Focus submenu opens on hover");
    click(&mut rt, "submenu-row-3");
    assert_eq!(rt.state.focus, FocusUnit::Typewriter);

    // With one open, pointing at another title switches to it.
    click(&mut rt, "menu-title-0");
    let (x, y) = rt.find("menu-title-1").unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt, 0.05);
    assert_eq!(rt.state.menus.open, Some(Source::Menu(1)));
    click(&mut rt, "menu-scrim");
    assert_eq!(rt.state.menus.open, None, "a click elsewhere closes it");
}

#[test]
fn menu_rows_carry_titles_shortcuts_and_states() {
    let (rt, _dir) = harness("");
    let rows = rt.state.source_rows(Source::Menu(0));
    let titles: Vec<&str> = rows.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(&titles[..5], ["New", "Open…", "Open Other File…", "Open Folder…", "Open over SSH…"]);
    let copy = rows.iter().find(|r| r.title == "Copy for Twitter").unwrap();
    assert!(!copy.enabled, "disabled for an empty draft");
    if crate::menubar::SHOWN {
        assert_eq!(rows.last().unwrap().title, format!("Quit {}", crate::app_name()));
        assert_eq!(rows[0].shortcut, "Ctrl+N");
    }
    let view = rt.state.source_rows(Source::Menu(3));
    assert!(view.iter().find(|r| r.title == "Show Counts").unwrap().checked);
    let ctx = rt.state.context_rows();
    assert_eq!(ctx[0].title, "Comment on This Line…");
}

#[test]
fn the_context_menu_falls_back_to_the_app_where_rune_has_no_native_one() {
    let (mut rt, _dir) = harness("Some text");
    rt.update(|s, _| s.edit.set_selection(0, 4));
    rt.update(|s, tx| s.open_context_menu(rune::PointerPress { x: 300.0, y: 200.0 }, tx));
    settle(&mut rt, 0.05);
    if rune::popup_menu_supported() {
        let menu = rt.popup_menu().expect("a native menu");
        assert_eq!(menu.find("comment").unwrap().title(), "Comment on Selection…");
    } else {
        assert_eq!(rt.state.menus.open, Some(Source::Context));
        click(&mut rt, "menu-row-3");
        assert_eq!(rt.state.edit.text, "Some text", "Copy leaves the text");
        assert_eq!(rt.state.menus.open, None);
    }
}

// MARK: Window

#[test]
fn the_window_frame_is_saved_when_the_window_loses_focus() {
    let dir = tempfile::tempdir().unwrap();
    let library = DraftLibrary::new(dir.path().join("drafts"), dir.path().join("support")).unwrap();
    let prefs = Prefs::load(library.support_directory());
    let doc = library.new_draft();
    let mut app = App::new(library, prefs, doc);
    app.screen_frame_changed([100.0, 80.0, 900.0, 700.0], 2.0);
    let mut rt = Runtime::new(app, crate::view::view, (900.0, 700.0), rune::Env { system_dark: false, scale: 1.0 });
    rt.window_focus(false);
    settle(&mut rt, 0.05);
    let prefs = Prefs::load(&dir.path().join("support"));
    assert_eq!(prefs.window_frame(), Some([100.0, 80.0, 900.0, 700.0, 2.0]));
    let opening = crate::window::opening(&prefs);
    assert_eq!(opening.size, (900.0, 700.0));
    assert_eq!(opening.options.position, Some((200, 160)));
}

#[test]
fn the_window_title_follows_the_draft() {
    let (mut rt, _dir) = harness("");
    type_text(&mut rt, "Title line");
    settle(&mut rt, 0.05);
    assert_eq!(rt.state.win.title.as_deref(), Some("Title line"));
}
