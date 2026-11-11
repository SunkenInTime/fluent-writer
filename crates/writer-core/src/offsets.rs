//! Conversions between UTF-8 byte offsets (this crate's API) and UTF-16 offsets (what the Swift app
//! stored on disk).

use std::ops::Range;

/// The UTF-8 byte offset of UTF-16 offset `utf16` in `s`.
///
/// Offsets past the end clamp to `s.len()`. An offset that falls between the two halves of a
/// surrogate pair snaps back to the start of that character, so the result is always a char boundary.
pub fn utf16_to_byte(s: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units >= utf16 {
            return i;
        }
        let n = c.len_utf16();
        if units + n > utf16 {
            return i;
        }
        units += n;
    }
    s.len()
}

/// The UTF-16 offset of UTF-8 byte offset `byte` in `s`.
///
/// Offsets past the end clamp to the end; an offset inside a character snaps back to its start.
pub fn byte_to_utf16(s: &str, byte: usize) -> usize {
    let b = floor_char_boundary(s, byte);
    utf16_len(&s[..b])
}

/// The length of `s` in UTF-16 code units (NSString's `length`).
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// A UTF-16 `(location, length)` pair as a byte range of `s`.
pub fn utf16_range_to_bytes(s: &str, location: usize, length: usize) -> Range<usize> {
    let start = utf16_to_byte(s, location);
    let end = utf16_to_byte(s, location.saturating_add(length));
    start..end.max(start)
}

/// A byte range of `s` as a UTF-16 `(location, length)` pair.
pub fn byte_range_to_utf16(s: &str, range: &Range<usize>) -> (usize, usize) {
    let start = byte_to_utf16(s, range.start);
    let end = byte_to_utf16(s, range.end.max(range.start));
    (start, end - start)
}

/// The largest char boundary at or below `i`, clamped to the string.
pub(crate) fn floor_char_boundary(s: &str, i: usize) -> usize {
    let mut b = i.min(s.len());
    while !s.is_char_boundary(b) {
        b -= 1;
    }
    b
}

/// The byte offset `units` UTF-16 code units before `end`, never splitting a character
/// (it stops short rather than take half a surrogate pair).
pub(crate) fn back_utf16(s: &str, end: usize, units: usize) -> usize {
    let mut taken = 0;
    let mut start = end;
    for c in s[..end].chars().rev() {
        let n = c.len_utf16();
        if taken + n > units {
            break;
        }
        taken += n;
        start -= c.len_utf8();
    }
    start
}

/// The byte offset `units` UTF-16 code units after `start`, never splitting a character.
pub(crate) fn forward_utf16(s: &str, start: usize, units: usize) -> usize {
    let mut taken = 0;
    let mut end = start;
    for c in s[start..].chars() {
        let n = c.len_utf16();
        if taken + n > units {
            break;
        }
        taken += n;
        end += c.len_utf8();
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_emoji_and_cjk() {
        let s = "a👍🏽中文b";
        // a(1) 👍(2) 🏽(2) 中(1) 文(1) b(1)
        assert_eq!(utf16_len(s), 8);
        assert_eq!(utf16_to_byte(s, 1), 1);
        assert_eq!(utf16_to_byte(s, 3), 5);
        assert_eq!(utf16_to_byte(s, 5), 9);
        assert_eq!(utf16_to_byte(s, 6), 12);
        assert_eq!(utf16_to_byte(s, 8), s.len());
        assert_eq!(utf16_to_byte(s, 99), s.len());
        // Inside a surrogate pair snaps back.
        assert_eq!(utf16_to_byte(s, 2), 1);
        for (i, _) in s.char_indices() {
            assert_eq!(utf16_to_byte(s, byte_to_utf16(s, i)), i);
        }
        assert_eq!(byte_to_utf16(s, 2), 1);
        assert_eq!(byte_to_utf16(s, 999), 8);
        assert_eq!(utf16_range_to_bytes(s, 1, 4), 1..9);
        assert_eq!(byte_range_to_utf16(s, &(9..15)), (5, 2));
    }

    #[test]
    fn utf16_walks() {
        let s = "x😀yz";
        // Three units back would split the emoji, so it stops after "yz".
        assert_eq!(back_utf16(s, s.len(), 3), 5);
        assert_eq!(back_utf16(s, s.len(), 4), 1);
        assert_eq!(back_utf16(s, 5, 1), 5);
        assert_eq!(forward_utf16(s, 0, 2), 1);
        assert_eq!(forward_utf16(s, 0, 3), 5);
    }
}
