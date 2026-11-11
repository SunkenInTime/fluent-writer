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
mod perf_tests;
#[cfg(test)]
mod chrome_tests;
#[cfg(test)]
mod audit_tests;

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

/// Which copy of the app this is. `FLUENT_WRITER_VARIANT=standard|dev` says so outright;
/// otherwise [`variant_for_exe`] reads it from where the executable sits.
pub(crate) fn variant() -> AppVariant {
    static VARIANT: std::sync::OnceLock<AppVariant> = std::sync::OnceLock::new();
    *VARIANT.get_or_init(|| match std::env::var("FLUENT_WRITER_VARIANT").ok().and_then(|v| v.parse().ok()) {
        Some(v) => v,
        None => variant_for_exe(std::env::current_exe().ok().as_deref()),
    })
}

/// As the Swift `AppVariant.current`: in a Mac app bundle, `FWVariant` in its Info.plist
/// (standard when it has none); a build run straight from Cargo's target folder (marked by
/// its `CACHEDIR.TAG`) is dev, so trying a change never touches the drafts you write in, as
/// `swift run` was; on a Mac any other unbundled binary is dev too.
/// `WriterCore/AppVariant.swift:9-13`
fn variant_for_exe(exe: Option<&std::path::Path>) -> AppVariant {
    let Some(exe) = exe else { return AppVariant::Standard };
    let contents = exe.parent().and_then(std::path::Path::parent).filter(|c| c.file_name().is_some_and(|n| n == "Contents"));
    if let Some(info) = contents.and_then(|c| std::fs::read_to_string(c.join("Info.plist")).ok()) {
        let raw = prefs::plist_dict(&info).and_then(|d| d.into_iter().find(|(k, _)| k == "FWVariant")).and_then(|(_, v)| match v {
            prefs::PlistValue::String(s) => s.parse().ok(),
            _ => None,
        });
        return raw.unwrap_or(AppVariant::Standard);
    }
    let from_cargo = exe.components().any(|c| c.as_os_str() == "target") || exe.ancestors().skip(1).take(4).any(|d| d.join("CACHEDIR.TAG").is_file());
    if from_cargo || cfg!(target_os = "macos") { AppVariant::Dev } else { AppVariant::Standard }
}

/// The bundle identifier the Swift app had, whose `UserDefaults` hold its settings.
/// `script/build-app.sh:17-21`
fn bundle_id(variant: AppVariant) -> &'static str {
    match variant {
        AppVariant::Standard => "com.sunkenintime.fluentwriter",
        AppVariant::Dev => "com.sunkenintime.fluentwriter.dev",
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
    // The app's own folders: a Mac that ran the Swift app brings its settings along once.
    let prefs = if arg("--support").is_some() { Prefs::load(library.support_directory()) } else { Prefs::load_or_import(library.support_directory(), bundle_id(variant())) };
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
        app.opened_at(opening.frame, opening.place);
        app
    })
    // The title bar is transparent with the content under it; only the top bar drags the window.
    .chrome(Chrome { full_size_content: true, traffic_lights: None, drag_regions: true })
    .window_options(opening.options)
    // The leave check runs before the window closes; when it asks, the window stays up.
    .on_close_request(|s, why, tx| s.close_requested(why, tx))
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
