//! Assertion registry and span matching.

use crate::assertion::{AssertionMismatch, SpanAssertion};
use crate::fixture::{Assertion, AssertionBody};
use layered_contracts::{DefinedTerm, ObligationPhrase, PronounReference};

/// Result of matching assertions against resolver output.
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub passed: Vec<AssertionResult>,
    pub failed: Vec<AssertionResult>,
}

/// Result of a single assertion check.
#[derive(Debug, Clone)]
pub struct AssertionResult {
    pub assertion: Assertion,
    pub span_text: String,
    pub outcome: AssertionOutcome,
}

/// Outcome of an assertion check.
#[derive(Debug, Clone)]
pub enum AssertionOutcome {
    Passed,
    Failed(AssertionMismatch),
    NotFound { reason: String },
    TypeMismatch { expected: String, actual: String },
}

/// Check if a span type name is supported.
pub fn is_supported_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "Obligation" | "ObligationPhrase" |
        "PronounReference" | "PronounRef" |
        "DefinedTerm"
    )
}

/// Get valid field names for a span type.
pub fn valid_fields_for_type(type_name: &str) -> Vec<&'static str> {
    match type_name {
        "Obligation" | "ObligationPhrase" => vec!["modal", "bearer", "action"],
        "PronounReference" | "PronounRef" => vec!["target", "pronoun_type", "confidence"],
        "DefinedTerm" => vec!["term_name", "definition_type"],
        _ => vec![],
    }
}

/// Parse and check an assertion against an ObligationPhrase.
pub fn check_obligation(
    obligation: &ObligationPhrase,
    body: &str,
) -> Result<(), AssertionMismatch> {
    let assertion = ObligationPhrase::parse_assertion(body)
        .map_err(|e| AssertionMismatch::new(&obligation.action, format!("parse error: {}", e)))?;
    obligation.check(&assertion)
}

/// Parse and check an assertion against a PronounReference.
pub fn check_pronoun(
    pronoun: &PronounReference,
    body: &str,
) -> Result<(), AssertionMismatch> {
    let assertion = PronounReference::parse_assertion(body)
        .map_err(|e| AssertionMismatch::new(&pronoun.pronoun, format!("parse error: {}", e)))?;
    pronoun.check(&assertion)
}

/// Parse and check an assertion against a DefinedTerm.
pub fn check_defined_term(
    term: &DefinedTerm,
    body: &str,
) -> Result<(), AssertionMismatch> {
    let assertion = DefinedTerm::parse_assertion(body)
        .map_err(|e| AssertionMismatch::new(&term.term_name, format!("parse error: {}", e)))?;
    term.check(&assertion)
}

/// Format an assertion body as a string for error messages.
pub fn format_body(body: &AssertionBody) -> String {
    body.field_checks
        .iter()
        .map(|fc| format!("{}{}{}", fc.field, fc.operator, fc.expected))
        .collect::<Vec<_>>()
        .join(", ")
}

impl MatchResult {
    pub fn new() -> Self {
        Self {
            passed: Vec::new(),
            failed: Vec::new(),
        }
    }

    pub fn add_passed(&mut self, assertion: Assertion, span_text: String) {
        self.passed.push(AssertionResult {
            assertion,
            span_text,
            outcome: AssertionOutcome::Passed,
        });
    }

    pub fn add_failed(&mut self, assertion: Assertion, span_text: String, mismatch: AssertionMismatch) {
        self.failed.push(AssertionResult {
            assertion,
            span_text,
            outcome: AssertionOutcome::Failed(mismatch),
        });
    }

    pub fn add_not_found(&mut self, assertion: Assertion, reason: String) {
        self.failed.push(AssertionResult {
            assertion,
            span_text: String::new(),
            outcome: AssertionOutcome::NotFound { reason },
        });
    }

    pub fn add_type_mismatch(&mut self, assertion: Assertion, span_text: String, expected: String, actual: String) {
        self.failed.push(AssertionResult {
            assertion,
            span_text,
            outcome: AssertionOutcome::TypeMismatch { expected, actual },
        });
    }

    pub fn all_passed(&self) -> bool {
        self.failed.is_empty()
    }

    pub fn summary(&self) -> String {
        format!("{} passed, {} failed", self.passed.len(), self.failed.len())
    }
}

impl Default for MatchResult {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layered_contracts::{ObligationType, ObligorReference, DefinitionType, PronounType, AntecedentCandidate};

    #[test]
    fn test_check_obligation_shall() {
        let obligation = ObligationPhrase {
            obligor: ObligorReference::NounPhrase { text: "The Tenant".to_string() },
            obligation_type: ObligationType::Duty,
            action: "pay rent".to_string(),
            conditions: vec![],
        };

        assert!(check_obligation(&obligation, "modal=shall").is_ok());
    }

    #[test]
    fn test_check_obligation_wrong_modal() {
        let obligation = ObligationPhrase {
            obligor: ObligorReference::NounPhrase { text: "The Tenant".to_string() },
            obligation_type: ObligationType::Permission,
            action: "pay rent".to_string(),
            conditions: vec![],
        };

        let result = check_obligation(&obligation, "modal=shall");
        assert!(result.is_err());
        let mismatch = result.unwrap_err();
        assert!(mismatch.fields.iter().any(|f| f.field == "modal"));
    }

    #[test]
    fn test_check_defined_term() {
        let term = DefinedTerm {
            term_name: "Tenant".to_string(),
            definition_type: DefinitionType::QuotedMeans,
        };

        assert!(check_defined_term(&term, "term_name=Tenant").is_ok());
        assert!(check_defined_term(&term, "definition_type=QuotedMeans").is_ok());
    }

    #[test]
    fn test_check_pronoun_type() {
        let pronoun = PronounReference {
            pronoun: "it".to_string(),
            pronoun_type: PronounType::ThirdSingularNeuter,
            candidates: vec![AntecedentCandidate {
                text: "The Tenant".to_string(),
                is_defined_term: true,
                token_distance: 5,
                confidence: 0.9,
            }],
        };

        assert!(check_pronoun(&pronoun, "pronoun_type=ThirdSingularNeuter").is_ok());
    }

    #[test]
    fn test_match_result() {
        let mut result = MatchResult::new();
        assert!(result.all_passed());

        result.add_not_found(
            Assertion {
                target: crate::fixture::RefTarget::Span(1),
                span_type: "Obligation".to_string(),
                body: crate::fixture::AssertionBody::default(),
                source_line: 1,
            },
            "span not found".to_string(),
        );

        assert!(!result.all_passed());
        assert_eq!(result.summary(), "0 passed, 1 failed");
    }

    #[test]
    fn test_is_supported_type() {
        assert!(is_supported_type("Obligation"));
        assert!(is_supported_type("ObligationPhrase"));
        assert!(is_supported_type("DefinedTerm"));
        assert!(is_supported_type("PronounReference"));
        assert!(!is_supported_type("Unknown"));
    }
}
