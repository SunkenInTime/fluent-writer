//! Markdown-aware editing commands: list continuation, indenting, and inline/line formatting
//! (Swift `TextEdit`, `ListContinuation` and `InlineFormatting`).

use std::ops::Range;

use crate::charsets::{is_alphanumeric, is_whitespace, trim};
use crate::markdown::{self, MarkdownBlockKind, is_paragraph_separator};
use crate::text_units::paragraph_range;

/// A replacement in byte coordinates plus the selection to apply afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range<usize>,
    pub replacement: String,
    pub selection_after: Range<usize>,
}

impl TextEdit {
    pub fn new(
        range: Range<usize>,
        replacement: impl Into<String>,
        selection_after: Range<usize>,
    ) -> Self {
        TextEdit {
            range,
            replacement: replacement.into(),
            selection_after,
        }
    }

    pub fn applied(&self, text: &str) -> String {
        let mut out = text.to_string();
        out.replace_range(self.range.clone(), &self.replacement);
        out
    }
}

// MARK: List continuation

/// The edit for pressing Return. Returns `None` when the default newline behavior applies.
pub fn newline_edit(text: &str, selection: Range<usize>) -> Option<TextEdit> {
    if !selection.is_empty() {
        return None;
    }
    let caret = selection.start;
    let line_range = paragraph_range(text, caret);
    let line = markdown::parse_line(&text[line_range.clone()], line_range.start);
    let marker = line.marker_range.clone()?;
    let continuing = matches!(
        line.kind,
        MarkdownBlockKind::BulletItem
            | MarkdownBlockKind::OrderedItem { .. }
            | MarkdownBlockKind::TaskItem { .. }
            | MarkdownBlockKind::Blockquote
    );
    if !continuing || caret < marker.end {
        return None;
    }

    let content = trim(&text[line.content_range.clone()], is_whitespace);
    if content.is_empty() {
        // Return on an empty item ends the list.
        let removal = line_range.start..line.content_range.end;
        return Some(TextEdit::new(
            removal,
            "",
            line_range.start..line_range.start,
        ));
    }

    let indent = &text[line.indent_range.clone()];
    let marker_text = &text[marker.clone()];
    let next = match line.kind {
        MarkdownBlockKind::OrderedItem { number, delimiter } => {
            format!("{indent}{}{delimiter} ", number as u64 + 1)
        }
        MarkdownBlockKind::TaskItem { .. } => {
            let bullet = marker_text
                .chars()
                .next()
                .map_or("-".to_string(), |c| c.to_string());
            format!("{indent}{bullet} [ ] ")
        }
        MarkdownBlockKind::BulletItem => format!("{indent}{} ", trim(marker_text, is_whitespace)),
        MarkdownBlockKind::Blockquote => {
            if marker_text.ends_with(' ') {
                marker_text.to_string()
            } else {
                format!("{marker_text} ")
            }
        }
        _ => return None,
    };
    // Drop trailing spaces before the caret so lines don't end in whitespace.
    let mut start = caret;
    while start > marker.end && text.as_bytes()[start - 1] == b' ' {
        start -= 1;
    }
    let insertion = format!("\n{next}");
    let after = start + insertion.len();
    Some(TextEdit::new(start..caret, insertion, after..after))
}

/// Indent (or outdent) list items touched by the selection by two spaces.
pub fn indent_edit(text: &str, selection: Range<usize>, outdent: bool) -> Option<TextEdit> {
    let lines = line_ranges(text, &selection);
    let block = lines.first()?.start..lines.last()?.end;
    let mut any_list = false;
    let mut new_lines: Vec<String> = Vec::new();
    let mut caret_shift_first: isize = 0;
    for (i, r) in lines.iter().enumerate() {
        let s = &text[r.clone()];
        let parsed = markdown::parse_line(s, 0);
        if parsed.kind.is_list_item() {
            any_list = true;
            if outdent {
                let mut removed = 0;
                let mut t = s;
                while removed < 2 && t.starts_with(' ') {
                    t = &t[1..];
                    removed += 1;
                }
                if removed == 0 && t.starts_with('\t') {
                    t = &t[1..];
                    removed = 1;
                }
                new_lines.push(t.to_string());
                if i == 0 {
                    caret_shift_first = -removed;
                }
            } else {
                new_lines.push(format!("  {s}"));
                if i == 0 {
                    caret_shift_first = 2;
                }
            }
        } else {
            new_lines.push(s.to_string());
        }
    }
    if !any_list {
        return None;
    }
    let replacement = new_lines.join("\n");
    let new_selection = if selection.is_empty() {
        let at = (selection.start as isize + caret_shift_first).max(block.start as isize) as usize;
        at..at
    } else {
        block.start..block.start + replacement.len()
    };
    Some(TextEdit::new(block, replacement, new_selection))
}

/// The paragraph ranges touched by `selection`.
///
/// Swift stepped one UTF-16 unit past each paragraph's end, which landed inside a CRLF pair and
/// looped forever on CRLF text. This steps past the whole terminator instead.
pub(crate) fn line_ranges(text: &str, selection: &Range<usize>) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut loc = selection.start;
    let end = selection
        .start
        .max(selection.end - usize::from(!selection.is_empty()));
    loop {
        let r = paragraph_range(text, loc);
        let r_end = r.end;
        ranges.push(r);
        let next = match text[r_end..].chars().next() {
            Some('\r') if text[r_end + 1..].starts_with('\n') => r_end + 2,
            Some(c) if is_paragraph_separator(c) => r_end + c.len_utf8(),
            _ => r_end + 1,
        };
        if next > end || next > text.len() {
            break;
        }
        loc = next;
    }
    ranges
}

// MARK: Inline formatting

/// Toggles a symmetric delimiter (`**`, `*`, `~~`) around the selection.
pub fn toggle(delimiter: &str, text: &str, selection: Range<usize>) -> TextEdit {
    let dl = delimiter.len();
    let mut sel = selection;

    if sel.is_empty() {
        // Inside an existing pair with nothing selected: expand to the word.
        let word = word_range(text, sel.start);
        if !word.is_empty() {
            sel = word;
        }
    }

    if sel.is_empty() {
        // Directly between an empty pair: remove it.
        let at = sel.start;
        if at >= dl
            && at + dl <= text.len()
            && text.get(at - dl..at) == Some(delimiter)
            && text.get(at..at + dl) == Some(delimiter)
        {
            let r = at - dl..at + dl;
            return TextEdit::new(r.clone(), "", r.start..r.start);
        }
        return TextEdit::new(
            sel.clone(),
            format!("{delimiter}{delimiter}"),
            at + dl..at + dl,
        );
    }

    let selected = &text[sel.clone()];
    // Selection includes the delimiters.
    if selected.starts_with(delimiter)
        && selected.ends_with(delimiter)
        && selected.len() >= dl * 2
        && is_exact_delimiter(delimiter, selected, 0)
        && is_exact_delimiter(delimiter, selected, selected.len() - dl)
    {
        let inner = &selected[dl..selected.len() - dl];
        return TextEdit::new(sel.clone(), inner, sel.start..sel.start + inner.len());
    }
    // Delimiters surround the selection.
    if sel.start >= dl
        && sel.end + dl <= text.len()
        && is_exact_delimiter(delimiter, text, sel.start - dl)
        && is_exact_delimiter(delimiter, text, sel.end)
    {
        let outer = sel.start - dl..sel.end + dl;
        return TextEdit::new(
            outer.clone(),
            selected,
            outer.start..outer.start + sel.len(),
        );
    }
    // Keep surrounding whitespace outside the delimiters.
    let leading = selected.len() - selected.trim_start_matches(' ').len();
    let trailing = selected.len() - selected.trim_end_matches(' ').len();
    let core_start = sel.start + leading;
    let core_len = sel.len().saturating_sub(leading + trailing);
    let core = core_start..core_start + core_len;
    let core_text = &text[core.clone()];
    TextEdit::new(
        core.clone(),
        format!("{delimiter}{core_text}{delimiter}"),
        core.start + dl..core.start + dl + core.len(),
    )
}

/// True when `delimiter` sits at `location` and is not part of a longer run of the same character.
fn is_exact_delimiter(delimiter: &str, text: &str, location: usize) -> bool {
    let dl = delimiter.len();
    if location + dl > text.len() || text.get(location..location + dl) != Some(delimiter) {
        return false;
    }
    let bytes = text.as_bytes();
    let ch = delimiter.as_bytes()[0];
    let before = if location > 0 { bytes[location - 1] } else { 0 };
    let after = if location + dl < bytes.len() {
        bytes[location + dl]
    } else {
        0
    };
    if delimiter == "*" || delimiter == "_" {
        // `*` must not be half of `**`, unless the run is `***`.
        let run_before = before == ch;
        let run_after = after == ch;
        if run_before != run_after {
            return false;
        }
    }
    true
}

/// The word around a caret, for formatting with nothing selected. Empty unless the caret is
/// strictly inside a word.
fn word_range(text: &str, location: usize) -> Range<usize> {
    if text.is_empty() {
        return location..location;
    }
    let is_word = |c: char| is_alphanumeric(c) || c == '\'' || c == '\u{2019}';
    let mut start = location;
    let mut end = location;
    while let Some(c) = text[..start].chars().next_back()
        && is_word(c)
    {
        start -= c.len_utf8();
    }
    while let Some(c) = text[end..].chars().next()
        && is_word(c)
    {
        end += c.len_utf8();
    }
    // Only expand when the caret is inside a word, not at its edge.
    if start == location || end == location {
        return location..location;
    }
    start..end
}

pub fn link(text: &str, selection: Range<usize>) -> TextEdit {
    let selected = &text[selection.clone()];
    let looks_like_url = selected.starts_with("http://")
        || selected.starts_with("https://")
        || selected.starts_with("www.");
    if selection.is_empty() {
        let at = selection.start + 1;
        return TextEdit::new(selection, "[]()", at..at);
    }
    if looks_like_url {
        let at = selection.start + 1;
        return TextEdit::new(selection.clone(), format!("[]({selected})"), at..at);
    }
    let replacement = format!("[{selected}]()");
    let at = selection.start + replacement.len() - 1;
    TextEdit::new(selection, replacement, at..at)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinePrefix {
    Heading(usize),
    Bullet,
    Numbered,
    Quote,
    Body,
}

/// Sets (or toggles off) a block prefix on every line touched by the selection.
pub fn set_line_prefix(prefix: LinePrefix, text: &str, selection: Range<usize>) -> TextEdit {
    let lines = line_ranges(text, &selection);
    let block = lines[0].start..lines[lines.len() - 1].end;
    let parsed: Vec<_> = lines
        .iter()
        .map(|r| markdown::parse_line(&text[r.clone()], r.start))
        .collect();

    let all_match = parsed.iter().all(|line| match (prefix, line.kind) {
        (LinePrefix::Heading(a), MarkdownBlockKind::Heading { level }) => a == level,
        (LinePrefix::Bullet, MarkdownBlockKind::BulletItem)
        | (LinePrefix::Numbered, MarkdownBlockKind::OrderedItem { .. })
        | (LinePrefix::Quote, MarkdownBlockKind::Blockquote) => true,
        _ => false,
    });
    let mut new_lines: Vec<String> = Vec::new();
    let mut number = 1;
    let mut caret_delta: isize = 0;
    for (i, line) in parsed.iter().enumerate() {
        let content = &text[line.content_range.clone()];
        let old_prefix_length = line.content_range.start - line.range.start;
        if line.kind == MarkdownBlockKind::Blank && lines.len() > 1 {
            new_lines.push(text[line.range.clone()].to_string());
            continue;
        }
        let new_prefix = if all_match || prefix == LinePrefix::Body {
            String::new()
        } else {
            match prefix {
                LinePrefix::Heading(level) => format!("{} ", "#".repeat(level)),
                LinePrefix::Bullet => "- ".to_string(),
                LinePrefix::Numbered => {
                    let p = format!("{number}. ");
                    number += 1;
                    p
                }
                LinePrefix::Quote => "> ".to_string(),
                LinePrefix::Body => String::new(),
            }
        };
        if i == 0 {
            caret_delta = new_prefix.len() as isize - old_prefix_length as isize;
        }
        new_lines.push(new_prefix + content);
    }
    let replacement = new_lines.join("\n");
    let after = if selection.is_empty() {
        let at = (selection.start as isize + caret_delta).max(block.start as isize) as usize;
        at..at
    } else {
        block.start..block.start + replacement.len()
    };
    TextEdit::new(block, replacement, after)
}
