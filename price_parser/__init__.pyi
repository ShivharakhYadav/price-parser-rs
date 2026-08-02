"""Type stubs for the price_parser extension module.

The implementation is Rust, so there are no inline annotations for a type
checker to read. These stubs restore what upstream's ``py.typed`` package
provides, keeping the port a drop-in replacement for type checking as well as
at runtime.

Kept deliberately in step with `src/python.rs`. Signatures mirror upstream's
`price_parser/parser.py`.
"""

from decimal import Decimal

__all__ = ["Price", "parse_price"]

class Price:
    """A price extracted from text."""

    amount: Decimal | None
    currency: str | None
    amount_text: str | None

    def __init__(
        self,
        amount: Decimal | None = ...,
        currency: str | None = ...,
        amount_text: str | None = ...,
    ) -> None: ...
    @property
    def amount_float(self) -> float | None: ...
    @classmethod
    def fromstring(
        cls,
        price: str | None = ...,
        currency_hint: str | None = ...,
        decimal_separator: str | None = ...,
        digit_group_separator: str | None = ...,
    ) -> Price: ...

def parse_price(
    price: str | None = ...,
    currency_hint: str | None = ...,
    decimal_separator: str | None = ...,
    digit_group_separator: str | None = ...,
) -> Price: ...
