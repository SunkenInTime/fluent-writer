//! Timing of one keystroke on a long draft. Run with
//! `cargo test --release -p fluent-writer perf -- --ignored --nocapture`.

use std::time::Instant;

use crate::tests::{harness, settle, type_text};

fn long_draft() -> String {
    let words = ["the", "quick", "brown", "fox", "jumps", "over", "a", "lazy", "dog", "while", "writing", "prose", "about", "focus", "and", "calm"];
    let mut out = String::new();
    for p in 0..140 {
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

#[test]
#[ignore]
fn perf_keystroke_on_a_long_draft() {
    let body = long_draft();
    let t = Instant::now();
    for _ in 0..20 {
        std::hint::black_box(writer_core::markdown::parse(&body));
    }
    eprintln!("markdown::parse: {:.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
    let t = Instant::now();
    for _ in 0..20 {
        std::hint::black_box(writer_core::text_stats::counts_for_markdown(&body));
    }
    eprintln!("counts_for_markdown: {:.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 20.0);

    let (mut rt, _dir) = harness(&body);
    settle(&mut rt, 0.5);
    let t = Instant::now();
    type_text(&mut rt, "typing forty characters into a long doc.");
    let per_key = t.elapsed().as_secs_f64() * 1000.0 / 40.0;
    eprintln!("one keystroke, handler + frame (no GPU): {per_key:.2} ms on {} words", body.split_whitespace().count());
    let t = Instant::now();
    for _ in 0..40 {
        rt.update(|s, tx| s.text_did_change(tx));
    }
    eprintln!("text_did_change alone: {:.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 40.0);
    let t = Instant::now();
    for _ in 0..40 {
        rt.frame(1.0 / 60.0);
    }
    eprintln!("idle frame: {:.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 40.0);
}
