//! Token-level comparison infrastructure.
//!
//! This module provides primitives for comparing token sequences across documents.
//! The core design principle is that `TokenAlignment` is a **queryable data structure**,
//! not just rendering output. This enables:
//!
//! - Version comparison (contract diff viewer)
//! - Cross-document comparison (future)
//! - Pattern extraction (future)
//! - Corpus analysis (future)
//!
//! # Example
//!
//! ```ignore
//! use layered_contracts::token_diff::{TokenAligner, TokenAlignmentConfig};
//! use layered_nlp::LLLine;
//!
//! let original_tokens = TokenAligner::extract_tokens(&original_line);
//! let revised_tokens = TokenAligner::extract_tokens(&revised_line);
//!
//! let alignment = TokenAligner::align(&original_tokens, &revised_tokens, &TokenAlignmentConfig::default());
//!
//! // Query the alignment
//! println!("Similarity: {:.2}", alignment.similarity());
//! for change in alignment.changes() {
//!     println!("{:?}", change);
//! }
//! ```

use layered_nlp::{LLLine, LToken, TextTag};
use serde::{Deserialize, Serialize};

/// Configuration for token alignment.
#[derive(Debug, Clone)]
pub struct TokenAlignmentConfig {
    /// How to handle whitespace tokens in alignment.
    pub whitespace_mode: WhitespaceMode,
    /// Minimum similarity threshold for fuzzy matching (0.0-1.0).
    /// None means exact matching only.
    pub fuzzy_threshold: Option<f64>,
}

impl Default for TokenAlignmentConfig {
    fn default() -> Self {
        Self {
            whitespace_mode: WhitespaceMode::Normalize,
            fuzzy_threshold: None,
        }
    }
}

/// How whitespace tokens are handled during alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceMode {
    /// Include whitespace tokens in alignment (shows all changes).
    Preserve,
    /// Collapse consecutive whitespace, normalize line endings.
    /// This is the default - matches the behavior of treating `\s+` as single space.
    Normalize,
    /// Exclude whitespace from alignment (focus on content only).
    Ignore,
}

/// A reference to a token with its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRef {
    /// The token text content.
    pub text: String,
    /// Byte position where token starts (inclusive).
    pub start: usize,
    /// Byte position where token ends (exclusive).
    pub end: usize,
    /// The token's classification tag.
    pub tag: TokenTag,
    /// Index in the source token sequence.
    pub index: usize,
}

/// Serializable version of TextTag for WASM export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TokenTag {
    /// Natural number
    Natn,
    /// Punctuation
    Punc,
    /// Symbol
    Symb,
    /// Whitespace
    Space,
    /// Word
    Word,
}

impl From<&TextTag> for TokenTag {
    fn from(tag: &TextTag) -> Self {
        match tag {
            TextTag::NATN => TokenTag::Natn,
            TextTag::PUNC => TokenTag::Punc,
            TextTag::SYMB => TokenTag::Symb,
            TextTag::SPACE => TokenTag::Space,
            TextTag::WORD => TokenTag::Word,
        }
    }
}

/// Queryable token-level alignment between two token sequences.
///
/// This is a data structure for analysis, not just rendering.
/// Use the query methods (`added()`, `removed()`, `changes()`) to analyze the alignment.
#[derive(Debug, Clone)]
pub struct TokenAlignment {
    /// The aligned token pairs with their relationships.
    pub pairs: Vec<AlignedTokenPair>,
    /// Summary statistics for the alignment.
    pub stats: AlignmentStats,
}

impl TokenAlignment {
    /// Iterate over tokens that were added (exist only in revised).
    pub fn added(&self) -> impl Iterator<Item = &AlignedTokenPair> {
        self.pairs
            .iter()
            .filter(|p| matches!(p.relation, TokenRelation::RightOnly))
    }

    /// Iterate over tokens that were removed (exist only in original).
    pub fn removed(&self) -> impl Iterator<Item = &AlignedTokenPair> {
        self.pairs
            .iter()
            .filter(|p| matches!(p.relation, TokenRelation::LeftOnly))
    }

    /// Iterate over all changes (non-identical pairs).
    pub fn changes(&self) -> impl Iterator<Item = &AlignedTokenPair> {
        self.pairs
            .iter()
            .filter(|p| !matches!(p.relation, TokenRelation::Identical))
    }

    /// Iterate over unchanged tokens (identical pairs).
    pub fn unchanged(&self) -> impl Iterator<Item = &AlignedTokenPair> {
        self.pairs
            .iter()
            .filter(|p| matches!(p.relation, TokenRelation::Identical))
    }

    /// Iterate over pairs matching a custom predicate.
    pub fn filter<F>(&self, predicate: F) -> impl Iterator<Item = &AlignedTokenPair>
    where
        F: Fn(&AlignedTokenPair) -> bool,
    {
        self.pairs.iter().filter(move |p| predicate(p))
    }

    /// Compute similarity score (0.0 = completely different, 1.0 = identical).
    ///
    /// Similarity is based on the proportion of identical tokens.
    pub fn similarity(&self) -> f64 {
        let total = self.stats.total_left.max(self.stats.total_right);
        if total == 0 {
            return 1.0; // Two empty sequences are identical
        }
        self.stats.identical as f64 / total as f64
    }
}

/// A single aligned pair of tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignedTokenPair {
    /// Token from the left (original) sequence. None if added.
    pub left: Option<TokenRef>,
    /// Token from the right (revised) sequence. None if removed.
    pub right: Option<TokenRef>,
    /// The relationship between left and right.
    pub relation: TokenRelation,
}

/// How two tokens relate to each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenRelation {
    /// Identical token text.
    Identical,
    /// Token only exists on the left (was removed).
    LeftOnly,
    /// Token only exists on the right (was added).
    RightOnly,
    /// Tokens differ only in whitespace (when using Normalize mode).
    WhitespaceEquivalent,
}

/// Summary statistics for a token alignment.
#[derive(Debug, Clone, Default)]
pub struct AlignmentStats {
    /// Total tokens in the left (original) sequence.
    pub total_left: usize,
    /// Total tokens in the right (revised) sequence.
    pub total_right: usize,
    /// Number of identical token pairs.
    pub identical: usize,
    /// Number of tokens added (right only).
    pub added: usize,
    /// Number of tokens removed (left only).
    pub removed: usize,
}

/// Token aligner providing token extraction and alignment functionality.
pub struct TokenAligner;

impl TokenAligner {
    /// Extract tokens from an LLLine into a sequence suitable for diffing.
    ///
    /// This converts the internal `LLToken` representation to `TokenRef` with
    /// all necessary metadata for alignment and position mapping.
    pub fn extract_tokens(line: &LLLine) -> Vec<TokenRef> {
        line.ll_tokens()
            .iter()
            .enumerate()
            .filter_map(|(idx, ll_token)| {
                match ll_token.get_token() {
                    LToken::Text(text, tag) => Some(TokenRef {
                        text: text.clone(),
                        start: ll_token.pos_starts_at(),
                        end: ll_token.pos_ends_at(),
                        tag: TokenTag::from(tag),
                        index: idx,
                    }),
                    LToken::Value => None, // Skip placeholder tokens
                }
            })
            .collect()
    }

    /// Extract tokens from raw text by creating a temporary LLLine.
    ///
    /// This is a convenience method for testing and simple use cases.
    pub fn extract_tokens_from_text(text: &str) -> Vec<TokenRef> {
        let line = layered_nlp::create_line_from_string(text);
        Self::extract_tokens(&line)
    }

    /// Align two token sequences and return a queryable alignment.
    ///
    /// Uses the Myers diff algorithm to compute minimal edit distance alignment.
    pub fn align(
        left: &[TokenRef],
        right: &[TokenRef],
        config: &TokenAlignmentConfig,
    ) -> TokenAlignment {
        // Preprocess tokens based on whitespace mode
        let (left_processed, right_processed) = match config.whitespace_mode {
            WhitespaceMode::Preserve => (left.to_vec(), right.to_vec()),
            WhitespaceMode::Normalize => (
                normalize_whitespace_tokens(left),
                normalize_whitespace_tokens(right),
            ),
            WhitespaceMode::Ignore => (filter_whitespace(left), filter_whitespace(right)),
        };

        // Run Myers diff on processed tokens
        let diff_ops = myers_diff(&left_processed, &right_processed);

        // Convert diff operations to aligned pairs
        let mut pairs = Vec::new();
        let mut stats = AlignmentStats {
            total_left: left_processed.len(),
            total_right: right_processed.len(),
            ..Default::default()
        };

        for op in diff_ops {
            match op {
                DiffOp::Equal(left_idx, right_idx) => {
                    pairs.push(AlignedTokenPair {
                        left: Some(left_processed[left_idx].clone()),
                        right: Some(right_processed[right_idx].clone()),
                        relation: TokenRelation::Identical,
                    });
                    stats.identical += 1;
                }
                DiffOp::Insert(right_idx) => {
                    pairs.push(AlignedTokenPair {
                        left: None,
                        right: Some(right_processed[right_idx].clone()),
                        relation: TokenRelation::RightOnly,
                    });
                    stats.added += 1;
                }
                DiffOp::Delete(left_idx) => {
                    pairs.push(AlignedTokenPair {
                        left: Some(left_processed[left_idx].clone()),
                        right: None,
                        relation: TokenRelation::LeftOnly,
                    });
                    stats.removed += 1;
                }
            }
        }

        TokenAlignment { pairs, stats }
    }
}

/// Normalize whitespace tokens by collapsing consecutive whitespace.
fn normalize_whitespace_tokens(tokens: &[TokenRef]) -> Vec<TokenRef> {
    let mut result = Vec::with_capacity(tokens.len());
    let mut prev_was_space = false;

    for token in tokens {
        if token.tag == TokenTag::Space {
            if !prev_was_space {
                // Normalize to single space
                result.push(TokenRef {
                    text: " ".to_string(),
                    start: token.start,
                    end: token.end,
                    tag: TokenTag::Space,
                    index: token.index,
                });
                prev_was_space = true;
            }
            // Skip consecutive whitespace
        } else {
            result.push(token.clone());
            prev_was_space = false;
        }
    }

    result
}

/// Filter out whitespace tokens entirely.
fn filter_whitespace(tokens: &[TokenRef]) -> Vec<TokenRef> {
    tokens
        .iter()
        .filter(|t| t.tag != TokenTag::Space)
        .cloned()
        .collect()
}

// =============================================================================
// Diff Algorithm Implementation (LCS-based)
// =============================================================================

/// Diff operation produced by the diff algorithm.
#[derive(Debug, Clone)]
enum DiffOp {
    /// Tokens at (left_idx, right_idx) are equal.
    Equal(usize, usize),
    /// Token at right_idx was inserted.
    Insert(usize),
    /// Token at left_idx was deleted.
    Delete(usize),
}

/// Compute diff using LCS (Longest Common Subsequence) algorithm.
///
/// This is a simpler and more robust approach than Myers for our use case.
/// Time complexity: O(N*M) where N, M are sequence lengths.
fn myers_diff(left: &[TokenRef], right: &[TokenRef]) -> Vec<DiffOp> {
    let n = left.len();
    let m = right.len();

    if n == 0 && m == 0 {
        return vec![];
    }
    if n == 0 {
        return (0..m).map(DiffOp::Insert).collect();
    }
    if m == 0 {
        return (0..n).map(DiffOp::Delete).collect();
    }

    // Build LCS table
    // dp[i][j] = length of LCS of left[0..i] and right[0..j]
    let mut dp = vec![vec![0usize; m + 1]; n + 1];

    for i in 1..=n {
        for j in 1..=m {
            if left[i - 1].text == right[j - 1].text {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to build the diff
    let mut ops = Vec::new();
    let mut i = n;
    let mut j = m;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && left[i - 1].text == right[j - 1].text {
            // Equal - both match
            ops.push(DiffOp::Equal(i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            // Insert from right
            ops.push(DiffOp::Insert(j - 1));
            j -= 1;
        } else {
            // Delete from left
            ops.push(DiffOp::Delete(i - 1));
            i -= 1;
        }
    }

    ops.reverse();
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_texts() {
        let left = TokenAligner::extract_tokens_from_text("hello world");
        let right = TokenAligner::extract_tokens_from_text("hello world");
        let alignment = TokenAligner::align(&left, &right, &TokenAlignmentConfig::default());

        assert_eq!(alignment.similarity(), 1.0);
        assert_eq!(alignment.stats.identical, alignment.stats.total_left);
        assert_eq!(alignment.stats.added, 0);
        assert_eq!(alignment.stats.removed, 0);
        assert_eq!(alignment.changes().count(), 0);
    }

    #[test]
    fn test_single_word_addition() {
        let left = TokenAligner::extract_tokens_from_text("hello world");
        let right = TokenAligner::extract_tokens_from_text("hello big world");
        let alignment = TokenAligner::align(&left, &right, &TokenAlignmentConfig::default());

        let added: Vec<_> = alignment.added().collect();
        // "big" and a space are added
        assert!(added.iter().any(|p| p.right.as_ref().unwrap().text == "big"));
        assert!(alignment.stats.added > 0);
    }

    #[test]
    fn test_single_word_removal() {
        let left = TokenAligner::extract_tokens_from_text("hello big world");
        let right = TokenAligner::extract_tokens_from_text("hello world");
        let alignment = TokenAligner::align(&left, &right, &TokenAlignmentConfig::default());

        let removed: Vec<_> = alignment.removed().collect();
        assert!(removed
            .iter()
            .any(|p| p.left.as_ref().unwrap().text == "big"));
        assert!(alignment.stats.removed > 0);
    }

    #[test]
    fn test_word_replacement() {
        let left = TokenAligner::extract_tokens_from_text("The Company shall deliver");
        let right = TokenAligner::extract_tokens_from_text("The Company may deliver");
        let alignment = TokenAligner::align(&left, &right, &TokenAlignmentConfig::default());

        let changes: Vec<_> = alignment.changes().collect();
        // "shall" removed, "may" added
        assert!(changes
            .iter()
            .any(|p| p.left.as_ref().map(|t| t.text.as_str()) == Some("shall")));
        assert!(changes
            .iter()
            .any(|p| p.right.as_ref().map(|t| t.text.as_str()) == Some("may")));
    }

    #[test]
    fn test_whitespace_normalization() {
        let left = TokenAligner::extract_tokens_from_text("hello   world");
        let right = TokenAligner::extract_tokens_from_text("hello world");

        // With normalization (default), these should be identical
        let alignment = TokenAligner::align(&left, &right, &TokenAlignmentConfig::default());
        assert_eq!(alignment.similarity(), 1.0);

        // With Preserve mode, they should differ
        let config = TokenAlignmentConfig {
            whitespace_mode: WhitespaceMode::Preserve,
            ..Default::default()
        };
        let alignment = TokenAligner::align(&left, &right, &config);
        assert!(alignment.similarity() < 1.0);
    }

    #[test]
    fn test_whitespace_ignore() {
        // Test that whitespace differences don't affect matching when ignored
        let left = TokenAligner::extract_tokens_from_text("hello  world  foo");
        let right = TokenAligner::extract_tokens_from_text("hello world foo");
        let config = TokenAlignmentConfig {
            whitespace_mode: WhitespaceMode::Ignore,
            ..Default::default()
        };
        let alignment = TokenAligner::align(&left, &right, &config);

        // When ignoring whitespace, words should match perfectly
        assert_eq!(alignment.similarity(), 1.0);
        assert_eq!(alignment.stats.identical, 3); // "hello", "world", "foo"
    }

    #[test]
    fn test_empty_sequences() {
        let left: Vec<TokenRef> = vec![];
        let right: Vec<TokenRef> = vec![];
        let alignment = TokenAligner::align(&left, &right, &TokenAlignmentConfig::default());

        assert_eq!(alignment.similarity(), 1.0);
        assert_eq!(alignment.pairs.len(), 0);
    }

    #[test]
    fn test_one_empty_sequence() {
        let left = TokenAligner::extract_tokens_from_text("hello");
        let right: Vec<TokenRef> = vec![];
        let alignment = TokenAligner::align(&left, &right, &TokenAlignmentConfig::default());

        assert_eq!(alignment.similarity(), 0.0);
        assert_eq!(alignment.removed().count(), left.len());
    }

    #[test]
    fn test_contract_clause_diff() {
        let original = "The Company shall deliver the goods within thirty (30) days.";
        let revised = "The Company may deliver the goods within sixty (60) days.";

        let left = TokenAligner::extract_tokens_from_text(original);
        let right = TokenAligner::extract_tokens_from_text(revised);
        let alignment = TokenAligner::align(&left, &right, &TokenAlignmentConfig::default());

        // Check that key changes are detected
        let changes: Vec<_> = alignment.changes().collect();

        // "shall" -> "may"
        let shall_removed = changes
            .iter()
            .any(|p| p.left.as_ref().map(|t| t.text.as_str()) == Some("shall"));
        let may_added = changes
            .iter()
            .any(|p| p.right.as_ref().map(|t| t.text.as_str()) == Some("may"));

        // "thirty" -> "sixty", "30" -> "60"
        let thirty_removed = changes
            .iter()
            .any(|p| p.left.as_ref().map(|t| t.text.as_str()) == Some("thirty"));
        let sixty_added = changes
            .iter()
            .any(|p| p.right.as_ref().map(|t| t.text.as_str()) == Some("sixty"));

        assert!(shall_removed, "should detect 'shall' removal");
        assert!(may_added, "should detect 'may' addition");
        assert!(thirty_removed, "should detect 'thirty' removal");
        assert!(sixty_added, "should detect 'sixty' addition");
    }

    #[test]
    fn test_position_preservation() {
        let text = "hello world";
        let tokens = TokenAligner::extract_tokens_from_text(text);

        // Check that positions map back to original text correctly
        for token in &tokens {
            assert_eq!(&text[token.start..token.end], token.text);
        }
    }

    #[test]
    fn test_similarity_scores() {
        // Identical = 1.0
        let left = TokenAligner::extract_tokens_from_text("hello");
        let right = TokenAligner::extract_tokens_from_text("hello");
        let alignment = TokenAligner::align(&left, &right, &TokenAlignmentConfig::default());
        assert!((alignment.similarity() - 1.0).abs() < f64::EPSILON);

        // Completely different = 0.0
        let left = TokenAligner::extract_tokens_from_text("hello");
        let right = TokenAligner::extract_tokens_from_text("world");
        let alignment = TokenAligner::align(&left, &right, &TokenAlignmentConfig::default());
        assert!(alignment.similarity() < 0.5);

        // Partial similarity
        let left = TokenAligner::extract_tokens_from_text("hello world foo");
        let right = TokenAligner::extract_tokens_from_text("hello world bar");
        let alignment = TokenAligner::align(&left, &right, &TokenAlignmentConfig::default());
        assert!(alignment.similarity() > 0.5);
        assert!(alignment.similarity() < 1.0);
    }
}
