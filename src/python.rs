//! CPython bindings.
//!
//! This is the only part of the crate that touches FFI. It is a thin shim over
//! the pure-Rust core: it owns no parsing logic of its own, existing solely so
//! the upstream project's original test suite can import and exercise this
//! implementation without modification.

use pyo3::prelude::*;

/// The `price_parser` extension module.
///
/// Named to match the upstream Python package so the vendored original tests
/// resolve `from price_parser import ...` to this code.
#[pymodule]
fn price_parser(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__doc__", "Rust port of scrapinghub/price-parser.")?;
    Ok(())
}
