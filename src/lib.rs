//! Extract a price amount and currency symbol from a raw text string.
//!
//! A Rust port of [`scrapinghub/price-parser`][upstream]. Not affiliated with,
//! nor endorsed by, Scrapinghub.
//!
//! The crate is usable as an ordinary Rust library. When built with the
//! `python` feature (as maturin does) it additionally exposes a native CPython
//! extension module presenting the same API as the upstream Python package, so
//! that project's original test suite runs against this code unmodified.
//!
//! # Safety
//!
//! The port logic contains no `unsafe`, and the pure-Rust build enforces that
//! with `forbid(unsafe_code)`. The prohibition is relaxed only when the
//! `python` feature is on, because the PyO3 macros that build the FFI boundary
//! expand to `unsafe` code. All of it is confined to [`python`].
//!
//! [upstream]: https://github.com/scrapinghub/price-parser

// Unsafe is forbidden outright in the pure-Rust build. With the `python`
// feature it must be permitted, since PyO3's macro expansion relies on it at
// the FFI boundary -- but no hand-written unsafe exists in this crate.
#![cfg_attr(not(feature = "python"), forbid(unsafe_code))]
#![warn(missing_docs)]

pub mod currencies;
pub mod number;
pub mod price;
pub mod symbols;
pub mod text;

pub use price::Price;

#[cfg(feature = "python")]
mod python;
