//! Shared helpers used across contract analysis layers.

/// Normalize party/beneficiary names by trimming punctuation, lowercasing, and
/// removing leading articles so we can compare display texts consistently.
pub(crate) fn normalize_party_name(name: &str) -> String {
    let trimmed = name
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '“' | '”'))
        .trim()
        .to_lowercase();

    for article in ["the ", "a ", "an "] {
        if trimmed.starts_with(article) {
            return trimmed[article.len()..].to_string();
        }
    }

    trimmed
}

/// Parse a Roman numeral from text.
///
/// Returns `(numeric_value, is_uppercase)` or `None` if not a valid Roman numeral.
/// Supports numerals up to 3999 (MMMCMXCIX).
///
/// # Examples
/// ```ignore
/// assert_eq!(parse_roman("IV"), Some((4, true)));
/// assert_eq!(parse_roman("xii"), Some((12, false)));
/// assert_eq!(parse_roman("MCMXCIV"), Some((1994, true)));
/// ```
pub(crate) fn parse_roman(text: &str) -> Option<(u32, bool)> {
    if text.is_empty() {
        return None;
    }

    let uppercase = text.chars().next()?.is_uppercase();
    let upper = text.to_uppercase();

    // Validate all characters are valid Roman numeral characters
    if !upper
        .chars()
        .all(|c| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'))
    {
        return None;
    }

    fn roman_value(c: char) -> u32 {
        match c {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            _ => 0,
        }
    }

    // Parse using subtractive principle (right to left)
    let chars: Vec<char> = upper.chars().collect();
    let mut total: u32 = 0;
    let mut prev_value: u32 = 0;

    for &c in chars.iter().rev() {
        let value = roman_value(c);
        if value < prev_value {
            total = total.checked_sub(value)?;
        } else {
            total = total.checked_add(value)?;
        }
        prev_value = value;
    }

    if total == 0 || total > 3999 {
        return None;
    }

    Some((total, uppercase))
}
