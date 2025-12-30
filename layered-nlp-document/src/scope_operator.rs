//! Scope-bearing operator types for representing scope relationships.
//!
//! ScopeOperator represents operators that have a trigger span and a domain
//! they scope over, like negation, quantifiers, precedence clauses, and deictic expressions.

use crate::{DocSpan, Scored};

/// Dimension of a scope-bearing operator.
///
/// Used to categorize operators for targeted queries and downstream processing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ScopeDimension {
    /// Negation operators ("not", "never", "neither")
    Negation,
    /// Quantifier operators ("each", "all", "any", "no")
    Quantifier,
    /// Precedence operators ("notwithstanding", "subject to")
    Precedence,
    /// Deictic operators (speaker/time anchors)
    Deictic,
    /// Other scope operators
    Other(String),
}

/// Domain with N-best ambiguity support.
///
/// Scope operators often have ambiguous domains (where does the scope end?).
/// This type preserves multiple candidate interpretations ranked by score.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScopeDomain {
    /// Candidates sorted by descending score (best first)
    pub candidates: Vec<Scored<DocSpan>>,
}

impl ScopeDomain {
    /// Create unambiguous domain from single span with full confidence.
    pub fn from_single(span: DocSpan) -> Self {
        Self {
            candidates: vec![Scored::verified(span)],
        }
    }

    /// Create domain from multiple scored candidates.
    /// Candidates should be sorted by descending score.
    pub fn from_candidates(candidates: Vec<Scored<DocSpan>>) -> Self {
        Self { candidates }
    }

    /// Best-scoring domain span.
    ///
    /// Returns `None` if no candidates exist. While well-constructed ScopeDomains
    /// should always have at least one candidate, returning Option allows callers
    /// to handle edge cases gracefully rather than panicking.
    pub fn primary(&self) -> Option<&DocSpan> {
        self.candidates.first().map(|s| &s.value)
    }

    /// Is this domain ambiguous (more than one candidate)?
    pub fn is_ambiguous(&self) -> bool {
        self.candidates.len() > 1
    }

    /// Number of candidate interpretations.
    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }
}

/// A scope-bearing operator with trigger and domain.
///
/// Represents operators like negation, quantifiers, and precedence clauses
/// that have a trigger position and scope over a domain.
///
/// # Type Parameters
/// - `O`: Operator-specific payload type
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScopeOperator<O> {
    /// High-level dimension (Negation, Quantifier, etc.)
    pub dimension: ScopeDimension,
    /// The trigger span ("not", "each", "notwithstanding")
    pub trigger: DocSpan,
    /// Domain this operator scopes over
    pub domain: ScopeDomain,
    /// Operator-specific payload
    pub payload: O,
}

impl<O> ScopeOperator<O> {
    pub fn new(
        dimension: ScopeDimension,
        trigger: DocSpan,
        domain: ScopeDomain,
        payload: O,
    ) -> Self {
        Self { dimension, trigger, domain, payload }
    }
}

// ============================================================================
// Operator Payload Types - Templates for downstream milestones
// ============================================================================

/// M7: Negation operator payload.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NegationOp {
    /// The negation marker ("not", "never", "neither", etc.)
    pub marker: String,
    /// Kind of negation
    pub kind: NegationKind,
}

/// Types of negation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NegationKind {
    /// Simple negation ("not", "no")
    Simple,
    /// Temporal negation ("never")
    Temporal,
    /// Correlative negation ("neither...nor")
    Correlative,
}

/// M7: Quantifier operator payload.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QuantifierOp {
    /// The quantifier marker ("each", "all", "any", "no")
    pub marker: String,
    /// Kind of quantifier
    pub kind: QuantifierKind,
}

/// Types of quantifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum QuantifierKind {
    /// Universal quantifier ("all", "every", "each")
    Universal,
    /// Existential quantifier ("some", "any")
    Existential,
    /// Negative quantifier ("no", "none")
    Negative,
}

/// M4: Precedence operator payload.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PrecedenceOp {
    /// The precedence connective ("notwithstanding", "subject to")
    pub connective: String,
    /// Whether this clause overrides the domain
    pub overrides_domain: bool,
    /// Referenced sections (if any)
    pub referenced_sections: Vec<String>,
}

/// M5: Deictic frame payload.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct DeicticFrame {
    /// Speaker anchor (if identified)
    pub speaker: Option<String>,
    /// Time anchor (if identified)
    pub time_anchor: Option<String>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DocPosition;

    fn sample_trigger() -> DocSpan {
        DocSpan::new(
            DocPosition { line: 0, token: 0 },
            DocPosition { line: 0, token: 1 },
        )
    }

    fn sample_domain_span() -> DocSpan {
        DocSpan::new(
            DocPosition { line: 0, token: 2 },
            DocPosition { line: 0, token: 10 },
        )
    }

    #[test]
    fn test_scope_domain_single() {
        let domain = ScopeDomain::from_single(sample_domain_span());
        assert!(!domain.is_ambiguous());
        assert_eq!(domain.candidate_count(), 1);
        assert_eq!(domain.primary(), Some(&sample_domain_span()));
    }

    #[test]
    fn test_scope_domain_multiple_candidates() {
        let span1 = sample_domain_span();
        let span2 = DocSpan::new(
            DocPosition { line: 0, token: 2 },
            DocPosition { line: 0, token: 15 },
        );

        let domain = ScopeDomain::from_candidates(vec![
            Scored::rule_based(span1.clone(), 0.9, "heuristic_a"),
            Scored::rule_based(span2.clone(), 0.7, "heuristic_b"),
        ]);

        assert!(domain.is_ambiguous());
        assert_eq!(domain.candidate_count(), 2);
        assert_eq!(domain.primary(), Some(&span1));
    }

    #[test]
    fn test_scope_operator_construction() {
        let op: ScopeOperator<NegationOp> = ScopeOperator::new(
            ScopeDimension::Negation,
            sample_trigger(),
            ScopeDomain::from_single(sample_domain_span()),
            NegationOp {
                marker: "not".to_string(),
                kind: NegationKind::Simple,
            },
        );

        assert_eq!(op.dimension, ScopeDimension::Negation);
        assert_eq!(op.trigger, sample_trigger());
        assert_eq!(op.payload.marker, "not");
    }

    #[test]
    fn test_scope_operator_with_different_payloads() {
        // Negation
        let _neg: ScopeOperator<NegationOp> = ScopeOperator::new(
            ScopeDimension::Negation,
            sample_trigger(),
            ScopeDomain::from_single(sample_domain_span()),
            NegationOp { marker: "never".to_string(), kind: NegationKind::Temporal },
        );

        // Quantifier
        let _quant: ScopeOperator<QuantifierOp> = ScopeOperator::new(
            ScopeDimension::Quantifier,
            sample_trigger(),
            ScopeDomain::from_single(sample_domain_span()),
            QuantifierOp { marker: "each".to_string(), kind: QuantifierKind::Universal },
        );

        // Precedence
        let _prec: ScopeOperator<PrecedenceOp> = ScopeOperator::new(
            ScopeDimension::Precedence,
            sample_trigger(),
            ScopeDomain::from_single(sample_domain_span()),
            PrecedenceOp {
                connective: "notwithstanding".to_string(),
                overrides_domain: true,
                referenced_sections: vec!["Section 3.1".to_string()],
            },
        );

        // Deictic
        let _deictic: ScopeOperator<DeicticFrame> = ScopeOperator::new(
            ScopeDimension::Deictic,
            sample_trigger(),
            ScopeDomain::from_single(sample_domain_span()),
            DeicticFrame {
                speaker: Some("Company".to_string()),
                time_anchor: Some("execution date".to_string()),
            },
        );
    }

    #[test]
    fn test_scope_dimension_equality() {
        assert_eq!(ScopeDimension::Negation, ScopeDimension::Negation);
        assert_ne!(ScopeDimension::Negation, ScopeDimension::Quantifier);
        assert_ne!(ScopeDimension::Precedence, ScopeDimension::Deictic);
    }
}
