//! Unicode identifier rules from [UAX #31], the `XID_Start` and `XID_Continue`
//! derived properties.
//!
//! These are the properties a lexer consults to recognise identifiers in a
//! programming language: the first scalar of an identifier must be
//! `XID_Start`, and each subsequent scalar must be `XID_Continue`. The `XID`
//! variants are the closure-stable forms — normalizing an identifier never
//! turns a valid identifier into an invalid one — which is why they, rather
//! than the older `ID_Start` / `ID_Continue`, are the recommended basis.
//!
//! [UAX #31]: https://www.unicode.org/reports/tr31/

use crate::lookup::in_ranges;
use crate::tables;

/// Returns `true` if `c` may begin a Unicode identifier — that is, if `c` has
/// the `XID_Start` property.
///
/// This is ASCII letters, plus the letters and letter-like marks of every
/// script. It excludes digits, whitespace, and punctuation — including the
/// underscore, which is `XID_Continue` but not `XID_Start`. A language that
/// permits leading underscores (as Rust does) must allow that separately.
///
/// # Examples
///
/// ```
/// use unicode_lang::is_xid_start;
///
/// assert!(is_xid_start('a'));
/// assert!(is_xid_start('Δ'));      // Greek capital delta
/// assert!(is_xid_start('日'));     // CJK ideograph
///
/// assert!(!is_xid_start('1'));     // a digit may continue, not start
/// assert!(!is_xid_start('_'));     // underscore continues but does not start
/// assert!(!is_xid_start(' '));
/// ```
#[inline]
#[must_use]
pub fn is_xid_start(c: char) -> bool {
    // In ASCII, XID_Start is exactly the letters; answering directly skips the
    // binary search for the overwhelmingly common case. Checked against the
    // table at every code point by `test_fast_paths_match_tables_exhaustive`.
    if c.is_ascii() {
        return c.is_ascii_alphabetic();
    }
    xid_start_table(c as u32)
}

/// Table lookup for `XID_Start`, with no fast path.
#[inline]
fn xid_start_table(cp: u32) -> bool {
    in_ranges(cp, tables::XID_START)
}

/// Returns `true` if `c` may continue a Unicode identifier — that is, if `c`
/// has the `XID_Continue` property.
///
/// `XID_Continue` is a superset of [`is_xid_start`]: it additionally admits
/// decimal digits, the combining marks that attach to letters, and the Unicode
/// connector punctuation — which includes the ASCII underscore `_` (category
/// `Pc`). The underscore therefore continues an identifier but, per
/// [`is_xid_start`], cannot begin one.
///
/// # Examples
///
/// ```
/// use unicode_lang::is_xid_continue;
///
/// assert!(is_xid_continue('a'));
/// assert!(is_xid_continue('9'));        // digits continue an identifier
/// assert!(is_xid_continue('_'));        // connector punctuation continues
///
/// // A base letter followed by a combining mark is a valid continuation.
/// assert!(is_xid_continue('\u{0301}')); // COMBINING ACUTE ACCENT
/// ```
#[inline]
#[must_use]
pub fn is_xid_continue(c: char) -> bool {
    // In ASCII, XID_Continue is exactly the alphanumerics plus `_`.
    if c.is_ascii() {
        return c.is_ascii_alphanumeric() || c == '_';
    }
    xid_continue_table(c as u32)
}

/// Table lookup for `XID_Continue`, with no fast path.
#[inline]
fn xid_continue_table(cp: u32) -> bool {
    in_ranges(cp, tables::XID_CONTINUE)
}

/// Returns `true` if `s` is a well-formed Unicode identifier under the default
/// [UAX #31] profile: it is non-empty, its first scalar is [`is_xid_start`],
/// and every remaining scalar is [`is_xid_continue`].
///
/// This is a convenience over iterating the scalars by hand. It applies no
/// language-specific extensions — notably it rejects a leading underscore, so
/// callers whose grammar allows `_name` should test that case themselves.
///
/// [UAX #31]: https://www.unicode.org/reports/tr31/
///
/// # Examples
///
/// ```
/// use unicode_lang::is_xid;
///
/// assert!(is_xid("total"));
/// assert!(is_xid("Δpressure"));
/// assert!(is_xid("café"));
///
/// assert!(!is_xid(""));         // empty is not an identifier
/// assert!(!is_xid("1st"));      // cannot start with a digit
/// assert!(!is_xid("a b"));      // no interior whitespace
/// assert!(!is_xid("_name"));    // underscore is not XID; opt in separately
/// ```
#[must_use]
pub fn is_xid(s: &str) -> bool {
    // Pure-ASCII identifiers (the common case in source code) are checked a
    // byte at a time, with no UTF-8 decoding and no table lookups.
    if s.is_ascii() {
        return match s.as_bytes().split_first() {
            Some((first, rest)) => {
                first.is_ascii_alphabetic()
                    && rest.iter().all(|&b| b.is_ascii_alphanumeric() || b == b'_')
            }
            None => false,
        };
    }
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => is_xid_start(first) && chars.all(is_xid_continue),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xid_start_ascii_matches_letters() {
        for c in '\0'..='\u{7F}' {
            let expected = c.is_ascii_alphabetic();
            assert_eq!(is_xid_start(c), expected, "ascii {c:?}");
        }
    }

    #[test]
    fn test_xid_continue_ascii_matches_alnum_plus_underscore() {
        // In ASCII, XID_Continue is exactly the alphanumerics plus the
        // underscore (LOW LINE, connector punctuation).
        for c in '\0'..='\u{7F}' {
            let expected = c.is_ascii_alphanumeric() || c == '_';
            assert_eq!(is_xid_continue(c), expected, "ascii {c:?}");
        }
    }

    #[test]
    fn test_xid_continue_superset_of_start() {
        for cp in 0u32..=0x2FFF {
            if let Some(c) = char::from_u32(cp) {
                if is_xid_start(c) {
                    assert!(is_xid_continue(c), "start but not continue: {c:?}");
                }
            }
        }
    }

    #[test]
    fn test_underscore_continues_but_does_not_start() {
        assert!(!is_xid_start('_'));
        assert!(is_xid_continue('_'));
        assert!(!is_xid("_name")); // leading underscore fails XID_Start
    }

    #[test]
    fn test_is_xid_empty_returns_false() {
        assert!(!is_xid(""));
    }

    #[test]
    fn test_is_xid_rejects_digit_start() {
        assert!(!is_xid("1st"));
    }

    #[test]
    fn test_fast_paths_match_tables_exhaustive() {
        // The ASCII shortcuts must agree with the generated tables at every
        // scalar value; above ASCII they are the table, so this also pins the
        // dispatch.
        for cp in 0u32..=0x10_FFFF {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            assert_eq!(is_xid_start(c), xid_start_table(cp), "start {cp:#X}");
            assert_eq!(
                is_xid_continue(c),
                xid_continue_table(cp),
                "continue {cp:#X}"
            );
        }
    }

    /// `is_xid` per scalar over the table lookups, with no shortcut.
    fn is_xid_reference(s: &str) -> bool {
        let mut chars = s.chars();
        match chars.next() {
            Some(first) => {
                xid_start_table(first as u32) && chars.all(|c| xid_continue_table(c as u32))
            }
            None => false,
        }
    }

    #[test]
    fn test_is_xid_ascii_path_matches_reference_exhaustive() {
        // Every ASCII string of length 1 and 2, alone and after a valid
        // identifier prefix ("id").
        let mut buf = [b'i', b'd', 0, 0];
        for a in 0u8..0x80 {
            let one = [a];
            let s = core::str::from_utf8(&one).unwrap_or_default();
            assert_eq!(is_xid(s), is_xid_reference(s), "{s:?}");
            for b in 0u8..0x80 {
                buf[2] = a;
                buf[3] = b;
                for bytes in [&buf[2..], &buf[..]] {
                    let s = core::str::from_utf8(bytes).unwrap_or_default();
                    assert_eq!(is_xid(s), is_xid_reference(s), "{s:?}");
                }
            }
        }
        // Non-ASCII strings take the general path.
        for s in [
            "café",
            "a\u{0301}",
            "\u{0301}a",
            "x_\u{0660}",
            "日本",
            "a b\u{00E9}",
        ] {
            assert_eq!(is_xid(s), is_xid_reference(s), "{s:?}");
        }
    }

    #[test]
    fn test_is_xid_accepts_unicode() {
        assert!(is_xid("café"));
        assert!(is_xid("Δx"));
        assert!(is_xid("naïve"));
    }
}
