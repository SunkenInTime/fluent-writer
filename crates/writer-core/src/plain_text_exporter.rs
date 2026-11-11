//! Converts a Markdown draft into clean plain text suitable for pasting into a post composer.
//! Delimiters are removed, links keep their URLs, lists and paragraph breaks survive.
//! No decorative Unicode styling is ever produced.

use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

use crate::charsets::{is_whitespace, trim};
use crate::markdown::{
    self, MarkdownBlockKind, MarkdownInlineKind, MarkdownInlineSpan, MarkdownLine,
};

static ESCAPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\([\\`*_{}\[\]()#+\-.!~>|])").expect("valid pattern"));

pub fn export(markdown: &str) -> String {
    let lines = markdown::parse(markdown);
    let output = lines.iter().filter_map(|line| export_line(markdown, line));
    // Collapse runs of blank lines and trim the ends.
    let mut collapsed: Vec<String> = Vec::new();
    for l in output {
        if l.is_empty() && collapsed.last().is_none_or(|p| p.is_empty()) {
            continue;
        }
        collapsed.push(l);
    }
    while collapsed.last().is_some_and(|l| l.is_empty()) {
        collapsed.pop();
    }
    collapsed.join("\n")
}

/// One parsed line of `markdown` as plain text, before blank lines are collapsed; `None` for a
/// line that leaves nothing (a code fence).
pub(crate) fn export_line(markdown: &str, line: &MarkdownLine) -> Option<String> {
    Some(match line.kind {
        MarkdownBlockKind::CodeFence => return None,
        MarkdownBlockKind::CodeBlock => markdown[line.range.clone()].to_string(),
        MarkdownBlockKind::Blank | MarkdownBlockKind::HorizontalRule => String::new(),
        MarkdownBlockKind::Heading { .. }
        | MarkdownBlockKind::Paragraph
        | MarkdownBlockKind::Blockquote => {
            trimming_trailing_whitespace(&render_line(markdown, line)).to_string()
        }
        MarkdownBlockKind::BulletItem | MarkdownBlockKind::TaskItem { .. } => format!(
            "{}• {}",
            indent(markdown, line),
            trimming_trailing_whitespace(&render_line(markdown, line))
        ),
        MarkdownBlockKind::OrderedItem { number, delimiter } => format!(
            "{}{}{} {}",
            indent(markdown, line),
            number,
            delimiter,
            trimming_trailing_whitespace(&render_line(markdown, line))
        ),
    })
}

fn indent(text: &str, line: &MarkdownLine) -> String {
    if line.indent_range.is_empty() {
        return String::new();
    }
    let raw = text[line.indent_range.clone()].replace('\t', "    ");
    let level = raw.chars().count() / 2;
    "  ".repeat(level)
}

fn render_line(text: &str, line: &MarkdownLine) -> String {
    render_inline(text, line.content_range.clone(), &line.inlines)
}

/// Renders an arbitrary single-line Markdown fragment.
pub fn render_inline_fragment(text: &str) -> String {
    let range = 0..text.len();
    render_inline(text, range.clone(), &markdown::parse_inlines(text, range))
}

fn intersects(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start.max(b.start) < a.end.min(b.end)
}

fn render_inline(text: &str, range: Range<usize>, spans: &[MarkdownInlineSpan]) -> String {
    let mut replacements: Vec<(Range<usize>, String)> = Vec::new();
    let mut claimed: Vec<Range<usize>> = Vec::new();
    let is_claimed = |claimed: &Vec<Range<usize>>, r: &Range<usize>| {
        claimed
            .iter()
            .any(|c| intersects(c, r) || (c.start <= r.start && r.end <= c.end && r.is_empty()))
    };

    let code_ranges: Vec<Range<usize>> = spans
        .iter()
        .filter(|s| s.kind == MarkdownInlineKind::Code)
        .map(|s| s.range.clone())
        .collect();
    for span in spans.iter().filter(|s| s.kind == MarkdownInlineKind::Link) {
        // Images don't survive as plain text.
        if text.as_bytes().get(span.range.start) == Some(&b'!') {
            replacements.push((span.range.clone(), String::new()));
            claimed.push(span.range.clone());
            continue;
        }
        let label = render_inline_fragment(&text[span.content_range.clone()]);
        let url = span
            .url_range
            .as_ref()
            .map(|r| &text[r.clone()])
            .unwrap_or("");
        let trimmed_label = trim(&label, is_whitespace);
        let replacement = if trimmed_label.is_empty()
            || trimmed_label == url
            || format!("https://{trimmed_label}") == url
            || format!("http://{trimmed_label}") == url
            || trimmed_label.starts_with("http")
        {
            url.to_string()
        } else {
            format!("{label} ({url})")
        };
        replacements.push((span.range.clone(), replacement));
        claimed.push(span.range.clone());
    }
    for span in spans.iter().filter(|s| s.kind != MarkdownInlineKind::Link) {
        if is_claimed(&claimed, &span.range) {
            continue;
        }
        match span.kind {
            MarkdownInlineKind::BareUrl => continue,
            MarkdownInlineKind::Code => {
                for m in &span.marker_ranges {
                    replacements.push((m.clone(), String::new()));
                }
            }
            _ => {
                if code_ranges.iter().any(|c| intersects(c, &span.range)) {
                    continue;
                }
                for m in &span.marker_ranges {
                    replacements.push((m.clone(), String::new()));
                }
            }
        }
    }
    // Apply replacements back to front. Duplicate ranges keep the first replacement given.
    // (Swift deduplicated through a Dictionary, so the order among replacements starting at the same
    // offset was unspecified; here it is the order they were produced.)
    let mut unique: Vec<(Range<usize>, String)> = Vec::new();
    for (r, s) in replacements {
        if !unique.iter().any(|(u, _)| *u == r) {
            unique.push((r, s));
        }
    }
    unique.sort_by_key(|u| std::cmp::Reverse(u.0.start));
    let mut out = text[range.clone()].to_string();
    let mut last_start = usize::MAX;
    for (r, s) in unique {
        if r.end > last_start {
            continue;
        }
        if r.start < range.start {
            continue;
        }
        let local = r.start - range.start..r.end - range.start;
        if local.end > out.len() {
            continue;
        }
        out.replace_range(local, &s);
        last_start = r.start;
    }
    if code_ranges.is_empty() {
        return ESCAPE.replace_all(&out, "$1").into_owned();
    }
    out
}

/// Removes trailing spaces and tabs.
pub(crate) fn trimming_trailing_whitespace(s: &str) -> &str {
    s.trim_end_matches([' ', '\t'])
}
