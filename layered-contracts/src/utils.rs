//! Shared helpers used across accountability layers.

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
