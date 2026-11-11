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

/// The code-fence state before a line: `Some(c)` inside a fence opened with `c` (a backtick
/// or a tilde), `None` outside one. It is the only parse state carried from line to line.
pub(crate) type Fence = Option<u8>;

/// Parses every line of `text`. Code-fence state is tracked across lines.
pub fn parse(text: &str) -> Vec<MarkdownLine> {
    let mut lines = Vec::new();
    parse_lines_from(text, 0, None, |line, _| lines.push(line), |_, _| false);
    lines
}

/// Parses the lines of `text` from the line starting at `location`, which starts with the
/// fence state `fence`. Each line goes to `emit` with the fence state before it. Before
/// parsing a line, `stop(location, fence)` may end the parse there.
pub(crate) fn parse_lines_from(
    text: &str,
    mut location: usize,
    mut fence: Fence,
    mut emit: impl FnMut(MarkdownLine, Fence),
    mut stop: impl FnMut(usize, Fence) -> bool,
) {
    let length = text.len();
    while location <= length {
        if stop(location, fence) {
            return;
        }
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
        let before = fence;
        let mut parsed = parse_line_in_fence(&text[line_range.clone()], location, &mut fence);
        parsed.range = line_range;
        emit(parsed, before);
        if line_end == location {
            return;
        }
        location = line_end;
    }
}

/// Parses a single line in isolation (no fence context). Ranges are offset by `offset`.
pub fn parse_line(line: &str, offset: usize) -> MarkdownLine {
    parse_line_in_fence(line, offset, &mut None)
}

fn parse_line_in_fence(line: &str, offset: usize, fence: &mut Fence) -> MarkdownLine {
    let full = 0..line.len();
    let shifted = |r: &Range<usize>| r.start + offset..r.end + offset;
    let line_range = offset..offset + line.len();
    let empty_indent = offset..offset;

    // Every fence starts with three backticks or tildes after at most three spaces or tabs.
    let fence_candidate = line.contains("```") || line.contains("~~~");
    if let Some(m) = fence_candidate.then(|| plain_matches(&FENCE, line, &full).into_iter().next()).flatten() {
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
        match *fence {
            None => {
                *fence = Some(marker.as_bytes()[0]);
                return fence_line(whole);
            }
            Some(open) if marker.as_bytes()[0] == open => {
                *fence = None;
                return fence_line(whole);
            }
            Some(_) => {}
        }
    }
    if fence.is_some() {
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
    parse_inlines_with(text, range, true)
}

/// `parse_inlines`, skipping each pattern whose literal the text lacks when `prefilter`.
fn parse_inlines_with(text: &str, range: Range<usize>, prefilter: bool) -> Vec<MarkdownInlineSpan> {
    let range = crate::offsets::floor_char_boundary(text, range.start)
        ..crate::offsets::floor_char_boundary(text, range.end);
    let mut spans: Vec<MarkdownInlineSpan> = Vec::new();
    let mut protected: Vec<Range<usize>> = Vec::new();
    let overlaps_protected =
        |p: &Vec<Range<usize>>, r: &Range<usize>| p.iter().any(|x| intersects(x, r));
    // Each pattern needs a literal that most prose lacks; without it the pattern can't match,
    // so it isn't run. (The patterns run on this slice, so their lookbehinds can't see past it.)
    let sub = &text[range.clone()];
    let has = |needle: &str| !prefilter || sub.contains(needle);
    let fancy_if = |ok: bool, re: &FancyRegex| if ok { fancy_matches(re, text, &range) } else { Vec::new() };
    let plain_if = |ok: bool, re: &Regex| if ok { plain_matches(re, text, &range) } else { Vec::new() };

    for m in fancy_if(has("`"), &CODE) {
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
    for m in plain_if(has("]("), &LINK) {
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
    for m in plain_if(has("<"), &AUTOLINK) {
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
    for m in fancy_if(has("http") || has("www."), &BARE_URL) {
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

    for m in fancy_if(has("***"), &TRIPLE) {
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
    for m in fancy_if(has("**") || has("__"), &STRONG) {
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
    for (regex, marker) in [(&*EMPHASIS_STAR, "*"), (&*EMPHASIS_UNDERSCORE, "_")] {
        for m in fancy_if(has(marker), regex) {
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
    for m in fancy_if(has("~~"), &STRIKE) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Skipping a pattern when its literal is missing finds exactly what running every
    /// pattern finds.
    #[test]
    fn prefiltered_inlines_match_running_every_pattern() {
        let pieces = [
            "a", "word ", " ", "*", "**", "***", "_", "__", "~", "~~", "`", "``", "[", "]", "(", ")", "](", "!", "<", ">",
            "http", "https://x.y/z", "www.", "mailto:", ":", ".", "\\", "\t", "é", "—", "x_y", "**b**", "*i*", "~~s~~",
            "`c`", "[l](u)", "<http://a>", "\u{a0}",
        ];
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        for _ in 0..20_000 {
            let line: String = (0..next(14)).map(|_| pieces[next(pieces.len())]).collect();
            let all = 0..line.len();
            assert_eq!(parse_inlines_with(&line, all.clone(), true), parse_inlines_with(&line, all, false), "{line:?}");
        }
    }
}
