//! Image previews under image lines. The Markdown stays in the text; a preview only occupies
//! the space left under its line, and follows the line as the text reflows.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::SystemTime;

use rune::{Align, Cx, El, Image, ImageCache, PopupItem, PopupMenu, TextHighlight, Tx, Waker, el, text};
use writer_core::{ImageSource, MarkdownImage, MarkdownLine, markdown_images};

use crate::app::{App, DRAFT};
use crate::theme::{Palette, ui_font};

/// What a preview shows.
#[derive(Clone)]
pub enum Content {
    /// A decoded image and its size in points.
    Image(Image, (f32, f32)),
    Placeholder { symbol: Symbol, message: String },
}

/// The placeholder's icon, named for the SF Symbol the Swift app used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    /// `photo`
    Photo,
    /// `questionmark.square.dashed`
    Unreadable,
    /// `photo.badge.exclamationmark`
    Missing,
    /// `wifi.exclamationmark`
    Offline,
}

#[derive(Clone)]
pub struct Block {
    pub image: MarkdownImage,
    /// Index of the image's line among the parsed lines.
    pub line: usize,
    pub source: Option<ImageSource>,
    pub content: Content,
    /// Size the preview draws at, in points.
    pub size: (f32, f32),
}

struct Cached {
    image: Image,
    points: (f32, f32),
    modified: Option<SystemTime>,
    max_pixel: f32,
}

/// The geometry previews are sized against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    /// Widest an image may be: the prose column.
    pub column_width: f32,
    pub max_height: f32,
    /// Device pixels per point.
    pub scale: f32,
}

pub struct Images {
    pub blocks: Vec<Block>,
    cache: Option<std::rc::Rc<ImageCache>>,
    local: HashMap<PathBuf, Cached>,
    web: HashMap<String, Cached>,
    failed: HashMap<String, String>,
    loading: HashSet<String>,
    fetched: (Sender<Fetched>, Receiver<Fetched>),
    waker: Option<Waker>,
}

/// A web image's URL and its bytes, or why they couldn't be fetched.
type Fetched = (String, Result<Vec<u8>, String>);

impl Default for Images {
    fn default() -> Self {
        Images { blocks: Vec::new(), cache: None, local: HashMap::new(), web: HashMap::new(), failed: HashMap::new(), loading: HashSet::new(), fetched: channel(), waker: None }
    }
}

fn host(url: &str) -> Option<&str> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?.split(':').next()?;
    (!host.is_empty()).then_some(host)
}

/// Pixels per inch a PNG or JPEG declares, when it does.
fn declared_dpi(bytes: &[u8]) -> Option<f32> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        let mut at = 8;
        while at + 12 <= bytes.len() {
            let len = u32::from_be_bytes(bytes[at..at + 4].try_into().ok()?) as usize;
            let kind = &bytes[at + 4..at + 8];
            if kind == b"pHYs" && at + 17 <= bytes.len() {
                let per_metre = u32::from_be_bytes(bytes[at + 8..at + 12].try_into().ok()?);
                return (bytes[at + 16] == 1 && per_metre > 0).then_some(per_metre as f32 * 0.0254);
            }
            if kind == b"IDAT" {
                return None;
            }
            at += 12 + len;
        }
        return None;
    }
    // JPEG: the JFIF header's density, in dots per inch or per centimetre.
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF, 0xE0]) && bytes.len() > 18 && &bytes[6..11] == b"JFIF\0" {
        let x = u16::from_be_bytes([bytes[14], bytes[15]]) as f32;
        return match bytes[13] {
            1 if x > 0.0 => Some(x),
            2 if x > 0.0 => Some(x * 2.54),
            _ => None,
        };
    }
    None
}

impl Images {
    /// Call once at launch with the app's waker, so decoded and fetched images redraw the window.
    pub fn setup(&mut self, waker: Waker) {
        self.cache = Some(std::rc::Rc::new(ImageCache::new(waker.clone())));
        self.waker = Some(waker);
    }

    /// Room to leave under each image line: the preview and a gap above and below it.
    pub fn space_after(&self, line: usize, gap: f32) -> f32 {
        self.blocks.iter().find(|b| b.line == line).map_or(0.0, |b| b.size.1 + gap * 2.0)
    }

    /// Re-reads image lines after the text was parsed, and sizes their previews.
    pub fn update(&mut self, text: &str, lines: &[MarkdownLine], base: &Path, remote_draft: bool, room: Room) {
        let found = markdown_images::blocks(text, lines);
        let mut blocks = Vec::with_capacity(found.len());
        for image in found {
            let Some(line) = lines.iter().position(|l| l.range == image.line_range) else { continue };
            let source = if remote_draft { None } else { markdown_images::resolve(&image.source, Some(base)) };
            let content = self.content(source.as_ref(), &image.source, remote_draft, room);
            let size = display_size(&content, room);
            blocks.push(Block { image, line, source, content, size });
        }
        self.blocks = blocks;
    }

    fn content(&mut self, source: Option<&ImageSource>, written: &str, remote_draft: bool, room: Room) -> Content {
        let placeholder = |symbol, message: String| Content::Placeholder { symbol, message };
        if remote_draft {
            return placeholder(Symbol::Photo, "Images aren't shown for files opened over SSH".into());
        }
        let Some(cache) = self.cache.clone() else { return placeholder(Symbol::Photo, String::new()) };
        match source {
            None => placeholder(Symbol::Unreadable, format!("Can't read image address “{written}”")),
            Some(ImageSource::File(path)) => {
                let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) else {
                    return placeholder(Symbol::Missing, format!("Image not found · {written}"));
                };
                let want_pixels = room.column_width * room.scale;
                if let Some(c) = self.local.get(path)
                    && c.modified == Some(modified)
                    && c.max_pixel >= want_pixels * 0.9
                {
                    return Content::Image(c.image.clone(), c.points);
                }
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let Some((image, points)) = load_local(&cache, path, room) else {
                    return placeholder(Symbol::Missing, format!("Can't open {name}"));
                };
                self.local.insert(path.clone(), Cached { image: image.clone(), points, modified: Some(modified), max_pixel: want_pixels });
                Content::Image(image, points)
            }
            Some(ImageSource::Web(url)) => {
                if let Some(c) = self.web.get(url) {
                    return Content::Image(c.image.clone(), c.points);
                }
                if let Some(message) = self.failed.get(url) {
                    return placeholder(Symbol::Offline, message.clone());
                }
                self.fetch(url);
                placeholder(Symbol::Photo, format!("Loading {}…", host(url).unwrap_or("image")))
            }
        }
    }

    fn fetch(&mut self, url: &str) {
        if !self.loading.insert(url.to_string()) {
            return;
        }
        let (tx, waker, url) = (self.fetched.0.clone(), self.waker.clone(), url.to_string());
        std::thread::spawn(move || {
            // An error page is data that isn't an image, as URLSession hands it over.
            let result = ureq::get(&url)
                .config()
                .http_status_as_error(false)
                .build()
                .call()
                .map_err(|e| e.to_string())
                .and_then(|mut r| r.body_mut().with_config().limit(64 << 20).read_to_vec().map_err(|e| e.to_string()));
            let _ = tx.send((url, result));
            if let Some(w) = waker {
                w();
            }
        });
    }

    /// Takes in images fetched from the web. Returns whether any arrived, so previews re-lay out.
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        while let Ok((url, result)) = self.fetched.1.try_recv() {
            any = true;
            self.loading.remove(&url);
            let decoded = result.as_ref().ok().and_then(|bytes| {
                let image = self.cache.as_ref()?.load(bytes.clone());
                let [w, h] = image.wait().ok()?;
                (w > 0).then_some((image, (w as f32, h as f32)))
            });
            match decoded {
                Some((image, points)) => {
                    self.web.insert(url, Cached { image, points, modified: None, max_pixel: f32::MAX });
                }
                None => {
                    let message = if result.is_ok() {
                        format!("Not an image · {}", host(&url).unwrap_or(""))
                    } else {
                        format!("Couldn't load image from {}", host(&url).unwrap_or("the web"))
                    };
                    self.failed.insert(url, message);
                }
            }
        }
        any
    }
}

/// A decoded image sized in points (screenshots taken at 2× keep their on-screen size).
fn load_local(cache: &ImageCache, path: &Path, room: Room) -> Option<(Image, (f32, f32))> {
    let bytes = std::fs::read(path).ok()?;
    let is_svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg") || e.eq_ignore_ascii_case("svgz"));
    let image = if is_svg { cache.svg(bytes.clone(), room.column_width, room.scale) } else { cache.load(bytes.clone()) };
    let [w, h] = image.wait().ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    let points = if is_svg {
        (w as f32 / room.scale, h as f32 / room.scale)
    } else {
        let dpi = declared_dpi(&bytes).unwrap_or(72.0);
        (w as f32 * 72.0 / dpi, h as f32 * 72.0 / dpi)
    };
    Some((image, points))
}

fn display_size(content: &Content, room: Room) -> (f32, f32) {
    match content {
        Content::Placeholder { .. } => (room.column_width.min(420.0), 40.0),
        Content::Image(_, natural) => {
            let natural = if natural.0 > 0.0 && natural.1 > 0.0 { *natural } else { (room.column_width, room.column_width * 0.6) };
            let mut w = room.column_width.min(natural.0);
            let mut h = w * natural.1 / natural.0;
            if h > room.max_height {
                h = room.max_height;
                w = h * natural.0 / natural.1;
            }
            (w.round(), h.round())
        }
    }
}

/// The key of the invisible highlight over image line `i`, which the view reads the line's place from.
fn line_key(i: usize) -> String {
    format!("image-line-{i}")
}

impl App {
    /// The gap between an image line and its preview, and under the preview.
    pub fn image_gap(&self) -> f32 {
        (self.metrics.line_height * 0.45).round()
    }

    pub fn image_room(&self) -> Room {
        let c = self.column();
        let (_, vh) = self.visible();
        Room { column_width: (c.width - self.metrics.hang_width()).max(80.0), max_height: (vh * 0.7).clamp(160.0, 560.0), scale: self.scale }
    }

    /// Re-reads image lines and sizes their previews. Runs after every parse and layout change.
    pub fn refresh_images(&mut self) {
        let base = self.draft.path().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_else(|| self.library.drafts_directory().to_path_buf());
        let room = self.image_room();
        self.images.update(&self.edit.text, self.parsed.lines(), &base, self.draft.remote().is_some(), room);
    }

    /// Invisible highlights over each image line, so the view can read where the line sits.
    pub fn image_line_marks(&self) -> Vec<TextHighlight> {
        self.images
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| !b.image.line_range.is_empty())
            .map(|(i, b)| TextHighlight::new(b.image.line_range.clone(), rune::Color::TRANSPARENT).key(line_key(i)))
            .collect()
    }

    /// Copies image files into an `images` folder beside the draft (or links them where they
    /// already sit under it), saves pasted image data there, and inserts the Markdown at
    /// `location` or over the selection. Returns false when there was nothing to insert.
    pub fn insert_images(&mut self, files: &[PathBuf], png: Option<&[u8]>, location: Option<usize>, tx: &mut Tx) -> bool {
        let files: Vec<&PathBuf> = files.iter().filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(markdown_images::is_image_extension)).collect();
        if files.is_empty() && png.is_none() {
            return false;
        }
        // The draft is hidden behind a version preview. `ImageBlocks.swift:311`
        if self.preview.is_active() {
            tx.beep();
            return true;
        }
        if self.draft.remote().is_some() {
            self.flash_message("Images can't be added to files opened over SSH yet", tx);
            return true;
        }
        let base = self.draft.path().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_else(|| self.library.drafts_directory().to_path_buf());
        let folder = base.join("images");
        let mut snippets = Vec::new();
        let mut copied = 0;
        let mut save = || -> std::io::Result<()> {
            for file in &files {
                let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let alt = markdown_images::alt_text_for_file_name(&name);
                if let Some(rel) = markdown_images::relative_path(file, &base) {
                    snippets.push(markdown_images::markdown(&alt, &rel));
                    continue;
                }
                std::fs::create_dir_all(&folder)?;
                let dest = markdown_images::unique_file_path(&name, &folder);
                std::fs::copy(file, &dest)?;
                copied += 1;
                snippets.push(markdown_images::markdown(&alt, &format!("images/{}", dest.file_name().unwrap_or_default().to_string_lossy())));
            }
            if let Some(data) = png {
                std::fs::create_dir_all(&folder)?;
                let stamp = chrono::Local::now().format("%Y-%m-%d at %H.%M.%S");
                let dest = markdown_images::unique_file_path(&format!("Pasted image {stamp}.png"), &folder);
                writer_core::atomic_file::write(data, &dest).map_err(|e| std::io::Error::other(e.to_string()))?;
                copied += 1;
                snippets.push(markdown_images::markdown("", &format!("images/{}", dest.file_name().unwrap_or_default().to_string_lossy())));
            }
            Ok(())
        };
        if let Err(e) = save() {
            self.flash_message(format!("Couldn't save the image: {}", writer_core::describe_io(&e)), tx);
            if snippets.is_empty() {
                return true;
            }
        }
        let text = &self.edit.text;
        let range = match location {
            Some(at) => {
                let at = at.min(text.len());
                at..at
            }
            None => {
                let (a, b) = self.edit.range();
                a.min(b)..a.max(b)
            }
        };
        let before_newline = range.start == 0 || text.as_bytes()[range.start - 1] == b'\n';
        let after_newline = range.end >= text.len() || text.as_bytes()[range.end] == b'\n';
        let insertion = format!("{}{}{}", if before_newline { "" } else { "\n\n" }, snippets.join("\n\n"), if after_newline { "" } else { "\n\n" });
        let caret = range.start + insertion.len();
        let name = if snippets.len() == 1 { "Insert Image" } else { "Insert Images" };
        self.apply(&writer_core::TextEdit::new(range, insertion, caret..caret), name, tx);
        tx.focus(DRAFT, false);
        let what = if snippets.len() == 1 { "Image added".to_string() } else { format!("{} images added", snippets.len()) };
        let how = if copied > 0 { "copied to the images folder beside this draft" } else { "linked where it already is" };
        self.flash_message(format!("{what} · {how}"), tx);
        true
    }

    /// A paste into the draft: image files on the clipboard go in as images; otherwise text
    /// wins when there is any, so pasting from a web page still pastes its words, and image
    /// data alone goes in as a pasted image. False lets the text paste happen.
    /// `ImageBlocks.swift:300-310`
    pub fn paste_images(&mut self, content: &rune::ClipboardContent, tx: &mut Tx) -> bool {
        if self.insert_images(content.files(), None, None, tx) {
            return true;
        }
        let png = content.image().filter(|_| content.text().is_empty()).map(|i| i.png().to_vec());
        png.is_some_and(|png| self.insert_images(&[], Some(&png), None, tx))
    }

    /// Format › Image…: pick one or more image files to insert, in the order the dialog gives
    /// them. The message shows on the Mac's panel; Windows and Linux show it as the dialog's
    /// title, where they have no message. `ImageBlocks.swift:361-374`
    pub fn insert_image_from_file(&mut self, tx: &mut Tx) {
        let extensions: Vec<&str> = markdown_images::IMAGE_EXTENSIONS.to_vec();
        let message = "Images are copied into an images folder next to this draft.";
        let options = rune::FileDialogOptions { title: message.into(), ..Default::default() }.message(message).filter("Images", &extensions);
        tx.open_files(options, |s: &mut App, result, tx| {
            if let Ok(paths) = result {
                s.insert_images(&paths, None, None, tx);
            }
        });
    }
}

/// Only web links and existing image files get opened, so a draft can't point the system at
/// an app or script.
fn openable(source: &ImageSource) -> Option<String> {
    match source {
        ImageSource::Web(url) => {
            let lower = url.to_ascii_lowercase();
            (lower.starts_with("http://") || lower.starts_with("https://")).then(|| url.clone())
        }
        ImageSource::File(path) => {
            let is_image = path.extension().and_then(|e| e.to_str()).is_some_and(markdown_images::is_image_extension);
            (is_image && path.is_file()).then(|| path.to_string_lossy().into_owned())
        }
    }
}

fn open(source: &ImageSource, tx: &mut Tx) {
    match source {
        ImageSource::Web(url) => {
            let _ = tx.open_url(url);
        }
        ImageSource::File(path) => crate::platform::open_file(path),
    }
}

fn symbol_icon(symbol: Symbol, ink: rune::Color) -> El {
    use rune::Shape;
    // 14 pt glyphs drawn in a 14-unit box, standing in for the SF Symbols the Swift app used.
    let frame: [[f32; 2]; 5] = [[1.5, 3.0], [12.5, 3.0], [12.5, 11.0], [1.5, 11.0], [1.5, 3.0]];
    let mut shape = Shape::new().view_box(14.0, 14.0);
    match symbol {
        Symbol::Photo | Symbol::Missing => {
            shape = shape.stroke(frame.to_vec(), 1.1, ink).stroke(vec![[2.5, 10.0], [5.5, 6.5], [8.0, 9.0], [9.5, 7.5], [11.5, 10.0]], 1.1, ink);
            if symbol == Symbol::Missing {
                shape = shape.stroke(vec![[10.5, 4.5], [10.5, 6.2]], 1.2, ink).stroke(vec![[10.5, 7.4], [10.5, 7.5]], 1.2, ink);
            }
        }
        Symbol::Unreadable => {
            // A dashed frame: short strokes along each side.
            for pair in frame.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                for k in 0..4 {
                    let (t0, t1) = (k as f32 / 4.0 + 0.04, k as f32 / 4.0 + 0.17);
                    let at = |t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                    shape = shape.stroke(vec![at(t0), at(t1)], 1.1, ink);
                }
            }
            shape = shape.stroke(vec![[5.6, 6.0], [6.0, 5.2], [7.0, 4.8], [8.0, 5.2], [8.4, 6.0], [8.0, 6.9], [7.0, 7.5], [7.0, 8.3]], 1.1, ink).stroke(vec![[7.0, 9.6], [7.0, 9.7]], 1.2, ink);
        }
        Symbol::Offline => {
            let arc = |x0: f32, x1: f32, top: f32, base: f32| -> Vec<[f32; 2]> {
                (0..=8).map(|i| { let t = i as f32 / 8.0; [x0 + (x1 - x0) * t, base - (base - top) * (1.0 - (2.0 * t - 1.0).powi(2))] }).collect()
            };
            shape = shape
                .stroke(arc(1.5, 12.5, 3.2, 6.0), 1.1, ink)
                .stroke(arc(3.6, 10.4, 5.6, 8.0), 1.1, ink)
                .stroke(vec![[7.0, 8.2], [7.0, 10.2]], 1.3, ink)
                .stroke(vec![[7.0, 11.6], [7.0, 11.7]], 1.3, ink);
        }
    }
    el().size(14.0, 14.0).shape(shape)
}

/// The previews, placed under their lines. Children of the page the draft field sits on, in
/// that page's coordinates.
pub fn view(s: &App, p: &Palette, cx: &mut Cx<App>) -> Vec<El> {
    let Some(field) = cx.layout(DRAFT) else { return Vec::new() };
    let x = s.column().inset_x + s.metrics.hang_width();
    let gap = s.image_gap();
    let focus = s.focus_range();
    let mut out = Vec::new();
    for (i, b) in s.images.blocks.iter().enumerate() {
        let line = &b.image.line_range;
        // An empty line has no highlight to read; the caret line stands in while it is typed.
        let Some(at) = cx.layout(&line_key(i)) else { continue };
        let y = at[1] + at[3] - field[1] + gap;
        let inside = focus.as_ref().is_none_or(|f| (f.start < line.end && line.start < f.end) || f.contains(&line.start));
        let label = if b.image.alt.is_empty() { "Image".to_string() } else { b.image.alt.clone() };
        let mut frame = el().key(format!("image-{i}")).role("image").label(label).absolute(x, y).size(b.size.0, b.size.1).radius(6.0).opacity(if inside { 1.0 } else { 0.28 });
        frame = match &b.content {
            Content::Image(image, _) => frame.pixels(image.pixels()).clip_rounded(6.0).border(1.0, p.hairline),
            Content::Placeholder { symbol, message } => {
                let ink = p.text.alpha(0.62);
                frame.fill(p.code_background).child(
                    rune::row().absolute(12.0, 0.0).size((b.size.0 - 24.0).max(0.0), b.size.1).gap(8.0).center_y().child(symbol_icon(*symbol, ink)).child(
                        text(message.as_str()).font(ui_font(11.0, false)).color(ink).align(Align::Start).truncate().grow(1.0),
                    ),
                )
            }
        };
        if let Some(source) = &b.source {
            frame = frame.help(match source {
                ImageSource::File(path) => crate::platform::abbreviate_home(path),
                ImageSource::Web(url) => url.clone(),
            });
        }
        let line_end = line.end;
        let (source_a, source_b) = (b.source.clone(), b.source.clone());
        let is_image = matches!(b.content, Content::Image(..));
        frame = frame
            .on_press_tx(move |s: &mut App, tx| {
                s.edit.set_caret(line_end.min(s.edit.text.len()));
                tx.focus(DRAFT, false);
            })
            .on_double_press_tx(move |_: &mut App, tx| {
                if let Some(source) = source_a.as_ref().filter(|s| openable(s).is_some()) {
                    open(source, tx);
                }
            })
            .on_secondary_press(move |_: &mut App, press, tx| {
                let Some(source) = source_b.clone() else { return };
                let mut menu = PopupMenu::new();
                if openable(&source).is_some() {
                    menu = menu.item(PopupItem::new("open", "Open Image"));
                }
                if matches!(source, ImageSource::File(_)) {
                    menu = menu.item(PopupItem::new("reveal", crate::platform::REVEAL_TITLE));
                }
                if is_image {
                    menu = menu.item(PopupItem::new("copy", "Copy Image"));
                }
                tx.popup_menu(menu, press, move |s: &mut App, picked, tx| match picked {
                    Some("open") => open(&source, tx),
                    Some("reveal") => {
                        if let ImageSource::File(path) = &source {
                            crate::platform::reveal(path);
                        }
                    }
                    Some("copy") => {
                        if let Err(e) = crate::platform::copy_image(&source) {
                            s.flash_message(format!("Couldn't copy the image: {e}"), tx);
                        }
                    }
                    _ => {}
                });
            });
        out.push(frame);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server's error page is data that isn't an image, as URLSession hands it over: "Not an
    /// image", not a network failure. `ImageBlocks.swift:283`
    #[test]
    fn an_error_page_is_not_an_image() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nNot found");
        });
        let mut images = Images::default();
        images.setup(std::sync::Arc::new(|| {}));
        let url = format!("http://127.0.0.1:{port}/a.png");
        images.fetch(&url);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !images.failed.contains_key(&url) && std::time::Instant::now() < deadline {
            images.poll();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(images.failed.get(&url).map(String::as_str), Some("Not an image · 127.0.0.1"));
        // An address nothing answers is a network failure.
        let closed = "http://127.0.0.1:9/b.png".to_string();
        images.fetch(&closed);
        while !images.failed.contains_key(&closed) && std::time::Instant::now() < deadline {
            images.poll();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(images.failed.get(&closed).map(String::as_str), Some("Couldn't load image from 127.0.0.1"));
    }
}
