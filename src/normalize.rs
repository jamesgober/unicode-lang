//! Unicode normalization: the four forms of [UAX #15].
//!
//! Normalization rewrites a string into a canonical shape so that sequences
//! which are *meant* to be equal compare byte-for-byte equal. The classic case
//! is `é`, which can be one precomposed scalar (`U+00E9`) or a base letter plus
//! a combining accent (`U+0065 U+0301`); both name the same character, and
//! normalization forces one spelling.
//!
//! Two axes give the four forms:
//!
//! - *Composition* vs *decomposition* — whether the result prefers precomposed
//!   scalars ([`Form::Nfc`], [`Form::Nfkc`]) or fully decomposed base + marks
//!   ([`Form::Nfd`], [`Form::Nfkd`]).
//! - *Canonical* vs *compatibility* — canonical forms preserve visual identity;
//!   the compatibility forms ([`Form::Nfkc`], [`Form::Nfkd`]) additionally fold
//!   formatting distinctions, mapping e.g. the ligature `ﬁ` to `fi` and
//!   fullwidth `Ａ` to `A`.
//!
//! For identifiers, [UAX #31] recommends NFC. For fold-and-compare tasks
//! (case-insensitive-style matching of visually similar text) NFKC is the usual
//! choice. When in doubt, NFC is the safe default.
//!
//! The implementation handles Hangul algorithmically (per UAX #15) and every
//! other scalar through the generated decomposition, combining-class, and
//! composition tables. It is verified against the official
//! `NormalizationTest.txt` conformance suite.
//!
//! [UAX #15]: https://www.unicode.org/reports/tr15/
//! [UAX #31]: https://www.unicode.org/reports/tr31/

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use crate::lookup::range_value;
use crate::tables;

/// A Unicode normalization form, selecting the target shape for [`normalize`]
/// and [`is_normalized`].
///
/// # Examples
///
/// ```
/// use unicode_lang::{normalize, Form};
///
/// // NFC composes; NFD decomposes. Both round-trip the same text.
/// let composed = normalize("e\u{0301}", Form::Nfc);
/// assert_eq!(composed, "é");
/// let decomposed = normalize("é", Form::Nfd);
/// assert_eq!(decomposed, "e\u{0301}");
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Form {
    /// Normalization Form C — canonical decomposition followed by canonical
    /// composition. The most compact canonical form; the identifier default.
    Nfc,
    /// Normalization Form D — canonical decomposition only.
    Nfd,
    /// Normalization Form KC — compatibility decomposition followed by
    /// canonical composition. Folds compatibility distinctions.
    Nfkc,
    /// Normalization Form KD — compatibility decomposition only.
    Nfkd,
}

impl Form {
    /// Whether this form recomposes after decomposing (NFC / NFKC).
    #[inline]
    const fn composes(self) -> bool {
        matches!(self, Form::Nfc | Form::Nfkc)
    }

    /// Whether this form uses compatibility decomposition (NFKC / NFKD).
    #[inline]
    const fn compat(self) -> bool {
        matches!(self, Form::Nfkc | Form::Nfkd)
    }
}

// Hangul jamo composition parameters (UAX #15, "Hangul").
const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT; // 588
const S_COUNT: u32 = L_COUNT * N_COUNT; // 11172

/// Returns `s` normalized to `form`.
///
/// When `s` is already in the requested form this returns it unchanged after a
/// fast allocation-free scan (the Unicode quick-check), so passing text that is
/// already normalized — the common case for ASCII and most well-formed input —
/// costs one linear pass and a single allocation for the returned `String`.
/// Pure-ASCII input is recognised by a word-at-a-time check and copied without
/// consulting any table.
///
/// Normalization runs in `O(n log n)` time in the length of `s` even on hostile
/// input such as a long run of combining marks with alternating classes.
///
/// # Examples
///
/// ```
/// use unicode_lang::{normalize, Form};
///
/// // Compatibility composition folds a ligature and a fullwidth digit.
/// assert_eq!(normalize("ﬁle", Form::Nfkc), "file");
/// assert_eq!(normalize("\u{FF11}", Form::Nfkc), "1");
///
/// // Canonical decomposition splits a precomposed character and orders marks.
/// assert_eq!(normalize("ǭ", Form::Nfd), "o\u{0328}\u{0304}");
///
/// // ASCII is returned untouched.
/// assert_eq!(normalize("plain ascii", Form::Nfc), "plain ascii");
/// ```
#[must_use]
pub fn normalize(s: &str, form: Form) -> String {
    // Every pure-ASCII string is a fixed point of all four forms: no ASCII
    // scalar decomposes or has a non-zero combining class, and no two ASCII
    // scalars compose with each other.
    // `str::is_ascii` checks a word at a time, far cheaper than the per-scalar
    // quick-check.
    if s.is_ascii() {
        return String::from(s);
    }
    if let Quick::Yes = quick_check(s, form) {
        return String::from(s);
    }
    let mut buf = decompose(s, form.compat());
    if form.composes() {
        compose(&mut buf);
    }
    buf.into_iter().collect()
}

/// Returns `true` if `s` is already in normalization form `form`.
///
/// The check begins with the allocation-free Unicode quick-check. Most inputs
/// are decided there; only genuinely ambiguous input triggers a full
/// normalization to resolve, and even then no `String` is allocated — the
/// normalized scalars are compared against `s` directly.
///
/// # Examples
///
/// ```
/// use unicode_lang::{is_normalized, Form};
///
/// assert!(is_normalized("é", Form::Nfc));            // precomposed: already NFC
/// assert!(!is_normalized("e\u{0301}", Form::Nfc));   // decomposed: not NFC
/// assert!(is_normalized("e\u{0301}", Form::Nfd));    // ...but it is NFD
///
/// assert!(is_normalized("ascii only", Form::Nfkc));
/// assert!(!is_normalized("ﬁ", Form::Nfkc));          // ligature folds under NFKC
/// ```
#[must_use]
pub fn is_normalized(s: &str, form: Form) -> bool {
    // ASCII is normalized under every form (see `normalize`).
    if s.is_ascii() {
        return true;
    }
    match quick_check(s, form) {
        Quick::Yes => true,
        Quick::No => false,
        Quick::Maybe => {
            // Resolve the ambiguous case by normalizing and comparing scalars,
            // without materialising an intermediate `String`.
            let mut buf = decompose(s, form.compat());
            if form.composes() {
                compose(&mut buf);
            }
            buf.into_iter().eq(s.chars())
        }
    }
}

/// Outcome of the Unicode quick-check: definitely normalized, definitely not,
/// or requires a full pass to decide.
enum Quick {
    Yes,
    No,
    Maybe,
}

/// The [quick-check algorithm][qc]: scans `s` once, consulting the per-form
/// `*_QC` property and the canonical ordering of combining marks.
///
/// [qc]: https://www.unicode.org/reports/tr15/#Detecting_Normalization_Forms
fn quick_check(s: &str, form: Form) -> Quick {
    let qc = match form {
        Form::Nfc => tables::NFC_QC,
        Form::Nfd => tables::NFD_QC,
        Form::Nfkc => tables::NFKC_QC,
        Form::Nfkd => tables::NFKD_QC,
    };
    let mut last_ccc = 0u8;
    let mut result = Quick::Yes;
    for c in s.chars() {
        // ASCII is a starter (class 0) with quick-check `Yes` in every form, so
        // it only resets the ordering state; skip both table lookups.
        if c.is_ascii() {
            last_ccc = 0;
            continue;
        }
        let cc = ccc(c);
        // Marks out of canonical order prove the string is not normalized.
        if last_ccc > cc && cc != 0 {
            return Quick::No;
        }
        match range_value(c as u32, qc) {
            1 => return Quick::No,
            2 => result = Quick::Maybe,
            _ => {}
        }
        last_ccc = cc;
    }
    result
}

/// Canonical combining class of `c` (0 for the vast majority of scalars).
#[inline]
fn ccc(c: char) -> u8 {
    range_value(c as u32, tables::CCC)
}

/// Fully decompose `s` (canonical, or compatibility when `compat`) and place
/// the result in canonical order.
fn decompose(s: &str, compat: bool) -> Vec<char> {
    let mut out: Vec<char> = Vec::with_capacity(s.len());
    for c in s.chars() {
        decompose_char(c, compat, &mut out);
    }
    canonical_order(&mut out);
    out
}

/// Append the full decomposition of one scalar to `out`.
fn decompose_char(c: char, compat: bool, out: &mut Vec<char>) {
    let cp = c as u32;
    // No ASCII scalar has a canonical or compatibility decomposition (checked
    // against the tables by `test_no_ascii_decompositions`).
    if cp < 0x80 {
        out.push(c);
        return;
    }
    if (S_BASE..S_BASE + S_COUNT).contains(&cp) {
        hangul_decompose(cp, out);
        return;
    }
    let (index, data): (&[(u32, u32, u32)], &[u32]) = if compat {
        (tables::COMPAT_DECOMP, tables::COMPAT_DATA)
    } else {
        (tables::CANON_DECOMP, tables::CANON_DATA)
    };
    if let Ok(i) = index.binary_search_by_key(&cp, |&(key, _, _)| key) {
        let (_, off, len) = index[i];
        let (off, len) = (off as usize, len as usize);
        out.extend(
            data[off..off + len]
                .iter()
                .filter_map(|&d| char::from_u32(d)),
        );
    } else {
        out.push(c);
    }
}

/// Algorithmic Hangul syllable decomposition (LV or LVT jamo).
fn hangul_decompose(cp: u32, out: &mut Vec<char>) {
    let s = cp - S_BASE;
    push_scalar(out, L_BASE + s / N_COUNT);
    push_scalar(out, V_BASE + (s % N_COUNT) / T_COUNT);
    let t = s % T_COUNT;
    if t != 0 {
        push_scalar(out, T_BASE + t);
    }
}

#[inline]
fn push_scalar(out: &mut Vec<char>, cp: u32) {
    if let Some(c) = char::from_u32(cp) {
        out.push(c);
    }
}

/// Reorder combining marks into canonical order: a stable sort by combining
/// class within each maximal run of non-starter scalars. Starters (class 0) are
/// fixed points and act as barriers, so marks never move past one.
///
/// Each run is scanned once; a run already in order (by far the common case —
/// one or two marks, usually decomposed in order) is left untouched. Only a run
/// with a descent is sorted, with the standard library's stable sort over
/// `(class, scalar)` pairs so each class is looked up once. That bounds the
/// whole pass at `O(n log n)` — an insertion sort here was `O(n²)` on a long run
/// of marks whose classes alternate, which hostile identifiers can supply
/// because combining marks are `XID_Continue`. The result is identical: both
/// are stable sorts by class within the same runs.
fn canonical_order(chars: &mut [char]) {
    // Scratch for out-of-order runs, allocated on first use and reused.
    let mut run: Vec<(u8, char)> = Vec::new();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        let first = ccc(chars[i]);
        if first == 0 {
            i += 1;
            continue;
        }
        let start = i;
        let mut prev = first;
        let mut sorted = true;
        i += 1;
        while i < n {
            let cc = ccc(chars[i]);
            if cc == 0 {
                break;
            }
            if cc < prev {
                sorted = false;
            }
            prev = cc;
            i += 1;
        }
        if !sorted {
            let marks = &mut chars[start..i];
            run.clear();
            run.extend(marks.iter().map(|&c| (ccc(c), c)));
            run.sort_by_key(|&(class, _)| class);
            for (slot, &(_, c)) in marks.iter_mut().zip(run.iter()) {
                *slot = c;
            }
        }
    }
}

/// Canonically compose a decomposed, canonically ordered scalar buffer in
/// place (the recomposition step of NFC / NFKC).
fn compose(chars: &mut Vec<char>) {
    if chars.len() < 2 {
        return;
    }
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    // Index in `out` of the most recent starter that can still absorb a
    // following combining mark, and the combining class of the last scalar
    // pushed (0 while the starter is still bare).
    let mut starter: Option<usize> = None;
    let mut last_ccc = 0u8;

    for &c in chars.iter() {
        let cc = ccc(c);
        if let Some(sp) = starter {
            // A combining mark is blocked from the starter if some scalar
            // between them has an equal-or-greater class. Because the buffer is
            // canonically ordered, tracking only the previous class suffices.
            if last_ccc == 0 || last_ccc < cc {
                if let Some(composite) = primary_composite(out[sp], c) {
                    out[sp] = composite;
                    continue;
                }
            }
        }
        out.push(c);
        if cc == 0 {
            starter = Some(out.len() - 1);
            last_ccc = 0;
        } else {
            last_ccc = cc;
        }
    }
    *chars = out;
}

/// The primary composite of a starter `a` and following scalar `b`, if one
/// exists: Hangul jamo by formula, everything else by table.
fn primary_composite(a: char, b: char) -> Option<char> {
    let (ca, cb) = (a as u32, b as u32);

    // No primary composite has an ASCII second element (checked against the
    // table by `test_no_ascii_composition_tail`), and the Hangul vowel and
    // trailing jamo are not ASCII either.
    if cb < 0x80 {
        return None;
    }

    // Hangul: leading + vowel jamo -> LV syllable.
    if (L_BASE..L_BASE + L_COUNT).contains(&ca) && (V_BASE..V_BASE + V_COUNT).contains(&cb) {
        let li = ca - L_BASE;
        let vi = cb - V_BASE;
        return char::from_u32(S_BASE + (li * V_COUNT + vi) * T_COUNT);
    }
    // Hangul: LV syllable + trailing jamo -> LVT syllable.
    if (S_BASE..S_BASE + S_COUNT).contains(&ca)
        && (ca - S_BASE) % T_COUNT == 0
        && (T_BASE + 1..T_BASE + T_COUNT).contains(&cb)
    {
        return char::from_u32(ca + (cb - T_BASE));
    }

    let key = (u64::from(ca) << 32) | u64::from(cb);
    match tables::COMPOSE.binary_search_by_key(&key, |&(k, _)| k) {
        Ok(i) => char::from_u32(tables::COMPOSE[i].1),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nfc_composes_base_and_mark() {
        assert_eq!(normalize("e\u{0301}", Form::Nfc), "é");
    }

    #[test]
    fn test_nfd_decomposes_precomposed() {
        assert_eq!(normalize("é", Form::Nfd), "e\u{0301}");
    }

    #[test]
    fn test_nfd_orders_marks_by_class() {
        // Two marks given out of canonical order (class 230 then 220) must be
        // reordered (220 before 230).
        let input = "a\u{0301}\u{0323}"; // acute (230) then dot-below (220)
        assert_eq!(normalize(input, Form::Nfd), "a\u{0323}\u{0301}");
    }

    #[test]
    fn test_nfkc_folds_ligature() {
        assert_eq!(normalize("ﬁ", Form::Nfkc), "fi");
        assert_eq!(normalize("\u{FF21}", Form::Nfkc), "A"); // fullwidth A
    }

    #[test]
    fn test_nfkd_expands_compatibility() {
        assert_eq!(normalize("½", Form::Nfkd), "1\u{2044}2");
    }

    #[test]
    fn test_hangul_roundtrip() {
        let syllable = "\u{AC00}"; // 가
        let decomposed = normalize(syllable, Form::Nfd);
        assert_eq!(decomposed, "\u{1100}\u{1161}");
        assert_eq!(normalize(&decomposed, Form::Nfc), syllable);
    }

    #[test]
    fn test_hangul_lvt() {
        let syllable = "\u{AC01}"; // 각 (LVT)
        assert_eq!(normalize(syllable, Form::Nfd), "\u{1100}\u{1161}\u{11A8}");
        assert_eq!(normalize("\u{1100}\u{1161}\u{11A8}", Form::Nfc), syllable);
    }

    #[test]
    fn test_ascii_unchanged() {
        for form in [Form::Nfc, Form::Nfd, Form::Nfkc, Form::Nfkd] {
            assert_eq!(normalize("hello world 123", form), "hello world 123");
        }
    }

    #[test]
    fn test_idempotent() {
        let samples = ["e\u{0301}", "ﬁ", "가", "½", "Ａ", "a\u{0323}\u{0301}"];
        for s in samples {
            for form in [Form::Nfc, Form::Nfd, Form::Nfkc, Form::Nfkd] {
                let once = normalize(s, form);
                let twice = normalize(&once, form);
                assert_eq!(once, twice, "not idempotent: {s:?} {form:?}");
            }
        }
    }

    /// The pre-1.0.1 canonical ordering (an insertion sort), kept as the
    /// reference the replacement must match exactly.
    fn canonical_order_reference(chars: &mut [char]) {
        let n = chars.len();
        let mut i = 1;
        while i < n {
            let cc = ccc(chars[i]);
            if cc != 0 {
                let mut j = i;
                while j > 0 && ccc(chars[j - 1]) > cc {
                    chars.swap(j - 1, j);
                    j -= 1;
                }
            }
            i += 1;
        }
    }

    /// Scalars covering starters and non-starters, including several marks of
    /// equal class (so stability is observable) and classes on both sides of
    /// each other.
    const ORDER_POOL: [char; 16] = [
        'a',         // starter
        'e',         // starter
        '\u{0300}',  // 230
        '\u{0301}',  // 230
        '\u{0302}',  // 230
        '\u{0316}',  // 220
        '\u{0323}',  // 220
        '\u{0327}',  // 202
        '\u{0328}',  // 202
        '\u{0334}',  // 1
        '\u{0335}',  // 1
        '\u{05B0}',  // 10
        '\u{0345}',  // 240
        '\u{1D16D}', // 226
        '\u{0F71}',  // 129
        '\u{1100}',  // starter (Hangul choseong)
    ];

    fn check_order_matches_reference(input: &[char]) {
        let mut fast = input.to_vec();
        let mut slow = input.to_vec();
        canonical_order(&mut fast);
        canonical_order_reference(&mut slow);
        assert_eq!(fast, slow, "canonical order differs for {input:?}");
    }

    #[test]
    fn test_canonical_order_matches_reference_exhaustive_small() {
        // Every sequence of length 0..=4 over the pool (69 905 sequences).
        let pool = ORDER_POOL;
        let mut buf = [' '; 4];
        for len in 0..=4u32 {
            let total = pool.len().pow(len);
            for mut idx in 0..total {
                for slot in buf.iter_mut().take(len as usize) {
                    *slot = pool[idx % pool.len()];
                    idx /= pool.len();
                }
                check_order_matches_reference(&buf[..len as usize]);
            }
        }
    }

    #[test]
    fn test_canonical_order_long_alternating_run() {
        let mut input = alloc::vec!['a'];
        for i in 0..2_000 {
            input.push(if i % 2 == 0 { '\u{0301}' } else { '\u{0323}' });
        }
        input.push('b');
        input.extend_from_slice(&['\u{0300}', '\u{0316}', '\u{0301}', '\u{0323}']);
        check_order_matches_reference(&input);
    }

    #[test]
    fn test_canonical_order_is_stable_within_class() {
        // U+0301 and U+0300 share class 230 and must keep their relative order
        // when the class-220 mark moves in front of them.
        let mut v = alloc::vec!['a', '\u{0301}', '\u{0300}', '\u{0323}'];
        canonical_order(&mut v);
        assert_eq!(v, ['a', '\u{0323}', '\u{0301}', '\u{0300}']);
    }

    #[test]
    fn test_canonical_order_run_at_buffer_edges() {
        // A run at the very start (no starter before it) and one at the end.
        check_order_matches_reference(&[
            '\u{0301}', '\u{0323}', '\u{0327}', 'a', '\u{0345}', '\u{0334}',
        ]);
    }

    proptest::proptest! {
        #[test]
        fn prop_canonical_order_matches_reference(
            picks in proptest::collection::vec(0usize..ORDER_POOL.len(), 0..64)
        ) {
            let input: Vec<char> = picks.iter().map(|&i| ORDER_POOL[i]).collect();
            let mut fast = input.clone();
            let mut slow = input;
            canonical_order(&mut fast);
            canonical_order_reference(&mut slow);
            proptest::prop_assert_eq!(fast, slow);
        }

        #[test]
        fn prop_canonical_order_matches_reference_any_scalar(
            input in proptest::collection::vec(proptest::prelude::any::<char>(), 0..48)
        ) {
            let mut fast = input.clone();
            let mut slow = input;
            canonical_order(&mut fast);
            canonical_order_reference(&mut slow);
            proptest::prop_assert_eq!(fast, slow);
        }
    }

    /// A plain binary search over a `(start, end, value)` table with no fast
    /// path: the reference `range_value` must agree with.
    fn range_value_reference(cp: u32, table: &[(u32, u32, u8)]) -> u8 {
        let idx = table.partition_point(|&(_, end, _)| end < cp);
        match table.get(idx) {
            Some(&(start, _, value)) if cp >= start => value,
            _ => 0,
        }
    }

    #[test]
    fn test_range_value_fast_path_exhaustive() {
        // The sub-0x80 shortcut in `range_value` (combining class and every
        // quick-check table) agrees with the full lookup at every code point.
        let tables_under_test = [
            tables::CCC,
            tables::NFC_QC,
            tables::NFD_QC,
            tables::NFKC_QC,
            tables::NFKD_QC,
        ];
        for table in tables_under_test {
            for cp in 0u32..=0x10_FFFF {
                assert_eq!(
                    range_value(cp, table),
                    range_value_reference(cp, table),
                    "{cp:#X}"
                );
            }
        }
    }

    #[test]
    fn test_no_ascii_decompositions() {
        // Justifies the ASCII shortcut in `decompose_char`.
        assert!(tables::CANON_DECOMP.iter().all(|&(cp, _, _)| cp >= 0x80));
        assert!(tables::COMPAT_DECOMP.iter().all(|&(cp, _, _)| cp >= 0x80));
    }

    #[test]
    fn test_no_ascii_composition_tail() {
        // Justifies the ASCII shortcut in `primary_composite`: no table pair
        // has an ASCII second element, so skipping the search changes nothing.
        assert!(
            tables::COMPOSE
                .iter()
                .all(|&(key, _)| (key & 0xFFFF_FFFF) >= 0x80)
        );
    }

    #[test]
    fn test_decompose_char_ascii_is_identity() {
        for c in '\0'..='\u{7F}' {
            for compat in [false, true] {
                let mut out = Vec::new();
                decompose_char(c, compat, &mut out);
                assert_eq!(out, [c]);
            }
        }
    }

    #[test]
    fn test_ascii_fast_paths_agree_with_general_path() {
        for c in '\0'..='\u{7F}' {
            // Pure ASCII: the whole-string shortcut.
            let alone = alloc::format!("{c}");
            for form in [Form::Nfc, Form::Nfd, Form::Nfkc, Form::Nfkd] {
                assert_eq!(normalize(&alone, form), alone);
                assert!(is_normalized(&alone, form));
                assert!(matches!(quick_check(&alone, form), Quick::Yes));
            }
            // Mixed: forces the general path through an ASCII scalar placed
            // between a mark and a base that composes.
            let mixed = alloc::format!("e\u{0301}{c}A\u{0300}");
            assert_eq!(normalize(&mixed, Form::Nfc), alloc::format!("é{c}À"));
            assert_eq!(normalize(&mixed, Form::Nfd), mixed);
            assert!(is_normalized(&mixed, Form::Nfd));
            assert!(!is_normalized(&mixed, Form::Nfc));
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    #[allow(clippy::unwrap_used)] // serde_json in a test; failure is a test failure
    fn test_form_serde_roundtrip() {
        for form in [Form::Nfc, Form::Nfd, Form::Nfkc, Form::Nfkd] {
            let json = serde_json::to_string(&form).unwrap();
            let back: Form = serde_json::from_str(&json).unwrap();
            assert_eq!(form, back);
        }
        assert_eq!(serde_json::to_string(&Form::Nfkc).unwrap(), "\"Nfkc\"");
    }

    #[test]
    fn test_is_normalized_agrees_with_normalize() {
        let samples = [
            "",
            "abc",
            "é",
            "e\u{0301}",
            "ﬁ",
            "가",
            "½",
            "a\u{0323}\u{0301}",
        ];
        for s in samples {
            for form in [Form::Nfc, Form::Nfd, Form::Nfkc, Form::Nfkd] {
                let expected = normalize(s, form) == s;
                assert_eq!(is_normalized(s, form), expected, "{s:?} {form:?}");
            }
        }
    }
}
