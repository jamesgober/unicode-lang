<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>CHANGELOG</b>
</h1>
<p>
  All notable changes to <code>unicode-lang</code> will be documented in this file. The format is based on <a href="https://keepachangelog.com/en/1.1.0/">Keep a Changelog</a>,
  and this project adheres to <a href="https://semver.org/spec/v2.0.0.html/">Semantic Versioning</a>.
</p>

---

## [Unreleased]

### Added

### Changed

### Fixed

### Security

---

## [1.0.0] - 2026-07-01

API freeze. The public surface introduced in 0.2.0 — the eight functions
(`is_xid_start`, `is_xid_continue`, `is_xid`, `char_width`, `str_width`,
`normalize`, `is_normalized`), the `Form` enum, the `UNICODE_VERSION` constant,
and the `std` / `alloc` / `serde` feature flags — is now stable and frozen under
Semantic Versioning. No functional changes from 0.2.0; this release records the
stability promise.

### Changed

- Marked `docs/API.md` stable and documented the SemVer promise: the frozen
  surface will not change in a breaking way within `1.x`; the Unicode data
  version may advance in a minor release without breaking the API.
- Bumped the version to `1.0.0`.

---

## [0.2.0] - 2026-07-01

The core release. Lands the three Unicode primitives the crate exists for —
identifier rules, display width, and all four normalization forms — as a
zero-dependency, `no_std` surface backed by tables generated from the Unicode
Character Database (16.0.0). This is the hard part of the roadmap, not deferred.

### Added

- **Identifiers (UAX #31):** `is_xid_start`, `is_xid_continue`, and the
  whole-string `is_xid`, backed by the `XID_Start` / `XID_Continue` derived
  properties. Allocation-free; available with no features.
- **Display width (UAX #11):** `char_width` and `str_width` return monospace
  column counts (0 / 1 / 2), `wcwidth`-style, from the East Asian Width property
  and general categories. Allocation-free.
- **Normalization (UAX #15):** `normalize` and `is_normalized`, selected by the
  new `Form` enum (`Nfc`, `Nfd`, `Nfkc`, `Nfkd`). Full canonical and
  compatibility decomposition, canonical ordering, primary-composite
  recomposition, and algorithmic Hangul. Validated against the official
  `NormalizationTest.txt` conformance suite (~19 000 vectors + whole-codespace
  identity).
- **`UNICODE_VERSION`** constant recording the UCD version the tables carry.
- `serde` feature deriving `Serialize` / `Deserialize` for `Form`.
- `alloc` feature (implied by `std`) gating the normalization API so identifier
  and width queries remain available in a bare `no_std` build.
- Generated lookup tables in `src/tables.rs` and the committed
  `dev/gen_tables.rs` generator that produces them from the UCD.
- Property tests (`tests/proptests.rs`), conformance tests
  (`tests/conformance.rs`), and Criterion benchmarks (`benches/bench.rs`).

### Fixed

- Manifest `keywords` / `categories` were unquoted bare identifiers and did not
  parse as valid TOML; they are now proper string arrays.
- `clippy.toml` MSRV (`1.87`) was ahead of the manifest `rust-version`; both are
  now aligned at `1.85`.

---

## [0.1.0] - 2026-06-18

Initial scaffold and repository bootstrap. No domain logic yet &mdash; this release establishes the structure, tooling, and quality gates the implementation will be built on.

### Added

- `Cargo.toml` with crate metadata, Rust 2024 edition, MSRV 1.85.
- Dual `Apache-2.0 OR MIT` license files.
- `README.md`, `CHANGELOG.md`, and a documentation skeleton.
- `REPS.md` compliance baseline.
- `.github/workflows/ci.yml` CI matrix; `deny.toml`, `clippy.toml`, `rustfmt.toml`.
- `dev/DIRECTIVES.md` and `dev/ROADMAP.md` (committed engineering standards + plan).

[Unreleased]: https://github.com/jamesgober/unicode-lang/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/jamesgober/unicode-lang/compare/v0.2.0...v1.0.0
[0.2.0]: https://github.com/jamesgober/unicode-lang/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/unicode-lang/releases/tag/v0.1.0
