//! A parsed draft that follows edits by re-parsing only the lines an edit touched
//! (Swift `EditorController.reparse(full:edited:)`), always equal to `markdown::parse` of
//! the whole text.
//!
//! A line's parse depends on its own text and on the code-fence state before it, nothing else.
//! So after an edit the lines are parsed again from just before the change until a line that
//! starts past it, in unchanged text, with the same fence state it had before: from there on
//! every old line is still right once its offsets are moved by the change in length.

use std::ops::Range;

use crate::markdown::{self, Fence, MarkdownLine};

#[derive(Clone, Debug, Default)]
pub struct MarkdownDocument {
    /// The text `lines` were parsed from.
    text: String,
    lines: Vec<MarkdownLine>,
    /// The fence state before each line, parallel to `lines`.
    fences: Vec<Fence>,
}

/// Which lines an update replaced: `removed` old lines starting at index `start` gave way to
/// `inserted` new ones. Lines after them are the old ones, moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineSplice {
    pub start: usize,
    pub removed: usize,
    pub inserted: usize,
}

impl LineSplice {
    /// The replaced lines' indices in the new list.
    pub fn new_lines(&self) -> Range<usize> {
        self.start..self.start + self.inserted
    }
}

impl MarkdownDocument {
    pub fn new(text: &str) -> MarkdownDocument {
        let mut doc = MarkdownDocument { text: text.to_string(), ..Default::default() };
        markdown::parse_lines_from(
            text,
            0,
            None,
            |line, fence| {
                doc.lines.push(line);
                doc.fences.push(fence);
            },
            |_, _| false,
        );
        doc
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn lines(&self) -> &[MarkdownLine] {
        &self.lines
    }

    /// Brings the parse up to `text`. Returns the lines that changed, or `None` when the text
    /// is the one already parsed.
    pub fn update(&mut self, text: &str) -> Option<LineSplice> {
        if self.text == text {
            return None;
        }
        let (old, new) = (self.text.as_bytes(), text.as_bytes());
        // The changed bytes: old[p..old.len() - s] became new[p..new_end].
        let p = old.iter().zip(new).take_while(|(a, b)| a == b).count();
        let room = old.len().min(new.len()) - p;
        let mut s = old.iter().rev().zip(new.iter().rev()).take(room).take_while(|(a, b)| a == b).count();
        // The unchanged tail starts on a character in both texts (`é` and U+2029 end in the
        // same byte), so the character before any line in it is unchanged too.
        while !self.text.is_char_boundary(old.len() - s) || !text.is_char_boundary(new.len() - s) {
            s -= 1;
        }
        let new_end = new.len() - s;
        let delta = new.len() as isize - old.len() as isize;

        // Start a line early: a line's terminator can change with the next line's first
        // character (a CR followed by an inserted LF becomes one CRLF).
        let containing = self.lines.partition_point(|l| l.range.start <= p).saturating_sub(1);
        let start = containing.saturating_sub(1);
        let (from, fence) = match self.lines.get(start) {
            Some(l) => (l.range.start, self.fences[start]),
            None => (0, None),
        };

        let mut fresh = Vec::new();
        let mut fresh_fences = Vec::new();
        // The old line the parse caught up with, where the rest is reused.
        let mut resume = self.lines.len();
        let (lines, fences) = (&self.lines, &self.fences);
        markdown::parse_lines_from(
            text,
            from,
            fence,
            |line, fence| {
                fresh.push(line);
                fresh_fences.push(fence);
            },
            |at, fence| {
                // A line starting after the change (so the character before it is unchanged
                // too) reads the same text as the old line there; with the same fence state
                // it parses the same.
                if at <= new_end {
                    return false;
                }
                let old_at = (at as isize - delta) as usize;
                let i = lines.partition_point(|l| l.range.start < old_at);
                if lines.get(i).is_some_and(|l| l.range.start == old_at) && fences[i] == fence {
                    resume = i;
                    return true;
                }
                false
            },
        );
        let splice = LineSplice { start, removed: resume - start, inserted: fresh.len() };
        for line in &mut self.lines[resume..] {
            shift_line(line, delta);
        }
        self.lines.splice(start..resume, fresh);
        self.fences.splice(start..resume, fresh_fences);
        self.text.clear();
        self.text.push_str(text);
        Some(splice)
    }
}

fn shift(r: &mut Range<usize>, delta: isize) {
    r.start = (r.start as isize + delta) as usize;
    r.end = (r.end as isize + delta) as usize;
}

fn shift_line(line: &mut MarkdownLine, delta: isize) {
    shift(&mut line.range, delta);
    shift(&mut line.indent_range, delta);
    if let Some(m) = &mut line.marker_range {
        shift(m, delta);
    }
    shift(&mut line.content_range, delta);
    for span in &mut line.inlines {
        shift(&mut span.range, delta);
        shift(&mut span.content_range, delta);
        for m in &mut span.marker_ranges {
            shift(m, delta);
        }
        if let Some(u) = &mut span.url_range {
            shift(u, delta);
        }
    }
}
