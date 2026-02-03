//! Assertions for TermReference.

use crate::assertion::{
    AssertionMismatch, AssertionSpec, FieldMismatch, FieldOperator, FieldValue, ParseError,
    SpanAssertion, TypeFieldCheck,
};
use layered_contracts::{DefinitionType, TermReference};

#[derive(Debug, Clone)]
pub struct TermReferenceAssertion {
    pub checks: Vec<TypeFieldCheck>,
}

impl AssertionSpec for TermReferenceAssertion {
    fn describe(&self) -> String {
        let fields: Vec<_> = self
            .checks
            .iter()
            .map(|c| format!("{}={:?}", c.field, c.value))
            .collect();
        format!("TermReference({})", fields.join(", "))
    }

    fn constrained_fields(&self) -> Vec<&'static str> {
        self.checks
            .iter()
            .map(|c| match c.field.as_str() {
                "term_name" => "term_name",
                "definition_type" => "definition_type",
                "target" => "term_name",
                _ => "unknown",
            })
            .collect()
    }
}

impl SpanAssertion for TermReference {
    type Assertion = TermReferenceAssertion;

    fn parse_assertion(input: &str) -> Result<Self::Assertion, ParseError> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(TermReferenceAssertion { checks: Vec::new() });
        }

        let mut checks = Vec::new();
        for part in input.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            checks.push(TypeFieldCheck::parse(part)?);
        }

        Ok(TermReferenceAssertion { checks })
    }

    fn check(&self, assertion: &Self::Assertion) -> Result<(), AssertionMismatch> {
        let mut mismatch = AssertionMismatch::new(&self.term_name, assertion.describe());

        for check in &assertion.checks {
            match check.field.as_str() {
                "term_name" => {
                    if let FieldValue::String(expected) = &check.value {
                        if self.term_name != *expected {
                            mismatch.fields.push(FieldMismatch::hard(
                                "term_name",
                                expected.clone(),
                                self.term_name.clone(),
                            ));
                        }
                    }
                }
                "target" => {
                    let expected = match &check.value {
                        FieldValue::String(value) => Some(value.clone()),
                        FieldValue::EntityRef(entity_id) => Some(entity_id.clone()),
                        _ => None,
                    };

                    if let Some(expected) = expected {
                        let actual = self.term_name.clone();
                        let matches = if check.operator == FieldOperator::Arrow {
                            actual.to_lowercase().contains(&expected.to_lowercase())
                        } else {
                            actual.eq_ignore_ascii_case(&expected)
                        };

                        if !matches {
                            mismatch.fields.push(FieldMismatch::hard(
                                "target",
                                expected,
                                actual,
                            ));
                        }
                    }
                }
                "definition_type" => {
                    if let FieldValue::String(expected) = &check.value {
                        let expected_type = match expected.as_str() {
                            "QuotedMeans" => Some(DefinitionType::QuotedMeans),
                            "Parenthetical" => Some(DefinitionType::Parenthetical),
                            "Hereinafter" => Some(DefinitionType::Hereinafter),
                            _ => None,
                        };

                        match expected_type {
                            Some(expected) if self.definition_type != expected => {
                                mismatch.fields.push(FieldMismatch::hard(
                                    "definition_type",
                                    format!("{:?}", expected),
                                    format!("{:?}", self.definition_type),
                                ));
                            }
                            None => {
                                mismatch.fields.push(FieldMismatch::hard(
                                    "definition_type",
                                    expected.clone(),
                                    "unknown type".to_string(),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        if mismatch.fields.is_empty() {
            Ok(())
        } else {
            Err(mismatch)
        }
    }

    fn span_type_name() -> &'static str {
        "TermReference"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_term_reference(term_name: &str, definition_type: DefinitionType) -> TermReference {
        TermReference {
            term_name: term_name.to_string(),
            definition_type,
        }
    }

    #[test]
    fn test_parse_empty_assertion() {
        let assertion = TermReference::parse_assertion("").unwrap();
        assert!(assertion.checks.is_empty());
    }

    #[test]
    fn test_parse_term_name() {
        let assertion = TermReference::parse_assertion("term_name=Agreement").unwrap();
        assert_eq!(assertion.checks.len(), 1);
        assert_eq!(assertion.checks[0].field, "term_name");
    }

    #[test]
    fn test_check_term_name_matches() {
        let term_ref = make_term_reference("Agreement", DefinitionType::QuotedMeans);
        let assertion = TermReference::parse_assertion("term_name=Agreement").unwrap();
        assert!(term_ref.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_term_name_mismatch() {
        let term_ref = make_term_reference("Agreement", DefinitionType::QuotedMeans);
        let assertion = TermReference::parse_assertion("term_name=Contract").unwrap();
        let err = term_ref.check(&assertion).unwrap_err();
        assert_eq!(err.fields.len(), 1);
        assert_eq!(err.fields[0].field, "term_name");
    }

    #[test]
    fn test_check_definition_type_matches() {
        let term_ref = make_term_reference("Agreement", DefinitionType::QuotedMeans);
        let assertion = TermReference::parse_assertion("definition_type=QuotedMeans").unwrap();
        assert!(term_ref.check(&assertion).is_ok());
    }
}
