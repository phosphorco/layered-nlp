//! N-best ambiguity support for handling multiple competing interpretations.
//!
//! When resolvers produce multiple candidates at the same position,
//! this module provides tools to aggregate them while preserving
//! information about ambiguity and confidence.

use crate::Scored;

/// Flag indicating the nature of ambiguity in an aggregated result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AmbiguityFlag {
    /// No ambiguity concerns - clear winner with high confidence
    None,
    /// Best score is below the low_confidence threshold
    LowConfidence,
    /// Multiple alternatives have scores close to the best
    CompetingAlternatives,
}

/// Configuration for ambiguity handling during aggregation.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AmbiguityConfig {
    /// Maximum candidates to keep (including best)
    pub n_best: usize,
    /// Absolute score floor - candidates below this are discarded
    pub min_score: f64,
    /// Threshold below which best candidate is flagged as low confidence
    pub low_confidence: f64,
    /// Margin within which alternatives are considered competing
    /// (alt >= best - margin triggers CompetingAlternatives)
    pub ambiguity_margin: f64,
}

impl Default for AmbiguityConfig {
    fn default() -> Self {
        Self {
            n_best: 4,
            min_score: 0.25,
            low_confidence: 0.6,
            ambiguity_margin: 0.1,
        }
    }
}

/// Aggregated result with N-best alternatives and ambiguity flag.
///
/// Wraps the best candidate along with alternatives, and provides
/// a computed flag indicating whether the result should be treated
/// as uncertain.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ambiguous<T> {
    /// Top-scoring candidate
    pub best: Scored<T>,
    /// Alternatives sorted by descending score (excludes best)
    pub alternatives: Vec<Scored<T>>,
    /// Computed ambiguity flag based on config thresholds
    pub flag: AmbiguityFlag,
}

impl<T> Ambiguous<T> {
    /// Create from a list of candidates using the given configuration.
    ///
    /// Returns `None` if no candidates survive the min_score filter.
    ///
    /// The algorithm:
    /// 1. Filter candidates below min_score
    /// 2. Sort by confidence descending
    /// 3. Truncate to n_best
    /// 4. Compute ambiguity flag based on thresholds
    pub fn from_candidates(
        mut candidates: Vec<Scored<T>>,
        cfg: &AmbiguityConfig,
    ) -> Option<Self> {
        // Prune below min_score
        candidates.retain(|c| c.confidence >= cfg.min_score);
        if candidates.is_empty() {
            return None;
        }

        // Sort by score descending
        candidates.sort_by(|a, b| {
            b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to n_best
        candidates.truncate(cfg.n_best);

        // Split best from alternatives
        let mut iter = candidates.into_iter();
        let best = iter.next().unwrap();
        let alternatives: Vec<_> = iter.collect();

        // Compute flag
        let flag = Self::compute_flag(&best, &alternatives, cfg);

        Some(Self { best, alternatives, flag })
    }

    /// Compute the ambiguity flag based on the best candidate, alternatives, and config.
    fn compute_flag(best: &Scored<T>, alts: &[Scored<T>], cfg: &AmbiguityConfig) -> AmbiguityFlag {
        // Low confidence if best is below threshold
        if best.confidence < cfg.low_confidence {
            return AmbiguityFlag::LowConfidence;
        }

        // Check for competing alternatives within margin
        let has_competing = alts.iter().any(|alt| {
            alt.confidence >= best.confidence - cfg.ambiguity_margin
        });

        if has_competing {
            AmbiguityFlag::CompetingAlternatives
        } else {
            AmbiguityFlag::None
        }
    }

    /// Get the best value, discarding confidence information.
    pub fn into_best(self) -> T {
        self.best.value
    }

    /// Check if this result has any alternatives.
    pub fn has_alternatives(&self) -> bool {
        !self.alternatives.is_empty()
    }

    /// Get the number of total candidates (best + alternatives).
    pub fn candidate_count(&self) -> usize {
        1 + self.alternatives.len()
    }

    /// Check if this result is flagged as ambiguous in any way.
    pub fn is_ambiguous(&self) -> bool {
        self.flag != AmbiguityFlag::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candidate(value: &str, confidence: f64) -> Scored<String> {
        Scored::rule_based(value.to_string(), confidence, "test")
    }

    #[test]
    fn test_empty_candidates() {
        let candidates: Vec<Scored<String>> = vec![];
        let result = Ambiguous::from_candidates(candidates, &AmbiguityConfig::default());
        assert!(result.is_none());
    }

    #[test]
    fn test_all_below_min_score() {
        let candidates = vec![
            make_candidate("a", 0.1),
            make_candidate("b", 0.2),
        ];
        let result = Ambiguous::from_candidates(candidates, &AmbiguityConfig::default());
        assert!(result.is_none()); // default min_score is 0.25
    }

    #[test]
    fn test_single_high_confidence() {
        let candidates = vec![
            make_candidate("a", 0.9),
        ];
        let result = Ambiguous::from_candidates(candidates, &AmbiguityConfig::default()).unwrap();

        assert_eq!(result.best.value, "a");
        assert_eq!(result.best.confidence, 0.9);
        assert!(result.alternatives.is_empty());
        assert_eq!(result.flag, AmbiguityFlag::None);
    }

    #[test]
    fn test_best_below_low_confidence() {
        let candidates = vec![
            make_candidate("a", 0.5), // below default 0.6 threshold
        ];
        let result = Ambiguous::from_candidates(candidates, &AmbiguityConfig::default()).unwrap();

        assert_eq!(result.flag, AmbiguityFlag::LowConfidence);
    }

    #[test]
    fn test_clear_winner_no_competition() {
        let candidates = vec![
            make_candidate("a", 0.9),
            make_candidate("b", 0.7), // 0.2 below best, outside margin
        ];
        let result = Ambiguous::from_candidates(candidates, &AmbiguityConfig::default()).unwrap();

        assert_eq!(result.best.value, "a");
        assert_eq!(result.alternatives.len(), 1);
        assert_eq!(result.flag, AmbiguityFlag::None); // 0.7 < 0.9 - 0.1 = 0.8
    }

    #[test]
    fn test_competing_alternatives() {
        let candidates = vec![
            make_candidate("a", 0.9),
            make_candidate("b", 0.85), // within 0.1 margin
        ];
        let result = Ambiguous::from_candidates(candidates, &AmbiguityConfig::default()).unwrap();

        assert_eq!(result.best.value, "a");
        assert_eq!(result.alternatives.len(), 1);
        assert_eq!(result.flag, AmbiguityFlag::CompetingAlternatives);
    }

    #[test]
    fn test_n_best_truncation() {
        let candidates = vec![
            make_candidate("a", 0.9),
            make_candidate("b", 0.8),
            make_candidate("c", 0.7),
            make_candidate("d", 0.65),
            make_candidate("e", 0.6),
            make_candidate("f", 0.5),
        ];
        let result = Ambiguous::from_candidates(candidates, &AmbiguityConfig::default()).unwrap();

        // default n_best is 4, so we keep a, b, c, d
        assert_eq!(result.candidate_count(), 4);
        assert_eq!(result.best.value, "a");
        assert_eq!(result.alternatives.len(), 3);
        assert_eq!(result.alternatives[0].value, "b");
        assert_eq!(result.alternatives[1].value, "c");
        assert_eq!(result.alternatives[2].value, "d");
    }

    #[test]
    fn test_score_ordering_maintained() {
        let candidates = vec![
            make_candidate("c", 0.7),
            make_candidate("a", 0.9),
            make_candidate("b", 0.8),
        ];
        let result = Ambiguous::from_candidates(candidates, &AmbiguityConfig::default()).unwrap();

        // Should be sorted: a(0.9), b(0.8), c(0.7)
        assert_eq!(result.best.value, "a");
        assert_eq!(result.alternatives[0].value, "b");
        assert_eq!(result.alternatives[1].value, "c");
    }

    #[test]
    fn test_helper_methods() {
        let candidates = vec![
            make_candidate("a", 0.9),
            make_candidate("b", 0.75),
        ];
        let result = Ambiguous::from_candidates(candidates, &AmbiguityConfig::default()).unwrap();

        assert!(result.has_alternatives());
        assert_eq!(result.candidate_count(), 2);
        assert!(!result.is_ambiguous()); // high confidence, no competing (0.75 < 0.9 - 0.1 = 0.8)
        assert_eq!(result.into_best(), "a");
    }
}
