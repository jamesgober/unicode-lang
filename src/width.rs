//! Monospace display width — how many terminal columns a scalar occupies.
//!
//! The contract follows the long-established `wcwidth` convention, refined by
//! the Unicode [East Asian Width][uax11] property:
//!
//! - **0 columns** — the null character, the C0/C1 control blocks, nonspacing
//!   and enclosing combining marks, format characters, and the conjoining
//!   Hangul jamo. These add nothing to the visible advance of the text.
//! - **2 columns** — East Asian *Wide* and *Fullwidth* characters: CJK
//!   ideographs, fullwidth forms, most emoji-range ideographic symbols.
//! - **1 column** — everything else, including East Asian *Ambiguous*, which
//!   is treated as narrow because its width depends on locale context this
//!   crate deliberately does not carry.
//!
//! Width is a rendering estimate for fixed-pitch output (terminals, column
//! alignment, truncation). It is not grapheme segmentation: a base letter and
//! its combining marks sum correctly (the marks contribute 0), but the crate
//! does not merge them into a single cluster.
//!
//! [uax11]: https://www.unicode.org/reports/tr11/

use crate::lookup::in_ranges;
use crate::tables;

/// Returns the number of monospace columns `c` occupies: `0`, `1`, or `2`.
///
/// See the module-level documentation for the exact contract.
///
/// # Examples
///
/// ```
/// use unicode_lang::char_width;
///
/// assert_eq!(char_width('A'), 1);
/// assert_eq!(char_width('世'), 2);          // CJK ideograph, wide
/// assert_eq!(char_width('\u{0301}'), 0);    // COMBINING ACUTE ACCENT
/// assert_eq!(char_width('\n'), 0);          // control characters take no column
/// assert_eq!(char_width('\u{200B}'), 0);    // ZERO WIDTH SPACE
/// ```
#[inline]
#[must_use]
pub fn char_width(c: char) -> usize {
    let cp = c as u32;
    // Printable ASCII is one column and the C0 controls are zero. DEL (0x7F)
    // begins the C1 control range, so it takes the general path. Checked
    // against the tables at every code point by
    // `test_fast_path_matches_tables_exhaustive`.
    if cp < 0x7F {
        return usize::from(cp >= 0x20);
    }
    width_from_tables(cp)
}

/// Column width from the control ranges and the generated tables, with no
/// ASCII shortcut.
#[inline]
fn width_from_tables(cp: u32) -> usize {
    // The C0 (< 0x20, includes NUL) and C1 (0x7F..=0x9F) control blocks have no
    // printable advance. They are not in the zero-width table (that is combining
    // and format characters), so they are handled explicitly.
    if cp < 0x20 || (0x7F..0xA0).contains(&cp) {
        return 0;
    }
    if in_ranges(cp, tables::ZERO_WIDTH) {
        return 0;
    }
    if in_ranges(cp, tables::WIDE) {
        return 2;
    }
    1
}

/// Returns the total monospace width of `s`: the sum of [`char_width`] over its
/// scalar values.
///
/// # Examples
///
/// ```
/// use unicode_lang::str_width;
///
/// assert_eq!(str_width("hello"), 5);
/// assert_eq!(str_width("日本語"), 6);      // three wide ideographs
/// assert_eq!(str_width("café"), 4);        // precomposed é is one column
///
/// // A base letter plus a combining mark still advances by one column.
/// assert_eq!(str_width("e\u{0301}"), 1);   // e + COMBINING ACUTE ACCENT
/// ```
#[inline]
#[must_use]
pub fn str_width(s: &str) -> usize {
    // Pure ASCII: every byte is one column except the C0 controls and DEL. A
    // byte count needs no UTF-8 decoding.
    if s.is_ascii() {
        return s.bytes().filter(|&b| b >= 0x20 && b != 0x7F).count();
    }
    s.chars().map(char_width).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_printable_is_one() {
        for c in '\u{20}'..='\u{7E}' {
            assert_eq!(char_width(c), 1, "{c:?}");
        }
    }

    #[test]
    fn test_controls_are_zero() {
        assert_eq!(char_width('\0'), 0);
        assert_eq!(char_width('\t'), 0);
        assert_eq!(char_width('\n'), 0);
        assert_eq!(char_width('\u{1B}'), 0); // ESC
        assert_eq!(char_width('\u{7F}'), 0); // DEL
        assert_eq!(char_width('\u{85}'), 0); // NEL, C1
    }

    #[test]
    fn test_wide_cjk_is_two() {
        assert_eq!(char_width('世'), 2);
        assert_eq!(char_width('界'), 2);
        assert_eq!(char_width('\u{FF21}'), 2); // FULLWIDTH LATIN CAPITAL A
    }

    #[test]
    fn test_combining_marks_are_zero() {
        assert_eq!(char_width('\u{0301}'), 0);
        assert_eq!(char_width('\u{0300}'), 0);
        assert_eq!(char_width('\u{200B}'), 0); // ZERO WIDTH SPACE
    }

    #[test]
    fn test_soft_hyphen_is_one() {
        // SOFT HYPHEN is category Cf but occupies a column when displayed.
        assert_eq!(char_width('\u{00AD}'), 1);
    }

    #[test]
    fn test_fast_path_matches_tables_exhaustive() {
        for cp in 0u32..=0x10_FFFF {
            if let Some(c) = char::from_u32(cp) {
                assert_eq!(char_width(c), width_from_tables(cp), "{cp:#X}");
            }
        }
    }

    #[test]
    fn test_str_width_ascii_path_matches_per_scalar() {
        // Every ASCII byte, alone and all 128 together.
        let mut all = [0u8; 128];
        for (slot, b) in all.iter_mut().zip(0u8..) {
            *slot = b;
        }
        let whole = core::str::from_utf8(&all).unwrap_or_default();
        assert_eq!(whole.len(), 128);
        let per_scalar: usize = whole.chars().map(|c| width_from_tables(c as u32)).sum();
        assert_eq!(str_width(whole), per_scalar);
        for c in whole.chars() {
            let mut b = [0u8; 4];
            assert_eq!(
                str_width(c.encode_utf8(&mut b)),
                width_from_tables(c as u32),
                "{c:?}"
            );
        }
        // Mixed input takes the per-scalar path.
        assert_eq!(str_width("a\u{7F}世\t"), 3);
    }

    #[test]
    fn test_str_width_sums() {
        assert_eq!(str_width(""), 0);
        assert_eq!(str_width("abc"), 3);
        assert_eq!(str_width("a世b"), 4);
        assert_eq!(str_width("e\u{0301}"), 1);
    }
}
