"""The public Python API surface, matching upstream's.

Distinct from the frozen suite in ``tests/original``: these cover the shape of
the binding itself -- exports, call conventions, equality semantics -- rather
than parsing behaviour, which the original suite already exercises.
"""

from decimal import Decimal

import pytest

import price_parser
from price_parser import Price, parse_price


def test_module_exports_match_upstream():
    assert price_parser.__all__ == ["Price", "parse_price"]


class TestFromstringCallConventions:
    """The suite calls this both ways, so both must work."""

    def test_positional(self):
        p = Price.fromstring("US$:12.99", None, None)
        assert p.amount == Decimal("12.99")
        assert p.currency == "US$"
        assert p.amount_text == "12.99"

    def test_keyword_only(self):
        # Upstream's test_price_decimal_separator skips currency_hint entirely.
        assert Price.fromstring("140.000", decimal_separator=".").amount == Decimal("140.000")

    def test_single_argument(self):
        assert Price.fromstring("$5").amount == Decimal("5")

    def test_no_arguments_is_all_none(self):
        p = Price.fromstring()
        assert (p.amount, p.currency, p.amount_text) == (None, None, None)

    def test_none_price(self):
        p = Price.fromstring(None, "$")
        assert p.currency == "$"
        assert p.amount is None

    def test_digit_group_separator(self):
        assert Price.fromstring("1,234.56", None, None, ",").amount == Decimal("1234.56")

    @pytest.mark.parametrize(
        ("text", "separator", "expected"),
        [
            ("140.000", None, Decimal("140000")),
            ("140.000", ",", Decimal("140000")),
            ("140.000", ".", Decimal("140.000")),
            ("140€33", "€", Decimal("140.33")),
            ("140,000€33", "€", Decimal("140000.33")),
            ("140.000€33", "€", Decimal("140000.33")),
        ],
    )
    def test_decimal_separator_cases(self, text, separator, expected):
        # Mirrors upstream's test_price_decimal_separator.
        assert Price.fromstring(text, decimal_separator=separator).amount == expected

    def test_empty_separator_falls_back_to_inference(self):
        # "" is falsy in upstream's `decimal_separator or get_decimal_separator(...)`,
        # so it infers rather than using the empty string literally.
        assert Price.fromstring("12,34", decimal_separator="").amount == Decimal("12.34")


class TestParsePriceAlias:
    def test_matches_fromstring(self):
        assert parse_price("R$ 50") == Price.fromstring("R$ 50")

    def test_accepts_the_same_arguments(self):
        p = parse_price("1,234.56", None, None, ",")
        assert p.amount == Decimal("1234.56")


class TestEquality:
    def test_equal_when_all_fields_match(self):
        assert Price(Decimal("1"), "$", "1") == Price(Decimal("1"), "$", "1")

    def test_unequal_when_a_field_differs(self):
        assert Price(Decimal("1"), "$", "1") != Price(Decimal("2"), "$", "1")
        assert Price(Decimal("1"), "$", "1") != Price(Decimal("1"), "€", "1")
        assert Price(Decimal("1"), "$", "1") != Price(Decimal("1"), "$", "one")

    def test_comparison_with_another_type_is_false_not_an_error(self):
        # __richcmp__ returns NotImplemented, and Python resolves that to False
        # once the reflected attempt also declines.
        assert Price(Decimal("1"), "$", "1") != "not a price"
        assert not (Price(Decimal("1"), "$", "1") == 42)

    def test_ordering_is_unsupported(self):
        with pytest.raises(TypeError):
            _ = Price(Decimal("1"), None, None) < Price(Decimal("2"), None, None)

    def test_subclass_equality_defers_to_the_subclass(self):
        # Upstream's attrs __eq__ compares only exact classes, returning
        # NotImplemented otherwise. That is what lets the real suite's
        # `assert parsed == example` reach Example.__eq__.
        class Loose(Price):
            def __eq__(self, other):
                return True

        assert Price(Decimal("1"), "$", "1") == Loose()
