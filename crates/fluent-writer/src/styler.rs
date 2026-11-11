//! Markdown presentation: one monospaced size, bold headings, grey markup, and block markers
//! that hang into the left margin so prose stays on one vertical edge.

use std::ops::Range;

use rune::{Block, Color, Span};
use writer_core::{MarkdownBlockKind as Kind, MarkdownInlineKind as Inline, MarkdownInlineSpan, MarkdownLine};

use crate::theme::{CHARACTER_WIDTH_EM, LINE_HEIGHT_MULTIPLE, Palette};

/// Type metrics for the draft at one text size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub font_size: f32,
    pub line_height: f32,
    pub character_width: f32,
}

impl Metrics {
    pub fn new(font_size: f32) -> Metrics {
        Metrics { font_size, line_height: (font_size * LINE_HEIGHT_MULTIPLE).round(), character_width: font_size * CHARACTER_WIDTH_EM }
    }

    /// Width reserved left of the prose edge for hanging markers like `## ` and `- `.
    pub fn hang_width(&self) -> f32 {
        self.character_width * 4.0
    }

    pub fn caret_width(&self) -> f32 {
        (self.font_size / 9.0).round().max(2.0)
    }

    /// The caret is shorter than the line: the line height includes leading it shouldn't cover.
    pub fn caret_height(&self) -> f32 {
        // AppKit's default line height for IBM Plex Mono is 1.3 em; the Swift caret is 1.12 of that.
        (self.font_size * 1.3 * 1.12).round()
    }
}

/// What the text field draws over the plain draft text.
#[derive(Clone, Debug, Default)]
pub struct Styled {
    pub spans: Vec<Span>,
    pub blocks: Vec<Block>,
    pub strikes: Vec<(usize, usize, Option<Color>)>,
}

fn chars(text: &str, r: &Option<Range<usize>>) -> f32 {
    r.as_ref().and_then(|r| text.get(r.clone())).map_or(0.0, |s| s.encode_utf16().count() as f32)
}

/// Styles every line. `space_after[i]` is the room to leave under line `i` (an image preview).
/// A line with room under it is an image line: its Markdown reads as a caption-like label
/// rather than prose.
pub fn style(text: &str, lines: &[MarkdownLine], m: &Metrics, p: &Palette, space_after: &dyn Fn(usize) -> f32) -> Styled {
    let mut out = Styled::default();
    let hang = m.hang_width();
    let cw = m.character_width;
    for (i, line) in lines.iter().enumerate() {
        let after = space_after(i);
        let marker_w = chars(text, &line.marker_range) * cw;
        let color = |out: &mut Styled, r: &Range<usize>, c: Color| {
            if r.start < r.end {
                out.spans.push(Span::new(r.start, r.end).color(c));
            }
        };
        match line.kind {
            Kind::Heading { .. } => {
                if line.range.start < line.range.end {
                    out.spans.push(Span::new(line.range.start, line.range.end).weight(700.0));
                }
                if let Some(mr) = &line.marker_range {
                    color(&mut out, mr, p.markup);
                }
                out.blocks.push(Block::hang((hang - marker_w).max(0.0), hang, after));
            }
            Kind::BulletItem | Kind::OrderedItem { .. } | Kind::TaskItem { .. } | Kind::Blockquote => {
                if let Some(mr) = &line.marker_range {
                    color(&mut out, mr, p.markup);
                }
                let indent_w = chars(text, &Some(line.indent_range.clone())) * cw;
                let first = (hang - marker_w).max(0.0);
                out.blocks.push(Block::hang(first, first + indent_w + marker_w, after));
            }
            Kind::CodeFence | Kind::HorizontalRule => {
                color(&mut out, &line.range, p.markup);
                out.blocks.push(Block::hang(hang, hang, after));
                continue;
            }
            Kind::CodeBlock => {
                color(&mut out, &line.range, p.quiet);
                out.blocks.push(Block::hang(hang, hang, after));
                continue;
            }
            Kind::Paragraph | Kind::Blank => out.blocks.push(Block::hang(hang, hang, after)),
        }
        for span in &line.inlines {
            style_inline(span, p, &mut out);
        }
        if after > 0.0 && line.range.start < line.range.end {
            out.spans.push(Span::new(line.range.start, line.range.end).color(p.quiet));
        }
    }
    out
}

fn style_inline(span: &MarkdownInlineSpan, p: &Palette, out: &mut Styled) {
    let (r, c) = (&span.range, &span.content_range);
    match span.kind {
        Inline::Strong => out.spans.push(Span::new(r.start, r.end).weight(700.0)),
        Inline::Emphasis => out.spans.push(Span::new(r.start, r.end).italic()),
        Inline::Strikethrough => out.strikes.push((c.start, c.end, Some(p.quiet))),
        Inline::Code => out.spans.push(Span::new(r.start, r.end).bg(p.code_background, 0.0)),
        Inline::Link => {
            out.spans.push(Span::new(c.start, c.end).underline(p.markup));
            if let Some(u) = &span.url_range {
                out.spans.push(Span::new(u.start, u.end).color(p.markup));
            }
        }
        Inline::Autolink | Inline::BareUrl => out.spans.push(Span::new(c.start, c.end).underline(p.markup)),
    }
    for mr in &span.marker_ranges {
        if mr.start < mr.end {
            out.spans.push(Span::new(mr.start, mr.end).color(p.markup));
        }
    }
}

/// Spans that dim everything outside `focus`.
pub fn dimming(len: usize, focus: &Range<usize>, p: &Palette) -> Vec<Span> {
    let mut out = Vec::new();
    if focus.start > 0 {
        out.push(Span::new(0, focus.start.min(len)).color(p.dimmed));
    }
    if focus.end < len {
        out.push(Span::new(focus.end, len).color(p.dimmed));
    }
    out
}
