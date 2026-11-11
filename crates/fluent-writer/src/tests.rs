//! Headless tests that drive the whole app through Rune's `Runtime`: real view, real layout,
//! real key routing, a virtual clock, no window.

use rune::{Env, Key, KeyEvent, Mods, Runtime};
use writer_core::{DraftLibrary, FocusUnit};

use crate::app::App;
use crate::prefs::Prefs;

/// The app on a fresh library in a temporary folder, with `body` as an unsaved draft.
pub fn harness(body: &str) -> (Runtime<App>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let library = DraftLibrary::new(dir.path().join("drafts"), dir.path().join("support")).unwrap();
    let mut doc = library.new_draft();
    doc.body = body.into();
    doc.selection = body.len()..body.len();
    let app = App::new(library, Prefs::in_memory(), doc);
    let mut rt = Runtime::new(app, crate::view::view, (1060.0, 760.0), Env { system_dark: false, scale: 1.0 });
    rt.poll = Some(|s, clock, tx| s.poll(clock, tx));
    rt.on_key = Some(crate::keys::window_key);
    settle(&mut rt, 0.2);
    (rt, dir)
}

/// Runs frames for `seconds` of virtual time.
pub fn settle(rt: &mut Runtime<App>, seconds: f64) {
    for _ in 0..(seconds * 60.0).ceil() as usize {
        rt.frame(1.0 / 60.0);
    }
}

pub fn type_text(rt: &mut Runtime<App>, text: &str) {
    for c in text.chars() {
        rt.key(KeyEvent { key: Key::Text(c.to_string()), mods: Mods::default() });
        rt.frame(1.0 / 60.0);
    }
}

pub fn press(rt: &mut Runtime<App>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(1.0 / 60.0);
}

/// ⌘ on macOS, Ctrl elsewhere, with `c`.
pub fn command(rt: &mut Runtime<App>, c: char, shift: bool, alt: bool) {
    let mods = Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), logo: cfg!(target_os = "macos"), shift, alt };
    press(rt, Key::Command(c), mods);
}

#[test]
fn every_menu_shortcut_parses() {
    for (_, entries) in crate::commands::MENUS {
        fn walk(entries: &[crate::commands::Entry]) {
            for e in entries {
                match e {
                    crate::commands::Entry::Item(_, _, keys) => {
                        keys.chord();
                    }
                    crate::commands::Entry::Submenu(_, inner) => walk(inner),
                    crate::commands::Entry::Separator => {}
                }
            }
        }
        walk(entries);
    }
}

#[test]
fn typing_edits_the_draft_and_autosaves_it() {
    let (mut rt, dir) = harness("");
    type_text(&mut rt, "Hello there");
    assert_eq!(rt.state.edit.text, "Hello there");
    assert_eq!(rt.state.status().0, "Edited");
    settle(&mut rt, 1.0);
    assert_eq!(rt.state.status().0, "Saved");
    let saved = std::fs::read_to_string(dir.path().join("drafts/Hello there.md")).unwrap();
    assert_eq!(saved, "Hello there");
}

#[test]
fn return_continues_a_list_and_bold_wraps_the_selection() {
    let (mut rt, _dir) = harness("- one");
    press(&mut rt, Key::Enter, Mods::default());
    type_text(&mut rt, "two");
    assert_eq!(rt.state.edit.text, "- one\n- two");
    rt.update(|s, _| s.edit.set_selection(8, 11));
    command(&mut rt, 'b', false, false);
    assert_eq!(rt.state.edit.text, "- one\n- **two**");
}

#[test]
fn typewriter_focus_keeps_the_caret_line_in_the_middle() {
    let body = "A paragraph of prose.\n\n".repeat(60);
    let (mut rt, _dir) = harness(&body);
    rt.update(|s, tx| s.set_focus(FocusUnit::Typewriter, tx));
    settle(&mut rt, 1.0);
    type_text(&mut rt, "x");
    settle(&mut rt, 1.0);
    let caret = rt.state.last_caret.expect("the draft has the caret");
    let middle = crate::app::TOP_BAR_HEIGHT + (760.0 - crate::app::TOP_BAR_HEIGHT) / 2.0;
    let caret_middle = caret[1] + caret[3] / 2.0;
    assert!((caret_middle - middle).abs() <= rt.state.metrics.line_height / 2.0, "caret at {caret_middle}, middle {middle}");
}

#[test]
fn an_image_line_gets_room_under_it_and_dropped_files_are_copied_beside_the_draft() {
    let (mut rt, dir) = harness("Before.\n");
    // A draft with no file yet keeps its images in the drafts folder.
    let source = dir.path().join("elsewhere/My Shot.png");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    let png = image::RgbaImage::from_pixel(40, 20, image::Rgba([200, 30, 30, 255]));
    png.save(&source).unwrap();
    rt.update(|s, tx| {
        assert!(s.insert_images(&[source.clone()], None, None, tx));
    });
    settle(&mut rt, 0.2);
    assert_eq!(rt.state.edit.text, "Before.\n![My Shot](images/My%20Shot.png)");
    assert!(dir.path().join("drafts/images/My Shot.png").exists());
    assert_eq!(rt.state.status().0, "Image added · copied to the images folder beside this draft");
    let block = &rt.state.images.blocks[0];
    assert!(matches!(block.content, crate::images::Content::Placeholder { .. }), "no image cache without a window's waker");
    assert_eq!(block.size, (420.0, 40.0));
    let gap = rt.state.image_gap();
    assert_eq!(rt.state.images.space_after(block.line, gap), 40.0 + gap * 2.0);
    // The preview sits under its line, at the prose edge.
    let ([x, y, w, h], _) = rt.presented("image-0").expect("the preview is drawn");
    let c = rt.state.column();
    assert!((x - (c.inset_x + rt.state.metrics.hang_width())).abs() < 0.1);
    assert_eq!((w, h), (420.0, 40.0));
    let line_bottom = crate::app::TOP_BAR_HEIGHT + c.inset_y + rt.state.metrics.line_height * 2.0;
    assert!((y - (line_bottom + gap)).abs() <= 1.0, "preview at {y}, line ends at {line_bottom}");
}

// MARK: Find and replace

fn sel(rt: &Runtime<App>) -> (usize, usize) {
    rt.state.edit.range()
}

fn shift() -> Mods {
    Mods { shift: true, ..Mods::default() }
}

fn click(rt: &mut Runtime<App>, key: &str) {
    settle(rt, 0.05);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("{key} is on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt, 0.1);
}

const CATS: &str = "One cat, two Cats, a catalogue and a cat.";

#[test]
fn find_shows_the_bar_searches_as_you_type_and_escape_returns_to_the_match() {
    let (mut rt, _dir) = harness(CATS);
    command(&mut rt, 'f', false, false);
    settle(&mut rt, 0.1);
    assert!(rt.state.find.visible);
    assert!(!rt.state.edit.focused, "the search field has the keyboard");
    type_text(&mut rt, "cat");
    assert_eq!(rt.state.find.query.text, "cat");
    assert_eq!(rt.state.edit.text, CATS, "typing went to the search, not the draft");
    // The caret was at the end: the search wraps to the first match.
    assert_eq!(sel(&rt), (4, 7));
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(sel(&rt), (13, 16), "Return goes to the next match, ignoring case");
    press(&mut rt, Key::Enter, shift());
    assert_eq!(sel(&rt), (4, 7), "Shift-Return goes back");
    settle(&mut rt, 0.2);
    assert!(rt.find(crate::find::CURRENT).is_some(), "the current match is highlighted");
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt, 0.1);
    assert!(!rt.state.find.visible);
    assert!(rt.state.edit.focused, "Escape gives the keyboard back to the draft");
    assert_eq!(sel(&rt), (4, 7), "with the match selected");
}

#[test]
fn find_next_and_previous_wrap_and_respect_the_options() {
    let (mut rt, _dir) = harness(CATS);
    rt.update(|s, _| {
        s.find.query.replace_text("cat");
        s.edit.set_caret(0);
    });
    command(&mut rt, 'g', false, false);
    assert_eq!(sel(&rt), (4, 7), "⌘G works with the bar hidden");
    command(&mut rt, 'g', false, false);
    command(&mut rt, 'g', false, false);
    command(&mut rt, 'g', false, false);
    assert_eq!(sel(&rt), (37, 40));
    command(&mut rt, 'g', false, false);
    assert_eq!(sel(&rt), (4, 7), "wraps round");
    command(&mut rt, 'g', true, false);
    assert_eq!(sel(&rt), (37, 40), "⇧⌘G goes back, wrapping");
    rt.update(|s, _| {
        s.find.options.wrap = false;
        s.find.options.ignore_case = false;
        s.find.options.mode = crate::find::Mode::FullWord;
    });
    command(&mut rt, 'g', false, false);
    assert_eq!(sel(&rt), (37, 40), "no wrap: stays at the last match");
    assert_eq!(rt.state.find.notice.as_deref(), Some("Not found"));
    command(&mut rt, 'g', true, false);
    assert_eq!(sel(&rt), (4, 7), "full word skips catalogue, case skips Cats");
}

#[test]
fn use_selection_for_find_then_find_next() {
    let (mut rt, _dir) = harness("two by two");
    rt.update(|s, _| s.edit.set_selection(0, 3));
    command(&mut rt, 'e', false, false);
    assert_eq!(rt.state.find.query.text, "two");
    assert!(!rt.state.find.visible, "⌘E leaves the bar as it was");
    command(&mut rt, 'g', false, false);
    assert_eq!(sel(&rt), (7, 10));
}

#[test]
fn replace_replaces_the_match_and_moves_on_as_one_undo_step() {
    let (mut rt, _dir) = harness(CATS);
    rt.update(|s, _| s.edit.set_caret(0));
    rt.update(|s, tx| s.run(crate::commands::Command::FindAndReplace, tx));
    settle(&mut rt, 0.1);
    assert!(rt.state.find.replacing);
    type_text(&mut rt, "cat");
    assert_eq!(sel(&rt), (4, 7));
    rt.update(|_, tx| tx.focus(crate::find::REPLACE_FIELD, false));
    settle(&mut rt, 0.1);
    type_text(&mut rt, "dog");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.edit.text, "One dog, two Cats, a catalogue and a cat.");
    assert_eq!(sel(&rt), (13, 16), "on to the next match");
    click(&mut rt, "replace-one");
    assert_eq!(rt.state.edit.text, "One dog, two dogs, a catalogue and a cat.");
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt, 0.1);
    assert!(!rt.state.find.visible && rt.state.edit.focused, "Escape closed the bar from the replace field");
    command(&mut rt, 'z', false, false);
    assert_eq!(rt.state.edit.text, "One dog, two Cats, a catalogue and a cat.", "one replacement per undo");
    assert_eq!(sel(&rt), (13, 16), "undo selects the match again");
    command(&mut rt, 'z', false, false);
    assert_eq!(rt.state.edit.text, CATS);
    command(&mut rt, 'z', true, false);
    assert_eq!(rt.state.edit.text, "One dog, two Cats, a catalogue and a cat.");
}

#[test]
fn replace_all_is_one_undo_step_and_reports_the_count() {
    let (mut rt, _dir) = harness(CATS);
    rt.update(|s, _| {
        s.find.query.replace_text("cat");
        s.find.replacement.replace_text("ox");
    });
    rt.update(|s, tx| s.run(crate::commands::Command::FindAndReplace, tx));
    settle(&mut rt, 0.1);
    click(&mut rt, "replace-all");
    assert_eq!(rt.state.edit.text, "One ox, two oxs, a oxalogue and a ox.");
    assert_eq!(rt.state.find.notice.as_deref(), Some("4 replaced"));
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt, 0.1);
    command(&mut rt, 'z', false, false);
    assert_eq!(rt.state.edit.text, CATS);
    command(&mut rt, 'z', true, false);
    assert_eq!(rt.state.edit.text, "One ox, two oxs, a oxalogue and a ox.");
}

#[test]
fn the_find_bar_pushes_the_draft_down_and_its_buttons_work() {
    let (mut rt, _dir) = harness(CATS);
    let before = rt.state.field_rect[1];
    command(&mut rt, 'f', false, false);
    settle(&mut rt, 0.2);
    assert_eq!(rt.state.field_rect[1] - before, crate::find::ROW_HEIGHT);
    rt.update(|s, _| s.find.query.replace_text("cat"));
    click(&mut rt, "find-next");
    assert_eq!(sel(&rt), (4, 7));
    click(&mut rt, "find-previous");
    assert_eq!(sel(&rt), (37, 40));
    click(&mut rt, "find-options");
    click(&mut rt, "find-full-word");
    assert_eq!(rt.state.find.options.mode, crate::find::Mode::FullWord);
    assert_eq!(rt.state.find_matches().len(), 2, "cat and cat. but not Cats or catalogue");
    rt.update(|s, _| s.find.query.set_caret(0));
    type_text(&mut rt, "s");
    assert_eq!(rt.state.find.query.text, "scat", "clicking the bar's buttons leaves the keyboard in its search field");
    click(&mut rt, "find-done");
    assert!(!rt.state.find.visible);
    assert_eq!(rt.state.field_rect[1], before);
}

#[test]
fn a_far_match_is_scrolled_into_view() {
    let body = format!("{}needle", "Filler paragraph of prose.\n\n".repeat(80));
    let (mut rt, _dir) = harness(&body);
    rt.update(|s, tx| {
        s.edit.set_caret(0);
        tx.scroll_to_y(crate::app::SCROLL, 0.0);
    });
    settle(&mut rt, 0.3);
    command(&mut rt, 'f', false, false);
    type_text(&mut rt, "needle");
    settle(&mut rt, 1.0);
    let (_, y) = rt.find(crate::find::CURRENT).expect("the match is highlighted");
    assert!(y > crate::app::TOP_BAR_HEIGHT && y < 760.0, "match at {y}");
}

#[test]
fn jump_to_selection_centres_it() {
    let body = "A paragraph of prose.\n\n".repeat(80);
    let (mut rt, _dir) = harness(&body);
    let at = body.len() - 30;
    rt.update(|s, tx| {
        s.edit.set_selection(at, at + 5);
        tx.scroll_to_y(crate::app::SCROLL, 0.0);
    });
    settle(&mut rt, 0.3);
    rt.update(|s, tx| s.run(crate::commands::Command::JumpToSelection, tx));
    settle(&mut rt, 1.0);
    let (_, y) = rt.find(crate::find::JUMP).expect("the selection is laid out");
    let middle = crate::app::TOP_BAR_HEIGHT + (760.0 - crate::app::TOP_BAR_HEIGHT) / 2.0;
    assert!((y - middle).abs() < 20.0, "selection at {y}, middle {middle}");
}

// MARK: Undo

#[test]
fn format_commands_undo_in_one_step_and_restore_the_selection() {
    let (mut rt, _dir) = harness("hello world");
    rt.update(|s, _| s.edit.set_selection(6, 11));
    command(&mut rt, 'b', false, false);
    assert_eq!(rt.state.edit.text, "hello **world**");
    let after = sel(&rt);
    command(&mut rt, 'z', false, false);
    assert_eq!(rt.state.edit.text, "hello world");
    assert_eq!(sel(&rt), (6, 11));
    command(&mut rt, 'z', true, false);
    assert_eq!(rt.state.edit.text, "hello **world**");
    assert_eq!(sel(&rt), after, "redo puts back the selection the command left");
}

#[test]
fn list_continuation_and_indent_are_one_step_each() {
    let (mut rt, _dir) = harness("- one");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.edit.text, "- one\n- ");
    press(&mut rt, Key::Tab, Mods::default());
    let indented = rt.state.edit.text.clone();
    assert_ne!(indented, "- one\n- ");
    command(&mut rt, 'z', false, false);
    assert_eq!(rt.state.edit.text, "- one\n- ");
    command(&mut rt, 'z', false, false);
    assert_eq!(rt.state.edit.text, "- one");
    assert_eq!(sel(&rt), (5, 5));
}

#[test]
fn typing_coalesces_until_the_caret_moves() {
    let (mut rt, _dir) = harness("");
    type_text(&mut rt, "hello world");
    press(&mut rt, Key::Left, Mods::default());
    type_text(&mut rt, "X");
    command(&mut rt, 'z', false, false);
    assert_eq!(rt.state.edit.text, "hello world");
    command(&mut rt, 'z', false, false);
    assert_eq!(rt.state.edit.text, "", "the whole typing run is one step");
}

// MARK: Keys and pointer

#[test]
fn escape_keeps_the_keyboard_in_the_draft() {
    let (mut rt, _dir) = harness("text");
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt, 0.1);
    assert!(rt.state.edit.focused);
    type_text(&mut rt, "!");
    assert_eq!(rt.state.edit.text, "text!");
}

#[test]
fn paragraph_keys_move_by_paragraph() {
    let (mut rt, _dir) = harness("one two\nthree\n\nfour");
    rt.update(|s, _| s.edit.set_caret(10));
    let by_paragraph = if cfg!(target_os = "macos") { Mods { alt: true, ..Mods::default() } } else { Mods { ctrl: true, cmd: true, ..Mods::default() } };
    press(&mut rt, Key::Up, by_paragraph);
    assert_eq!(sel(&rt), (8, 8));
    press(&mut rt, Key::Up, by_paragraph);
    assert_eq!(sel(&rt), (0, 0));
    press(&mut rt, Key::Down, Mods { shift: true, ..by_paragraph });
    assert_eq!(sel(&rt), (0, 7), "Shift extends");
}

#[test]
fn word_and_line_keys_follow_the_platform() {
    let (mut rt, _dir) = harness("alpha beta gamma");
    let word = if cfg!(target_os = "macos") { Mods { alt: true, ..Mods::default() } } else { Mods { ctrl: true, cmd: true, ..Mods::default() } };
    press(&mut rt, Key::Left, word);
    assert_eq!(sel(&rt), (11, 11));
    press(&mut rt, Key::Left, Mods { shift: true, ..word });
    assert_eq!(sel(&rt), (6, 11));
    press(&mut rt, Key::Right, Mods::default());
    press(&mut rt, Key::Backspace, word);
    assert_eq!(rt.state.edit.text, "alpha gamma");
    press(&mut rt, Key::Home, Mods { shift: true, ..Mods::default() });
    assert_eq!(sel(&rt), (0, 6));
    press(&mut rt, Key::Right, Mods::default());
    press(&mut rt, Key::Delete, word);
    assert_eq!(rt.state.edit.text, "alpha ");
}

#[test]
fn double_and_triple_click_select_a_word_and_a_paragraph_and_drag_selects() {
    let (mut rt, _dir) = harness("first second third\nnext line");
    rt.update(|s, _| s.edit.set_caret(0));
    settle(&mut rt, 0.2);
    let caret = rt.state.last_caret.expect("caret drawn");
    rt.update(|s, _| s.edit.set_caret(10));
    settle(&mut rt, 0.2);
    let cw = (rt.state.last_caret.unwrap()[0] - caret[0]) / 10.0;
    let (x, y) = (caret[0] + cw * 8.5, caret[1] + caret[3] / 2.0);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert_eq!(sel(&rt), (6, 12), "double-click takes the word");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert_eq!(sel(&rt).0, 0, "triple-click takes the paragraph");
    assert!(sel(&rt).1 >= 18);
    settle(&mut rt, 1.0);
    rt.pointer_down(caret[0] + 1.0, y);
    for i in 1..=10 {
        rt.pointer_move(caret[0] + 1.0 + cw * i as f32, y);
        rt.frame(1.0 / 60.0);
    }
    rt.pointer_up(caret[0] + cw * 10.0, y);
    rt.frame(1.0 / 60.0);
    assert_eq!(sel(&rt), (0, 10), "a drag selects");
}

#[test]
fn menu_edit_items_act_on_the_focused_field() {
    let (mut rt, _dir) = harness("draft text");
    command(&mut rt, 'f', false, false);
    type_text(&mut rt, "query");
    rt.update(|s, _| s.find.query.set_caret(5));
    rt.update(|s, tx| s.run_menu(crate::commands::Command::SelectAll, tx));
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.find.query.range(), (0, 5), "Select All went to the search field");
    rt.update(|s, tx| s.run_menu(crate::commands::Command::Undo, tx));
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.find.query.text, "", "Undo went to the search field");
    assert_eq!(rt.state.edit.text, "draft text");
    rt.update(|_, tx| tx.focus(crate::app::DRAFT, false));
    settle(&mut rt, 0.1);
    rt.update(|s, tx| s.run_menu(crate::commands::Command::SelectAll, tx));
    assert_eq!(sel(&rt), (0, 10));
    rt.update(|s, tx| s.run_menu(crate::commands::Command::Delete, tx));
    assert_eq!(rt.state.edit.text, "");
}

#[test]
fn cut_copy_and_paste_go_through_the_clipboard() {
    let (mut rt, _dir) = harness("alpha beta");
    let clip = rune::clipboard::memory();
    rt.use_clipboard(&clip);
    rt.update(|s, _| s.edit.set_selection(0, 5));
    command(&mut rt, 'c', false, false);
    settle(&mut rt, 0.1);
    assert_eq!(clip.text().as_deref().map(String::as_str), Some("alpha"));
    rt.update(|s, _| s.edit.set_selection(6, 10));
    command(&mut rt, 'x', false, false);
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.edit.text, "alpha ");
    assert_eq!(clip.text().as_deref().map(String::as_str), Some("beta"));
    rt.paste("gamma");
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.edit.text, "alpha gamma");
    command(&mut rt, 'z', false, false);
    assert_eq!(rt.state.edit.text, "alpha ", "a paste is its own undo step");
}

#[test]
fn loading_a_draft_clears_the_undo_history() {
    let (mut rt, _dir) = harness("");
    type_text(&mut rt, "first draft");
    rt.update(|s, tx| s.run(crate::commands::Command::NewDraft, tx));
    settle(&mut rt, 0.1);
    assert_eq!(rt.state.edit.text, "");
    command(&mut rt, 'z', false, false);
    assert_eq!(rt.state.edit.text, "", "nothing from the last draft to undo");
}

#[test]
fn check_spelling_while_typing_is_on_by_default_and_toggles() {
    let (mut rt, _dir) = harness("");
    assert!(rt.state.command_checked(crate::commands::Command::ToggleSpellCheck));
    rt.update(|s, tx| s.run(crate::commands::Command::ToggleSpellCheck, tx));
    assert!(!rt.state.prefs.spell_check());
    // The draft's own menu carries the same setting.
    rt.update(|s, tx| s.run(crate::commands::Command::ToggleSpellCheck, tx));
    settle(&mut rt, 0.1);
    let (x, y) = rt.find(crate::app::DRAFT).expect("draft on screen");
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    settle(&mut rt, 0.1);
    let item = rt.popup_menu().and_then(|m| m.find("spellcheck")).expect("the draft's menu offers spell checking");
    assert!(item.is_checked());
    rt.resolve_popup(Some("spellcheck")).unwrap();
    settle(&mut rt, 0.1);
    assert!(!rt.state.prefs.spell_check(), "picking it turns checking off");
}

// MARK: Substitutions

#[test]
fn substitutions_follow_their_settings() {
    let (mut rt, _dir) = harness("");
    type_text(&mut rt, "\"it's\" a--b");
    assert_eq!(rt.state.edit.text, "\"it's\" a--b", "quotes and dashes are off by default");
    assert!(rt.state.prefs.text_replacement(), "text replacement is on by default");
    rt.update(|s, tx| {
        s.edit.replace_text("");
        s.run(crate::commands::Command::SmartQuotes, tx);
        s.run(crate::commands::Command::SmartDashes, tx);
    });
    assert!(rt.state.command_checked(crate::commands::Command::SmartQuotes));
    type_text(&mut rt, "\"it's\" a--b ('x')");
    assert_eq!(rt.state.edit.text, "“it’s” a—b (‘x’)");
    command(&mut rt, 'z', false, false);
    command(&mut rt, 'z', false, false);
    assert!(rt.state.edit.text.ends_with("a--"), "undo takes back the dash: {:?}", rt.state.edit.text);
}
