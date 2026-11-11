//! Fluent Writer: a quiet writing app built around focus.

mod app;
mod commands;
mod keys;
mod open_picker;
mod platform;
mod prefs;
mod remote_editing;
mod sidebar;
mod styler;
mod theme;
mod view;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use rune::AppSpec;
use writer_core::{AppVariant, DraftLibrary};

use app::App;
use prefs::Prefs;

/// The value after `flag` among the app's own arguments.
fn arg(flag: &str) -> Option<&'static str> {
    let args = rune::extra_args();
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// A build run straight from Cargo's target directory uses the dev folders, so trying a change
/// never touches the drafts you write in.
fn variant() -> AppVariant {
    let from_cargo = std::env::current_exe().ok().is_some_and(|p| p.components().any(|c| c.as_os_str() == "target"));
    match std::env::var("FLUENT_WRITER_VARIANT").as_deref() {
        Ok("standard") => AppVariant::Standard,
        Ok("dev") => AppVariant::Dev,
        _ if from_cargo => AppVariant::Dev,
        _ => AppVariant::Standard,
    }
}

fn init() -> App {
    let variant = variant();
    // `--drafts DIR --support DIR` point the app at other folders, for tests and screenshots.
    let library = match (arg("--drafts"), arg("--support")) {
        (Some(drafts), Some(support)) => DraftLibrary::new(PathBuf::from(drafts), PathBuf::from(support)),
        _ => DraftLibrary::default_library(variant),
    };
    let mut library = match library {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Fluent Writer couldn't open its folders: {e}");
            std::process::exit(1);
        }
    };
    let prefs = Prefs::load(library.support_directory());
    let opened = arg("--open").and_then(|p| library.open(&PathBuf::from(p)).ok());
    let document = match opened {
        Some(doc) => doc,
        None => match library.orphaned_recoveries().first() {
            Some(orphan) => library.restore(orphan),
            None => library.new_draft(),
        },
    };
    App::new(library, prefs, document)
}

fn main() {
    rune::run(AppSpec::<App> {
        fonts: theme::FONTS,
        families: (theme::FONT_FAMILY, theme::FONT_FAMILY),
        min_size: Some((420.0, 320.0)),
        poll: Some(|s, clock, tx| s.poll(clock, tx)),
        setup: Some(|s, waker| s.set_waker(waker)),
        on_key: Some(keys::window_key),
        ..AppSpec::new("Fluent Writer", (1060.0, 760.0), init, view::view)
    });
}
