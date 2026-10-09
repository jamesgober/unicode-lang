//! Hostile-input bounds.
//!
//! Combining marks are `XID_Continue`, so an identifier a lexer accepts can be
//! one base letter followed by a million marks. Before 1.0.1, canonical
//! ordering was an insertion sort, and a run of marks whose classes alternate
//! (U+0301 is class 230, U+0323 is class 220) made it quadratic: about ten
//! minutes for a million marks in a release build. These tests pin the
//! replacement's behaviour on that input: exact output, and time that grows
//! like `n log n`, not `n²`.
//!
//! The timing assertions are deliberately loose (they must hold in an
//! unoptimized build on a busy CI runner); the quadratic algorithm misses them
//! by orders of magnitude, not by a margin.

#![cfg(feature = "alloc")]
#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};
use unicode_lang::{Form, is_normalized, is_xid, normalize, str_width};

const ACUTE: char = '\u{0301}'; // COMBINING ACUTE ACCENT, class 230
const DOT_BELOW: char = '\u{0323}'; // COMBINING DOT BELOW, class 220

/// `a` followed by `marks` combining marks alternating 230, 220, 230, ...
fn alternating(marks: usize) -> String {
    let mut s = String::with_capacity(1 + marks * 2);
    s.push('a');
    for i in 0..marks {
        s.push(if i % 2 == 0 { ACUTE } else { DOT_BELOW });
    }
    s
}

/// The canonical order of `alternating(marks)`: every class-220 mark before
/// every class-230 mark, each group in its original (stable) order.
fn alternating_nfd(marks: usize) -> String {
    let dots = marks / 2;
    let acutes = marks - dots;
    let mut s = String::from("a");
    s.extend(std::iter::repeat_n(DOT_BELOW, dots));
    s.extend(std::iter::repeat_n(ACUTE, acutes));
    s
}

/// NFC of `alternating(marks)` for even `marks >= 2`: `a` absorbs the first
/// dot below into U+1EA1 (LATIN SMALL LETTER A WITH DOT BELOW); every later
/// dot below is blocked by the one before it (equal class), and U+1EA1 has no
/// composite with the acute, so the rest stays decomposed.
fn alternating_nfc(marks: usize) -> String {
    let dots = marks / 2;
    let acutes = marks - dots;
    let mut s = String::from("\u{1EA1}");
    s.extend(std::iter::repeat_n(DOT_BELOW, dots - 1));
    s.extend(std::iter::repeat_n(ACUTE, acutes));
    s
}

/// The fastest of `reps` timed runs of `f`, to filter scheduler noise.
fn best_of(reps: usize, mut f: impl FnMut()) -> Duration {
    (0..reps)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .min()
        .unwrap()
}

#[test]
fn million_alternating_marks_normalize_exactly() {
    const MARKS: usize = 1_000_000;
    let input = alternating(MARKS);
    let started = Instant::now();

    // The hostile identifier is a valid identifier and one column wide.
    assert!(is_xid(&input));
    assert_eq!(str_width(&input), 1);

    let nfd = normalize(&input, Form::Nfd);
    assert_eq!(nfd, alternating_nfd(MARKS));
    // No compatibility mappings are involved, so NFKD agrees with NFD.
    assert_eq!(normalize(&input, Form::Nfkd), nfd);

    let nfc = normalize(&input, Form::Nfc);
    assert_eq!(nfc, alternating_nfc(MARKS));
    assert_eq!(normalize(&input, Form::Nfkc), nfc);

    // Out of order: the quick-check rejects it without normalizing.
    assert!(!is_normalized(&input, Form::Nfd));
    // In order but containing `Maybe` scalars: decided by a full pass.
    assert!(is_normalized(&nfd, Form::Nfd));
    assert!(is_normalized(&nfc, Form::Nfc));
    assert!(!is_normalized(&nfd, Form::Nfc));

    // Quadratic ordering needed about ten minutes for this in a release
    // build; the n log n sort needs well under a second there and a few
    // seconds unoptimized.
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(60),
        "1M alternating marks took {elapsed:?}"
    );
}

#[test]
fn alternating_marks_scale_near_linearly() {
    // Eight times the input must cost far less than the 64x a quadratic sort
    // needs. n log n predicts about 9x; 24x leaves room for cache effects and
    // a noisy runner while still failing any quadratic regression.
    const SMALL: usize = 64_000;
    const LARGE: usize = SMALL * 8;
    let small = alternating(SMALL);
    let large = alternating(LARGE);

    // Warm up allocator and caches before timing.
    let _ = normalize(&small, Form::Nfd);

    let t_small = best_of(5, || {
        let _ = std::hint::black_box(normalize(std::hint::black_box(&small), Form::Nfd));
    });
    let t_large = best_of(3, || {
        let _ = std::hint::black_box(normalize(std::hint::black_box(&large), Form::Nfd));
    });

    // Guard against a timer too coarse to measure the small case.
    let floor = Duration::from_micros(200);
    let ratio = t_large.as_secs_f64() / t_small.max(floor).as_secs_f64();
    assert!(
        ratio < 24.0,
        "8x input took {ratio:.1}x the time ({t_small:?} -> {t_large:?}); \
         canonical ordering may have regressed to quadratic"
    );
}

#[test]
fn many_short_runs_are_linear() {
    // Many independent out-of-order runs separated by starters: each run is
    // sorted on its own, and the starters act as barriers.
    let unit = "a\u{0301}\u{0323}";
    let input = unit.repeat(200_000);
    let expected = "a\u{0323}\u{0301}".repeat(200_000);
    assert_eq!(normalize(&input, Form::Nfd), expected);
    assert_eq!(
        normalize(&input, Form::Nfc),
        "\u{1EA1}\u{0301}".repeat(200_000)
    );
}

#[test]
fn long_run_of_equal_class_marks_is_unchanged() {
    // A run already in order (one class) needs no sort and must keep its
    // order exactly.
    let input = format!("a{}", "\u{0301}\u{0300}".repeat(100_000));
    assert_eq!(normalize(&input, Form::Nfd), input);
    assert!(is_normalized(&input, Form::Nfd));
}

#[test]
fn leading_marks_with_no_starter() {
    // A run of marks at the very start of the text, with no base letter.
    let input: String = alternating(10_001).chars().skip(1).collect();
    let nfd = normalize(&input, Form::Nfd);
    let dots = nfd.chars().take_while(|&c| c == DOT_BELOW).count();
    assert_eq!(dots, 5_000);
    assert!(nfd.chars().skip(dots).all(|c| c == ACUTE));
    assert_eq!(nfd.chars().count(), 10_001);
}
