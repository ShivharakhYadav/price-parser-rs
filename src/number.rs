//! Reading a numeric amount out of price text.

use std::str::FromStr;
use std::sync::LazyLock;

use regex::Regex;
use rust_decimal::Decimal;

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

/// True for characters Python's `str.strip()` treats as whitespace.
///
/// Rust's [`char::is_whitespace`] follows the Unicode `White_Space` property,
/// while Python's `str.isspace()` additionally covers `U+001C`--`U+001F`, the
/// file, group, record and unit separators. Upstream calls `.strip()`, so
/// `"\x1c1.5"` parses to `1.5` there; plain `trim()` would leave the control
/// character in place and fail.
fn is_python_whitespace(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}

/// Parse a number out of price text, resolving its separators.
///
/// `decimal_separator` forces the interpretation; passing `None` infers it via
/// [`get_decimal_separator`]. Returns `None` when the text holds no number.
///
/// ```
/// use price_parser::number::parse_number;
/// use rust_decimal::Decimal;
/// use std::str::FromStr;
///
/// // A three-digit group reads as thousands, so the comma is not a decimal point.
/// assert_eq!(parse_number("1,234", None), Some(Decimal::from_str("1234").unwrap()));
/// // Two digits, and it is.
/// assert_eq!(parse_number("12,34", None), Some(Decimal::from_str("12.34").unwrap()));
///
/// // Forcing the separator overrides that inference.
/// assert_eq!(parse_number("140.000", Some(',')), Some(Decimal::from_str("140000").unwrap()));
/// assert_eq!(parse_number("140.000", Some('.')), Some(Decimal::from_str("140.000").unwrap()));
///
/// assert_eq!(parse_number("foo", None), None);
/// ```
///
/// # Separators other than `.`, `,` and `€`
///
/// Upstream guards its final branch with `assert decimal_separator == "€"`, so
/// an unexpected value raises there. Any separator that is not `.` or `,` is
/// treated as the euro branch here instead of panicking: a panic would cross
/// the FFI boundary as a `PanicException` rather than the `AssertionError`
/// upstream raises, so it would not be faithful anyway, and the whole suite
/// only ever supplies `.`, `,`, `€` or nothing.
///
/// # Range
///
/// [`Decimal`] holds a 96-bit mantissa and a scale up to 28, where Python's
/// `Decimal` is arbitrary precision and also accepts `Infinity`, `NaN`,
/// exponent notation and PEP 515 underscores. Values beyond that return `None`
/// here. None are reachable through `Price::fromstring`, whose text extraction
/// yields only digits, spaces and separators, and the entire upstream corpus
/// sits well inside the range -- its widest amount is `123456.789` and its
/// longest digit run is 20.
pub fn parse_number(num: &str, decimal_separator: Option<char>) -> Option<Decimal> {
    if num.is_empty() {
        return None;
    }

    let prepared: String = num
        .trim_matches(is_python_whitespace)
        .chars()
        .filter(|c| *c != ' ')
        .collect();

    let separator = decimal_separator.or_else(|| get_decimal_separator(&prepared));

    // Strip whatever groups the digits, then normalise the decimal mark to '.'.
    let normalised: String = match separator {
        None => prepared
            .chars()
            .filter(|c| !matches!(c, '.' | ','))
            .collect(),
        Some('.') => prepared.chars().filter(|c| *c != ',').collect(),
        Some(',') => prepared
            .chars()
            .filter(|c| *c != '.')
            .map(|c| if c == ',' { '.' } else { c })
            .collect(),
        Some(_) => prepared
            .chars()
            .filter(|c| !matches!(c, '.' | ','))
            .map(|c| if c == '€' { '.' } else { c })
            .collect(),
    };

    Decimal::from_str(&normalised).ok()
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

    fn dec(s: &str) -> Option<Decimal> {
        Some(Decimal::from_str(s).unwrap())
    }

    /// The full set of upstream doctests for `parse_number`.
    #[test]
    fn parse_number_matches_upstream_doctests() {
        let cases: &[(&str, Option<char>, Option<Decimal>)] = &[
            ("1,234", None, dec("1234")),
            ("12,34", None, dec("12.34")),
            ("12,345", None, dec("12345")),
            ("1,1", None, dec("1.1")),
            ("1.1", None, dec("1.1")),
            ("1234", None, dec("1234")),
            ("12€34", None, dec("12.34")),
            ("12€ 34", None, dec("12.34")),
            ("1 234.99", None, dec("1234.99")),
            ("1,235€99", None, dec("1235.99")),
            ("1 235€99", None, dec("1235.99")),
            ("1.235€99", None, dec("1235.99")),
            ("140.000", Some(','), dec("140000")),
            ("140.000", Some('.'), dec("140.000")),
            ("", None, None),
            ("foo", None, None),
        ];
        for (num, sep, expected) in cases {
            assert_eq!(
                parse_number(num, *sep),
                *expected,
                "for {num:?} sep={sep:?}"
            );
        }
    }

    #[test]
    fn explicit_separator_overrides_inference() {
        assert_eq!(parse_number("1.234", Some(',')), dec("1234"));
        assert_eq!(parse_number("1.234", Some('.')), dec("1.234"));
        assert_eq!(parse_number("1,234", Some(',')), dec("1.234"));
        assert_eq!(parse_number("1,234", Some('.')), dec("1234"));
    }

    #[test]
    fn euro_separator_cases() {
        // Drawn from upstream's test_price_decimal_separator.
        assert_eq!(parse_number("140€33", Some('€')), dec("140.33"));
        assert_eq!(parse_number("140,000€33", Some('€')), dec("140000.33"));
        assert_eq!(parse_number("140.000€33", Some('€')), dec("140000.33"));
    }

    #[test]
    fn scale_is_preserved() {
        // 140.000 must not collapse to 140: upstream distinguishes them.
        assert_eq!(
            parse_number("140.000", Some('.')).unwrap().to_string(),
            "140.000"
        );
        assert_eq!(parse_number("0.00", None).unwrap().to_string(), "0.00");
    }

    #[test]
    fn python_whitespace_is_stripped() {
        // U+001C is whitespace to Python but not to Rust's trim(), so upstream
        // parses this and a plain trim() would not.
        assert_eq!(parse_number("\u{1c}1.5", None), dec("1.5"));
        assert_eq!(parse_number(" 1.5 ", None), dec("1.5"));
        assert_eq!(parse_number("\t1.5\n", None), dec("1.5"));
        assert_eq!(parse_number("\u{a0}1.5", None), dec("1.5"));
        assert_eq!(parse_number("1 000 000,50", None), dec("1000000.50"));
    }

    #[test]
    fn junk_yields_none() {
        for junk in ["1'234", "1..2", ".", ",", "€", "-", "--1", "foo", ""] {
            assert_eq!(parse_number(junk, None), None, "for {junk:?}");
        }
    }

    #[test]
    fn signs_are_accepted() {
        assert_eq!(parse_number("+1.5", None), dec("1.5"));
        assert_eq!(parse_number("-1.5", None), dec("-1.5"));
    }

    #[test]
    fn documented_divergences_from_python_decimal() {
        // Python's Decimal is arbitrary precision and accepts several forms
        // rust_decimal cannot represent. Pinned so they stay deliberate rather
        // than becoming surprises. None of these are reachable through
        // Price::fromstring, whose extraction yields only digits, spaces and
        // separators.
        //
        // Upstream returns Decimal('Infinity') / Decimal('NaN') for these.
        assert_eq!(parse_number("Infinity", None), None);
        assert_eq!(parse_number("NaN", None), None);
        assert_eq!(parse_number("inf", None), None);
        // Beyond the 96-bit mantissa, parsing fails outright and yields None
        // where upstream keeps full precision.
        assert_eq!(
            parse_number("1234567890123456789012345678901234567890", None),
            None
        );

        // Beyond scale 28 it does something quieter and worse: rather than
        // failing, rust_decimal rounds to zero. Upstream gives Decimal('1E-29').
        // Pinned explicitly because a silent zero is far more dangerous than a
        // None, and this is the shape a future bug here would take.
        assert_eq!(
            parse_number("0.00000000000000000000000000001", None),
            dec("0")
        );
    }

    #[test]
    fn underscores_are_accepted_like_python() {
        // Python's Decimal takes PEP 515 underscores, and so does rust_decimal,
        // so no special handling is needed. Pinned because it is easy to assume
        // otherwise.
        assert_eq!(parse_number("1_000", None), dec("1000"));
    }

    #[test]
    fn widest_corpus_values_are_representable() {
        // The real corpus tops out well inside rust_decimal's range. Note the
        // forced separator: unforced, "123456.789" has exactly three trailing
        // digits and so reads as a thousands group, giving 123456789 -- which
        // is what upstream returns too.
        assert_eq!(parse_number("123456.789", None), dec("123456789"));
        assert_eq!(parse_number("123456.789", Some('.')), dec("123456.789"));
        assert_eq!(
            parse_number("1.11000000000000009770", Some('.')),
            dec("1.11000000000000009770")
        );
    }
}
