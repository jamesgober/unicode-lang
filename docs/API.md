<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br><b>unicode-lang</b><br>
    <sub><sup>API REFERENCE</sup></sub>
</h1>
<div align="center">
    <sup>
        <a href="../README.md" title="Project Home"><b>HOME</b></a>
        <span>&nbsp;│&nbsp;</span>
        <span>API</span>
        <span>&nbsp;│&nbsp;</span>
        <a href="../CHANGELOG.md" title="Changelog"><b>CHANGELOG</b></a>
    </sup>
</div>
<br>

The Unicode text primitives a language front end needs — identifier rules,
normalization, and display width — from tables generated directly from the
Unicode Character Database, with no third-party dependencies. The whole public
surface is eight free functions, one selector enum, and one version constant.

- **Version:** 1.0.0
- **Unicode data:** 16.0.0 (see [`UNICODE_VERSION`](#unicode_version))
- **MSRV:** Rust 1.85 (2024 edition)
- **`no_std`:** yes — identifiers and width need no allocator; normalization needs `alloc`
- **Stability:** stable — the surface below is frozen (see [Stability](#stability))

## Table of Contents

- **[Stability](#stability)**
- **[Installation](#installation)**
- **[Quick Start](#quick-start)**
- **[Identifiers (UAX #31)](#identifiers)**
  - [`is_xid_start`](#is_xid_start)
  - [`is_xid_continue`](#is_xid_continue)
  - [`is_xid`](#is_xid)
- **[Display width (UAX #11)](#width)**
  - [`char_width`](#char_width)
  - [`str_width`](#str_width)
- **[Normalization (UAX #15)](#normalization)**
  - [`Form`](#form)
  - [`normalize`](#normalize)
  - [`is_normalized`](#is_normalized)
- **[`UNICODE_VERSION`](#unicode_version)**
- **[Serde support](#serde-support)**
- **[Design notes](#design-notes)**
  - [Generated tables](#generated-tables)
  - [The width contract](#the-width-contract)
  - [The normalization algorithm](#the-normalization-algorithm)
  - [Conformance](#conformance)

<br>

## Stability

As of **1.0.0** the public API documented here is **stable and frozen**.
`unicode-lang` follows [Semantic Versioning](https://semver.org):

- Nothing in the frozen surface — the eight functions, the [`Form`](#form) enum
  and its variants, the [`UNICODE_VERSION`](#unicode_version) constant, the
  `std` / `alloc` / `serde` feature flags, and the `serde` representation of
  `Form` — will be removed or changed in a breaking way within the `1.x` series.
  A breaking change means a new major version.
- `1.x` releases may **add** to the surface (new functions, new trait impls)
  without breaking existing code.
- The **Unicode version** the tables are built from
  ([`UNICODE_VERSION`](#unicode_version)) may advance in a minor release; that
  can add assigned characters and refine width / normalization for
  previously-unassigned code points, and is not a breaking change to the API.

MSRV (Rust 1.85) is treated as a compatibility surface: a raise is a minor,
documented change, never a patch.

<br>

## Installation

```toml
[dependencies]
unicode-lang = "1"

# no_std without normalization (identifier + width only, no allocator):
unicode-lang = { version = "1", default-features = false }

# Serde support for the `Form` selector:
unicode-lang = { version = "1", features = ["serde"] }
```

| Feature | Default | Description                                                                 |
|---------|:-------:|-----------------------------------------------------------------------------|
| `std`   | ✅      | Enables `alloc`. The crate uses no other std facilities.                     |
| `alloc` | ✅      | Gates [`normalize`](#normalize) / [`is_normalized`](#is_normalized). Off ⇒ identifiers + width only. |
| `serde` | ❌      | `Serialize` / `Deserialize` for [`Form`](#form).                             |

<br>

## Quick Start

```rust
use unicode_lang::{char_width, is_xid, normalize, Form};

assert!(is_xid("café"));
assert_eq!(normalize("ﬁle", Form::Nfkc), "file");
assert_eq!(char_width('世'), 2);
```

<br>
<hr>

<h2 id="identifiers">Identifiers (UAX #31)</h2>

The `XID_Start` / `XID_Continue` derived properties from
[UAX #31](https://www.unicode.org/reports/tr31/). These are the predicates a
lexer consults to recognise identifiers: the first scalar must be `XID_Start`,
each subsequent scalar `XID_Continue`. The `XID` forms are *closure-stable* —
normalizing a valid identifier never makes it invalid — which is why they, not
the older `ID_Start` / `ID_Continue`, are the recommended basis.

Both predicates are allocation-free and available with no features enabled.

<br>

<h3 id="is_xid_start"><code>is_xid_start</code></h3>

```rust
pub fn is_xid_start(c: char) -> bool
```

Returns `true` if `c` may **begin** a Unicode identifier — that is, if `c` has
the `XID_Start` property. This is the ASCII letters plus the letters and
letter-like marks of every script. It excludes digits, whitespace, and
punctuation — including the underscore, which is `XID_Continue` but not
`XID_Start`.

**Parameters**

- `c: char` — the scalar to classify.

**Returns:** `bool` — `true` when `c` has `XID_Start`.

**Examples**

```rust
use unicode_lang::is_xid_start;

assert!(is_xid_start('a'));
assert!(is_xid_start('Δ'));   // Greek capital delta
assert!(is_xid_start('日'));  // CJK ideograph

assert!(!is_xid_start('1'));  // a digit may continue, not start
assert!(!is_xid_start('_'));  // underscore continues but does not start
assert!(!is_xid_start(' '));
```

```rust
use unicode_lang::is_xid_start;

// A language that allows a leading underscore opts in explicitly:
let starts_ident = |c: char| is_xid_start(c) || c == '_';
assert!(starts_ident('_'));
assert!(starts_ident('x'));
assert!(!starts_ident('9'));
```

<br>

<h3 id="is_xid_continue"><code>is_xid_continue</code></h3>

```rust
pub fn is_xid_continue(c: char) -> bool
```

Returns `true` if `c` may **continue** a Unicode identifier — that is, if `c`
has the `XID_Continue` property. `XID_Continue` is a superset of
[`is_xid_start`](#is_xid_start): it additionally admits decimal digits, the
combining marks that attach to letters, and Unicode connector punctuation —
which includes the ASCII underscore `_` (category `Pc`). The underscore
therefore continues an identifier but, per `XID_Start`, cannot begin one.

**Parameters**

- `c: char` — the scalar to classify.

**Returns:** `bool` — `true` when `c` has `XID_Continue`.

**Examples**

```rust
use unicode_lang::is_xid_continue;

assert!(is_xid_continue('a'));
assert!(is_xid_continue('9'));        // digits continue
assert!(is_xid_continue('_'));        // connector punctuation continues
assert!(is_xid_continue('\u{0301}')); // COMBINING ACUTE ACCENT

assert!(!is_xid_continue(' '));
assert!(!is_xid_continue('-'));
```

```rust
use unicode_lang::{is_xid_continue, is_xid_start};

// Every XID_Start scalar is also XID_Continue.
for c in ['a', 'Δ', '日', 'ℵ'] {
    assert!(is_xid_start(c) && is_xid_continue(c));
}
```

<br>

<h3 id="is_xid"><code>is_xid</code></h3>

```rust
pub fn is_xid(s: &str) -> bool
```

Returns `true` if `s` is a well-formed Unicode identifier under the default
UAX #31 profile: it is **non-empty**, its first scalar is
[`is_xid_start`](#is_xid_start), and every remaining scalar is
[`is_xid_continue`](#is_xid_continue). A convenience over iterating the scalars
by hand.

It applies no language-specific extensions — notably it rejects a leading
underscore, so a grammar that allows `_name` should test that case itself (see
the second example).

**Parameters**

- `s: &str` — the string to test in full.

**Returns:** `bool` — `true` when the whole string is a UAX #31 identifier.

**Examples**

```rust
use unicode_lang::is_xid;

assert!(is_xid("total"));
assert!(is_xid("Δpressure"));
assert!(is_xid("café"));
assert!(is_xid("naïve"));

assert!(!is_xid(""));       // empty is not an identifier
assert!(!is_xid("1st"));    // cannot start with a digit
assert!(!is_xid("a b"));    // no interior whitespace
assert!(!is_xid("_name"));  // underscore is not XID_Start
```

```rust
use unicode_lang::{is_xid, is_xid_continue, is_xid_start};

// A Rust-flavoured check that also accepts a leading underscore.
fn is_rust_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if is_xid_start(c) || c == '_' => chars.all(is_xid_continue),
        _ => false,
    }
}

assert!(is_xid("name") && is_rust_ident("name"));
assert!(!is_xid("_name") && is_rust_ident("_name"));
```

<br>
<hr>

<h2 id="width">Display width (UAX #11)</h2>

Monospace display width — how many terminal columns a scalar occupies. The
contract follows the long-established `wcwidth` convention, refined by the
Unicode [East Asian Width](https://www.unicode.org/reports/tr11/) property:

- **0 columns** — the null character, the C0/C1 control blocks, nonspacing and
  enclosing combining marks, format characters, and conjoining Hangul jamo.
- **2 columns** — East Asian *Wide* and *Fullwidth* characters (CJK ideographs,
  fullwidth forms).
- **1 column** — everything else, including East Asian *Ambiguous*, treated as
  narrow because its width depends on locale context this crate does not carry.

Width is a rendering estimate for fixed-pitch output, not grapheme
segmentation: a base letter and its combining marks sum correctly (the marks
contribute 0), but the crate does not merge them into a single cluster. Both
functions are allocation-free and available with no features enabled.

<br>

<h3 id="char_width"><code>char_width</code></h3>

```rust
pub fn char_width(c: char) -> usize
```

Returns the number of monospace columns `c` occupies: `0`, `1`, or `2`, per the
contract above.

**Parameters**

- `c: char` — the scalar to measure.

**Returns:** `usize` in `0..=2`.

**Examples**

```rust
use unicode_lang::char_width;

assert_eq!(char_width('A'), 1);
assert_eq!(char_width('世'), 2);        // wide CJK ideograph
assert_eq!(char_width('\u{FF21}'), 2);  // FULLWIDTH LATIN CAPITAL A

assert_eq!(char_width('\u{0301}'), 0);  // COMBINING ACUTE ACCENT
assert_eq!(char_width('\u{200B}'), 0);  // ZERO WIDTH SPACE
assert_eq!(char_width('\n'), 0);        // control characters take no column
```

```rust
use unicode_lang::char_width;

// East Asian Ambiguous is treated as narrow (one column).
assert_eq!(char_width('§'), 1);
// Soft hyphen is a format character but occupies a column when shown.
assert_eq!(char_width('\u{00AD}'), 1);
```

<br>

<h3 id="str_width"><code>str_width</code></h3>

```rust
pub fn str_width(s: &str) -> usize
```

Returns the total monospace width of `s`: the sum of [`char_width`](#char_width)
over its scalar values. Useful for column alignment, right-padding, and
truncation of fixed-pitch output.

**Parameters**

- `s: &str` — the string to measure.

**Returns:** `usize` — the summed column count.

**Examples**

```rust
use unicode_lang::str_width;

assert_eq!(str_width("hello"), 5);
assert_eq!(str_width("日本語"), 6);      // three wide ideographs
assert_eq!(str_width("café"), 4);        // precomposed é is one column

// A base letter plus a combining mark still advances by one column.
assert_eq!(str_width("e\u{0301}"), 1);
```

```rust
use unicode_lang::str_width;

// Right-pad a label to a fixed column budget, accounting for wide glyphs.
fn pad_to(label: &str, columns: usize) -> String {
    let used = str_width(label);
    let mut out = label.to_string();
    out.extend(std::iter::repeat(' ').take(columns.saturating_sub(used)));
    out
}

assert_eq!(pad_to("id", 5), "id   ");   // 2 columns + 3 spaces
assert_eq!(pad_to("名前", 5), "名前 "); // 4 columns + 1 space
```

<br>
<hr>

<h2 id="normalization">Normalization (UAX #15)</h2>

The four normalization forms of
[UAX #15](https://www.unicode.org/reports/tr15/). Normalization rewrites a
string into a canonical shape so that sequences meant to be equal compare
byte-for-byte equal — the canonical case being `é`, which is either one
precomposed scalar (`U+00E9`) or a base letter plus a combining accent
(`U+0065 U+0301`).

This API requires the `alloc` feature (enabled by default through `std`).

<br>

<h3 id="form"><code>Form</code></h3>

```rust
pub enum Form {
    Nfc,
    Nfd,
    Nfkc,
    Nfkd,
}
```

Selects the target shape for [`normalize`](#normalize) and
[`is_normalized`](#is_normalized). Two axes give the four forms:

| Variant | Decomposition | Recomposes | Folds compatibility | Use for |
|---|---|:---:|:---:|---|
| `Nfc`  | canonical      | yes | no  | the general default; UAX #31 identifiers |
| `Nfd`  | canonical      | no  | no  | base + marks, e.g. for mark-aware processing |
| `Nfkc` | compatibility  | yes | yes | fold-and-compare matching of visually similar text |
| `Nfkd` | compatibility  | no  | yes | fully expanded compatibility form |

*Canonical* forms preserve visual identity; the *compatibility* forms
(`Nfkc` / `Nfkd`) additionally fold formatting distinctions — mapping the
ligature `ﬁ` to `fi` and fullwidth `Ａ` to `A`. When in doubt, `Nfc` is the safe
default.

`Form` derives `Copy`, `Clone`, `Debug`, `PartialEq`, `Eq`, and `Hash`. With
the `serde` feature it also derives `Serialize` / `Deserialize`.

**Examples**

```rust
use unicode_lang::{normalize, Form};

// NFC composes; NFD decomposes. They round-trip the same text.
assert_eq!(normalize("e\u{0301}", Form::Nfc), "é");
assert_eq!(normalize("é", Form::Nfd), "e\u{0301}");
```

<br>

<h3 id="normalize"><code>normalize</code></h3>

```rust
pub fn normalize(s: &str, form: Form) -> String
```

Returns `s` normalized to `form`. When `s` is already in the requested form
this returns it unchanged after a fast allocation-free scan (the Unicode
quick-check), so passing already-normalized text — the common case for ASCII and
most well-formed input — costs one linear pass plus the single allocation for
the returned `String`.

**Parameters**

- `s: &str` — the input string.
- `form: Form` — the target [`Form`](#form).

**Returns:** `String` — the normalized text.

**Examples**

```rust
use unicode_lang::{normalize, Form};

// Canonical composition and decomposition.
assert_eq!(normalize("e\u{0301}", Form::Nfc), "é");
assert_eq!(normalize("é", Form::Nfd), "e\u{0301}");

// ASCII (and already-normalized text) is returned untouched.
assert_eq!(normalize("plain ascii", Form::Nfc), "plain ascii");
```

```rust
use unicode_lang::{normalize, Form};

// Compatibility folding (NFKC): ligatures and fullwidth forms collapse.
assert_eq!(normalize("ﬁle", Form::Nfkc), "file");
assert_eq!(normalize("\u{FF21}\u{FF22}\u{FF23}", Form::Nfkc), "ABC");
assert_eq!(normalize("½", Form::Nfkc), "1\u{2044}2");
```

```rust
use unicode_lang::{normalize, Form};

// Canonical ordering: combining marks are sorted by combining class, so two
// spellings of the same character normalize to one.
let a = "q\u{0307}\u{0323}"; // q + dot-above (230) + dot-below (220)
let b = "q\u{0323}\u{0307}"; // q + dot-below (220) + dot-above (230)
assert_eq!(normalize(a, Form::Nfd), normalize(b, Form::Nfd));
```

<br>

<h3 id="is_normalized"><code>is_normalized</code></h3>

```rust
pub fn is_normalized(s: &str, form: Form) -> bool
```

Returns `true` if `s` is already in normalization form `form`. The check begins
with the allocation-free Unicode quick-check; most inputs are decided there.
Only genuinely ambiguous input triggers a full normalization to resolve, and
even then no `String` is allocated — the normalized scalars are compared against
`s` directly. Testing before normalizing lets a caller skip the allocation
entirely for the overwhelmingly common already-normalized case.

**Parameters**

- `s: &str` — the input string.
- `form: Form` — the [`Form`](#form) to test against.

**Returns:** `bool` — `true` when `s` equals `normalize(s, form)`.

**Examples**

```rust
use unicode_lang::{is_normalized, Form};

assert!(is_normalized("é", Form::Nfc));          // precomposed: already NFC
assert!(!is_normalized("e\u{0301}", Form::Nfc)); // decomposed: not NFC
assert!(is_normalized("e\u{0301}", Form::Nfd));  // ...but it is NFD

assert!(is_normalized("ascii only", Form::Nfkc));
assert!(!is_normalized("ﬁ", Form::Nfkc));        // ligature folds under NFKC
```

```rust
use unicode_lang::{is_normalized, normalize, Form};

// Normalize only when needed, avoiding the allocation on the common path.
fn to_nfc(s: &str) -> std::borrow::Cow<'_, str> {
    if is_normalized(s, Form::Nfc) {
        std::borrow::Cow::Borrowed(s)
    } else {
        std::borrow::Cow::Owned(normalize(s, Form::Nfc))
    }
}

assert!(matches!(to_nfc("ready"), std::borrow::Cow::Borrowed(_)));
assert!(matches!(to_nfc("e\u{0301}"), std::borrow::Cow::Owned(_)));
```

<br>
<hr>

<h2 id="unicode_version"><code>UNICODE_VERSION</code></h2>

```rust
pub const UNICODE_VERSION: (u8, u8, u8);
```

The Unicode version the embedded tables were generated from, as
`(major, minor, patch)`. Read it to record or assert which Unicode release your
build's identifier, width, and normalization behaviour corresponds to.

**Examples**

```rust
use unicode_lang::UNICODE_VERSION;

let (major, _minor, _patch) = UNICODE_VERSION;
assert!(major >= 16);
```

<br>
<hr>

## Serde support

With the `serde` feature enabled, [`Form`](#form) implements
`serde::Serialize` and `serde::Deserialize` as a plain externally-tagged enum,
so a form can be read from or written to a config file.

```rust
# #[cfg(feature = "serde")]
# {
use unicode_lang::Form;

let json = serde_json::to_string(&Form::Nfkc).unwrap();
assert_eq!(json, "\"Nfkc\"");

let back: Form = serde_json::from_str(&json).unwrap();
assert_eq!(back, Form::Nfkc);
# }
```

Only `Form` is serializable; the functions operate on ordinary `&str` /
`String`, which serde already handles.

<br>
<hr>

## Design notes

### Generated tables

Every query is backed by a sorted, disjoint range table embedded in the crate.
The tables are produced by `dev/gen_tables.rs` — a committed, dependency-free
generator that reads the Unicode Character Database text files
(`UnicodeData`, `DerivedCoreProperties`, `DerivedNormalizationProps`,
`EastAsianWidth`) and emits `src/tables.rs`. The shipped crate contains only the
generated data, so there is no build-time download and no runtime dependency.
A lookup is a single `partition_point` binary search — branch-predictable and
cache-friendly. Regenerating against a newer Unicode release is one command;
[`UNICODE_VERSION`](#unicode_version) records which release a build carries.

### The width contract

Width is derived from two sources: the East Asian Width property (Wide and
Fullwidth ⇒ two columns) and the general category (nonspacing / enclosing marks
and format characters ⇒ zero columns), with the C0/C1 control blocks and the
conjoining Hangul jamo handled explicitly. East Asian *Ambiguous* is deliberately
narrow, and grapheme clustering is out of scope — see [Display width](#width).

### The normalization algorithm

`normalize` runs the standard UAX #15 pipeline: full canonical (or
compatibility) decomposition of every scalar, a stable sort of combining marks
by canonical combining class, and — for the composing forms — recomposition
through the primary-composite table. Hangul syllables are decomposed and
composed by formula rather than table. `is_normalized` and the fast path in
`normalize` use the quick-check algorithm over the per-form `*_QC` properties and
the combining-class ordering, resolving the residual *Maybe* case by a single
normalizing pass with no intermediate `String`.

### Conformance

The normalization implementation is validated against the official
`NormalizationTest.txt` from the UCD: every one of its ~19 000 published test
vectors across all four forms, plus the whole-codespace identity rule (every
code point not named in the test file is a fixed point of all four forms). The
suite runs in CI whenever the UCD data is present, and a curated subset of hard
cases (canonical ordering, Hangul LV/LVT, compatibility folds, partial
composition) runs unconditionally.

<br>
<hr>

<div align="center">
    <sup>
        <a href="../README.md" title="Project Home"><b>HOME</b></a>
        <span>&nbsp;│&nbsp;</span>
        <span>API</span>
        <span>&nbsp;│&nbsp;</span>
        <a href="../CHANGELOG.md" title="Changelog"><b>CHANGELOG</b></a>
    </sup>
</div>

<sub>Copyright &copy; 2026 <strong>James Gober</strong>.</sub>
