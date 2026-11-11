//! Foundation `CharacterSet`s the Swift code relied on, as char predicates.

/// `CharacterSet.whitespaces`: Unicode General Category Zs plus tab.
pub(crate) fn is_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// `CharacterSet.newlines`: U+000A–U+000D, U+0085, U+2028 and U+2029.
pub(crate) fn is_newline(c: char) -> bool {
    matches!(
        c,
        '\n' | '\u{B}' | '\u{C}' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

/// `CharacterSet.whitespacesAndNewlines`.
pub(crate) fn is_whitespace_or_newline(c: char) -> bool {
    is_whitespace(c) || is_newline(c)
}

/// `CharacterSet.alphanumerics`: General Categories L*, M* and N*.
pub(crate) fn is_alphanumeric(c: char) -> bool {
    c.is_alphanumeric() || unicode_normalization::char::is_combining_mark(c)
}

/// `CharacterSet.controlCharacters`: General Categories Cc and Cf.
pub(crate) fn is_control(c: char) -> bool {
    c.is_control() || is_format(c)
}

/// General Category Cf (format characters).
fn is_format(c: char) -> bool {
    matches!(
        c,
        '\u{AD}'
            | '\u{600}'..='\u{605}'
            | '\u{61C}'
            | '\u{6DD}'
            | '\u{70F}'
            | '\u{890}'..='\u{891}'
            | '\u{8E2}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{110BD}'
            | '\u{110CD}'
            | '\u{13430}'..='\u{1343F}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0001}'
            | '\u{E0020}'..='\u{E007F}'
    )
}

/// `String.trimmingCharacters(in:)` for a predicate.
pub(crate) fn trim(s: &str, pred: impl Fn(char) -> bool) -> &str {
    s.trim_matches(|c: char| pred(c))
}

/// Swift `Character.isNewline`, applied per char (CRLF is handled by the splitters that need it).
pub(crate) fn is_character_newline(c: char) -> bool {
    matches!(
        c,
        '\n' | '\u{B}' | '\u{C}' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

/// `split(whereSeparator: \.isNewline)` with empty pieces omitted.
pub(crate) fn split_newlines(s: &str) -> impl Iterator<Item = &str> {
    s.split(is_character_newline).filter(|p| !p.is_empty())
}
