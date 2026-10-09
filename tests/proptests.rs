//! Property-based tests over the public surface.
//!
//! These assert the algebraic laws the API promises for *arbitrary* input,
//! rather than the specific vectors in `conformance.rs`: normalization is
//! idempotent and stable, `is_normalized` agrees with `normalize`, the forms
//! compose as UAX #15 requires, width is additive, and `is_xid` is exactly the
//! per-scalar predicate applied across a string. The ASCII strategies exercise
//! the whole-string fast paths added in 1.0.1. The normalization properties
//! need the `alloc` feature; the rest run in every configuration.

#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use unicode_lang::{char_width, is_xid, is_xid_continue, is_xid_start, str_width};

fn any_string() -> impl Strategy<Value = String> {
    proptest::collection::vec(any::<char>(), 0..24).prop_map(|v| v.into_iter().collect())
}

/// Pure-ASCII strings, which take the whole-string fast paths.
fn ascii_string() -> impl Strategy<Value = String> {
    proptest::collection::vec(0u8..0x80, 0..48)
        .prop_map(|v| v.into_iter().map(char::from).collect())
}

proptest! {
    #[test]
    fn width_is_additive(a in any_string(), b in any_string()) {
        prop_assert_eq!(str_width(&a) + str_width(&b), str_width(&(a.clone() + &b)));
    }

    #[test]
    fn char_width_in_range(c in any::<char>()) {
        prop_assert!(char_width(c) <= 2);
    }

    #[test]
    fn is_xid_is_per_scalar_predicate(s in any_string()) {
        let mut chars = s.chars();
        let expected = match chars.next() {
            Some(first) => is_xid_start(first) && chars.all(is_xid_continue),
            None => false,
        };
        prop_assert_eq!(is_xid(&s), expected);
    }

    #[test]
    fn ascii_str_width_matches_per_scalar(s in ascii_string()) {
        prop_assert_eq!(str_width(&s), s.chars().map(char_width).sum::<usize>());
    }

    #[test]
    fn ascii_is_xid_is_per_scalar_predicate(s in ascii_string()) {
        let mut chars = s.chars();
        let expected = match chars.next() {
            Some(first) => is_xid_start(first) && chars.all(is_xid_continue),
            None => false,
        };
        prop_assert_eq!(is_xid(&s), expected);
    }

    #[test]
    fn xid_start_implies_continue(c in any::<char>()) {
        if is_xid_start(c) {
            prop_assert!(is_xid_continue(c));
        }
    }
}

#[cfg(feature = "alloc")]
mod normalization {
    use super::{any_string, ascii_string};
    use proptest::prelude::*;
    use unicode_lang::{Form, is_normalized, normalize};

    const FORMS: [Form; 4] = [Form::Nfc, Form::Nfd, Form::Nfkc, Form::Nfkd];

    /// Strings weighted toward combining marks of mixed classes, so canonical
    /// ordering has real work (long runs, descents, equal classes).
    fn marky_string() -> impl Strategy<Value = String> {
        let pool = vec![
            'a', 'A', 'o', '\u{0300}', '\u{0301}', '\u{0302}', '\u{0316}', '\u{0323}', '\u{0327}',
            '\u{0328}', '\u{0334}', '\u{05B0}', '\u{0345}', '\u{0F71}', '\u{1100}', '\u{1161}',
            '\u{11A8}', '\u{00E9}', '\u{1EA1}',
        ];
        proptest::collection::vec(proptest::sample::select(pool), 0..64)
            .prop_map(|v| v.into_iter().collect())
    }

    proptest! {
        #[test]
        fn normalize_is_idempotent(s in any_string()) {
            for form in FORMS {
                let once = normalize(&s, form);
                let twice = normalize(&once, form);
                prop_assert_eq!(&once, &twice, "form {:?}", form);
            }
        }

        #[test]
        fn normalized_output_reports_normalized(s in any_string()) {
            for form in FORMS {
                let n = normalize(&s, form);
                prop_assert!(is_normalized(&n, form), "form {:?} on {:?}", form, n);
            }
        }

        #[test]
        fn is_normalized_matches_normalize(s in any_string()) {
            for form in FORMS {
                let expected = normalize(&s, form) == s;
                prop_assert_eq!(is_normalized(&s, form), expected, "form {:?}", form);
            }
        }

        #[test]
        fn composition_then_decomposition(s in any_string()) {
            // NFD(NFC(s)) == NFD(s), and the compatibility analogue.
            let nfd = normalize(&s, Form::Nfd);
            prop_assert_eq!(normalize(&normalize(&s, Form::Nfc), Form::Nfd), nfd);
            let nfkd = normalize(&s, Form::Nfkd);
            prop_assert_eq!(normalize(&normalize(&s, Form::Nfkc), Form::Nfkd), nfkd);
        }

        #[test]
        fn ascii_is_fixed_point_of_every_form(s in ascii_string()) {
            for form in FORMS {
                prop_assert_eq!(&normalize(&s, form), &s);
                prop_assert!(is_normalized(&s, form));
            }
        }

        #[test]
        fn marks_normalize_idempotent_and_consistent(s in marky_string()) {
            for form in FORMS {
                let once = normalize(&s, form);
                prop_assert_eq!(&normalize(&once, form), &once, "form {:?}", form);
                prop_assert!(is_normalized(&once, form));
                prop_assert_eq!(is_normalized(&s, form), once == s);
            }
            let nfd = normalize(&s, Form::Nfd);
            prop_assert_eq!(normalize(&normalize(&s, Form::Nfc), Form::Nfd), nfd);
        }
    }
}
