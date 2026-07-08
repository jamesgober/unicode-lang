//! Unicode normalization conformance.
//!
//! `curated_cases` always runs: a hand-picked set of hard cases (canonical
//! ordering, Hangul LV/LVT, compatibility folds, chained composites) that guard
//! against regressions with no external data.
//!
//! `full_conformance_suite` runs the complete official `NormalizationTest.txt`
//! when it is available on disk — locally under `dev/ucd/` after the generator
//! download step, or in CI when the UCD is present. It exercises every one of
//! the ~19 000 published test vectors plus the whole-codespace identity rule
//! (every code point not named in Part 1 must be a fixed point of all four
//! forms). When the file is absent the test is skipped, so the suite stays
//! green without committing the multi-megabyte data file.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use unicode_lang::{Form, normalize};

/// One conformance record: the five columns `c1..=c5` as owned strings.
struct Vectors {
    c: [String; 5],
}

fn parse_seq(field: &str) -> String {
    field
        .split_whitespace()
        .map(|h| char::from_u32(u32::from_str_radix(h, 16).unwrap()).unwrap())
        .collect()
}

/// Assert every invariant the file header specifies for a single record.
fn check_record(v: &Vectors) {
    let [c1, c2, c3, c4, c5] = &v.c;

    // NFC
    for src in [c1, c2, c3] {
        assert_eq!(&normalize(src, Form::Nfc), c2, "NFC({src:?})");
    }
    for src in [c4, c5] {
        assert_eq!(&normalize(src, Form::Nfc), c4, "NFC({src:?})");
    }
    // NFD
    for src in [c1, c2, c3] {
        assert_eq!(&normalize(src, Form::Nfd), c3, "NFD({src:?})");
    }
    for src in [c4, c5] {
        assert_eq!(&normalize(src, Form::Nfd), c5, "NFD({src:?})");
    }
    // NFKC
    for src in [c1, c2, c3, c4, c5] {
        assert_eq!(&normalize(src, Form::Nfkc), c4, "NFKC({src:?})");
    }
    // NFKD
    for src in [c1, c2, c3, c4, c5] {
        assert_eq!(&normalize(src, Form::Nfkd), c5, "NFKD({src:?})");
    }
}

#[test]
fn curated_cases() {
    // (c1, c2=NFC, c3=NFD, c4=NFKC, c5=NFKD)
    let raw = [
        // Precomposed vs decomposed.
        ("00C0", "00C0", "0041 0300", "00C0", "0041 0300"),
        // Canonical ordering: two marks supplied out of order.
        (
            "0044 0307 0323",
            "1E0C 0307",
            "0044 0323 0307",
            "1E0C 0307",
            "0044 0323 0307",
        ),
        // Hangul LV.
        ("AC00", "AC00", "1100 1161", "AC00", "1100 1161"),
        // Hangul LVT.
        ("AC01", "AC01", "1100 1161 11A8", "AC01", "1100 1161 11A8"),
        // Compatibility ligature.
        ("FB01", "FB01", "FB01", "0066 0069", "0066 0069"),
        // Fullwidth Latin.
        ("FF21", "FF21", "FF21", "0041", "0041"),
        // Singleton canonical decomposition (OHM SIGN -> GREEK OMEGA).
        ("2126", "03A9", "03A9", "03A9", "03A9"),
        // Compatibility with combining reorder (vulgar fraction one half).
        ("00BD", "00BD", "00BD", "0031 2044 0032", "0031 2044 0032"),
        // Reorder + partial composition: A + acute(230) + dot-below(220).
        // Marks reorder to dot-below, acute; A composes with dot-below to 1EA0
        // (LATIN CAPITAL A WITH DOT BELOW); the acute has no further composite.
        (
            "0041 0301 0323",
            "1EA0 0301",
            "0041 0323 0301",
            "1EA0 0301",
            "0041 0323 0301",
        ),
    ];
    for (c1, c2, c3, c4, c5) in raw {
        let v = Vectors {
            c: [
                parse_seq(c1),
                parse_seq(c2),
                parse_seq(c3),
                parse_seq(c4),
                parse_seq(c5),
            ],
        };
        check_record(&v);
    }
}

#[test]
fn full_conformance_suite() {
    let path = std::path::Path::new("dev/ucd/NormalizationTest.txt");
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("skipping full conformance: {} not present", path.display());
        return;
    };

    let mut part1_singletons: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut in_part1 = false;
    let mut records = 0usize;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix('@') {
            in_part1 = rest.starts_with("Part1");
            continue;
        }
        let data = line.split('#').next().unwrap_or("").trim();
        if data.is_empty() {
            continue;
        }
        let cols: Vec<&str> = data.split(';').collect();
        if cols.len() < 5 {
            continue;
        }
        let v = Vectors {
            c: [
                parse_seq(cols[0]),
                parse_seq(cols[1]),
                parse_seq(cols[2]),
                parse_seq(cols[3]),
                parse_seq(cols[4]),
            ],
        };
        if in_part1 {
            let mut chars = v.c[0].chars();
            if let (Some(only), None) = (chars.next(), chars.next()) {
                let _ = part1_singletons.insert(only as u32);
            }
        }
        check_record(&v);
        records += 1;
    }

    assert!(records > 15_000, "unexpectedly few records: {records}");

    // Whole-codespace identity: any code point not named in Part 1 is a fixed
    // point of all four forms.
    for cp in 0u32..=0x10_FFFF {
        if part1_singletons.contains(&cp) {
            continue;
        }
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        let s = c.to_string();
        for form in [Form::Nfc, Form::Nfd, Form::Nfkc, Form::Nfkd] {
            assert_eq!(normalize(&s, form), s, "identity {cp:#06X} {form:?}");
        }
    }

    eprintln!("full conformance: {records} records + whole-codespace identity OK");
}
