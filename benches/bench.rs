//! Criterion benchmarks for the public surface: identifier predicates, display
//! width, and the four normalization forms across representative inputs.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use unicode_lang::{Form, char_width, is_normalized, is_xid, normalize, str_width};

// A mix of scripts, marks, ligatures, Hangul, and fullwidth forms.
const MIXED: &str = "The quick brown fox — Δpressure, café, ﬁle, 日本語, 가나다, Ａ½";
const ASCII: &str = "the quick brown fox jumps over the lazy dog 0123456789";
const DECOMPOSED: &str = "cafe\u{0301} nai\u{0308}ve o\u{0328}\u{0304}"; // marks, some out of order

fn bench_ident(c: &mut Criterion) {
    let mut g = c.benchmark_group("ident");
    g.bench_function("is_xid_start", |b| {
        b.iter(|| unicode_lang::is_xid_start(black_box('Δ')))
    });
    g.bench_function("is_xid_continue", |b| {
        b.iter(|| unicode_lang::is_xid_continue(black_box('\u{0301}')))
    });
    g.bench_function("is_xid(word)", |b| {
        b.iter(|| is_xid(black_box("Δpressure_2")))
    });
    g.finish();
}

fn bench_width(c: &mut Criterion) {
    let mut g = c.benchmark_group("width");
    g.bench_function("char_width/ascii", |b| {
        b.iter(|| char_width(black_box('A')))
    });
    g.bench_function("char_width/wide", |b| {
        b.iter(|| char_width(black_box('世')))
    });
    g.bench_function("str_width/mixed", |b| {
        b.iter(|| str_width(black_box(MIXED)))
    });
    g.finish();
}

fn bench_normalize(c: &mut Criterion) {
    let mut g = c.benchmark_group("normalize");
    for (label, input) in [
        ("ascii", ASCII),
        ("mixed", MIXED),
        ("decomposed", DECOMPOSED),
    ] {
        for form in [Form::Nfc, Form::Nfd, Form::Nfkc, Form::Nfkd] {
            let name = format!("{label}/{form:?}");
            g.bench_function(&name, |b| b.iter(|| normalize(black_box(input), form)));
        }
    }
    g.finish();
}

fn bench_is_normalized(c: &mut Criterion) {
    let mut g = c.benchmark_group("is_normalized");
    g.bench_function("ascii/Nfc", |b| {
        b.iter(|| is_normalized(black_box(ASCII), Form::Nfc))
    });
    g.bench_function("mixed/Nfc", |b| {
        b.iter(|| is_normalized(black_box(MIXED), Form::Nfc))
    });
    g.finish();
}

criterion_group!(
    benches,
    bench_ident,
    bench_width,
    bench_normalize,
    bench_is_normalized
);
criterion_main!(benches);
