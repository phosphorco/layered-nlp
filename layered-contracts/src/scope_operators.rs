//! Scope operator detection for negation and quantifiers.
//!
//! This module provides M7 Gate 1-3: Negation and quantifier detection using
//! the `ScopeOperator<NegationOp>` and `ScopeOperator<QuantifierOp>` types from M0.
//!
//! # Architecture
//!
//! - **M0 foundation**: Uses scope operator types from layered-nlp-document
//! - **M7 implementation**: Detects negation and quantifier markers in contract text
//! - **Scope domains**: Computes rightward scope using clause boundary heuristics
//!
//! # Example
//!
//! ```ignore
//! use layered_contracts::{ContractDocument, NegationDetector, QuantifierDetector};
//!
//! let doc = ContractDocument::from_text("The Company shall not disclose any information.");
//! let neg_detector = NegationDetector::new();
//! let quant_detector = QuantifierDetector::new();
//!
//! let negations = neg_detector.detect(&doc);
//! let quantifiers = quant_detector.detect(&doc);
//! ```

use crate::{
    ContractDocument, DocPosition, DocSpan, NegationKind, NegationOp, QuantifierKind,
    QuantifierOp, ScopeDimension, ScopeDomain, ScopeOperator, Scored,
};
use layered_nlp::{LLLine, LToken};
use std::collections::{HashMap, HashSet};

// ============================================================================
// Gate 1: Negation Detection
// ============================================================================

/// Detects negation operators in contract text.
///
/// Recognizes negation markers like "not", "never", "neither", "nor" and
/// computes their scope domains using clause boundary heuristics.
pub struct NegationDetector {
    /// Negation markers to detect
    markers: HashSet<&'static str>,
}

impl NegationDetector {
    pub fn new() -> Self {
        let markers: HashSet<&'static str> = [
            "not", "never", "no", "neither", "nor", "nothing", "nobody", "nowhere", "none",
        ]
        .iter()
        .copied()
        .collect();

        Self { markers }
    }

    /// Detect negation operators in a document.
    ///
    /// Returns scored scope operators with trigger spans and computed domains.
    pub fn detect(&self, doc: &ContractDocument) -> Vec<Scored<ScopeOperator<NegationOp>>> {
        let mut results = Vec::new();

        for (line_idx, line) in doc.lines_enumerated() {
            let tokens = line.ll_tokens();

            for (token_idx, token) in tokens.iter().enumerate() {
                let text = match token.get_token() {
                    LToken::Text(text, _) => text.to_lowercase(),
                    LToken::Value => continue,
                };

                if self.markers.contains(text.as_str()) {
                    let kind = self.classify_negation(&text);
                    let trigger = self.make_trigger_span(line_idx, token_idx);
                    let domain = self.compute_domain(line, line_idx, token_idx);

                    let op = ScopeOperator::new(
                        ScopeDimension::Negation,
                        trigger,
                        domain,
                        NegationOp {
                            marker: text.clone(),
                            kind,
                        },
                    );

                    results.push(Scored::rule_based(op, 0.9, "negation_detector"));
                }
            }
        }

        results
    }

    fn classify_negation(&self, marker: &str) -> NegationKind {
        match marker {
            "never" => NegationKind::Temporal,
            "neither" | "nor" => NegationKind::Correlative,
            _ => NegationKind::Simple,
        }
    }

    fn make_trigger_span(&self, line: usize, token: usize) -> DocSpan {
        DocSpan::new(
            DocPosition { line, token },
            DocPosition {
                line,
                token: token + 1,
            },
        )
    }

    /// Compute scope domain for negation.
    ///
    /// Strategy:
    /// - Scope extends rightward from negation marker
    /// - Ends at clause boundary (comma, semicolon, "except", "unless")
    /// - Ends at coordinating conjunction ("and", "or") at same clause level
    /// - Ends at end of line (sentence)
    fn compute_domain(&self, line: &LLLine, line_idx: usize, start_token: usize) -> ScopeDomain {
        let tokens = line.ll_tokens();

        // Start after the negation marker
        let domain_start = start_token + 1;
        let mut domain_end = tokens.len();

        // Scan rightward for scope boundaries
        for idx in domain_start..tokens.len() {
            let text = match tokens[idx].get_token() {
                LToken::Text(text, _) => text.to_lowercase(),
                LToken::Value => continue,
            };

            // Clause boundaries end scope
            if matches!(text.as_str(), "," | ";" | "except" | "unless" | "but" | ".") {
                domain_end = idx;
                break;
            }

            // Coordinating conjunction at clause level
            if matches!(text.as_str(), "and" | "or") {
                // Heuristic: if preceded by comma or at low nesting, ends scope
                if idx > 0 {
                    if let LToken::Text(prev_text, _) = tokens[idx - 1].get_token() {
                        if prev_text == "," {
                            domain_end = idx;
                            break;
                        }
                    }
                }
            }
        }

        let span = DocSpan::new(
            DocPosition {
                line: line_idx,
                token: domain_start,
            },
            DocPosition {
                line: line_idx,
                token: domain_end,
            },
        );

        ScopeDomain::from_single(span)
    }
}

impl Default for NegationDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Gate 2: Quantifier Detection
// ============================================================================

/// Detects quantifier operators in contract text.
///
/// Recognizes universal ("each", "all", "every"), existential ("any", "some"),
/// and negative ("no", "none") quantifiers with scope computation.
pub struct QuantifierDetector {
    /// Quantifier markers mapped to their kind
    markers: HashMap<&'static str, QuantifierKind>,
}

impl QuantifierDetector {
    pub fn new() -> Self {
        let mut markers = HashMap::new();

        // Universal quantifiers
        markers.insert("each", QuantifierKind::Universal);
        markers.insert("every", QuantifierKind::Universal);
        markers.insert("all", QuantifierKind::Universal);

        // Existential quantifiers
        markers.insert("any", QuantifierKind::Existential);
        markers.insert("some", QuantifierKind::Existential);

        // Negative quantifiers
        markers.insert("no", QuantifierKind::Negative);
        markers.insert("none", QuantifierKind::Negative);

        Self { markers }
    }

    /// Detect quantifier operators in a document.
    ///
    /// Returns scored scope operators with trigger spans and computed domains.
    pub fn detect(&self, doc: &ContractDocument) -> Vec<Scored<ScopeOperator<QuantifierOp>>> {
        let mut results = Vec::new();

        for (line_idx, line) in doc.lines_enumerated() {
            let tokens = line.ll_tokens();

            for (token_idx, token) in tokens.iter().enumerate() {
                let text = match token.get_token() {
                    LToken::Text(text, _) => text.to_lowercase(),
                    LToken::Value => continue,
                };

                if let Some(&kind) = self.markers.get(text.as_str()) {
                    let trigger = self.make_trigger_span(line_idx, token_idx);
                    let domain = self.compute_domain(line, line_idx, token_idx, kind);

                    let op = ScopeOperator::new(
                        ScopeDimension::Quantifier,
                        trigger,
                        domain,
                        QuantifierOp {
                            marker: text.clone(),
                            kind,
                        },
                    );

                    results.push(Scored::rule_based(op, 0.85, "quantifier_detector"));
                }
            }
        }

        results
    }

    fn make_trigger_span(&self, line: usize, token: usize) -> DocSpan {
        DocSpan::new(
            DocPosition { line, token },
            DocPosition {
                line,
                token: token + 1,
            },
        )
    }

    /// Compute scope domain for quantifier.
    ///
    /// Strategy:
    /// - Quantifiers scope over the noun phrase they modify
    /// - For "each/every/all X", scope is the NP following the quantifier
    /// - For "any/some X", scope is the NP
    /// - Domain extends to verb phrase if quantifier is subject
    fn compute_domain(
        &self,
        line: &LLLine,
        line_idx: usize,
        start_token: usize,
        kind: QuantifierKind,
    ) -> ScopeDomain {
        let tokens = line.ll_tokens();

        // Start after the quantifier marker
        let domain_start = start_token + 1;
        let mut domain_end = tokens.len();

        // For universal quantifiers, scope over NP + VP
        if matches!(kind, QuantifierKind::Universal) {
            // Heuristic: extend to clause boundary or verb
            for idx in domain_start..tokens.len() {
                let text = match tokens[idx].get_token() {
                    LToken::Text(text, _) => text.to_lowercase(),
                    LToken::Value => continue,
                };

                // Stop at clause boundaries
                if matches!(text.as_str(), "," | ";" | "and" | "or" | ".") {
                    domain_end = idx;
                    break;
                }
            }
        } else {
            // For existential/negative, scope is typically just the NP
            // Heuristic: extend to next verb or clause boundary
            for idx in domain_start..tokens.len() {
                let text = match tokens[idx].get_token() {
                    LToken::Text(text, _) => text.to_lowercase(),
                    LToken::Value => continue,
                };

                // Stop at clause boundaries or modals
                if matches!(
                    text.as_str(),
                    "," | ";" | "." | "shall" | "may" | "must" | "will"
                ) {
                    domain_end = idx;
                    break;
                }
            }
        }

        let span = DocSpan::new(
            DocPosition {
                line: line_idx,
                token: domain_start,
            },
            DocPosition {
                line: line_idx,
                token: domain_end,
            },
        );

        ScopeDomain::from_single(span)
    }
}

impl Default for QuantifierDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Gate 3: Scope Boundary Detection
// ============================================================================

/// Helper for computing scope boundaries.
///
/// Provides utilities for finding where scope domains end based on
/// clause boundaries, exception markers, and parenthetical nesting.
pub struct ScopeBoundaryDetector {
    /// Clause boundary markers
    clause_boundaries: HashSet<&'static str>,
    /// Exception markers that end scope
    exception_markers: HashSet<&'static str>,
}

impl ScopeBoundaryDetector {
    pub fn new() -> Self {
        let clause_boundaries: HashSet<&'static str> =
            [",", ";", ":", ".", "and", "or", "but"]
                .iter()
                .copied()
                .collect();
        let exception_markers: HashSet<&'static str> =
            ["except", "unless", "save", "but", "provided"]
                .iter()
                .copied()
                .collect();

        Self {
            clause_boundaries,
            exception_markers,
        }
    }

    /// Find the end of scope domain within a line.
    ///
    /// Returns token index where scope ends (exclusive).
    pub fn find_scope_end_in_line(&self, line: &LLLine, start_idx: usize) -> usize {
        let tokens = line.ll_tokens();
        let mut depth: i32 = 0; // Track parenthetical nesting

        for idx in start_idx..tokens.len() {
            let text = match tokens[idx].get_token() {
                LToken::Text(text, _) => text.to_lowercase(),
                LToken::Value => continue,
            };

            // Track nesting depth
            if text == "(" {
                depth += 1;
                continue;
            }
            if text == ")" {
                depth = depth.saturating_sub(1);
                continue;
            }

            // Only check boundaries at depth 0
            if depth == 0 {
                // Exception markers end scope
                if self.exception_markers.contains(text.as_str()) {
                    return idx;
                }

                // Clause boundaries end scope
                if self.clause_boundaries.contains(text.as_str()) {
                    return idx;
                }
            }
        }

        tokens.len()
    }

    /// Check if negation and quantifier scopes interact.
    ///
    /// Returns true if quantifier appears within negation scope or vice versa.
    pub fn scopes_interact(
        neg_op: &ScopeOperator<NegationOp>,
        quant_op: &ScopeOperator<QuantifierOp>,
    ) -> bool {
        let neg_domain = neg_op.domain.primary();
        let quant_domain = quant_op.domain.primary();

        if let (Some(neg_span), Some(quant_span)) = (neg_domain, quant_domain) {
            // Check if spans overlap
            neg_span.overlaps(quant_span)
        } else {
            false
        }
    }
}

impl Default for ScopeBoundaryDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negation_simple() {
        let text = "The Company shall not disclose confidential information.";
        let doc = ContractDocument::from_text(text);

        let detector = NegationDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        let op = &ops[0].value;
        assert_eq!(op.dimension, ScopeDimension::Negation);
        assert_eq!(op.payload.marker, "not");
        assert_eq!(op.payload.kind, NegationKind::Simple);

        // Domain should cover "disclose confidential information"
        let domain = op.domain.primary().unwrap();
        assert!(domain.start.token > 3); // After "not"
    }

    #[test]
    fn test_negation_never() {
        let text = "The Buyer shall never terminate this Agreement.";
        let doc = ContractDocument::from_text(text);

        let detector = NegationDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].value.payload.marker, "never");
        assert_eq!(ops[0].value.payload.kind, NegationKind::Temporal);
    }

    #[test]
    fn test_negation_scope_ends_at_except() {
        let text = "Seller shall not be liable except as provided in Section 5.";
        let doc = ContractDocument::from_text(text);

        let detector = NegationDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        // Domain should end before "except"
        let domain = ops[0].value.domain.primary().unwrap();
        // Verify scope ends at "except" token
        let line = doc.lines().first().unwrap();
        let tokens = line.ll_tokens();
        let except_idx = tokens
            .iter()
            .position(|t| match t.get_token() {
                LToken::Text(text, _) => text.to_lowercase() == "except",
                LToken::Value => false,
            })
            .unwrap();
        assert_eq!(domain.end.token, except_idx);
    }

    #[test]
    fn test_quantifier_universal_each() {
        let text = "Each party shall comply with applicable laws.";
        let doc = ContractDocument::from_text(text);

        let detector = QuantifierDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        let op = &ops[0].value;
        assert_eq!(op.dimension, ScopeDimension::Quantifier);
        assert_eq!(op.payload.marker, "each");
        assert_eq!(op.payload.kind, QuantifierKind::Universal);
    }

    #[test]
    fn test_quantifier_existential_any() {
        let text = "The Company shall not disclose any confidential information.";
        let doc = ContractDocument::from_text(text);

        let detector = QuantifierDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].value.payload.marker, "any");
        assert_eq!(ops[0].value.payload.kind, QuantifierKind::Existential);
    }

    #[test]
    fn test_quantifier_negative_no() {
        let text = "There shall be no liability for indirect damages.";
        let doc = ContractDocument::from_text(text);

        let detector = QuantifierDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].value.payload.marker, "no");
        assert_eq!(ops[0].value.payload.kind, QuantifierKind::Negative);
    }

    #[test]
    fn test_negation_quantifier_interaction() {
        let text = "Each party shall not disclose any information.";
        let doc = ContractDocument::from_text(text);

        let neg_detector = NegationDetector::new();
        let quant_detector = QuantifierDetector::new();

        let negations = neg_detector.detect(&doc);
        let quantifiers = quant_detector.detect(&doc);

        assert_eq!(negations.len(), 1);
        assert_eq!(quantifiers.len(), 2); // "each" and "any"

        // Verify interaction detection
        let neg_op = &negations[0].value;

        let mut interaction_count = 0;
        for quant in &quantifiers {
            if ScopeBoundaryDetector::scopes_interact(neg_op, &quant.value) {
                interaction_count += 1;
            }
        }
        // At least one quantifier should interact with negation
        assert!(interaction_count > 0);
    }

    #[test]
    fn test_scope_boundary_detector() {
        let detector = ScopeBoundaryDetector::new();
        let text = "shall not disclose information, and the Company shall comply";
        let doc = ContractDocument::from_text(text);
        let line = doc.lines().first().unwrap();
        let tokens = line.ll_tokens();

        // Find scope end starting after "not"
        let not_idx = tokens
            .iter()
            .position(|t| match t.get_token() {
                LToken::Text(text, _) => text == "not",
                LToken::Value => false,
            })
            .unwrap();
        let scope_end = detector.find_scope_end_in_line(line, not_idx + 1);

        // Should end at the comma
        let comma_idx = tokens
            .iter()
            .position(|t| match t.get_token() {
                LToken::Text(text, _) => text == ",",
                LToken::Value => false,
            })
            .unwrap();
        assert_eq!(scope_end, comma_idx);
    }

    #[test]
    fn test_negation_correlative_neither() {
        let text = "Neither party shall be responsible for consequential damages.";
        let doc = ContractDocument::from_text(text);

        let detector = NegationDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].value.payload.marker, "neither");
        assert_eq!(ops[0].value.payload.kind, NegationKind::Correlative);
    }

    #[test]
    fn test_negation_scope_ends_at_comma() {
        let text = "The Company shall not use confidential information, except with consent.";
        let doc = ContractDocument::from_text(text);

        let detector = NegationDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        let domain = ops[0].value.domain.primary().unwrap();

        // Verify scope ends at comma
        let line = doc.lines().first().unwrap();
        let tokens = line.ll_tokens();
        let comma_idx = tokens
            .iter()
            .position(|t| match t.get_token() {
                LToken::Text(text, _) => text == ",",
                LToken::Value => false,
            })
            .unwrap();
        assert_eq!(domain.end.token, comma_idx);
    }

    #[test]
    fn test_quantifier_universal_all() {
        let text = "All employees must comply with the policy.";
        let doc = ContractDocument::from_text(text);

        let detector = QuantifierDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].value.payload.marker, "all");
        assert_eq!(ops[0].value.payload.kind, QuantifierKind::Universal);
    }

    #[test]
    fn test_quantifier_existential_some() {
        let text = "The Company may disclose some information under certain conditions.";
        let doc = ContractDocument::from_text(text);

        let detector = QuantifierDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].value.payload.marker, "some");
        assert_eq!(ops[0].value.payload.kind, QuantifierKind::Existential);
    }

    #[test]
    fn test_negation_scope_ends_at_unless() {
        let text = "The Company shall not modify the terms unless both parties agree.";
        let doc = ContractDocument::from_text(text);

        let detector = NegationDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        let domain = ops[0].value.domain.primary().unwrap();

        // Scope should end at "unless"
        let line = doc.lines().first().unwrap();
        let tokens = line.ll_tokens();
        let unless_idx = tokens
            .iter()
            .position(|t| match t.get_token() {
                LToken::Text(text, _) => text.to_lowercase() == "unless",
                LToken::Value => false,
            })
            .unwrap();
        assert_eq!(domain.end.token, unless_idx);
    }

    #[test]
    fn test_multiple_negations_in_sentence() {
        let text = "The Buyer shall not assign this Agreement, and shall never transfer rights.";
        let doc = ContractDocument::from_text(text);

        let detector = NegationDetector::new();
        let ops = detector.detect(&doc);

        // Should detect both "not" and "never"
        assert_eq!(ops.len(), 2);

        // Verify both negations detected
        let markers: Vec<&str> = ops.iter().map(|op| op.value.payload.marker.as_str()).collect();
        assert!(markers.contains(&"not"));
        assert!(markers.contains(&"never"));
    }

    #[test]
    fn test_negation_scope_ends_at_period() {
        let text = "The Company shall not disclose.";
        let doc = ContractDocument::from_text(text);

        let detector = NegationDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        let domain = ops[0].value.domain.primary().unwrap();

        // Scope should end at the period
        let line = doc.lines().first().unwrap();
        let tokens = line.ll_tokens();
        let period_idx = tokens
            .iter()
            .position(|t| match t.get_token() {
                LToken::Text(text, _) => text == ".",
                LToken::Value => false,
            })
            .unwrap();
        assert_eq!(domain.end.token, period_idx);
    }

    #[test]
    fn test_quantifier_universal_every() {
        let text = "Every provision in this Agreement is binding.";
        let doc = ContractDocument::from_text(text);

        let detector = QuantifierDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].value.payload.marker, "every");
        assert_eq!(ops[0].value.payload.kind, QuantifierKind::Universal);
    }

    #[test]
    fn test_quantifier_negative_none() {
        let text = "None of the parties shall have liability for punitive damages.";
        let doc = ContractDocument::from_text(text);

        let detector = QuantifierDetector::new();
        let ops = detector.detect(&doc);

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].value.payload.marker, "none");
        assert_eq!(ops[0].value.payload.kind, QuantifierKind::Negative);
    }
}
