//! Sentence and paragraph ranges for focus mode (Swift `TextUnits`).

use std::ops::Range;

use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::charsets::is_whitespace_or_newline;
use crate::markdown::{is_newline_unit, is_paragraph_separator, terminator_after};
use crate::offsets::floor_char_boundary;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FocusUnit {
    Off,
    Sentence,
    Paragraph,
    Typewriter,
}

impl FocusUnit {
    pub const ALL: [FocusUnit; 4] = [
        FocusUnit::Off,
        FocusUnit::Sentence,
        FocusUnit::Paragraph,
        FocusUnit::Typewriter,
    ];

    pub fn dims_surroundings(self) -> bool {
        self == FocusUnit::Sentence || self == FocusUnit::Paragraph
    }

    pub fn centers_caret(self) -> bool {
        self != FocusUnit::Off
    }
}

/// The start of the paragraph containing `loc` (NSString `getParagraphStart`). A location inside a
/// CRLF pair belongs to the paragraph that pair ends.
pub(crate) fn paragraph_start(text: &str, loc: usize) -> usize {
    let mut index = loc;
    if index > 0
        && index < text.len()
        && text.as_bytes()[index] == b'\n'
        && text.as_bytes()[index - 1] == b'\r'
    {
        index -= 1;
    }
    for (i, c) in text[..index].char_indices().rev() {
        if is_paragraph_separator(c) {
            return i + c.len_utf8();
        }
    }
    0
}

/// The line (hard-wrapped paragraph) containing `location`, without its terminator.
pub fn paragraph_range(text: &str, location: usize) -> Range<usize> {
    let loc = floor_char_boundary(text, location);
    if text.is_empty() {
        return 0..0;
    }
    if loc == text.len() && text.chars().next_back().is_some_and(is_newline_unit) {
        return loc..loc;
    }
    let start = paragraph_start(text, loc);
    let mut from = loc;
    if from > 0
        && from < text.len()
        && text.as_bytes()[from] == b'\n'
        && text.as_bytes()[from - 1] == b'\r'
    {
        from -= 1;
    }
    let (contents_end, _) = terminator_after(text, from, is_paragraph_separator);
    start..contents_end
}

/// The sentence containing `location`, trimmed of trailing whitespace.
/// A caret placed right after a sentence's final punctuation belongs to that sentence.
///
/// Swift used `NLTokenizer(unit: .sentence)`; this uses Unicode (UAX #29) sentence boundaries.
pub fn sentence_range(text: &str, location: usize) -> Range<usize> {
    let paragraph = paragraph_range(text, location);
    if paragraph.is_empty() {
        return paragraph;
    }
    let paragraph_text = &text[paragraph.clone()];
    let relative = location.saturating_sub(paragraph.start);
    let mut result: Option<Range<usize>> = None;
    let mut last: Option<Range<usize>> = None;
    for (start, sentence) in paragraph_text.split_sentence_bound_indices() {
        let r = start..start + sentence.len();
        last = Some(r.clone());
        if relative >= r.start && relative < r.end {
            result = Some(r);
            break;
        }
    }
    let mut range = result.or(last).unwrap_or(0..paragraph_text.len());
    // Trim trailing whitespace.
    while let Some(c) = paragraph_text[range.clone()].chars().next_back() {
        if !is_whitespace_or_newline(c) {
            break;
        }
        range.end -= c.len_utf8();
    }
    // A caret sitting in the whitespace after a sentence still belongs to it,
    // unless that is the start of the next sentence.
    range.start + paragraph.start..range.end + paragraph.start
}

pub fn focus_range(unit: FocusUnit, text: &str, location: usize) -> Option<Range<usize>> {
    match unit {
        FocusUnit::Sentence => Some(sentence_range(text, location)),
        FocusUnit::Paragraph => Some(paragraph_range(text, location)),
        FocusUnit::Off | FocusUnit::Typewriter => None,
    }
}
