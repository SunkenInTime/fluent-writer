//! Timing of typing, caret moves and scrolling on long drafts, through the real view, layout
//! and key routing (headless, no GPU). Run with
//! `cargo test --release -p fluent-writer perf -- --ignored --nocapture --test-threads 1`.

use std::time::Instant;

use rune::{Key, Mods, Runtime};
use writer_core::FocusUnit;

use crate::app::App;
use crate::tests::{harness, press, settle, type_text};

/// A draft of `paragraphs` paragraphs of 75 words, with a heading every 12 paragraphs.
fn draft(paragraphs: usize) -> String {
    let words = ["the", "quick", "brown", "fox", "jumps", "over", "a", "lazy", "dog", "while", "writing", "prose", "about", "focus", "and", "calm"];
    let mut out = String::new();
    for p in 0..paragraphs {
        if p % 12 == 0 {
            out.push_str(&format!("## Section {p}\n\n"));
        }
        for w in 0..75 {
            out.push_str(words[(p * 7 + w * 13) % words.len()]);
            out.push(if w == 74 { '.' } else { ' ' });
        }
        out.push_str("\n\n");
    }
    out
}

fn long_draft() -> String {
    draft(140)
}

fn ms(t: Instant, n: usize) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0 / n as f64
}

/// The app on `body` with the caret at `caret`, settled.
fn at(body: &str, caret: usize) -> (Runtime<App>, tempfile::TempDir) {
    let (mut rt, dir) = harness(body);
    rt.update(|s, _| s.edit.set_selection(caret, caret));
    settle(&mut rt, 0.5);
    (rt, dir)
}

/// Milliseconds per keystroke (key handler plus the frame that draws it), typing 40 characters.
fn keystroke(body: &str, caret: usize) -> f64 {
    let (mut rt, _dir) = at(body, caret);
    let t = Instant::now();
    type_text(&mut rt, "typing forty characters into a long doc.");
    ms(t, 40)
}

#[test]
#[ignore]
fn perf_keystroke_on_a_long_draft() {
    let body = long_draft();
    let short = draft(7);
    eprintln!("long draft: {} words; short draft: {} words", body.split_whitespace().count(), short.split_whitespace().count());
    let t = Instant::now();
    for _ in 0..20 {
        std::hint::black_box(writer_core::markdown::parse(&body));
    }
    eprintln!("markdown::parse: {:.2} ms", ms(t, 20));
    let t = Instant::now();
    for _ in 0..20 {
        std::hint::black_box(writer_core::text_stats::counts_for_markdown(&body));
    }
    eprintln!("counts_for_markdown: {:.2} ms", ms(t, 20));

    for (name, text) in [("long", &body), ("short", &short)] {
        let mid = text.len() / 2;
        eprintln!("{name}: one keystroke, handler + frame, caret at end: {:.2} ms", keystroke(text, text.len()));
        eprintln!("{name}: one keystroke, handler + frame, caret in middle: {:.2} ms", keystroke(text, mid));
        eprintln!("{name}: one keystroke, handler + frame, caret at start: {:.2} ms", keystroke(text, 0));
    }

    let (mut rt, _dir) = at(&body, body.len() / 2);
    let t = Instant::now();
    for _ in 0..40 {
        rt.update(|s, tx| s.text_did_change(tx));
    }
    eprintln!("long: text_did_change alone: {:.2} ms", ms(t, 40));
    let t = Instant::now();
    for _ in 0..40 {
        rt.frame(1.0 / 60.0);
    }
    eprintln!("long: idle frame: {:.2} ms", ms(t, 40));

    // Arrow keys in sentence focus: each move re-runs focus dimming.
    rt.update(|s, tx| s.set_focus(FocusUnit::Sentence, tx));
    settle(&mut rt, 3.0);
    let t = Instant::now();
    for i in 0..40 {
        press(&mut rt, if i % 10 < 5 { Key::Right } else { Key::Down }, Mods::default());
    }
    eprintln!("long: arrow key in sentence focus, handler + frame: {:.2} ms", ms(t, 40));
    rt.update(|s, tx| s.set_focus(FocusUnit::Off, tx));
    settle(&mut rt, 3.0);

    // Scrolling: a wheel step over the draft, then the frame that draws it.
    let (x, y) = (530.0, 400.0);
    let top = rt.state.field_rect[1];
    let t = Instant::now();
    for i in 0..60 {
        rt.wheel(x, y, if i < 30 { 40.0 } else { -40.0 });
        rt.frame(1.0 / 60.0);
        if i == 29 {
            assert!((rt.state.field_rect[1] - top).abs() > 400.0, "the draft scrolled");
        }
    }
    eprintln!("long: scrolling frame: {:.2} ms", ms(t, 60));
}

/// Types for a while in the middle of the long draft (`SHORT=1`: the short one), for a
/// profiler to sample: `perf record` on the test binary with `perf_profile_typing --ignored`.
#[test]
#[ignore]
fn perf_profile_typing() {
    let body = if std::env::var_os("SHORT").is_some() { draft(7) } else { long_draft() };
    let (mut rt, _dir) = at(&body, body.len() / 2);
    for _ in 0..15 {
        type_text(&mut rt, "typing forty characters into a long doc.");
    }
}
