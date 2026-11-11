//! The few Foundation string behaviours the Swift agents relied on.

use unicode_segmentation::UnicodeSegmentation;

/// `CharacterSet.whitespaces`: General Category Zs plus tab.
pub fn is_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// `CharacterSet.whitespacesAndNewlines`.
pub fn is_whitespace_or_newline(c: char) -> bool {
    is_whitespace(c)
        || matches!(
            c,
            '\n' | '\u{B}' | '\u{C}' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}'
        )
}

/// `trimmingCharacters(in: .whitespacesAndNewlines)`.
pub fn trim(s: &str) -> &str {
    s.trim_matches(is_whitespace_or_newline)
}

/// `trimmingCharacters(in: .whitespaces)`.
pub fn trim_whitespace(s: &str) -> &str {
    s.trim_matches(is_whitespace)
}

/// `String.prefix(n)`: the first `n` characters (grapheme clusters).
pub fn prefix(s: &str, n: usize) -> &str {
    match s.grapheme_indices(true).nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// `String.suffix(n)`: the last `n` characters (grapheme clusters).
pub fn suffix(s: &str, n: usize) -> &str {
    match s.grapheme_indices(true).rev().nth(n.saturating_sub(1)) {
        Some((i, _)) if n > 0 => &s[i..],
        _ if n == 0 => "",
        _ => s,
    }
}

/// `split(separator: "\n", omittingEmptySubsequences: false)` on a Swift `String`. Swift compares
/// characters, and `"\r\n"` is one character that isn't `"\n"`, so CRLF line ends don't split.
pub fn split_lf(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' && (i == 0 || bytes[i - 1] != b'\r') {
            out.push(&s[start..i]);
            start = i + 1;
        }
    }
    out.push(&s[start..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(trim("\u{3000} a \n"), "a");
        assert_eq!(prefix("e\u{301}xy", 2), "e\u{301}x");
        assert_eq!(prefix("ab", 5), "ab");
        assert_eq!(suffix("abcd", 2), "cd");
        assert_eq!(suffix("ab", 5), "ab");
        assert_eq!(suffix("ab", 0), "");
        assert_eq!(split_lf("a\nb\r\nc\n"), vec!["a", "b\r\nc", ""]);
    }
}
