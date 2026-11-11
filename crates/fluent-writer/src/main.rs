//! Fluent Writer: a quiet writing app built around focus.

mod app;
mod assist;
mod assist_lists;
mod assist_view;
mod chrome;
mod commands;
mod comment_composer;
mod connect_agent;
mod images;
mod editing;
mod find;
mod notes;
mod documents;
mod footer;
mod keys;
mod menubar;
mod open_picker;
mod platform;
mod prefs;
mod remote_editing;
mod sidebar;
mod styler;
mod substitutions;
mod theme;
mod version_preview;
mod title;
mod ui;
mod view;
mod window;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod chrome_tests;

use std::path::PathBuf;

use rune::{AppSpec, Chrome, Cx, El, Tx, col, text};
use writer_core::{AppVariant, DraftLibrary};

use app::{App, TOP_BAR_HEIGHT};
use prefs::Prefs;
use theme::{Palette, ui_font};

/// The value after `flag` among the app's own arguments.
fn arg(flag: &str) -> Option<&'static str> {
    let args = rune::extra_args();
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// A file named on its own on the command line: what Windows' and Linux's "Open With" pass.
fn bare_path() -> Option<PathBuf> {
    let args = rune::extra_args();
    let mut skip = false;
    for a in args {
        if std::mem::take(&mut skip) {
            continue;
        }
        if a.starts_with("--") {
            skip = !a.contains('=');
            continue;
        }
        let p = PathBuf::from(a);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// A build run straight from Cargo's target directory uses the dev folders, so trying a change
/// never touches the drafts you write in.
pub(crate) fn variant() -> AppVariant {
    let from_cargo = std::env::current_exe().ok().is_some_and(|p| p.components().any(|c| c.as_os_str() == "target"));
    match std::env::var("FLUENT_WRITER_VARIANT").as_deref() {
        Ok("standard") => AppVariant::Standard,
        Ok("dev") => AppVariant::Dev,
        _ if from_cargo => AppVariant::Dev,
        _ => AppVariant::Standard,
    }
}

/// "Fluent Writer", or "Fluent Writer Dev" for the dev variant.
pub(crate) fn app_name() -> &'static str {
    variant().app_name()
}

/// The library, settings and first draft, before the window opens.
fn start() -> Result<(DraftLibrary, Prefs, writer_core::DraftDocument), String> {
    // `--drafts DIR --support DIR` point the app at other folders, for tests and screenshots.
    let library = match (arg("--drafts"), arg("--support")) {
        (Some(drafts), Some(support)) => DraftLibrary::new(PathBuf::from(drafts), PathBuf::from(support)),
        _ => DraftLibrary::default_library(variant()),
    };
    let mut library = library.map_err(|e| e.to_string())?;
    let prefs = Prefs::load(library.support_directory());
    // `--open <path>` if it opens; else the newest recovered draft that has no file; else a new one.
    let wanted = arg("--open").map(PathBuf::from).or_else(bare_path);
    let opened = wanted.and_then(|p| library.open(&p).ok());
    let document = match opened {
        Some(doc) => doc,
        None => match library.orphaned_recoveries().first() {
            Some(orphan) => library.restore(orphan),
            None => library.new_draft(),
        },
    };
    Ok((library, prefs, document))
}

fn main() {
    let (library, prefs, document) = match start() {
        Ok(started) => started,
        Err(message) => return fatal(message),
    };
    substitutions::load_system_replacements();
    let app_name = variant().app_name().to_string();
    let opening = window::opening(&prefs);
    let mut app = rune::App::new(AppSpec::<App> {
        fonts: theme::FONTS,
        families: (theme::FONT_FAMILY, theme::FONT_FAMILY),
        min_size: Some((420.0, 320.0)),
        poll: Some(|s, clock, tx| s.poll(clock, tx)),
        on_key: Some(keys::window_key),
        setup: Some(|s, waker| s.set_waker(waker)),
        ..AppSpec::new("Fluent Writer", opening.size, || unreachable!("the app starts from `init`"), view::view)
    })
    .init(move || {
        let mut app = App::new(library, prefs, document);
        app.notes.app_name = app_name;
        app.opened_at(opening.frame);
        app
    })
    // The title bar is transparent with the content under it; only the top bar drags the window.
    .chrome(Chrome { full_size_content: true, traffic_lights: None, drag_regions: true })
    .window_options(opening.options)
    // Closing hides the window first, so the leave check can bring it back to ask.
    .keep_running()
    .modifier_keys()
    .on_close(|s, tx| s.window_closing(tx))
    .on_quit(|s, tx| s.app_quitting(tx));
    // Windows: the app draws its own title bar, the system's buttons over its right end. Linux
    // keeps the window manager's frame, which Rune can't replace there.
    if cfg!(target_os = "windows") {
        app = app.titlebar(rune::Titlebar::new(TOP_BAR_HEIGHT));
    }
    let _ = window::HANDLE.set(app.handle());
    app.run();
}

/// The folders couldn't be made: say why, then quit.
fn fatal(message: String) {
    struct Fatal(String);
    fn view(s: &Fatal, cx: &mut Cx<Fatal>) -> El {
        let p = Palette::new(cx.system_dark());
        let body = col()
            .gap(12.0)
            .child(text(s.0.as_str()).font(ui_font(13.0, true)).color(p.text).wrap())
            .child(rune::row().justify_end().child(ui::button("ok", "OK", true, &p, |_: &mut Fatal, tx: &mut Tx| tx.quit())));
        col().w_full().h_full().fill(p.canvas).child(ui::sheet("fatal", &p, (480.0, 200.0), 420.0, body, |_: &mut Fatal, tx: &mut Tx| tx.quit()))
    }
    eprintln!("{} couldn't open its folders: {message}", app_name());
    rune::App::new(AppSpec::<Fatal>::new("Fluent Writer", (480.0, 200.0), || unreachable!(), view)).init(move || Fatal(message)).run();
}
