//! Titles and file names derived from a draft's text (Swift `TitleDeriver`).

use unicode_segmentation::UnicodeSegmentation;

use crate::charsets::{is_control, is_whitespace};
use crate::markdown::{self, MarkdownBlockKind};
use crate::plain_text_exporter;

pub const UNTITLED: &str = "Untitled";

/// Splits on `\n` the way Swift's `split(separator: "\n")` does on a `String`: a CRLF pair is one
/// `Character`, so it is not a split point. Empty pieces are omitted.
fn split_lf_characters(body: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = body.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' && !(i > 0 && bytes[i - 1] == b'\r') {
            if i > start {
                out.push(&body[start..i]);
            }
            start = i + 1;
        }
    }
    if start < body.len() {
        out.push(&body[start..]);
    }
    out
}

/// A local title derived from the first nonempty line of the body.
pub fn derived_title(body: &str) -> String {
    derived_title_with_max(body, 60)
}

pub fn derived_title_with_max(body: &str, max_length: usize) -> String {
    for raw in split_lf_characters(body) {
        let line = markdown::parse_line(raw, 0);
        if matches!(
            line.kind,
            MarkdownBlockKind::CodeFence
                | MarkdownBlockKind::HorizontalRule
                | MarkdownBlockKind::Blank
        ) {
            continue;
        }
        let rendered =
            plain_text_exporter::render_inline_fragment(&raw[line.content_range.clone()]);
        let collapsed = rendered
            .split(char::is_whitespace)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if collapsed.is_empty() {
            continue;
        }
        return truncate(&collapsed, max_length);
    }
    UNTITLED.to_string()
}

fn truncate(s: &str, max_length: usize) -> String {
    let graphemes: Vec<&str> = s.graphemes(true).collect();
    if graphemes.len() <= max_length {
        return s.to_string();
    }
    let cut = &graphemes[..max_length];
    if let Some(space) = cut.iter().rposition(|g| *g == " ")
        && space > max_length / 2
    {
        let kept: String = cut[..space].concat();
        return kept
            .trim_matches(|c| matches!(c, ' ' | ',' | ';' | ':' | '-'))
            .to_string();
    }
    cut.concat()
}

/// A string safe to use as a file name on macOS. On Windows, the characters Windows forbids in
/// file names (`\ * ? " < > |`) are also replaced with `-`.
pub fn file_name_stem(title: &str) -> String {
    let mut s = title.replace(['/', ':'], "-");
    if cfg!(windows) {
        s = s.replace(['\\', '*', '?', '"', '<', '>', '|'], "-");
    }
    s = s.chars().filter(|&c| !is_control(c)).collect();
    s = s
        .trim_matches(|c: char| is_whitespace(c) || c == '.')
        .to_string();
    if s.is_empty() {
        s = UNTITLED.to_string();
    }
    if s.len() > 200 {
        s = s.graphemes(true).take(120).collect();
    }
    s
}
