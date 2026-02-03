//! Assertions for Clause.

use crate::assertion::{
    AssertionMismatch, AssertionSpec, FieldMismatch, FieldValue, ParseError, SpanAssertion,
    TypeFieldCheck,
};
use layered_clauses::Clause;

/// Parsed assertion for Clause.
#[derive(Debug, Clone)]
pub struct ClauseAssertion {
    pub checks: Vec<TypeFieldCheck>,
}

impl AssertionSpec for ClauseAssertion {
    fn describe(&self) -> String {
        let fields: Vec<_> = self
            .checks
            .iter()
            .map(|c| format!("{}={:?}", c.field, c.value))
            .collect();
        format!("Clause({})", fields.join(", "))
    }

    fn constrained_fields(&self) -> Vec<&'static str> {
        self.checks
            .iter()
            .map(|c| match c.field.as_str() {
                "type" | "category" => "category",
                _ => "unknown",
            })
            .collect()
    }
}

impl SpanAssertion for Clause {
    type Assertion = ClauseAssertion;

    fn parse_assertion(input: &str) -> Result<Self::Assertion, ParseError> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(ClauseAssertion { checks: Vec::new() });
        }

        let mut checks = Vec::new();
        for part in input.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            checks.push(TypeFieldCheck::parse(part)?);
        }

        Ok(ClauseAssertion { checks })
    }

    fn check(&self, assertion: &Self::Assertion) -> Result<(), AssertionMismatch> {
        let mut mismatch = AssertionMismatch::new(format!("{:?}", self), assertion.describe());

        for check in &assertion.checks {
            match check.field.as_str() {
                "type" | "category" => {
                    if let FieldValue::String(expected) = &check.value {
                        match parse_clause_kind(expected) {
                            Some(expected_kind) => {
                                if *self != expected_kind {
                                    mismatch.fields.push(FieldMismatch::hard(
                                        "type",
                                        expected.clone(),
                                        format!("{:?}", self),
                                    ));
                                }
                            }
                            None => mismatch.fields.push(FieldMismatch::hard(
                                "type",
                                expected.clone(),
                                "unknown clause type".to_string(),
                            )),
                        }
                    }
                }
                _ => {
                    mismatch.fields.push(FieldMismatch::info(
                        "unknown",
                        format!("valid field for {}", Self::span_type_name()),
                        check.field.clone(),
                    ));
                }
            }
        }

        if mismatch.fields.is_empty() {
            Ok(())
        } else {
            Err(mismatch)
        }
    }

    fn span_type_name() -> &'static str {
        "Clause"
    }
}

fn parse_clause_kind(input: &str) -> Option<Clause> {
    let normalized = input
        .trim()
        .to_lowercase()
        .replace('-', "_")
        .replace(' ', "_");

    match normalized.as_str() {
        "condition" => Some(Clause::Condition),
        "trailing_effect" | "trailing" => Some(Clause::TrailingEffect),
        "leading_effect" | "leading" => Some(Clause::LeadingEffect),
        "independent" => Some(Clause::Independent),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_assertion() {
        let assertion = Clause::parse_assertion("").unwrap();
        assert!(assertion.checks.is_empty());
    }

    #[test]
    fn test_parse_type() {
        let assertion = Clause::parse_assertion("type=condition").unwrap();
        assert_eq!(assertion.checks.len(), 1);
        assert_eq!(assertion.checks[0].field, "type");
    }

    #[test]
    fn test_check_type_match() {
        let clause = Clause::Condition;
        let assertion = Clause::parse_assertion("type=condition").unwrap();
        assert!(clause.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_type_mismatch() {
        let clause = Clause::Independent;
        let assertion = Clause::parse_assertion("type=condition").unwrap();
        let err = clause.check(&assertion).unwrap_err();
        assert_eq!(err.fields.len(), 1);
        assert_eq!(err.fields[0].field, "type");
    }
}
