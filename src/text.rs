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

/// The euro-as-decimal-separator pattern, with the group **participating**.
///
/// Upstream writes one pattern using a conditional group:
///
/// ```text
/// [\d\s.,']*?\d    number, probably with thousand separators
/// \s*?€(\s*?)?     euro, probably separated by whitespace   <- group 1
/// \d(?(1)\d|\d*?)  group 1 matched -> one more digit; else -> a lazy run
/// (?:$|[^\d])
/// ```
///
/// `(?(1)yes|no)` is an if-then-else with no equivalent in Rust's `regex`. It
/// does not need one: the conditional has exactly two outcomes, so the pattern
/// splits cleanly into two, tried in the same order the engine would.
///
/// This is the *yes* arm. `(\s*?)?` participates -- matching zero or more
/// whitespace -- so exactly two digits must follow.
fn euro_participating_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"[\d\s.,']*?\d\s*?€\s*?\d\d(?:$|[^\d])")
            .expect("euro participating pattern must compile")
    });
    &RE
}

/// The same pattern with the group **skipped**.
///
/// Reached only by backtracking, when the arm above fails. Skipping the group
/// consumes no whitespace at all, so a digit must follow the euro sign
/// immediately -- and the digit run is then lazy and unbounded rather than
/// fixed at two.
///
/// That distinction is the whole point of the conditional, and it is load
/// bearing: `"12€345"` matches here with three digits, while `"12€ 345"`
/// matches neither arm and falls through to the ordinary rule.
fn euro_skipped_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"[\d\s.,']*?\d\s*?€\d\d*?(?:$|[^\d])")
            .expect("euro skipped pattern must compile")
    });
    &RE
}

/// Find the euro-separated price, reproducing the engine's search order.
///
/// Python tries start positions left to right and, at each one, the
/// participating arm before the skipped arm. Running both patterns separately
/// and taking the earlier match -- preferring the participating arm on a tie --
/// gives exactly that: whichever arm starts earlier is the one the engine would
/// have reached first, because a later start is only ever tried after every
/// earlier one has failed for *both* arms.
fn find_euro_price(text: &str) -> Option<&str> {
    let participating = euro_participating_regex().find(text);
    let skipped = euro_skipped_regex().find(text);

    match (participating, skipped) {
        (Some(a), Some(b)) => Some(if a.start() <= b.start() { a } else { b }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
    .map(|m| m.as_str())
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
/// # The euro as a decimal separator
///
/// When the text holds exactly one `€`, the sign itself may be acting as the
/// decimal point, so `"35€ 99"` means `35.99` and yields `"35€99"`. Whitespace
/// around it is removed, and the amount keeps the euro sign for
/// [`crate::number::parse_number`] to interpret.
///
/// ```
/// use price_parser::text::extract_price_text;
///
/// assert_eq!(extract_price_text("35€ 99").as_deref(), Some("35€99"));
/// assert_eq!(extract_price_text("1,235€ 99").as_deref(), Some("1,235€99"));
///
/// // Three digits do not fit the pattern, so this is an ordinary price.
/// assert_eq!(extract_price_text("35€ 999").as_deref(), Some("35"));
///
/// // Two euro signs, and the rule does not apply at all.
/// assert_eq!(extract_price_text("99 €, 79 €").as_deref(), Some("99"));
/// ```
pub fn extract_price_text(price: &str) -> Option<String> {
    let normalised = whitespace_regex().replace_all(price, " ");

    // Only when there is exactly one euro sign can it be the decimal point;
    // two or more mean it is just currency, as in "99 €, 79 €".
    if normalised.matches('€').count() == 1 {
        if let Some(matched) = find_euro_price(&normalised) {
            return Some(matched.replace(' ', ""));
        }
    }

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

    /// The euro branch, against upstream's own doctests.
    #[test]
    fn euro_doctests_match_upstream() {
        assert_eq!(extract("35€ 99").as_deref(), Some("35€99"));
        assert_eq!(extract("1,235€ 99").as_deref(), Some("1,235€99"));
        // Three digits do not fit, so the ordinary rule takes over.
        assert_eq!(extract("35€ 999").as_deref(), Some("35"));
        // Two euro signs: the branch is skipped entirely.
        assert_eq!(extract("99 €, 79 €").as_deref(), Some("99"));
        assert_eq!(extract("99 € 79 €").as_deref(), Some("99"));
    }

    /// The exact digit-count behaviour, taken from running upstream's pattern.
    ///
    /// With whitespace after the euro the group participates and **exactly
    /// two** digits must follow. Without it, backtracking skips the group and
    /// the run becomes unbounded.
    #[test]
    fn euro_digit_counts_match_upstream() {
        let cases: &[(&str, Option<&str>)] = &[
            // No whitespace: any number of digits is accepted.
            ("12€3", Some("12€3")),
            ("12€34", Some("12€34")),
            ("12€345", Some("12€345")),
            ("12€3456", Some("12€3456")),
            ("12€34567", Some("12€34567")),
            // Whitespace after the euro: exactly two digits, or no match.
            ("12€ 34", Some("12€34")),
            ("12€  34", Some("12€34")),
            // One digit or three-plus after whitespace falls through to the
            // ordinary rule, which returns just the leading number.
            ("12€ 3", Some("12")),
            ("12€ 345", Some("12")),
            ("12€ 3456", Some("12")),
            // Whitespace before the euro is allowed on either arm.
            ("12 €34", Some("12€34")),
            ("12 € 34", Some("12€34")),
            ("12 €345", Some("12€345")),
            ("12 € 345", Some("12")),
        ];
        for (input, expected) in cases {
            assert_eq!(extract(input).as_deref(), *expected, "for {input:?}");
        }
    }

    #[test]
    fn euro_keeps_separators_in_the_leading_number() {
        assert_eq!(extract("1,235€99").as_deref(), Some("1,235€99"));
        assert_eq!(extract("1.235€99").as_deref(), Some("1.235€99"));
        // An internal space is removed along with the whitespace around the euro.
        assert_eq!(extract("1 235€99").as_deref(), Some("1235€99"));
        assert_eq!(extract("1'235€99").as_deref(), Some("1'235€99"));
    }

    #[test]
    fn euro_with_nothing_usable_falls_through() {
        // No digits after the euro at all.
        assert_eq!(extract("12€").as_deref(), Some("12"));
        assert_eq!(extract("12€ ").as_deref(), Some("12"));
        // No digits before it either, so the ordinary rule finds the trailing run.
        assert_eq!(extract("€34").as_deref(), Some("34"));
        assert_eq!(extract(" €34").as_deref(), Some("34"));
    }

    #[test]
    fn euro_tolerates_surrounding_text() {
        // The match may begin on whitespace, which is then stripped out.
        assert_eq!(extract("price: 12€34").as_deref(), Some("12€34"));
        assert_eq!(extract("abc12€34").as_deref(), Some("12€34"));
        // A trailing non-digit ends the match and is carried along, matching
        // upstream -- group(0) includes it.
        assert_eq!(extract("12€34x").as_deref(), Some("12€34x"));
        assert_eq!(extract("12€345x").as_deref(), Some("12€345x"));
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
