//! The incremental parse and the per-line counts against a full parse and a full count, after
//! thousands of random edits: typing, deleting, multi-line pastes, code fences opening and
//! closing, CR, CRLF and the other line terminators.

use writer_core::markdown;
use writer_core::text_stats::{self, DraftCounts};
use writer_core::MarkdownDocument;

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n.max(1) as u64) as usize
    }
}

const PIECES: &[&str] = &[
    "a", "word ", "prose here. ", " ", "\t", "\n", "\n\n", "\r", "\r\n", "\u{85}", "\u{2028}", "\u{2029}", "```",
    "```\n", "~~~", "~~~\n", "````", "  ```rust\n", "# ", "## ", "- ", "* ", "1. ", "2) ", "- [ ] ", "- [x] ", "> ",
    "---", "***", "**", "*", "_", "__", "~~", "`", "[link](url)", "![alt](images/a.png)", "<http://x.y>",
    "https://example.com/a", "www.a.b", "é", "—", "日本", "👍🏽", "\u{a0}",
];

fn random_text(rng: &mut Rng, pieces: usize) -> String {
    (0..pieces).map(|_| PIECES[rng.below(PIECES.len())]).collect()
}

fn boundary(text: &str, mut at: usize) -> usize {
    at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// One random edit: a typed piece, a deletion, a replaced selection, or a multi-line paste.
fn edit(rng: &mut Rng, text: &str) -> String {
    let a = boundary(text, rng.below(text.len() + 1));
    let len = match rng.below(4) {
        0 => 0,
        1 => rng.below(3),
        2 => rng.below(12),
        _ => rng.below(80),
    };
    let b = boundary(text, a + len);
    // Long drafts shrink, so they stay small enough to check after every edit.
    let pieces = if text.len() > 1500 { rng.below(2) } else if rng.below(8) == 0 { rng.below(25) } else { rng.below(3) };
    let insert = random_text(rng, pieces);
    format!("{}{}{}", &text[..a], insert, &text[b..])
}

fn check(doc: &MarkdownDocument, counts: &DraftCounts, text: &str) {
    assert_eq!(doc.text(), text);
    assert_eq!(doc.lines(), markdown::parse(text).as_slice(), "after editing to {text:?}");
    assert_eq!(counts.total(), text_stats::counts_for_markdown(text), "counts of {text:?}");
}

#[test]
fn incremental_parse_and_counts_match_a_full_parse_after_random_edits() {
    for seed in 1..=12u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let mut text = random_text(&mut rng, 30);
        let mut doc = MarkdownDocument::new(&text);
        let mut counts = DraftCounts::new(&doc);
        check(&doc, &counts, &text);
        for _ in 0..1000 {
            text = edit(&mut rng, &text);
            if let Some(splice) = doc.update(&text) {
                counts.follow(&doc, splice);
            }
            check(&doc, &counts, &text);
        }
    }
}

#[test]
fn typing_in_a_long_draft_reparses_only_the_lines_around_the_caret() {
    let mut text = "A paragraph of prose.\n\n".repeat(400);
    let mut doc = MarkdownDocument::new(&text);
    let at = text.len() / 2;
    text.insert(at, 'x');
    let splice = doc.update(&text).unwrap();
    assert!(splice.inserted <= 3 && splice.removed <= 3, "{splice:?}");
    assert_eq!(doc.lines(), markdown::parse(&text).as_slice());
}

#[test]
fn opening_a_fence_reparses_to_the_end_and_closing_it_restores_the_lines() {
    let mut text = "one\n\ntwo\n\nthree\n".to_string();
    let mut doc = MarkdownDocument::new(&text);
    text.insert_str(0, "```\n");
    let opened = doc.update(&text).unwrap();
    assert_eq!(opened.start + opened.inserted, doc.lines().len(), "every line after the fence is code now");
    assert_eq!(doc.lines(), markdown::parse(&text).as_slice());
    text.push_str("```\nafter\n");
    doc.update(&text).unwrap();
    assert_eq!(doc.lines(), markdown::parse(&text).as_slice());
    assert_eq!(doc.update(&text), None);
}

#[test]
fn an_lf_typed_after_a_cr_joins_them_into_one_terminator() {
    let mut text = "one\rtwo".to_string();
    let mut doc = MarkdownDocument::new(&text);
    assert_eq!(doc.lines().len(), 2);
    text.insert(4, '\n');
    doc.update(&text).unwrap();
    assert_eq!(doc.lines(), markdown::parse(&text).as_slice());
    assert_eq!(doc.lines().len(), 2);
}
