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
