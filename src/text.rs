//! Locating the price substring inside arbitrary text.

use std::sync::LazyLock;

use regex::Regex;

/// Collapses runs of whitespace to a single space.
///
/// The class is `\s` plus `U+001C`--`U+001F` deliberately. Python's regex `\s`
/// matches the file, group, record and unit separators, while Rust's `\s` is
/// the Unicode `White_Space` property, which does not. Upstream normalises
/// `"1\x1c234"` to `"1 234"`; a bare `\s+` here would leave the control
/// character in place and change what the price regex then sees.
fn whitespace_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"[\s\x{1c}-\x{1f}]+").expect("whitespace pattern must compile")
    });
    &RE
}

/// Finds a run of digits, optionally preceded by a decimal point and carrying
/// group separators.
///
/// ```text
/// ([.]?\d[\d\s.,']*)   number, probably with thousand separators
/// \s*?                 skip whitespace
/// (?:[^%\d]|$)         next symbol, which must not be a percent sign
/// ```
///
/// The trailing condition is what rejects percentages: `"50%"` yields nothing
/// because the only thing following the digits is `%`, while `"50% OFF"` is
/// rejected for the same reason. Note this is an ordinary trailing group, not
/// a lookahead, so it consumes the character -- which is why the result is read
/// from capture group 1 rather than from the whole match.
///
/// Inside this pattern `\s` is safe as-is: normalisation has already replaced
/// every whitespace character with a plain space.
fn price_text_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"([.]?\d[\d\s.,']*)\s*?(?:[^%\d]|$)").expect("price pattern must compile")
    });
    &RE
}

/// Extract the substring holding the price from text that may contain other
/// things.
///
/// Where several price-looking substrings are present, the first wins.
///
/// ```
/// use price_parser::text::extract_price_text;
///
/// assert_eq!(extract_price_text("price: $12.99").as_deref(), Some("12.99"));
/// assert_eq!(extract_price_text("1,235 USD").as_deref(), Some("1,235"));
/// assert_eq!(extract_price_text("$.75").as_deref(), Some(".75"));
///
/// // A percentage is not a price.
/// assert_eq!(extract_price_text("50% OFF"), None);
///
/// // "free" counts as zero, but only when no number was found.
/// assert_eq!(extract_price_text("Free").as_deref(), Some("0"));
/// assert_eq!(extract_price_text("Foo"), None);
/// ```
///
/// # Not yet handled
///
/// Upstream has a second branch for text containing exactly one `€`, where the
/// euro sign itself acts as the decimal separator (`"35€ 99"` means `35.99`).
/// That branch uses a regex conditional group with no equivalent in Rust and
/// lands separately. Until then, such inputs fall through to the rule below and
/// may differ from upstream.
pub fn extract_price_text(price: &str) -> Option<String> {
    let normalised = whitespace_regex().replace_all(price, " ");

    if let Some(caps) = price_text_regex().captures(&normalised) {
        let matched = caps.get(1).expect("group 1 is not optional").as_str();

        // Trailing separators are never part of the number; a leading one is,
        // but only when it is the sole dot and so reads as a decimal point.
        let trimmed = matched.trim_end_matches([',', '.']).replace('\'', "");
        let text = if trimmed.matches('.').count() == 1 {
            trimmed.trim()
        } else {
            trimmed.trim_start_matches([',', '.']).trim()
        };
        return Some(text.to_string());
    }

    // Only consulted when no number was found at all. A plain substring test,
    // so "freedom" and "not free" both count -- as they do upstream.
    if normalised.to_lowercase().contains("free") {
        return Some("0".to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(text: &str) -> Option<String> {
        extract_price_text(text)
    }

    /// Expectations produced by running upstream's `extract_price_text()` on
    /// the same inputs. Every case here has a euro count other than one, so the
    /// branch that is not yet ported cannot affect it.
    #[test]
    fn matches_upstream() {
        let cases: &[(&str, Option<&str>)] = &[
            // upstream doctests reaching this branch
            ("price: $12.99", Some("12.99")),
            ("Free", Some("0")),
            ("Foo", None),
            ("1,235 USD", Some("1,235")),
            ("99 €, 79 €", Some("99")),
            ("99 € 79 €", Some("99")),
            ("50% OFF", None),
            ("50%", None),
            ("50", Some("50")),
            ("$1\u{a0}298,00", Some("1 298,00")),
            ("$.75", Some(".75")),
            // percent handling
            ("50 %", Some("50")),
            ("50%off", None),
            ("1.5%", Some("1")),
            ("%50", Some("50")),
            ("50%50", Some("50")),
            // empty and junk
            ("", None),
            (" ", None),
            ("abc", None),
            ("---", None),
            ("$", None),
            ("$$$", None),
            // separators and grouping
            ("1,235", Some("1,235")),
            ("1.235", Some("1.235")),
            ("1 235", Some("1 235")),
            ("1'235", Some("1235")),
            ("1,234.56", Some("1,234.56")),
            ("1.234,56", Some("1.234,56")),
            ("12.99", Some("12.99")),
            ("12,99", Some("12,99")),
            (".75", Some(".75")),
            (",75", Some("75")),
            ("0.5", Some("0.5")),
            ("00.50", Some("00.50")),
            // several numbers: the first wins
            ("12.99 and 15.99", Some("12.99")),
            ("from 5 to 10", Some("5")),
            ("1 2 3", Some("1 2 3")),
            // currency around the number
            ("USD 1,235", Some("1,235")),
            ("$12.99", Some("12.99")),
            ("12.99$", Some("12.99")),
            ("R$ 50", Some("50")),
            // zero or two euros, so the missing branch does not apply
            ("€€ 12.99", Some("12.99")),
            ("12.99 €€", Some("12.99")),
            ("€12€34€", Some("12")),
            // digits adjacent to letters
            ("abc123", Some("123")),
            ("123abc", Some("123")),
            ("a1b2c3", Some("1")),
            ("1234567890", Some("1234567890")),
            ("1,234,567.89", Some("1,234,567.89")),
        ];
        for (input, expected) in cases {
            assert_eq!(extract(input).as_deref(), *expected, "for {input:?}");
        }
    }

    #[test]
    fn trailing_separators_are_dropped() {
        assert_eq!(extract("12.99.").as_deref(), Some("12.99"));
        assert_eq!(extract("12.99,").as_deref(), Some("12.99"));
        assert_eq!(extract("12,").as_deref(), Some("12"));
        assert_eq!(extract("12.").as_deref(), Some("12"));
        assert_eq!(extract("1.2.3.").as_deref(), Some("1.2.3"));
    }

    #[test]
    fn a_leading_dot_survives_only_when_it_is_the_only_one() {
        // One dot reads as a decimal point and is kept.
        assert_eq!(extract(".5").as_deref(), Some(".5"));
        assert_eq!(extract("..5").as_deref(), Some(".5"));
        assert_eq!(extract("..5..").as_deref(), Some(".5"));
        // More than one, and the leading separators are stripped instead.
        assert_eq!(extract(".5.5").as_deref(), Some("5.5"));
        // Commas are never a leading decimal point.
        assert_eq!(extract(",5").as_deref(), Some("5"));
        assert_eq!(extract(",,5").as_deref(), Some("5"));
    }

    #[test]
    fn apostrophes_are_removed() {
        assert_eq!(extract("1'000").as_deref(), Some("1000"));
        assert_eq!(extract("1'000'000").as_deref(), Some("1000000"));
        assert_eq!(extract("1'2'3").as_deref(), Some("123"));
    }

    #[test]
    fn free_is_a_substring_test_not_a_word_test() {
        // Upstream checks `"free" in price.lower()`, so these all count.
        for text in [
            "free",
            "FREE",
            "FrEe",
            "Free shipping",
            "freedom",
            "not free",
        ] {
            assert_eq!(extract(text).as_deref(), Some("0"), "for {text:?}");
        }
        // But only when no number was found first.
        assert_eq!(extract("free 12.99").as_deref(), Some("12.99"));
    }

    #[test]
    fn python_whitespace_is_normalised() {
        assert_eq!(extract("  12.99  ").as_deref(), Some("12.99"));
        assert_eq!(extract("\t12.99\n").as_deref(), Some("12.99"));
        // Non-breaking space becomes a plain space and stays inside the number.
        assert_eq!(extract("12\u{a0}99").as_deref(), Some("12 99"));
        assert_eq!(
            extract("1\u{2009}12\u{2009}345").as_deref(),
            Some("1 12 345")
        );
        // U+001C is whitespace to Python's regex but not to Rust's, so these
        // would differ without the widened character class.
        assert_eq!(extract("\u{1c}12.99").as_deref(), Some("12.99"));
        assert_eq!(extract("12.99\u{1c}").as_deref(), Some("12.99"));
        assert_eq!(extract("1\u{1c}234").as_deref(), Some("1 234"));
    }
}
