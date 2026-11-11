//! A line-oriented Markdown parser that keeps every delimiter's range, so the editor can style
//! the source in place (Swift `MarkdownParser`).
//!
//! The regular expressions mirror the Swift `NSRegularExpression` patterns. ICU's `\s` is
//! `[\t\n\f\r\p{Z}]` and its `.` excludes every line terminator, so those are spelled out below
//! instead of using the Rust engines' slightly different defaults.

use std::ops::Range;
use std::sync::LazyLock;

use fancy_regex::Regex as FancyRegex;
use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkdownBlockKind {
    Paragraph,
    Blank,
    Heading { level: usize },
    BulletItem,
    OrderedItem { number: u32, delimiter: char },
    TaskItem { checked: bool },
    Blockquote,
    CodeFence,
    CodeBlock,
    HorizontalRule,
}

impl MarkdownBlockKind {
    pub fn is_list_item(&self) -> bool {
        matches!(
            self,
            Self::BulletItem | Self::OrderedItem { .. } | Self::TaskItem { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkdownInlineKind {
    Strong,
    Emphasis,
    Strikethrough,
    Code,
    Link,
    Autolink,
    BareUrl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownInlineSpan {
    pub kind: MarkdownInlineKind,
    /// Full range including delimiters.
    pub range: Range<usize>,
    /// The visible text range (for links, the link text).
    pub content_range: Range<usize>,
    /// Delimiter ranges (e.g. `**`, `[`, `](url)`).
    pub marker_ranges: Vec<Range<usize>>,
    /// For links and URLs, the range of the destination.
    pub url_range: Option<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownLine {
    /// Range of the line, excluding its line terminator.
    pub range: Range<usize>,
    pub kind: MarkdownBlockKind,
    /// Leading indentation (spaces/tabs before the marker).
    pub indent_range: Range<usize>,
    /// The block marker including its trailing whitespace, e.g. `## `, `- `, `1. `, `> `, `- [ ] `.
    pub marker_range: Option<Range<usize>>,
    /// Text after the marker.
    pub content_range: Range<usize>,
    pub inlines: Vec<MarkdownInlineSpan>,
}

// ICU character classes, spelled out for the Rust engines.
macro_rules! ws {
    () => {
        r"\t\n\x0C\r\p{Z}"
    };
}
macro_rules! dot {
    () => {
        r"[^\n\x0B\x0C\r\x{85}\x{2028}\x{2029}]"
    };
}

fn re(p: &str) -> Regex {
    // Patterns are compile-time constants.
    Regex::new(p).expect("valid pattern")
}

fn fancy(p: &str) -> FancyRegex {
    FancyRegex::new(p).expect("valid pattern")
}

static HEADING: LazyLock<Regex> = LazyLock::new(|| re(r"^(#{1,6})(?:[ \t]+|$)"));
static TASK: LazyLock<Regex> =
    LazyLock::new(|| re(r"^([ \t]*)([-*+])[ \t]+\[([ xX])\](?:[ \t]+|$)"));
static BULLET: LazyLock<Regex> = LazyLock::new(|| re(r"^([ \t]*)([-*+])(?:[ \t]+)"));
static ORDERED: LazyLock<Regex> = LazyLock::new(|| re(r"^([ \t]*)(\d{1,9})([.)])(?:[ \t]+)"));
static QUOTE: LazyLock<Regex> = LazyLock::new(|| re(r"^(?:>[ \t]?)+"));
// Swift: ^[ \t]{0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$ — the backreference expanded per character.
static RULE: LazyLock<Regex> = LazyLock::new(|| {
    re(r"^[ \t]{0,3}(?:-(?:[ \t]*-){2,}|\*(?:[ \t]*\*){2,}|_(?:[ \t]*_){2,})[ \t]*$")
});
static FENCE: LazyLock<Regex> = LazyLock::new(|| re(r"^[ \t]{0,3}(`{3,}|~{3,})"));

static CODE: LazyLock<FancyRegex> =
    LazyLock::new(|| fancy(concat!(r"(`+)(?!`)(", dot!(), r"+?)(?<!`)\1(?!`)")));
static LINK: LazyLock<Regex> = LazyLock::new(|| {
    re(concat!(
        r#"(!?)\[([^\]\n]*)\]\(([^)"#,
        ws!(),
        r#"]+)(?:[ \t]+"[^"\n]*")?\)"#
    ))
});
static AUTOLINK: LazyLock<Regex> =
    LazyLock::new(|| re(concat!(r"<((?:https?|mailto):[^>", ws!(), r"]+)>")));
static BARE_URL: LazyLock<FancyRegex> = LazyLock::new(|| {
    fancy(concat!(
        r"(?<![\w(\[<])(?:https?://|www\.)[^",
        ws!(),
        r"<>]*[^",
        ws!(),
        r#"<>.,;:!?'")\]*_~]"#
    ))
});
static TRIPLE: LazyLock<FancyRegex> = LazyLock::new(|| {
    fancy(concat!(
        r"(?<!\*)\*\*\*(?=[^",
        ws!(),
        r"*])(",
        dot!(),
        r"+?)(?<=[^",
        ws!(),
        r"*])\*\*\*(?!\*)"
    ))
});
static STRONG: LazyLock<FancyRegex> = LazyLock::new(|| {
    fancy(concat!(
        r"(\*\*|__)(?=[^",
        ws!(),
        r"])(",
        dot!(),
        r"+?)(?<=[^",
        ws!(),
        r"])\1"
    ))
});
static EMPHASIS_STAR: LazyLock<FancyRegex> = LazyLock::new(|| {
    fancy(concat!(
        r"(?<![\*\\])\*(?=[^",
        ws!(),
        r"\*])(",
        dot!(),
        r"+?)(?<=[^",
        ws!(),
        r"\*\\])\*(?!\*)"
    ))
});
static EMPHASIS_UNDERSCORE: LazyLock<FancyRegex> = LazyLock::new(|| {
    fancy(concat!(
        r"(?<![\w_\\])_(?=[^",
        ws!(),
        r"_])(",
        dot!(),
        r"+?)(?<=[^",
        ws!(),
        r"_\\])_(?![\w_])"
    ))
});
static STRIKE: LazyLock<FancyRegex> = LazyLock::new(|| {
    fancy(concat!(
        r"~~(?=[^",
        ws!(),
        r"])(",
        dot!(),
        r"+?)(?<=[^",
        ws!(),
        r"])~~"
    ))
});

/// A match's groups as byte ranges offset into the full text.
struct Match {
    groups: Vec<Option<Range<usize>>>,
}

impl Match {
    fn range(&self) -> Range<usize> {
        self.groups[0].clone().expect("group 0")
    }
    fn group(&self, i: usize) -> Range<usize> {
        // NSRegularExpression reports an unmatched group as {NSNotFound, 0}; every group used here
        // always participates, so an empty range at the match start is a safe stand-in.
        self.groups
            .get(i)
            .cloned()
            .flatten()
            .unwrap_or(self.range().start..self.range().start)
    }
}

fn plain_matches(re: &Regex, text: &str, range: &Range<usize>) -> Vec<Match> {
    let sub = &text[range.clone()];
    re.captures_iter(sub)
        .map(|c| Match {
            groups: (0..c.len())
                .map(|i| {
                    c.get(i)
                        .map(|m| m.start() + range.start..m.end() + range.start)
                })
                .collect(),
        })
        .collect()
}

fn fancy_matches(re: &FancyRegex, text: &str, range: &Range<usize>) -> Vec<Match> {
    let sub = &text[range.clone()];
    re.captures_iter(sub)
        // A pathological backtracking limit is the only possible error; Swift would simply not match.
        .filter_map(|c| c.ok())
        .map(|c| Match {
            groups: (0..c.len())
                .map(|i| {
                    c.get(i)
                        .map(|m| m.start() + range.start..m.end() + range.start)
                })
                .collect(),
        })
        .collect()
}

/// NSString's line terminators (`getLineStart`): LF, CR, CRLF, NEL, U+2028, U+2029.
pub(crate) fn is_line_separator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// NSString's paragraph separators (`getParagraphStart`): LF, CR, CRLF, U+2029.
pub(crate) fn is_paragraph_separator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2029}')
}

/// The Swift `isNewline(unichar)` helper: LF, CR, U+2028, U+2029.
pub(crate) fn is_newline_unit(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `(contentsEnd, end)` of the line or paragraph starting at or containing `from`, scanning forward.
pub(crate) fn terminator_after(
    text: &str,
    from: usize,
    is_sep: fn(char) -> bool,
) -> (usize, usize) {
    for (i, c) in text[from..].char_indices() {
        if is_sep(c) {
            let at = from + i;
            let mut end = at + c.len_utf8();
            if c == '\r' && text[end..].starts_with('\n') {
                end += 1;
            }
            return (at, end);
        }
    }
    (text.len(), text.len())
}

/// Parses every line of `text`. Code-fence state is tracked across lines.
pub fn parse(text: &str) -> Vec<MarkdownLine> {
    let mut lines = Vec::new();
    let mut in_fence = false;
    let mut fence_marker = String::new();
    let mut location = 0;
    let length = text.len();
    while location <= length {
        let (contents_end, line_end) = if location == length {
            // Trailing empty line after a final newline (or empty document).
            if location > 0
                && !text[..location]
                    .chars()
                    .next_back()
                    .is_some_and(is_newline_unit)
            {
                break;
            }
            (length, length)
        } else {
            terminator_after(text, location, is_line_separator)
        };
        let line_range = location..contents_end;
        let mut parsed = parse_line_in_fence(
            &text[line_range.clone()],
            location,
            &mut in_fence,
            &mut fence_marker,
        );
        parsed.range = line_range;
        lines.push(parsed);
        if line_end == location {
            break;
        }
        location = line_end;
    }
    lines
}

/// Parses a single line in isolation (no fence context). Ranges are offset by `offset`.
pub fn parse_line(line: &str, offset: usize) -> MarkdownLine {
    let mut in_fence = false;
    let mut marker = String::new();
    parse_line_in_fence(line, offset, &mut in_fence, &mut marker)
}

fn parse_line_in_fence(
    line: &str,
    offset: usize,
    in_fence: &mut bool,
    fence_marker: &mut String,
) -> MarkdownLine {
    let full = 0..line.len();
    let shifted = |r: &Range<usize>| r.start + offset..r.end + offset;
    let line_range = offset..offset + line.len();
    let empty_indent = offset..offset;

    if let Some(m) = plain_matches(&FENCE, line, &full).into_iter().next() {
        let marker = &line[m.group(1)];
        let whole = m.range();
        let fence_line = |r: Range<usize>| MarkdownLine {
            range: line_range.clone(),
            kind: MarkdownBlockKind::CodeFence,
            indent_range: empty_indent.clone(),
            marker_range: Some(shifted(&r)),
            content_range: shifted(&(r.end..line.len())),
            inlines: Vec::new(),
        };
        if !*in_fence {
            *in_fence = true;
            *fence_marker = marker[..1].to_string();
            return fence_line(whole);
        } else if marker.starts_with(fence_marker.as_str()) {
            *in_fence = false;
            return fence_line(whole);
        }
    }
    if *in_fence {
        return MarkdownLine {
            range: line_range.clone(),
            kind: MarkdownBlockKind::CodeBlock,
            indent_range: empty_indent,
            marker_range: None,
            content_range: line_range,
            inlines: Vec::new(),
        };
    }

    if crate::charsets::trim(line, crate::charsets::is_whitespace).is_empty() {
        return MarkdownLine {
            range: line_range.clone(),
            kind: MarkdownBlockKind::Blank,
            indent_range: empty_indent,
            marker_range: None,
            content_range: line_range,
            inlines: Vec::new(),
        };
    }
    if RULE.is_match(line) {
        return MarkdownLine {
            range: line_range.clone(),
            kind: MarkdownBlockKind::HorizontalRule,
            indent_range: empty_indent,
            marker_range: Some(line_range.clone()),
            content_range: line_range.end..line_range.end,
            inlines: Vec::new(),
        };
    }

    let mut kind = MarkdownBlockKind::Paragraph;
    let mut indent = 0..0;
    let mut marker: Option<Range<usize>> = None;
    if let Some(m) = plain_matches(&HEADING, line, &full).into_iter().next() {
        kind = MarkdownBlockKind::Heading {
            level: m.group(1).len(),
        };
        marker = Some(m.range());
    } else if let Some(m) = plain_matches(&TASK, line, &full).into_iter().next() {
        let check = &line[m.group(3)];
        kind = MarkdownBlockKind::TaskItem {
            checked: check.eq_ignore_ascii_case("x"),
        };
        indent = m.group(1);
        marker = Some(indent.end..m.range().end);
    } else if let Some(m) = plain_matches(&BULLET, line, &full).into_iter().next() {
        kind = MarkdownBlockKind::BulletItem;
        indent = m.group(1);
        marker = Some(indent.end..m.range().end);
    } else if let Some(m) = plain_matches(&ORDERED, line, &full).into_iter().next() {
        let number = line[m.group(2)].parse::<u32>().unwrap_or(1);
        let delimiter = line[m.group(3)].chars().next().unwrap_or('.');
        kind = MarkdownBlockKind::OrderedItem { number, delimiter };
        indent = m.group(1);
        marker = Some(indent.end..m.range().end);
    } else if let Some(m) = plain_matches(&QUOTE, line, &full).into_iter().next() {
        kind = MarkdownBlockKind::Blockquote;
        marker = Some(m.range());
    }

    let content_start = marker.as_ref().map_or(0, |m| m.end);
    let content = content_start..line.len();
    let inlines = parse_inlines(line, content.clone())
        .into_iter()
        .map(|mut s| {
            s.range = shifted(&s.range);
            s.content_range = shifted(&s.content_range);
            s.marker_ranges = s.marker_ranges.iter().map(shifted).collect();
            s.url_range = s.url_range.as_ref().map(shifted);
            s
        })
        .collect();
    MarkdownLine {
        range: line_range,
        kind,
        indent_range: if indent.is_empty() {
            offset..offset
        } else {
            shifted(&indent)
        },
        marker_range: marker.as_ref().map(shifted),
        content_range: shifted(&content),
        inlines,
    }
}

fn intersects(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start.max(b.start) < a.end.min(b.end)
}

/// Finds inline spans in `text` within `range`. Ranges are relative to `text`.
pub fn parse_inlines(text: &str, range: Range<usize>) -> Vec<MarkdownInlineSpan> {
    let range = crate::offsets::floor_char_boundary(text, range.start)
        ..crate::offsets::floor_char_boundary(text, range.end);
    let mut spans: Vec<MarkdownInlineSpan> = Vec::new();
    let mut protected: Vec<Range<usize>> = Vec::new();
    let overlaps_protected =
        |p: &Vec<Range<usize>>, r: &Range<usize>| p.iter().any(|x| intersects(x, r));

    for m in fancy_matches(&CODE, text, &range) {
        let r = m.range();
        let ticks = m.group(1).len();
        let open = r.start..r.start + ticks;
        let close = r.end - ticks..r.end;
        spans.push(MarkdownInlineSpan {
            kind: MarkdownInlineKind::Code,
            range: r.clone(),
            content_range: m.group(2),
            marker_ranges: vec![open, close],
            url_range: None,
        });
        protected.push(r);
    }
    for m in plain_matches(&LINK, text, &range) {
        let r = m.range();
        if overlaps_protected(&protected, &r) {
            continue;
        }
        let bang = m.group(1);
        let text_range = m.group(2);
        let url = m.group(3);
        let open = r.start..r.start + bang.len() + 1;
        let close = text_range.end..r.end;
        spans.push(MarkdownInlineSpan {
            kind: MarkdownInlineKind::Link,
            range: r,
            content_range: text_range,
            marker_ranges: vec![open, close.clone()],
            url_range: Some(url),
        });
        // Only the destination is protected; the link text may carry emphasis.
        protected.push(close);
    }
    for m in plain_matches(&AUTOLINK, text, &range) {
        let r = m.range();
        if overlaps_protected(&protected, &r) {
            continue;
        }
        spans.push(MarkdownInlineSpan {
            kind: MarkdownInlineKind::Autolink,
            range: r.clone(),
            content_range: m.group(1),
            marker_ranges: vec![r.start..r.start + 1, r.end - 1..r.end],
            url_range: Some(m.group(1)),
        });
        protected.push(r);
    }
    for m in fancy_matches(&BARE_URL, text, &range) {
        let r = m.range();
        if overlaps_protected(&protected, &r) {
            continue;
        }
        spans.push(MarkdownInlineSpan {
            kind: MarkdownInlineKind::BareUrl,
            range: r.clone(),
            content_range: r.clone(),
            marker_ranges: Vec::new(),
            url_range: Some(r.clone()),
        });
        protected.push(r);
    }

    for m in fancy_matches(&TRIPLE, text, &range) {
        let r = m.range();
        if overlaps_protected(&protected, &r) {
            continue;
        }
        let inner = r.start + 2..r.end - 2;
        spans.push(MarkdownInlineSpan {
            kind: MarkdownInlineKind::Strong,
            range: r.clone(),
            content_range: inner.clone(),
            marker_ranges: vec![r.start..r.start + 2, r.end - 2..r.end],
            url_range: None,
        });
        spans.push(MarkdownInlineSpan {
            kind: MarkdownInlineKind::Emphasis,
            range: inner.clone(),
            content_range: m.group(1),
            marker_ranges: vec![inner.start..inner.start + 1, inner.end - 1..inner.end],
            url_range: None,
        });
        protected.push(r.start..r.start + 3);
        protected.push(r.end - 3..r.end);
    }
    let mut strong_ranges: Vec<Range<usize>> = Vec::new();
    for m in fancy_matches(&STRONG, text, &range) {
        let r = m.range();
        if overlaps_protected(&protected, &r) {
            continue;
        }
        let d = m.group(1).len();
        let open = r.start..r.start + d;
        let close = r.end - d..r.end;
        spans.push(MarkdownInlineSpan {
            kind: MarkdownInlineKind::Strong,
            range: r,
            content_range: m.group(2),
            marker_ranges: vec![open.clone(), close.clone()],
            url_range: None,
        });
        strong_ranges.push(open);
        strong_ranges.push(close);
    }
    let overlaps_strong_marker = |r: &Range<usize>| strong_ranges.iter().any(|x| intersects(x, r));
    for regex in [&*EMPHASIS_STAR, &*EMPHASIS_UNDERSCORE] {
        for m in fancy_matches(regex, text, &range) {
            let r = m.range();
            if overlaps_protected(&protected, &r) {
                continue;
            }
            let open = r.start..r.start + 1;
            let close = r.end - 1..r.end;
            if overlaps_strong_marker(&open) || overlaps_strong_marker(&close) {
                continue;
            }
            spans.push(MarkdownInlineSpan {
                kind: MarkdownInlineKind::Emphasis,
                range: r,
                content_range: m.group(1),
                marker_ranges: vec![open, close],
                url_range: None,
            });
        }
    }
    for m in fancy_matches(&STRIKE, text, &range) {
        let r = m.range();
        if overlaps_protected(&protected, &r) {
            continue;
        }
        spans.push(MarkdownInlineSpan {
            kind: MarkdownInlineKind::Strikethrough,
            range: r.clone(),
            content_range: m.group(1),
            marker_ranges: vec![r.start..r.start + 2, r.end - 2..r.end],
            url_range: None,
        });
    }
    spans.sort_by_key(|s| s.range.start);
    spans
}
