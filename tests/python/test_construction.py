"""Construction-protocol tests for the Rust-backed `Price` class.

These are our own tests, distinct from the frozen upstream suite in
``tests/original``. They exist because Python's two-phase construction is the
single most fragile part of exposing a Rust type: the upstream suite subclasses
``Price`` and calls ``super().__init__(...)``, and a binding that handles only
one of ``__new__``/``__init__`` fails silently rather than loudly.

``Example`` below is deliberately shaped like the real one in the upstream
suite -- five positional arguments that have nothing to do with ``Price``'s own
signature, followed by an explicit ``super().__init__`` with keywords.
"""

from decimal import Decimal

import pytest

from price_parser import Price


class Example(Price):
    """Mirrors the upstream suite's test wrapper."""

    def __init__(
        self,
        currency_raw,
        price_raw,
        currency,
        amount_text,
        amount_float,
        decimal_separator=None,
    ):
        self.currency_raw = currency_raw
        self.price_raw = price_raw
        self.decimal_separator = decimal_separator
        amount_decimal = None
        if isinstance(amount_float, Decimal):
            amount_decimal = amount_float
        elif amount_float is not None:
            amount_decimal = Decimal(str(amount_float))
        super().__init__(
            amount=amount_decimal,
            currency=currency,
            amount_text=amount_text,
        )


class TestDirectConstruction:
    """`type.__call__` reaches tp_new, so `#[new]` must populate the fields."""

    def test_positional(self):
        p = Price(Decimal("12.99"), "$", "12.99")
        assert p.amount == Decimal("12.99")
        assert p.currency == "$"
        assert p.amount_text == "12.99"

    def test_keyword(self):
        p = Price(amount=Decimal("5.00"), currency="€", amount_text="5,00")
        assert p.amount == Decimal("5.00")
        assert p.currency == "€"
        assert p.amount_text == "5,00"

    def test_all_none(self):
        p = Price(None, None, None)
        assert p.amount is None
        assert p.currency is None
        assert p.amount_text is None

    def test_no_arguments(self):
        p = Price()
        assert p.amount is None


class TestSubclassConstruction:
    """The case that gates the whole upstream suite."""

    def test_super_init_stores_the_real_values(self):
        e = Example("US$", "US$:12.99", "US$", "12.99", 12.99)
        # Set directly by Example.__init__, needs `dict`.
        assert e.currency_raw == "US$"
        assert e.price_raw == "US$:12.99"
        # Only present if super().__init__ reached the binding. Note tp_new saw
        # Example's five unrelated arguments first and must have been overwritten.
        assert e.amount == Decimal("12.99")
        assert e.currency == "US$"
        assert e.amount_text == "12.99"

    def test_subclass_is_a_price(self):
        assert isinstance(Example("$", "1", "$", "1", 1.0), Price)

    def test_none_amount_survives_the_subclass_path(self):
        e = Example(None, "no price", None, None, None)
        assert e.amount is None
        assert e.currency is None


class TestAmountFloat:
    """Mirrors upstream's test_price_amount_float."""

    @pytest.mark.parametrize(
        ("amount", "expected"),
        [(None, None), (Decimal("1.23"), 1.23)],
    )
    def test_amount_float(self, amount, expected):
        assert Price(amount, None, None).amount_float == expected


class TestDecimalFidelity:
    """Exactness is the correctness story; floats must not creep in."""

    def test_no_float_drift(self):
        a = Price(Decimal("0.10"), None, None).amount
        b = Price(Decimal("0.20"), None, None).amount
        assert a + b == Decimal("0.30")

    def test_amount_is_a_real_decimal(self):
        assert isinstance(Price(Decimal("1.5"), None, None).amount, Decimal)

    def test_trailing_zeros_preserved(self):
        # Upstream's decimal-separator cases distinguish 140.000 from 140.
        assert str(Price(Decimal("140.000"), None, None).amount) == "140.000"


class TestAttributesAreWritable:
    def test_setters(self):
        p = Price(None, None, None)
        p.currency = "£"
        p.amount_text = "9.99"
        p.amount = Decimal("9.99")
        assert (p.currency, p.amount_text, p.amount) == ("£", "9.99", Decimal("9.99"))


def test_repr_omits_amount_text():
    # attrs marks amount_text repr=False upstream.
    r = repr(Price(Decimal("12.99"), "$", "12.99"))
    assert "12.99" in r
    assert "amount_text" not in r
