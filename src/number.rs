//! Reading a numeric amount out of price text.

use std::sync::LazyLock;

use regex::Regex;

/// Matches a decimal separator and the digits trailing it.
///
/// ```text
/// \d*([.,€])(?:\d{1,2}?|\d{4}\d*?)$
/// ```
///
/// The digit count carries the meaning. One, two, or four-or-more trailing
/// digits mark a decimal separator; **exactly three** does not, because
/// `1.234` is far more likely to be a thousands group than a number with three
/// decimal places.
fn decimal_separator_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\d*([.,€])(?:\d{1,2}?|\d{4}\d*?)$")
            .expect("decimal separator pattern must compile")
    });
    &RE
}

/// Identify the decimal separator in `price`, if it has one.
///
/// Returns `None` when the text carries no separator, or when the separator
/// present looks like a thousands group instead.
///
/// ```
/// use price_parser::number::get_decimal_separator;
///
/// assert_eq!(get_decimal_separator("12.99"), Some('.'));
/// assert_eq!(get_decimal_separator("12,99"), Some(','));
/// assert_eq!(get_decimal_separator("1,235€99"), Some('€'));
///
/// // Exactly three trailing digits reads as a thousands group.
/// assert_eq!(get_decimal_separator("12.999"), None);
/// // Four or more reads as a decimal again.
/// assert_eq!(get_decimal_separator("3,0000"), Some(','));
/// ```
///
/// # Trailing newline
///
/// Python's `$` matches at the end of the string *or* just before a single
/// newline at the end of it; Rust's `$` matches only at the end of the
/// haystack. One trailing newline is therefore removed before matching, so
/// `"12.99\n"` yields `'.'` here exactly as it does upstream. Only one is
/// removed, and trailing spaces are not, which also matches: `"12.99 "` yields
/// `None` in both.
pub fn get_decimal_separator(price: &str) -> Option<char> {
    let text = price.strip_suffix('\n').unwrap_or(price);
    decimal_separator_regex()
        .captures(text)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().chars().next())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every expectation was produced by running upstream's
    /// `get_decimal_separator()` on the same input.
    #[test]
    fn matches_upstream() {
        let cases: &[(&str, Option<char>)] = &[
            // The upstream doctests.
            ("1000", None),
            ("12.99", Some('.')),
            ("12,99", Some(',')),
            ("12.999", None),
            ("3,0000", Some(',')),
            ("1,235€99", Some('€')),
            (".75", Some('.')),
            // No separator present.
            ("", None),
            ("abc", None),
            ("123", None),
            ("1 234", None),
            // A separator with nothing after it is not a decimal point.
            ("12.", None),
            ("12,", None),
            ("12€", None),
            // Leading separator.
            (",75", Some(',')),
            ("€75", Some('€')),
            // Trailing whitespace stops `$` from matching.
            ("12.99 ", None),
            (" 12.99", Some('.')),
        ];
        for (input, expected) in cases {
            assert_eq!(get_decimal_separator(input), *expected, "for {input:?}");
        }
    }

    #[test]
    fn three_trailing_digits_reads_as_thousands() {
        // The crux of the rule, swept across the boundary for each separator.
        for sep in ['.', ',', '€'] {
            assert_eq!(get_decimal_separator(&format!("1{sep}1")), Some(sep));
            assert_eq!(get_decimal_separator(&format!("1{sep}12")), Some(sep));
            assert_eq!(get_decimal_separator(&format!("1{sep}123")), None);
            assert_eq!(get_decimal_separator(&format!("1{sep}1234")), Some(sep));
            assert_eq!(get_decimal_separator(&format!("1{sep}12345")), Some(sep));
        }
    }

    #[test]
    fn last_separator_wins_when_several_are_present() {
        assert_eq!(get_decimal_separator("1.234,56"), Some(','));
        assert_eq!(get_decimal_separator("1,234.56"), Some('.'));
        assert_eq!(get_decimal_separator("1.234,5"), Some(','));
        assert_eq!(get_decimal_separator("1,234.5"), Some('.'));
        // Uniform grouping with three trailing digits is not a decimal.
        assert_eq!(get_decimal_separator("1.234.567"), None);
        assert_eq!(get_decimal_separator("1,234,567"), None);
    }

    #[test]
    fn euro_acts_as_a_separator_alongside_the_others() {
        assert_eq!(get_decimal_separator("1.235€99"), Some('€'));
        assert_eq!(get_decimal_separator("1,235€9"), Some('€'));
        assert_eq!(get_decimal_separator("1 235€99"), Some('€'));
    }

    #[test]
    fn single_trailing_newline_is_tolerated() {
        // Python's `$` matches before a final newline; Rust's does not, so this
        // would silently return None without the explicit strip.
        assert_eq!(get_decimal_separator("12.99\n"), Some('.'));
        assert_eq!(get_decimal_separator("12,99\n"), Some(','));
        assert_eq!(get_decimal_separator("1000\n"), None);
    }
}
