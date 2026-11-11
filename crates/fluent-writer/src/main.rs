//! Spike: the writing surface on Rune.
use rune::tok::{Family, TextStyle};
use rune::{AppSpec, Block, Color, Cx, El, ScrollSpec, Span, TextEdit, col, text_input};

struct State {
    edit: TextEdit,
}

const SAMPLE: &str = "# A quiet place to write\n\nFluent Writer keeps one column of prose. This paragraph is long enough to wrap onto a second and a third line so the hanging indent is visible on every line it wraps to.\n\n## Lists\n\n- A bullet item that is also long enough to wrap so the wrapped line aligns under the text and not under the marker\n- Second item\n\n> A quote that hangs its marker too.\n\nA ~~struck~~ word, some **bold**, some *italic*.\n";

fn init() -> State {
    let mut edit = TextEdit::new(SAMPLE);
    edit.focused = true;
    State { edit }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    rune::color::hex(&format!("#{r:02x}{g:02x}{b:02x}")).unwrap()
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let size = 19.0;
    let style = TextStyle::new(size, 400.0).family(Family::Mono).lh((size * 1.55f32).round());
    let cw = size * 0.6;
    let hang = cw * 4.0;
    let (markup, ink) = (rgb(172, 170, 168), rgb(25, 25, 25));
    let mut blocks = Vec::new();
    let mut spans = Vec::new();
    let mut strikes = Vec::new();
    let mut at = 0;
    for line in s.edit.text.split('\n') {
        let marker = if line.starts_with("## ") { 3 } else if line.starts_with("# ") || line.starts_with("- ") || line.starts_with("> ") { 2 } else { 0 };
        blocks.push(Block::hang(hang - marker as f32 * cw, hang, if line.starts_with("## ") { 60.0 } else { 0.0 }));
        if marker > 0 { spans.push(Span::new(at, at + marker).color(markup)); }
        if line.starts_with('#') { spans.push(Span::new(at, at + line.len()).weight(700.0)); }
        if let (Some(a), Some(b)) = (line.find("~~"), line.rfind("~~")) && b > a { strikes.push((at + a + 2, at + b, Some(markup))); }
        at += line.len() + 1;
    }
    let field = text_input(&s.edit, style)
        .key("draft")
        .color(ink)
        .caret(rgb(0, 190, 255), rgb(178, 228, 250))
        .caret_size((size / 9.0).round().max(2.0), size * 1.3)
        .spans(spans)
        .paragraph_blocks(blocks)
        .input_strikes(strikes)
        .on_edit(|s: &mut State| &mut s.edit)
        .on_submit(|s: &mut State, _| s.edit.insert("\n"))
        .w(68.0 * cw + hang);
    col().w_full().h_full().fill(rgb(247, 247, 247)).child(
        col().key("scroll").w_full().h_full().scroll(ScrollSpec { stick_bottom: false }).child(col().w_full().items_center().padding(56.0).child(field)),
    )
}

fn main() {
    rune::run(AppSpec::<State> {
        fonts: &[
            include_bytes!("../assets/fonts/IBMPlexMono-Regular.otf"),
            include_bytes!("../assets/fonts/IBMPlexMono-Italic.otf"),
            include_bytes!("../assets/fonts/IBMPlexMono-Bold.otf"),
            include_bytes!("../assets/fonts/IBMPlexMono-BoldItalic.otf"),
            include_bytes!("../assets/fonts/IBMPlexMono-Medium.otf"),
        ],
        families: ("IBM Plex Mono", "IBM Plex Mono"),
        ..AppSpec::new("Fluent Writer", (1060.0, 760.0), init, view)
    });
}
