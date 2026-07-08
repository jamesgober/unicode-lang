//! # unicode_lang
//!
//! The Unicode text primitives a language front end needs: identifier rules,
//! normalization, and display width — with correct-by-construction tables and
//! no third-party dependencies.
//!
//! A lexer has to answer three Unicode questions that the standard library does
//! not: *may this scalar start or continue an identifier?* (UAX #31), *are
//! these two strings the same once normalized?* (UAX #15), and *how many
//! columns does this text occupy?* (UAX #11). `unicode-lang` answers all three
//! from compact lookup tables generated directly from the Unicode Character
//! Database, so the crate carries no runtime dependencies and every query is a
//! branch-predictable binary search.
//!
//! ## At a glance
//!
//! - **Identifiers** — [`is_xid_start`], [`is_xid_continue`], and the
//!   whole-string [`is_xid`] implement the UAX #31 `XID` profile.
//! - **Width** — [`char_width`] and [`str_width`] give monospace column counts
//!   (0 / 1 / 2), `wcwidth`-style.
//! - **Normalization** — [`normalize`] and [`is_normalized`], selected by
//!   [`Form`], implement all four forms (NFC, NFD, NFKC, NFKD) and are verified
//!   against the official conformance suite. *(Requires the `alloc` feature,
//!   enabled by default via `std`.)*
//! - [`UNICODE_VERSION`] records the UCD version the tables were built from.
//!
//! ## Example
//!
//! ```
//! use unicode_lang::{char_width, is_xid, normalize, Form};
//!
//! // Recognise an identifier that mixes scripts.
//! assert!(is_xid("Δpressure"));
//!
//! // Fold a compatibility ligature and recompose.
//! assert_eq!(normalize("ﬁle", Form::Nfkc), "file");
//!
//! // Measure a mixed-width string for column alignment.
//! assert_eq!(char_width('世'), 2);
//! ```
//!
//! ## `no_std`
//!
//! The crate is `no_std`. Identifier and width queries need no allocator and
//! are available with no features at all. Normalization allocates its output,
//! so it is gated behind the `alloc` feature (implied by the default `std`
//! feature); disable default features for a bare `no_std` build without
//! normalization. The optional `serde` feature derives `Serialize` /
//! `Deserialize` for [`Form`].

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(unused_must_use)]
#![deny(unused_results)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::unreachable)]

mod ident;
mod lookup;
mod tables;
mod width;

#[cfg(feature = "alloc")]
mod normalize;

pub use ident::{is_xid, is_xid_continue, is_xid_start};
pub use tables::UNICODE_VERSION;
pub use width::{char_width, str_width};

#[cfg(feature = "alloc")]
pub use normalize::{Form, is_normalized, normalize};
