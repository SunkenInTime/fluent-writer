//! Images on lines of their own, which the editor previews below the line (Swift `MarkdownImages`).

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use regex::Regex;

use crate::charsets::{is_whitespace, trim};
use crate::markdown::{MarkdownBlockKind, MarkdownLine};
use crate::paths::{normalize, resolve_symlinks, standardize};

/// An image on a line of its own, `![alt](source "title")`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownImage {
    pub line_range: Range<usize>,
    pub alt: String,
    pub source: String,
}

/// Where an image source points. Swift returned a `URL`; here web and file locations are split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageSource {
    /// An `http` or `https` URL, as written.
    Web(String),
    /// A file on this machine (from a `file:` URL, an absolute path, `~`, or a path relative to the draft).
    File(PathBuf),
}

static BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^[ ]{0,3}!\[([^\]\n]*)\]\([\t\n\x0C\r\p{Z}]*(<[^>\n]+>|[^\t\n\x0C\r\p{Z})]+)(?:[\t\n\x0C\r\p{Z}]+(?:"[^"\n]*"|'[^'\n]*'))?[\t\n\x0C\r\p{Z}]*\)[\t\n\x0C\r\p{Z}]*$"#,
    )
    .expect("valid pattern")
});
static SEPARATORS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[-_]+").expect("valid pattern"));

pub const IMAGE_EXTENSIONS: [&str; 11] = [
    "png", "jpg", "jpeg", "gif", "heic", "heif", "webp", "tiff", "tif", "bmp", "svg",
];

/// True when `ext` (without the dot, any case) is one of [`IMAGE_EXTENSIONS`].
pub fn is_image_extension(ext: &str) -> bool {
    let lower = ext.to_lowercase();
    IMAGE_EXTENSIONS.contains(&lower.as_str())
}

/// Image lines among `lines`. Code blocks and fences are skipped.
pub fn blocks(text: &str, lines: &[MarkdownLine]) -> Vec<MarkdownImage> {
    let mut out = Vec::new();
    for line in lines
        .iter()
        .filter(|l| l.kind == MarkdownBlockKind::Paragraph)
    {
        if line.range.len() < 5 || line.range.end > text.len() {
            continue;
        }
        let s = &text[line.range.clone()];
        if !trim(s, is_whitespace).starts_with("![") {
            continue;
        }
        let Some(c) = BLOCK.captures(s) else { continue };
        let mut source = c.get(2).map_or("", |m| m.as_str());
        if source.starts_with('<') && source.ends_with('>') && source.len() >= 2 {
            source = &source[1..source.len() - 1];
        }
        out.push(MarkdownImage {
            line_range: line.range.clone(),
            alt: c.get(1).map_or("", |m| m.as_str()).to_string(),
            source: source.to_string(),
        });
    }
    out
}

/// The scheme of `source` per RFC 3986 (`ALPHA *( ALPHA / DIGIT / "+" / "-" / "." ) ":"`), if any.
fn scheme(source: &str) -> Option<&str> {
    let colon = source.find(':')?;
    let s = &source[..colon];
    let mut chars = s.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        Some(s)
    } else {
        None
    }
}

/// `removingPercentEncoding`: nil (here `None`) on a malformed escape or invalid UTF-8.
pub(crate) fn remove_percent_encoding(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3)?;
            let h = std::str::from_utf8(hex).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// A `file:` URL's path.
fn file_url_path(url: &str) -> Option<PathBuf> {
    let rest = &url[url.find(':')? + 1..];
    let rest = rest.strip_prefix("//").map_or(rest, |r| {
        // Skip an authority (`localhost` or empty).
        r.find('/').map_or("", |i| &r[i..])
    });
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let decoded = remove_percent_encoding(rest)?;
    // `file:///C:/x.png` on Windows.
    #[cfg(windows)]
    let decoded = {
        let b = decoded.as_bytes();
        if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
            decoded[1..].to_string()
        } else {
            decoded
        }
    };
    Some(PathBuf::from(decoded))
}

/// `~` and `~/…` expanded to the home folder. (`~user` is left as written.)
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        if path == "~" {
            return home;
        }
        if let Some(rest) = path.strip_prefix("~/") {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Where `source` points: a web URL, a file URL, or a path relative to `base` (the draft's folder).
pub fn resolve(source: &str, base: Option<&Path>) -> Option<ImageSource> {
    if let Some(scheme) = scheme(source) {
        let lower = scheme.to_lowercase();
        if lower == "http" || lower == "https" {
            return Some(ImageSource::Web(source.to_string()));
        }
        if lower == "file" {
            return file_url_path(source).map(|p| ImageSource::File(normalize(&p)));
        }
        // A one-letter scheme is a Windows drive letter (`C:/…`), so it falls through as a path.
        if scheme.chars().count() > 1 {
            return None;
        }
    }
    let decoded = remove_percent_encoding(source).unwrap_or_else(|| source.to_string());
    if decoded.starts_with('/') || Path::new(&decoded).is_absolute() {
        return Some(ImageSource::File(standardize(Path::new(&decoded))));
    }
    if decoded.starts_with('~') {
        return Some(ImageSource::File(standardize(&expand_tilde(&decoded))));
    }
    let base = base?;
    Some(ImageSource::File(standardize(&base.join(decoded))))
}

/// `file` relative to `directory` when it's inside it, else `None`. Always `/`-separated, for Markdown.
pub fn relative_path(file: &Path, directory: &Path) -> Option<String> {
    let dir = resolve_symlinks(directory);
    let path = resolve_symlinks(file);
    let rel = path.strip_prefix(&dir).ok()?;
    if rel.as_os_str().is_empty() {
        return None;
    }
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

/// `CharacterSet.urlPathAllowed` minus `(` and `)`: everything else is percent-encoded.
const PATH_ENCODE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}')
    .add(b'(')
    .add(b')');

/// The Markdown for an image, with the path escaped so spaces and parentheses survive.
pub fn markdown(alt: &str, path: &str) -> String {
    let encoded = utf8_percent_encode(path, PATH_ENCODE).to_string();
    let clean_alt = alt.replace(['[', ']'], "").replace('\n', " ");
    format!("![{clean_alt}]({encoded})")
}

/// A readable alt text from a file name: `team-offsite_2026.png` → `team offsite 2026`.
pub fn alt_text_for_file_name(name: &str) -> String {
    let stem = Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    trim(&SEPARATORS.replace_all(&stem, " "), is_whitespace).to_string()
}

/// A name in `directory` that doesn't exist yet: `photo.png`, `photo 2.png`, …
pub fn unique_file_path(name: &str, directory: &Path) -> PathBuf {
    let p = Path::new(name);
    let ext = p
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut candidate = directory.join(name);
    let mut n = 2;
    while candidate.exists() {
        candidate = directory.join(if ext.is_empty() {
            format!("{stem} {n}")
        } else {
            format!("{stem} {n}.{ext}")
        });
        n += 1;
    }
    candidate
}

/// The text to insert at `location` so the image sits on a line of its own.
pub fn block_insertion(markdown: &str, location: usize, text: &str) -> String {
    let bytes = text.as_bytes();
    let before = if location > 0 {
        bytes.get(location - 1).copied().unwrap_or(b'\n')
    } else {
        b'\n'
    };
    let after = if location < bytes.len() {
        bytes[location]
    } else {
        b'\n'
    };
    let lead = if before == b'\n' { "" } else { "\n\n" };
    let trail = if after == b'\n' { "" } else { "\n\n" };
    format!("{lead}{markdown}{trail}")
}
