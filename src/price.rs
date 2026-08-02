//! The parsed price value.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

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
}
