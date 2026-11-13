//! Default-ignorable code points: characters that render as nothing.
//!
//! The Unicode `Default_Ignorable_Code_Point` property marks characters that a
//! renderer shows as nothing at all when it has no special handling for them:
//! the zero-width space and joiners, the word joiner, the byte order mark, the
//! Hangul fillers, variation selectors, and the tag characters, among others.
//!
//! Several of them are valid inside identifiers. ZERO WIDTH JOINER (U+200D) and
//! ZERO WIDTH NON-JOINER (U+200C) are `XID_Continue`, because some scripts need
//! them, and the Hangul fillers U+3164 and U+FFA0 are even `XID_Start`. So
//! [`is_xid`](crate::is_xid) accepts `admin\u{200D}`, which looks exactly like
//! `admin` in an editor, and accepts `\u{3164}`, which looks like nothing.
//! Normalization (NFC or NFKC) leaves these characters in place.
//!
//! These functions let a lexer or linter find such characters so it can reject
//! the identifier or warn about it. They do not decide policy: a language that
//! wants ZWNJ in Persian identifiers can allow it and still reject the rest.

use crate::lookup::in_ranges;
use crate::tables;

/// Returns `true` if `c` has the Unicode `Default_Ignorable_Code_Point`
/// property, meaning it is normally rendered as nothing.
///
/// This covers U+200B..U+200F (zero-width space, non-joiner, joiner and the
/// directional marks), U+2060..U+206F (word joiner, invisible operators and
/// deprecated format controls), U+FEFF (byte order mark), the Hangul fillers
/// U+115F, U+1160, U+3164 and U+FFA0, the variation selectors, the tag
/// characters U+E0000..U+E0FFF, SOFT HYPHEN, and a few more. No ASCII
/// character is default-ignorable.
///
/// # Examples
///
/// ```
/// use unicode_lang::is_default_ignorable;
///
/// assert!(is_default_ignorable('\u{200D}')); // ZERO WIDTH JOINER
/// assert!(is_default_ignorable('\u{3164}')); // HANGUL FILLER
/// assert!(is_default_ignorable('\u{E0041}')); // TAG LATIN CAPITAL LETTER A
///
/// assert!(!is_default_ignorable('a'));
/// assert!(!is_default_ignorable(' '));
/// ```
#[inline]
#[must_use]
pub fn is_default_ignorable(c: char) -> bool {
    // No ASCII scalar is default-ignorable; checked against the table by
    // `test_ascii_fast_path_matches_table`.
    if c.is_ascii() {
        return false;
    }
    in_ranges(c as u32, tables::DEFAULT_IGNORABLE)
}

/// Returns the byte offset and value of the first default-ignorable character
/// in `s`, or `None` if `s` has none.
///
/// Run it on each identifier a lexer accepts to catch names that hide
/// invisible characters, such as `admin\u{200D}` posing as `admin`. Pure-ASCII
/// text is answered without decoding.
///
/// # Examples
///
/// ```
/// use unicode_lang::{first_default_ignorable, is_xid};
///
/// let spoofed = "admin\u{200D}";
/// assert!(is_xid(spoofed)); // a valid identifier by UAX #31
/// assert_eq!(first_default_ignorable(spoofed), Some((5, '\u{200D}')));
///
/// assert_eq!(first_default_ignorable("admin"), None);
/// assert_eq!(first_default_ignorable("café"), None);
/// ```
#[must_use]
pub fn first_default_ignorable(s: &str) -> Option<(usize, char)> {
    if s.is_ascii() {
        return None;
    }
    s.char_indices().find(|&(_, c)| is_default_ignorable(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::is_xid;

    #[test]
    fn test_ascii_fast_path_matches_table() {
        for cp in 0u32..0x80 {
            assert!(!in_ranges(cp, tables::DEFAULT_IGNORABLE), "{cp:#X}");
        }
    }

    #[test]
    fn test_known_invisible_characters() {
        for c in [
            '\u{00AD}',
            '\u{034F}',
            '\u{115F}',
            '\u{1160}',
            '\u{180E}',
            '\u{200B}',
            '\u{200C}',
            '\u{200D}',
            '\u{2060}',
            '\u{2064}',
            '\u{3164}',
            '\u{FE0F}',
            '\u{FEFF}',
            '\u{FFA0}',
            '\u{E0001}',
            '\u{E007F}',
        ] {
            assert!(is_default_ignorable(c), "{:#X}", c as u32);
        }
        for c in [
            'a', 'Z', '_', ' ', '\u{00E9}', '\u{0301}', '\u{4E16}', '\u{FFFD}',
        ] {
            assert!(!is_default_ignorable(c), "{:#X}", c as u32);
        }
    }

    #[test]
    fn test_invisible_identifiers_are_xid_but_flagged() {
        for (s, at, c) in [
            ("admin\u{200D}", 5, '\u{200D}'),
            ("admin\u{200C}", 5, '\u{200C}'),
            ("\u{3164}", 0, '\u{3164}'),
            ("\u{FFA0}", 0, '\u{FFA0}'),
            ("a\u{3164}b", 1, '\u{3164}'),
        ] {
            assert!(is_xid(s), "{s:?}");
            assert_eq!(first_default_ignorable(s), Some((at, c)), "{s:?}");
        }
    }

    #[test]
    fn test_first_default_ignorable_reports_first() {
        assert_eq!(
            first_default_ignorable("x\u{00E9}\u{2060}y\u{200B}"),
            Some((3, '\u{2060}'))
        );
        assert_eq!(first_default_ignorable(""), None);
        assert_eq!(first_default_ignorable("na\u{00EF}ve"), None);
    }
}
