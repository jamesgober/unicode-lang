# unicode-lang - Roadmap

> Path from scaffold to a stable 1.0. Hard parts are front-loaded; each phase has hard exit criteria.
> Master plan: ../../_strategy/LANG_COLLECTION.md
>
> **Anti-deferral rule:** no listed hard task moves to a later phase unless this file records the move and the reason.

## v0.1.0 - Scaffold (DONE)
Compiles, CI green, structure correct, no domain logic.
- [x] Manifest, README, CHANGELOG, REPS, dual license, CI, deny, clippy, rustfmt.

## v0.2.0 - Core (DONE)
Unicode identifier rules (XID), normalization, and character width.
Dependencies (none) are wired here, when first used.
Exit criteria:
- [x] Every public item has rustdoc + a runnable example.
- [x] Core invariants property-tested.

## v1.0.0 - API freeze (DONE)
Public surface stable and frozen until 2.0.
- [x] docs/API.md marked stable; SemVer promise recorded.
- [x] Full test + benchmark suite green on all three platforms.

## v1.0.1 - Hardening patch (DONE)
Fixes from the LexerSketch audit (`_lexersketch/ISSUES.md` H11, M67, and the patch-level part of M68). No public API change, and normalization output is byte-identical: the full `NormalizationTest.txt` suite passes unchanged, and the new ordering is checked against the old one directly.

Delivered:
- [x] **H11** Canonical ordering is a stable sort per run of non-starters instead of an insertion sort over the buffer. Runs already in order are only scanned; an out-of-order run is sorted once over `(class, scalar)` pairs with the standard library's stable sort, so the pass is `O(n log n)`. One base letter plus alternating U+0301 / U+0323 marks, release build: 64 000 marks went from 2.6 s to 1.4 ms, 256 000 from 57 s to 5.9 ms, and 1 000 000 marks take 24 ms (about ten minutes before, extrapolated from the measured quadratic growth). The old insertion sort is kept in the unit tests as a reference: every sequence of up to four scalars from a pool of starters and marks (including equal classes, so stability shows), plus property tests over arbitrary scalars, must order identically. `tests/adversarial.rs` normalizes a million alternating marks exactly and checks that eight times the input costs less than 24 times the time (quadratic is 64).
- [x] **M67** CI runs the full conformance suite on every platform. `dev/fetch_ucd.sh` downloads pinned Unicode 16.0.0 files and checks them against `dev/ucd.sha256` (a mismatch deletes the file and fails); the workflow fetches `NormalizationTest.txt` and sets `UNICODE_LANG_REQUIRE_UCD=1`, which makes a missing file a test failure. Without the variable, a local run with no data passes and prints a skip notice that the harness does not capture. The test also checks that the file's header names the release in `UNICODE_VERSION` and that it has more than 19 000 records (it has 19 965).
- [x] **M68** (patch part) ASCII fast paths: `is_xid_start`, `is_xid_continue`, and `char_width` answer ASCII without a table; `is_xid` and `str_width` check pure-ASCII strings a byte at a time; `normalize` and `is_normalized` recognise pure-ASCII input with `str::is_ascii` and do no per-scalar work (`is_normalized` allocates nothing; `normalize` allocates only the returned `String`, which the frozen signature requires); inside mixed text, ASCII skips the quick-check, decomposition, and composition lookups. Each shortcut is checked against the table at every code point (or, for decomposition and composition, by asserting no table entry involves ASCII).
- [x] Test and bench targets build without default features (`tests/*.rs` gate their normalization parts on `alloc`; the bench requires it), and the crate-level doctest no longer fails under `--no-default-features`. Both were broken in 1.0.0.
- [x] `dev/gen_tables.rs` documents the real regeneration steps; with the pinned data it reproduces `src/tables.rs` byte for byte (checked).

Dependency wiring: unchanged — no runtime dependencies; dev-dependencies unchanged (`criterion`, `proptest`, `serde_json`).

Not in this patch (recorded under the anti-deferral rule; each changes output or adds API):
- **Stream-safe text format (UAX #15 D4)** → 1.1, additive. A separate entry point (or an options value) that inserts U+034F COMBINING GRAPHEME JOINER so that no more than 30 non-starters ever appear in a row, which bounds the buffer a streaming normalizer needs. It cannot be the default: it changes output, and `normalize` is frozen to the standard forms. With H11 fixed, long runs are no longer a time hazard, only a memory one proportional to the input.
- **`Cow` API** → 1.1, additive. `normalize` must return a `String`, so already-normalized input is copied. A new `normalize_cow(&str, Form) -> Cow<'_, str>` (or similar) can return the input borrowed.
- **Scratch buffers** → 1.1, internal. Normalization decomposes into a `Vec<char>` (four bytes per scalar) and composes into a second one. Decomposing only from the first scalar that needs it, and writing UTF-8 directly, removes most of that; it changes no output, but it rewrites the pipeline, so it is not patch material.
- **Trie tables** → 1.1, internal. Replacing the range tables (binary search over about 700 ranges for `XID_Continue`) with two-level tries changes the generator and table format, not any output. Worth doing with the buffer work and measured against these 1.0.1 numbers.
- **Width corrections** → 1.1, documented output change. Hangul Jamo Extended-B medial and final jamo (U+D7B0–U+D7C6, U+D7CB–U+D7FB) should be zero width like U+1160–U+11FF; visible format characters (the prepended concatenation marks U+0600–U+0605, U+06DD, U+070F, U+0890–U+0891, U+08E2, U+110BD, U+110CD) should be one column, not zero. These change `char_width` for assigned code points. The 1.0 promise allows refinement for previously unassigned code points in a minor release; correcting assigned ones is a judgement call between a documented 1.1 fix and 2.0, to be decided when the work is done.

## Later 1.x candidates (additive)
None of these changes the frozen surface; each is a new function or type.
- Stream-safe normalization, `Cow` normalization, scratch-buffer and trie work (above).
- A normalization iterator adaptor over `chars()`.
