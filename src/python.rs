//! CPython bindings.
//!
//! The only part of the crate that touches FFI. A thin shim over the pure-Rust
//! core, holding no parsing logic of its own, so the upstream project's
//! original test suite can import and exercise this implementation unmodified.
//!
//! # Construction
//!
//! The suite subclasses `Price` and calls `super().__init__(...)`, which forces
//! both halves of Python's construction protocol to be handled:
//!
//! * `#[new]` becomes `tp_new`. `type.__call__` always routes through it, so it
//!   is what direct `Price(a, b, c)` construction hits.
//! * `__init__` defined here lands in the type's dict but **not** in the
//!   `tp_init` slot. An explicit `super().__init__(...)` finds it by name;
//!   `type.__call__` does not.
//!
//! Neither alone is enough. Direct construction reaches only `tp_new`, while a
//! subclass routes its own arguments through `tp_new` before calling
//! `super().__init__` with the real ones. Both are therefore implemented, and
//! `__init__` assigns every field so it fully overwrites whatever `tp_new` made
//! of a subclass's unrelated signature.
//!
//! This was established by experiment, not assumption: defining only `__init__`
//! leaves direct construction silently returning empty values.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::price::Price as CorePrice;

/// Python-facing `Price`.
///
/// `subclass` allows the suite's `class Example(Price)`; `dict` lets that
/// subclass assign attributes of its own. Both are required.
#[pyclass(name = "Price", module = "price_parser", subclass, dict)]
#[derive(Default)]
pub struct PyPrice {
    inner: CorePrice,
}

/// Read a Python object as an exact decimal.
///
/// Goes via the string form so a `decimal.Decimal` keeps its exact value and
/// scale. Ints and floats are accepted too, since upstream's constructor is
/// not fussy. Anything unparseable becomes `None` rather than an error, which
/// matches how a subclass's unrelated positional arguments must be tolerated.
fn to_decimal(value: &Bound<'_, PyAny>) -> Option<Decimal> {
    if value.is_none() {
        return None;
    }
    if let Ok(text) = value.extract::<String>() {
        return Decimal::from_str(&text).ok();
    }
    value
        .str()
        .ok()
        .and_then(|s| s.extract::<String>().ok())
        .and_then(|s| Decimal::from_str(&s).ok())
}

/// Convert an exact decimal back into a `decimal.Decimal`.
fn to_py_decimal<'py>(py: Python<'py>, value: &Decimal) -> PyResult<Bound<'py, PyAny>> {
    py.import("decimal")?
        .getattr("Decimal")?
        .call1((value.to_string(),))
}

/// Pull one argument from the positional tuple, falling back to keywords.
fn argument<'py>(
    args: &Bound<'py, PyTuple>,
    kwargs: Option<&Bound<'py, PyDict>>,
    index: usize,
    name: &str,
) -> Option<Bound<'py, PyAny>> {
    args.get_item(index)
        .ok()
        .or_else(|| kwargs.and_then(|d| d.get_item(name).ok().flatten()))
}

#[pymethods]
impl PyPrice {
    /// `tp_new`, and so the path taken by direct construction.
    ///
    /// Permissive by necessity: a Python subclass passes its own, unrelated
    /// signature through here before `__init__` runs, and raising on arity
    /// would break every such subclass.
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        let amount = argument(args, kwargs, 0, "amount").and_then(|v| to_decimal(&v));
        let currency = argument(args, kwargs, 1, "currency").and_then(|v| v.extract().ok());
        let amount_text = argument(args, kwargs, 2, "amount_text").and_then(|v| v.extract().ok());
        PyPrice {
            inner: CorePrice::new(amount, currency, amount_text),
        }
    }

    /// Reachable through an explicit `super().__init__(...)`.
    ///
    /// Assigns all three fields unconditionally, so a subclass's real values
    /// replace whatever `tp_new` made of that subclass's own arguments.
    #[pyo3(signature = (amount=None, currency=None, amount_text=None))]
    fn __init__(
        &mut self,
        amount: Option<Bound<'_, PyAny>>,
        currency: Option<String>,
        amount_text: Option<String>,
    ) {
        self.inner = CorePrice::new(amount.as_ref().and_then(to_decimal), currency, amount_text);
    }

    /// The numeric value as a `decimal.Decimal`, or `None`.
    #[getter]
    fn amount<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .amount
            .as_ref()
            .map(|d| to_py_decimal(py, d))
            .transpose()
    }

    #[setter]
    fn set_amount(&mut self, value: Option<Bound<'_, PyAny>>) {
        self.inner.amount = value.as_ref().and_then(to_decimal);
    }

    /// The currency symbol as it appeared in the text.
    #[getter]
    fn currency(&self) -> Option<String> {
        self.inner.currency.clone()
    }

    #[setter]
    fn set_currency(&mut self, value: Option<String>) {
        self.inner.currency = value;
    }

    /// The raw substring the amount was read from.
    #[getter]
    fn amount_text(&self) -> Option<String> {
        self.inner.amount_text.clone()
    }

    #[setter]
    fn set_amount_text(&mut self, value: Option<String>) {
        self.inner.amount_text = value;
    }

    /// The amount as a float, or `None`.
    #[getter]
    fn amount_float(&self) -> Option<f64> {
        self.inner.amount_float()
    }

    fn __repr__(&self) -> String {
        // attrs marks amount_text repr=False upstream, so it is omitted here.
        let amount = match &self.inner.amount {
            Some(d) => format!("Decimal('{d}')"),
            None => "None".to_string(),
        };
        let currency = match &self.inner.currency {
            Some(c) => format!("'{c}'"),
            None => "None".to_string(),
        };
        format!("Price(amount={amount}, currency={currency})")
    }
}

/// The `price_parser` extension module.
///
/// Named to match the upstream Python package so the vendored original tests
/// resolve `from price_parser import ...` to this code.
#[pymodule]
fn price_parser(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__doc__", "Rust port of scrapinghub/price-parser.")?;
    m.add_class::<PyPrice>()?;
    Ok(())
}
