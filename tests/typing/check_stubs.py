"""Verify the type stubs describe what the module actually exposes.

The stubs in ``price_parser/__init__.pyi`` are hand-written, so nothing stops
them drifting from ``src/python.rs``. This file is checked by ``mypy --strict``
in CI, and ``assert_type`` makes every expectation a hard failure rather than a
silent ``Any``.

It is not a pytest file and is deliberately outside ``testpaths`` -- at runtime
``assert_type`` does nothing, so running it proves nothing. The type checker is
the test.

    mypy --strict tests/typing/
"""

from __future__ import annotations

from decimal import Decimal
from typing import assert_type

from price_parser import Price, parse_price

# --- the classmethod constructor ------------------------------------------

assert_type(Price.fromstring("$12.99"), Price)
assert_type(Price.fromstring("$12.99", "USD"), Price)
assert_type(Price.fromstring("140.000", decimal_separator="."), Price)
assert_type(Price.fromstring("1,234.56", None, None, ","), Price)
assert_type(Price.fromstring(None), Price)

# --- the module-level alias -----------------------------------------------

assert_type(parse_price("R$ 50"), Price)
assert_type(parse_price(None, None, None, None), Price)

# --- fields and properties ------------------------------------------------

_price = Price.fromstring("$12.99")
assert_type(_price.amount, Decimal | None)
assert_type(_price.currency, str | None)
assert_type(_price.amount_text, str | None)
assert_type(_price.amount_float, float | None)

# --- direct construction --------------------------------------------------

assert_type(Price(Decimal("1.23"), "$", "1.23"), Price)
assert_type(Price(), Price)
assert_type(Price(amount=Decimal("5"), currency="EUR", amount_text="5"), Price)

# --- fields are writable, as the Rust setters allow -----------------------

_price.amount = Decimal("99.99")
_price.currency = "GBP"
_price.amount_text = "99.99"
_price.amount = None
_price.currency = None
