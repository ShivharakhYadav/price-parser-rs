//! The parsed price value.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use crate::number::{is_python_whitespace, parse_number};
use crate::symbols::extract_currency_symbol;
use crate::text::extract_price_text;

/// A price extracted from text: an amount, a currency, and the raw text the
/// amount was read from.
///
/// Every field is optional, because a string may carry a number with no
/// currency, a currency with no number, or neither.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Price {
    /// The numeric value, exact.
    ///
    /// [`Decimal`] rather than a float, so `0.10 + 0.20` is `0.30` and money
    /// does not drift.
    pub amount: Option<Decimal>,

    /// The currency symbol exactly as it appeared in the text.
    ///
    /// Not normalised to an ISO code: `"$"`, `"US$"` and `"USD"` are all
    /// reported as written.
    pub currency: Option<String>,

    /// The substring the amount was parsed from, before separators were
    /// interpreted.
    pub amount_text: Option<String>,
}

impl Price {
    /// Construct a price from its parts.
    pub fn new(
        amount: Option<Decimal>,
        currency: Option<String>,
        amount_text: Option<String>,
    ) -> Self {
        Price {
            amount,
            currency,
            amount_text,
        }
    }

    /// The amount as an `f64`, or `None` if there is no amount.
    ///
    /// Lossy by nature, and offered only because upstream exposes the same
    /// convenience. Prefer [`Price::amount`] for anything that must be exact.
    pub fn amount_float(&self) -> Option<f64> {
        self.amount.and_then(|amount| amount.to_f64())
    }

    /// Parse a price out of text.
    ///
    /// `price` is the text believed to hold the price. `currency_hint` is any
    /// nearby text that might name the currency -- a neighbouring element
    /// scraped from a page, say -- and is consulted only when `price` itself
    /// does not resolve one.
    ///
    /// `decimal_separator` and `digit_group_separator` override the inference
    /// when the format is already known.
    ///
    /// ```
    /// use price_parser::Price;
    /// use rust_decimal::Decimal;
    /// use std::str::FromStr;
    ///
    /// let p = Price::fromstring(Some("$12.99"), None, None, None);
    /// assert_eq!(p.currency.as_deref(), Some("$"));
    /// assert_eq!(p.amount, Some(Decimal::from_str("12.99").unwrap()));
    /// assert_eq!(p.amount_text.as_deref(), Some("12.99"));
    ///
    /// // A three-digit group is thousands, not decimals.
    /// let p = Price::fromstring(Some("1,235 USD"), None, None, None);
    /// assert_eq!(p.amount, Some(Decimal::from_str("1235").unwrap()));
    ///
    /// // Unless told otherwise.
    /// let p = Price::fromstring(Some("140.000"), None, Some('.'), None);
    /// assert_eq!(p.amount, Some(Decimal::from_str("140.000").unwrap()));
    /// ```
    ///
    /// # Ordering
    ///
    /// The currency is read from the text **before** `digit_group_separator`
    /// is removed, and the amount from the text **after**. That ordering is
    /// upstream's and it matters: stripping the separator first could delete
    /// characters the currency matcher needed.
    pub fn fromstring(
        price: Option<&str>,
        currency_hint: Option<&str>,
        decimal_separator: Option<char>,
        digit_group_separator: Option<&str>,
    ) -> Price {
        // Read from the original text, before any separator is stripped.
        let currency = extract_currency_symbol(price, currency_hint)
            // Upstream strips here rather than in the matcher, because at least
            // one safe symbol is stored with a leading space (" تومان").
            .map(|symbol| symbol.trim_matches(is_python_whitespace).to_string());

        let without_groups = match (price, digit_group_separator) {
            (Some(text), Some(separator)) => Some(text.replace(separator, "")),
            (Some(text), None) => Some(text.to_string()),
            (None, _) => None,
        };

        let amount_text = without_groups.as_deref().and_then(extract_price_text);
        let amount = amount_text
            .as_deref()
            .and_then(|text| parse_number(text, decimal_separator));

        Price {
            amount,
            currency,
            amount_text,
        }
    }
}

/// Parse a price out of text.
///
/// A free-function alias for [`Price::fromstring`], mirroring upstream's
/// `parse_price = Price.fromstring`.
pub fn parse_price(
    price: Option<&str>,
    currency_hint: Option<&str>,
    decimal_separator: Option<char>,
    digit_group_separator: Option<&str>,
) -> Price {
    Price::fromstring(
        price,
        currency_hint,
        decimal_separator,
        digit_group_separator,
    )
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn amount_float_converts_or_returns_none() {
        // Mirrors upstream's test_price_amount_float.
        assert_eq!(Price::default().amount_float(), None);
        assert_eq!(
            Price::new(Some(Decimal::from_str("1.23").unwrap()), None, None).amount_float(),
            Some(1.23)
        );
    }

    #[test]
    fn decimal_arithmetic_is_exact() {
        let a = Decimal::from_str("0.10").unwrap();
        let b = Decimal::from_str("0.20").unwrap();
        assert_eq!(a + b, Decimal::from_str("0.30").unwrap());
    }

    #[test]
    fn trailing_zeros_are_preserved() {
        // "140.000" must not collapse to "140": upstream's decimal-separator
        // tests distinguish the two by scale.
        let d = Decimal::from_str("140.000").unwrap();
        assert_eq!(d.to_string(), "140.000");
    }

    fn parse(price: &str) -> Price {
        Price::fromstring(Some(price), None, None, None)
    }

    #[test]
    fn parses_all_three_fields() {
        let p = parse("$12.99");
        assert_eq!(p.amount, Decimal::from_str("12.99").ok());
        assert_eq!(p.currency.as_deref(), Some("$"));
        assert_eq!(p.amount_text.as_deref(), Some("12.99"));
    }

    #[test]
    fn missing_pieces_are_none_not_errors() {
        let p = parse("no price here");
        assert_eq!(p.amount, None);
        assert_eq!(p.currency, None);
        assert_eq!(p.amount_text, None);

        // A number with no currency, and a currency with no number.
        assert_eq!(parse("12.99").currency, None);
        assert_eq!(parse("$").amount, None);

        // No price string at all.
        let p = Price::fromstring(None, Some("$"), None, None);
        assert_eq!(p.currency.as_deref(), Some("$"));
        assert_eq!(p.amount, None);
        assert_eq!(p.amount_text, None);
    }

    #[test]
    fn currency_is_trimmed() {
        // The matcher returns " تومان" with its leading space intact; trimming
        // is fromstring's job.
        let p = parse("100 تومان");
        assert_eq!(p.currency.as_deref(), Some("تومان"));
    }

    #[test]
    fn hint_supplies_the_currency_when_the_price_does_not() {
        let p = Price::fromstring(Some("12.99"), Some("EUR"), None, None);
        assert_eq!(p.currency.as_deref(), Some("EUR"));
        assert_eq!(p.amount, Decimal::from_str("12.99").ok());
    }

    #[test]
    fn separators_can_be_forced() {
        // Upstream's test_price_decimal_separator cases.
        let cases: &[(&str, Option<char>, &str)] = &[
            ("140.000", None, "140000"),
            ("140.000", Some(','), "140000"),
            ("140.000", Some('.'), "140.000"),
            ("140€33", Some('€'), "140.33"),
            ("140,000€33", Some('€'), "140000.33"),
            ("140.000€33", Some('€'), "140000.33"),
        ];
        for (text, separator, expected) in cases {
            let p = Price::fromstring(Some(text), None, *separator, None);
            assert_eq!(
                p.amount,
                Decimal::from_str(expected).ok(),
                "for {text:?} sep={separator:?}"
            );
        }
    }

    #[test]
    fn digit_group_separator_is_removed_before_the_amount_is_read() {
        let p = Price::fromstring(Some("1,234.56"), None, None, Some(","));
        assert_eq!(p.amount, Decimal::from_str("1234.56").ok());
    }

    #[test]
    fn currency_is_read_before_the_group_separator_is_stripped() {
        // Ordering guard. Stripping "$" as a group separator first would leave
        // no currency to find; upstream reads the currency from the original
        // text, so it survives.
        let p = Price::fromstring(Some("$1,234"), None, None, Some("$"));
        assert_eq!(p.currency.as_deref(), Some("$"));
    }

    #[test]
    fn parse_price_alias_matches_fromstring() {
        assert_eq!(
            parse_price(Some("$12.99"), None, None, None),
            Price::fromstring(Some("$12.99"), None, None, None)
        );
    }
}
