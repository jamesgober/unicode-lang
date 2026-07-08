//! Property-based tests over the public surface.
//!
//! These assert the algebraic laws the API promises for *arbitrary* input,
//! rather than the specific vectors in `conformance.rs`: normalization is
//! idempotent and stable, `is_normalized` agrees with `normalize`, the forms
//! compose as UAX #15 requires, width is additive, and `is_xid` is exactly the
//! per-scalar predicate applied across a string.

#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use unicode_lang::{
    Form, char_width, is_normalized, is_xid, is_xid_continue, is_xid_start, normalize, str_width,
};

const FORMS: [Form; 4] = [Form::Nfc, Form::Nfd, Form::Nfkc, Form::Nfkd];

fn any_string() -> impl Strategy<Value = String> {
    proptest::collection::vec(any::<char>(), 0..24).prop_map(|v| v.into_iter().collect())
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
    fn xid_start_implies_continue(c in any::<char>()) {
        if is_xid_start(c) {
            prop_assert!(is_xid_continue(c));
        }
    }
}
