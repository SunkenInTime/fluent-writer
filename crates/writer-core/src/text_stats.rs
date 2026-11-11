//! Word and character counts for the footer (Swift `TextStats`).

use unicode_segmentation::UnicodeSegmentation;

use crate::markdown::MarkdownLine;
use crate::markdown_document::{LineSplice, MarkdownDocument};
use crate::plain_text_exporter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextCounts {
    pub words: usize,
    /// User-perceived characters (grapheme clusters), as Swift's `String.count`.
    pub characters: usize,
}

impl TextCounts {
    pub const ZERO: TextCounts = TextCounts {
        words: 0,
        characters: 0,
    };

    pub fn new(words: usize, characters: usize) -> Self {
        TextCounts { words, characters }
    }
}

/// Counts what would actually be pasted: Markdown delimiters are excluded.
pub fn counts_for_markdown(markdown: &str) -> TextCounts {
    counts_for_plain_text(&plain_text_exporter::export(markdown))
}

/// What one line adds to the counts of `counts_for_markdown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineCounts {
    counts: TextCounts,
    output: LineOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum LineOutput {
    /// Leaves nothing in the plain text (a code fence).
    #[default]
    Nothing,
    /// A blank line, which collapses with the blank lines next to it.
    Blank,
    Text,
}

/// `counts_for_markdown` of a whole draft, kept per line so an edit recounts only the lines
/// it changed. Plain text lines are joined with line feeds, which always end a word and a
/// grapheme, so the totals are sums over lines plus one character for each line feed kept.
#[derive(Debug, Clone, Default)]
pub struct DraftCounts {
    /// Parallel to the document's lines.
    lines: Vec<LineCounts>,
}

impl DraftCounts {
    pub fn new(doc: &MarkdownDocument) -> DraftCounts {
        DraftCounts { lines: doc.lines().iter().map(|l| line_counts(doc.text(), l)).collect() }
    }

    /// Recounts the lines `splice` replaced in `doc`.
    pub fn follow(&mut self, doc: &MarkdownDocument, splice: LineSplice) {
        let fresh = doc.lines()[splice.new_lines()].iter().map(|l| line_counts(doc.text(), l));
        self.lines.splice(splice.start..splice.start + splice.removed, fresh);
    }

    /// Equal to `counts_for_markdown(doc.text())`.
    pub fn total(&self) -> TextCounts {
        let mut total = TextCounts::ZERO;
        let mut kept = 0;
        // `export` keeps one blank line between two lines of text and drops the rest.
        let mut blank_pending = false;
        for line in &self.lines {
            match line.output {
                LineOutput::Nothing => {}
                LineOutput::Blank => blank_pending = kept > 0,
                LineOutput::Text => {
                    kept += 1 + blank_pending as usize;
                    blank_pending = false;
                    total.words += line.counts.words;
                    total.characters += line.counts.characters;
                }
            }
        }
        total.characters += kept.saturating_sub(1);
        total
    }
}

fn line_counts(markdown: &str, line: &MarkdownLine) -> LineCounts {
    match plain_text_exporter::export_line(markdown, line) {
        None => LineCounts::default(),
        Some(t) if t.is_empty() => LineCounts { counts: TextCounts::ZERO, output: LineOutput::Blank },
        Some(t) => LineCounts { counts: counts_for_plain_text(&t), output: LineOutput::Text },
    }
}

/// Words are Unicode (UAX #29) words containing a letter or digit, standing in for
/// NSString's `.byWords` enumeration.
pub fn counts_for_plain_text(text: &str) -> TextCounts {
    TextCounts {
        words: text.unicode_words().count(),
        characters: text.graphemes(true).count(),
    }
}

/// `1,234 words  ·  5,678 characters`, or `12 of 1,234 words  ·  …` with a selection.
///
/// Swift used the current locale's `NumberFormatter`; this always groups thousands with commas.
pub fn format(counts: TextCounts, selection: Option<TextCounts>) -> String {
    let words = |v: usize| if v == 1 { "word" } else { "words" };
    let chars = |v: usize| if v == 1 { "character" } else { "characters" };
    if let Some(s) = selection {
        return format!(
            "{} of {} {}  ·  {} of {} {}",
            group(s.words),
            group(counts.words),
            words(counts.words),
            group(s.characters),
            group(counts.characters),
            chars(counts.characters)
        );
    }
    format!(
        "{} {}  ·  {} {}",
        group(counts.words),
        words(counts.words),
        group(counts.characters),
        chars(counts.characters)
    )
}

fn group(v: usize) -> String {
    let digits = v.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(
            format(TextCounts::new(1, 1234), None),
            "1 word  ·  1,234 characters"
        );
        assert_eq!(
            format(TextCounts::new(1_000_000, 2), Some(TextCounts::new(3, 1))),
            "3 of 1,000,000 words  ·  1 of 2 characters"
        );
    }
}
