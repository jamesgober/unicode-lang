//! Criterion benchmarks for the public surface: identifier predicates, display
//! width, and the four normalization forms across representative inputs, plus
//! the two shapes the 1.0.1 patch targets — long pure-ASCII text (the fast
//! paths) and long runs of alternating combining marks (canonical ordering).

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use unicode_lang::{Form, char_width, is_normalized, is_xid, normalize, str_width};

// A mix of scripts, marks, ligatures, Hangul, and fullwidth forms.
const MIXED: &str = "The quick brown fox — Δpressure, café, ﬁle, 日本語, 가나다, Ａ½";
const ASCII: &str = "the quick brown fox jumps over the lazy dog 0123456789";
const DECOMPOSED: &str = "cafe\u{0301} nai\u{0308}ve o\u{0328}\u{0304}"; // marks, some out of order

/// About 4 KiB of ASCII source text: the shape a lexer feeds these functions
/// most of the time.
fn ascii_source() -> String {
    let line = "    let total_count = compute_value(input_buffer, 42) + offset_7;\n";
    line.repeat(4096 / line.len() + 1)
}

/// A base letter followed by `marks` combining marks alternating between
/// class 230 (U+0301) and class 220 (U+0323): the worst case for canonical
/// ordering, since every other mark must move.
fn alternating_marks(marks: usize) -> String {
    let mut s = String::with_capacity(1 + marks * 2);
    s.push('a');
    for i in 0..marks {
        s.push(if i % 2 == 0 { '\u{0301}' } else { '\u{0323}' });
    }
    s
}

fn bench_ident(c: &mut Criterion) {
    let mut g = c.benchmark_group("ident");
    g.bench_function("is_xid_start", |b| {
        b.iter(|| unicode_lang::is_xid_start(black_box('Δ')))
    });
    g.bench_function("is_xid_start/ascii", |b| {
        b.iter(|| unicode_lang::is_xid_start(black_box('q')))
    });
    g.bench_function("is_xid_continue", |b| {
        b.iter(|| unicode_lang::is_xid_continue(black_box('\u{0301}')))
    });
    g.bench_function("is_xid_continue/ascii", |b| {
        b.iter(|| unicode_lang::is_xid_continue(black_box('7')))
    });
    g.bench_function("is_xid(word)", |b| {
        b.iter(|| is_xid(black_box("Δpressure_2")))
    });
    g.bench_function("is_xid(ascii word)", |b| {
        b.iter(|| is_xid(black_box("input_buffer_len")))
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
    let src = ascii_source();
    let _ = g.throughput(Throughput::Bytes(src.len() as u64));
    g.bench_function("str_width/ascii_4k", |b| {
        b.iter(|| str_width(black_box(&src)))
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
    let src = ascii_source();
    g.bench_function("ascii_4k/Nfc", |b| {
        b.iter(|| normalize(black_box(&src), Form::Nfc))
    });
    // ASCII text with one decomposed letter at the end: the quick-check cannot
    // answer, so the whole string goes through decompose + compose.
    let mut tail = ascii_source();
    tail.push_str("e\u{0301}");
    g.bench_function("ascii_4k_tail_mark/Nfc", |b| {
        b.iter(|| normalize(black_box(&tail), Form::Nfc))
    });
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
    let src = ascii_source();
    g.bench_function("ascii_4k/Nfc", |b| {
        b.iter(|| is_normalized(black_box(&src), Form::Nfc))
    });
    g.finish();
}

fn bench_canonical_ordering(c: &mut Criterion) {
    // Hostile input: a long run of combining marks with alternating classes.
    // Before 1.0.1 canonical ordering was an insertion sort, quadratic here.
    let mut g = c.benchmark_group("alternating_marks");
    g.sample_size(10);
    for marks in [1_000usize, 4_000, 16_000] {
        let input = alternating_marks(marks);
        let _ = g.throughput(Throughput::Elements(marks as u64));
        g.bench_with_input(BenchmarkId::new("Nfd", marks), &input, |b, s| {
            b.iter(|| normalize(black_box(s), Form::Nfd))
        });
        g.bench_with_input(BenchmarkId::new("Nfc", marks), &input, |b, s| {
            b.iter(|| normalize(black_box(s), Form::Nfc))
        });
    }
    g.finish();
}

criterion_group!(
    benches,
    bench_ident,
    bench_width,
    bench_normalize,
    bench_is_normalized,
    bench_canonical_ordering
);
criterion_main!(benches);
