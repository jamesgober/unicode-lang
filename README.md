<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <b>unicode-lang</b>
    <br>
    <sub><sup>UNICODE RULES</sup></sub>
</h1>

<div align="center">
    <a href="https://crates.io/crates/unicode-lang"><img alt="Crates.io" src="https://img.shields.io/crates/v/unicode-lang"></a>
    <a href="https://crates.io/crates/unicode-lang"><img alt="Downloads" src="https://img.shields.io/crates/d/unicode-lang?color=%230099ff"></a>
    <a href="https://docs.rs/unicode-lang"><img alt="docs.rs" src="https://img.shields.io/docsrs/unicode-lang"></a>
    <a href="https://github.com/jamesgober/unicode-lang/actions"><img alt="CI" src="https://github.com/jamesgober/unicode-lang/actions/workflows/ci.yml/badge.svg"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85%2B-blue"></a>
</div>

<br>

<div align="left">
    <p>
        unicode-lang is the FEAT-tier crate: the Unicode text primitives a language front end needs — identifier rules, normalization, and display width. Part of the -lang language-construction family; see _strategy/LANG_COLLECTION.md for the master plan.
    </p>
    <br>
    <hr>
    <p>
        <strong>MSRV is 1.85+</strong> (Rust 2024 edition).
    </p>
    <blockquote>
        <strong>Status: stable.</strong> As of <code>1.0.0</code> the public API is frozen under Semantic Versioning; see <a href="./docs/API.md#stability"><code>docs/API.md</code></a> for the promise and <a href="./CHANGELOG.md"><code>CHANGELOG.md</code></a> for the history.
    </blockquote>
</div>

<hr>
<br>

<div align="left">
    <p>
        <strong>unicode-lang</strong> answers the three Unicode questions a lexer has to ask that the standard library will not: <em>may this scalar start or continue an identifier?</em> (<a href="https://www.unicode.org/reports/tr31/">UAX&nbsp;#31</a>), <em>are these two strings the same once normalized?</em> (<a href="https://www.unicode.org/reports/tr15/">UAX&nbsp;#15</a>), and <em>how many columns does this text occupy?</em> (<a href="https://www.unicode.org/reports/tr11/">UAX&nbsp;#11</a>).
    </p>
    <p>
        Every answer comes from compact, sorted lookup tables generated directly from the <b>Unicode Character Database</b>, so the crate carries <b>no third-party dependencies</b> and every query is a branch-predictable binary search. It is <b><code>no_std</code></b>: identifier and width checks need no allocator at all; normalization allocates only its output. Normalization is verified against the official <code>NormalizationTest.txt</code> conformance suite — every one of its ~19&nbsp;000 vectors plus the whole-codespace identity rule.
    </p>
    <p>
        The whole crate is <b>safe Rust</b> — <code>#![forbid(unsafe_code)]</code>. It owns Unicode rules and nothing else; a lexer, a symbol table, or a terminal renderer composes it with the rest of the family.
    </p>
</div>

<hr>
<br>

## Performance First

Identifier and width queries are single-digit nanoseconds; normalization is dominated by its one output allocation and short-circuits when the input is already normalized. Latest local Criterion means (`cargo bench --bench bench`, Windows x86_64, Rust stable, release build):

| Operation                                   | Time      |
|---------------------------------------------|----------:|
| `is_xid_start` / `is_xid_continue`          |  ~3.2 ns  |
| `is_xid` (11-scalar identifier)             |  ~37 ns   |
| `char_width` (per scalar)                   |  ~5.2 ns  |
| `str_width` (mixed 48-column string)        |  ~288 ns  |
| `normalize` (ASCII, already normalized)     |  ~33 ns   |
| `normalize` (mixed scripts → NFC)           |  ~128 ns  |
| `is_normalized` (ASCII → NFC)               |  ~13 ns   |

Numbers vary by CPU and environment; run the suite on your target to establish a baseline. Data is generated from **Unicode 16.0.0** (`UNICODE_VERSION`).

<br>
<hr>

## Features

- **Identifier rules** — `is_xid_start`, `is_xid_continue`, and the whole-string `is_xid` implement the UAX #31 `XID` profile a lexer needs.
- **Normalization** — all four forms (NFC, NFD, NFKC, NFKD) through one `normalize`, plus a fast `is_normalized` quick-check. Verified against the official conformance suite.
- **Display width** — `char_width` and `str_width` give monospace column counts (0 / 1 / 2), `wcwidth`-style.
- **No dependencies** — tables are generated from the UCD and embedded; nothing is pulled in at build time.
- **`no_std`** — identifier and width queries are allocation-free; normalization is gated behind `alloc` (on by default).
- **Fully safe** — `#![forbid(unsafe_code)]`.
- **Property-tested** — algebraic laws (idempotence, form composition, width additivity) checked across randomized inputs with `proptest`.

<br>
<hr>

## Installation

```toml
[dependencies]
unicode-lang = "1"

# no_std without normalization (identifier + width only, no allocator):
unicode-lang = { version = "1", default-features = false }

# Serde support for the `Form` selector:
unicode-lang = { version = "1", features = ["serde"] }
```

**MSRV is 1.85+** (Rust 2024 edition).

<hr>
<br>

## Quick Start

```rust
use unicode_lang::{char_width, is_xid, normalize, Form};

// Recognise an identifier that mixes scripts.
assert!(is_xid("Δpressure"));
assert!(!is_xid("1st"));      // cannot start with a digit

// Normalize: compose a base letter and a combining accent.
assert_eq!(normalize("e\u{0301}", Form::Nfc), "é");

// Compatibility folding: a ligature and a fullwidth digit.
assert_eq!(normalize("ﬁle", Form::Nfkc), "file");
assert_eq!(normalize("\u{FF11}", Form::Nfkc), "1");

// Measure display width for column alignment.
assert_eq!(char_width('A'), 1);
assert_eq!(char_width('世'), 2);   // wide CJK ideograph
```

### Lexing identifiers

The two per-scalar predicates are what a hand-written lexer calls in its inner loop:

```rust
use unicode_lang::{is_xid_continue, is_xid_start};

fn lex_identifier(src: &str) -> Option<(&str, &str)> {
    let mut chars = src.char_indices();
    let (_, first) = chars.next()?;
    // Rust-style: an identifier may also start with '_'.
    if !(is_xid_start(first) || first == '_') {
        return None;
    }
    let end = chars
        .find(|&(_, c)| !is_xid_continue(c))
        .map_or(src.len(), |(i, _)| i);
    Some(src.split_at(end))
}

assert_eq!(lex_identifier("total99 = 1"), Some(("total99", " = 1")));
assert_eq!(lex_identifier("_private;"), Some(("_private", ";")));
assert_eq!(lex_identifier("1st"), None);
```

<hr>
<br>

## How it works

The public API is three small modules over one generated data file:

- **Identifiers** consult the `XID_Start` and `XID_Continue` ranges from `DerivedCoreProperties`. A query is one binary search over sorted, disjoint code-point ranges.
- **Width** classifies a scalar as zero-width (controls, combining marks, format characters, conjoining Hangul jamo), wide (East Asian *Wide* / *Fullwidth*), or the default one column.
- **Normalization** decomposes each scalar through the full canonical or compatibility mapping, reorders combining marks by canonical class, and — for NFC / NFKC — recomposes via the primary-composite table. Hangul is handled by formula. An already-normalized string is detected by the quick-check and returned untouched.

The tables are produced by `dev/gen_tables.rs`, a committed, dependency-free generator that reads the UCD text files and emits `src/tables.rs`. Regenerating against a newer Unicode release is a one-command step; the shipped crate contains only the generated data.

<hr>
<br>

## API Overview

For the complete reference with examples, see [`docs/API.md`](./docs/API.md).

- [`is_xid_start`](./docs/API.md#is_xid_start) / [`is_xid_continue`](./docs/API.md#is_xid_continue) / [`is_xid`](./docs/API.md#is_xid) — UAX #31 identifier rules.
- [`char_width`](./docs/API.md#char_width) / [`str_width`](./docs/API.md#str_width) — monospace display width.
- [`normalize`](./docs/API.md#normalize) / [`is_normalized`](./docs/API.md#is_normalized) / [`Form`](./docs/API.md#form) — the four normalization forms.
- [`UNICODE_VERSION`](./docs/API.md#unicode_version) — the UCD version the tables were built from.

<br>

### Feature Flags

| Feature | Default | Description                                                                    |
|---------|:-------:|--------------------------------------------------------------------------------|
| `std`   | ✅      | Enables `alloc`. The crate uses no other std facilities.                        |
| `alloc` | ✅      | Gates the allocating `normalize` / `is_normalized` API. Off ⇒ identifiers + width only. |
| `serde` | ❌      | `Serialize` / `Deserialize` for the [`Form`](./docs/API.md#form) selector.      |

<hr>
<br>

## Testing

```bash
cargo test                 # unit + property + doctests + curated conformance
cargo test --all-features  # adds the serde-gated paths
cargo bench --bench bench  # Criterion benchmarks
```

The property suite in [`tests/proptests.rs`](./tests/proptests.rs) checks the algebraic laws — normalization is idempotent and stable, the forms compose as UAX #15 requires, and width is additive. [`tests/conformance.rs`](./tests/conformance.rs) always runs a curated set of hard cases, and runs the **entire** official `NormalizationTest.txt` suite whenever the UCD data is present locally or in CI.

<hr>
<br>

## Cross-Platform Support

The crate is pure table lookups with no platform-specific code, so it behaves identically everywhere Rust runs. CI covers **Linux**, **macOS**, and **Windows** on both stable and the 1.85 MSRV; the full conformance suite is validated on Windows and Linux (WSL2 Ubuntu).

<hr>
<br>

## Contributing

See <a href="./REPS.md"><code>REPS.md</code></a> for the engineering standards and the definition of done. Before a PR: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` must be clean.

<br>

<div id="license">
    <h2>License</h2>
    <p>Licensed under either of</p>
    <ul>
        <li><b>Apache License, Version 2.0</b> &mdash; <a href="./LICENSE-APACHE">LICENSE-APACHE</a></li>
        <li><b>MIT License</b> &mdash; <a href="./LICENSE-MIT">LICENSE-MIT</a></li>
    </ul>
    <p>at your option.</p>
</div>

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober <me@jamesgober.com>.</strong></sup>
</div>
