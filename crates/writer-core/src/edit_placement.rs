//! Agent edits anchored by text rather than offsets, so they can land anywhere in the draft
//! (Swift `ProposedEdit`, `EditPlacement` and `EditPlacer`).

use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::charsets::{is_newline, is_whitespace, trim};
use crate::offsets::byte_to_utf16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditPosition {
    Start,
    End,
}

/// One change an agent wants. Either `old` names existing text to replace, or `position` inserts
/// at the start or end of the draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedEdit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    #[serde(default)]
    pub new: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<EditPosition>,
    /// Comment ids this edit addresses.
    #[serde(default, rename = "commentIDs")]
    pub comment_ids: Vec<String>,
}

impl ProposedEdit {
    /// Replace `old` with `new`.
    pub fn replace(old: impl Into<String>, new: impl Into<String>) -> Self {
        ProposedEdit {
            old: Some(old.into()),
            new: new.into(),
            position: None,
            comment_ids: Vec::new(),
        }
    }

    /// Insert `new` as a block at the start or end of the draft.
    pub fn insert(new: impl Into<String>, position: EditPosition) -> Self {
        ProposedEdit {
            old: None,
            new: new.into(),
            position: Some(position),
            comment_ids: Vec::new(),
        }
    }

    pub fn with_comment_ids(mut self, ids: Vec<String>) -> Self {
        self.comment_ids = ids;
        self
    }
}

/// An edit resolved to a byte range of the current text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedEdit {
    pub index: usize,
    pub range: Range<usize>,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditFailure {
    pub index: usize,
    pub edit: ProposedEdit,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditPlacement {
    pub placed: Vec<PlacedEdit>,
    pub failed: Vec<EditFailure>,
}

impl EditPlacement {
    /// The text with every placed edit applied.
    pub fn applied(&self, text: &str) -> String {
        let mut out = text.to_string();
        // Back to front. At the same offset, replacements go before insertions (so they can't eat
        // inserted text), and insertions go in reverse so they read in the order they were given.
        let mut order: Vec<&PlacedEdit> = self.placed.iter().collect();
        order.sort_by(|a, b| {
            b.range
                .start
                .cmp(&a.range.start)
                .then(b.range.len().cmp(&a.range.len()))
                .then(b.index.cmp(&a.index))
        });
        for e in order {
            out.replace_range(e.range.clone(), &e.replacement);
        }
        out
    }
}

/// A per-edit focus: the passage a particular edit was about (for example its comment's span).
pub type FocusFor<'a> = &'a dyn Fn(&ProposedEdit) -> Option<Range<usize>>;

/// Resolves each edit against `text`. When `old` appears more than once, the occurrence nearest
/// `focus` wins (the selection or commented passage the request was about). `focus_for` can give a
/// focus per edit, which takes precedence when it returns `Some`.
pub fn place(
    edits: &[ProposedEdit],
    text: &str,
    focus: Option<Range<usize>>,
    focus_for: Option<FocusFor<'_>>,
) -> EditPlacement {
    let mut placed: Vec<PlacedEdit> = Vec::new();
    let mut failed: Vec<EditFailure> = Vec::new();
    for (i, edit) in edits.iter().enumerate() {
        if let Some(position) = edit.position
            && edit.old.as_deref().is_none_or(str::is_empty)
        {
            placed.push(insertion(position, &edit.new, text, i));
            continue;
        }
        let Some(old) = edit.old.as_deref().filter(|o| !o.is_empty()) else {
            failed.push(EditFailure {
                index: i,
                edit: edit.clone(),
                reason: "The edit didn't say what text to change.".into(),
            });
            continue;
        };
        let all = occurrences(old, text);
        let candidates: Vec<Range<usize>> = all
            .iter()
            .filter(|r| !placed.iter().any(|p| overlaps(&p.range, r)))
            .cloned()
            .collect();
        let f = focus_for.and_then(|f| f(edit)).or_else(|| focus.clone());
        let Some(range) = nearest(&candidates, f.as_ref(), text) else {
            let reason = if all.is_empty() {
                "Couldn't find the text this edit changes. It may have been edited since."
            } else {
                "This edit overlaps another one."
            };
            failed.push(EditFailure {
                index: i,
                edit: edit.clone(),
                reason: reason.into(),
            });
            continue;
        };
        placed.push(PlacedEdit {
            index: i,
            range,
            replacement: edit.new.clone(),
        });
    }
    EditPlacement { placed, failed }
}

fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    if a.is_empty() || b.is_empty() {
        return (a.start > b.start && a.start < b.end) || (b.start > a.start && b.start < a.end);
    }
    a.start.max(b.start) < a.end.min(b.end)
}

/// The candidate nearest `focus`; the first one on a tie or without a focus. Distances are measured
/// in UTF-16 units, as Swift did, so the same occurrence wins.
fn nearest(
    ranges: &[Range<usize>],
    focus: Option<&Range<usize>>,
    text: &str,
) -> Option<Range<usize>> {
    let Some(focus) = focus else {
        return ranges.first().cloned();
    };
    let u = |b: usize| byte_to_utf16(text, b) as isize;
    let (fs, fe) = (u(focus.start), u(focus.end));
    ranges
        .iter()
        .min_by_key(|r| {
            let (rs, re) = (u(r.start), u(r.end));
            distance(
                rs,
                re,
                fs,
                fe,
                focus.start.max(r.start) < focus.end.min(r.end),
            )
        })
        .cloned()
}

fn distance(rs: isize, re: isize, fs: isize, fe: isize, intersects: bool) -> isize {
    if intersects || (rs >= fs && re <= fe) {
        return 0;
    }
    (rs - fe).abs().min((re - fs).abs())
}

/// Adds a block at the start or end, keeping one blank line between it and the existing text.
fn insertion(position: EditPosition, new: &str, text: &str, index: usize) -> PlacedEdit {
    let block = trim(new, is_newline);
    if text.is_empty() {
        return PlacedEdit {
            index,
            range: 0..0,
            replacement: block.to_string(),
        };
    }
    match position {
        EditPosition::End => {
            let separator = if text.ends_with("\n\n") {
                ""
            } else if text.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            PlacedEdit {
                index,
                range: text.len()..text.len(),
                replacement: format!("{separator}{block}"),
            }
        }
        EditPosition::Start => {
            let separator = if text.starts_with('\n') { "\n" } else { "\n\n" };
            PlacedEdit {
                index,
                range: 0..0,
                replacement: format!("{block}{separator}"),
            }
        }
    }
}

/// Exact matches first. Failing that, matches that differ only in whitespace runs or curly quotes.
pub fn occurrences(needle: &str, text: &str) -> Vec<Range<usize>> {
    let mut exact = Vec::new();
    if !needle.is_empty() {
        let mut from = 0;
        while from < text.len() {
            let Some(i) = text[from..].find(needle) else {
                break;
            };
            let r = from + i..from + i + needle.len();
            from = r.end;
            exact.push(r);
        }
    }
    if !exact.is_empty() {
        return exact;
    }

    let (hay, map) = normalize(text);
    let (pin, _) = normalize(needle);
    let trimmed = trim(&pin, is_whitespace);
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut from = 0;
    while from < hay.len() {
        let Some(i) = hay[from..].find(trimmed) else {
            break;
        };
        let (s, e) = (from + i, from + i + trimmed.len());
        // `map` is indexed by the normalized text's char starts.
        let start = map[&s];
        let last_char_start = hay[..e]
            .char_indices()
            .next_back()
            .map(|(j, _)| j)
            .expect("non-empty match");
        let last_src = map[&last_char_start];
        let end = last_src + text[last_src..].chars().next().map_or(0, char::len_utf8);
        found.push(start..end);
        from = e;
    }
    found
}

/// Collapses whitespace runs to one space and straightens quotes. The map takes each normalized
/// char's byte offset to the byte offset of the source char it came from.
fn normalize(text: &str) -> (String, std::collections::HashMap<usize, usize>) {
    let mut out = String::with_capacity(text.len());
    let mut map = std::collections::HashMap::new();
    let mut last_was_space = false;
    for (i, mut c) in text.char_indices() {
        let is_space = matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{A0}');
        if is_space {
            if last_was_space {
                continue;
            }
            c = ' ';
        }
        c = match c {
            '\u{2018}' | '\u{2019}' | '\u{201B}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201F}' => '"',
            '\u{2013}' | '\u{2014}' => '-',
            other => other,
        };
        last_was_space = is_space;
        map.insert(out.len(), i);
        out.push(c);
    }
    (out, map)
}
