//! Assertions for DefinedTerm.

use crate::assertion::{
    AssertionMismatch, AssertionSpec, FieldMismatch, FieldValue, ParseError, SpanAssertion,
    TypeFieldCheck,
};
use layered_contracts::{DefinedTerm, DefinitionType};

#[derive(Debug, Clone)]
pub struct DefinedTermAssertion {
    pub checks: Vec<TypeFieldCheck>,
}

impl AssertionSpec for DefinedTermAssertion {
    fn describe(&self) -> String {
        let fields: Vec<_> = self
            .checks
            .iter()
            .map(|c| format!("{}={:?}", c.field, c.value))
            .collect();
        format!("DefinedTerm({})", fields.join(", "))
    }

    fn constrained_fields(&self) -> Vec<&'static str> {
        self.checks
            .iter()
            .map(|c| match c.field.as_str() {
                "term_name" => "term_name",
                "definition_type" => "definition_type",
                _ => "unknown",
            })
            .collect()
    }
}

impl SpanAssertion for DefinedTerm {
    type Assertion = DefinedTermAssertion;

    fn parse_assertion(input: &str) -> Result<Self::Assertion, ParseError> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(DefinedTermAssertion { checks: Vec::new() });
        }

        let mut checks = Vec::new();
        for part in input.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            checks.push(TypeFieldCheck::parse(part)?);
        }

        Ok(DefinedTermAssertion { checks })
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
        "DefinedTerm"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_defined_term(term_name: &str, definition_type: DefinitionType) -> DefinedTerm {
        DefinedTerm {
            term_name: term_name.to_string(),
            definition_type,
        }
    }

    #[test]
    fn test_parse_empty_assertion() {
        let assertion = DefinedTerm::parse_assertion("").unwrap();
        assert!(assertion.checks.is_empty());
    }

    #[test]
    fn test_parse_term_name() {
        let assertion = DefinedTerm::parse_assertion("term_name=Company").unwrap();
        assert_eq!(assertion.checks.len(), 1);
        assert_eq!(assertion.checks[0].field, "term_name");
    }

    #[test]
    fn test_parse_definition_type() {
        let assertion = DefinedTerm::parse_assertion("definition_type=QuotedMeans").unwrap();
        assert_eq!(assertion.checks.len(), 1);
        assert_eq!(assertion.checks[0].field, "definition_type");
    }

    #[test]
    fn test_check_term_name_matches() {
        let term = make_defined_term("Company", DefinitionType::QuotedMeans);
        let assertion = DefinedTerm::parse_assertion("term_name=Company").unwrap();
        assert!(term.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_term_name_mismatch() {
        let term = make_defined_term("Company", DefinitionType::QuotedMeans);
        let assertion = DefinedTerm::parse_assertion("term_name=Contractor").unwrap();
        let err = term.check(&assertion).unwrap_err();
        assert_eq!(err.fields.len(), 1);
        assert_eq!(err.fields[0].field, "term_name");
    }

    #[test]
    fn test_check_definition_type_quoted_means() {
        let term = make_defined_term("Company", DefinitionType::QuotedMeans);
        let assertion = DefinedTerm::parse_assertion("definition_type=QuotedMeans").unwrap();
        assert!(term.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_definition_type_parenthetical() {
        let term = make_defined_term("Tenant", DefinitionType::Parenthetical);
        let assertion = DefinedTerm::parse_assertion("definition_type=Parenthetical").unwrap();
        assert!(term.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_definition_type_hereinafter() {
        let term = make_defined_term("Contractor", DefinitionType::Hereinafter);
        let assertion = DefinedTerm::parse_assertion("definition_type=Hereinafter").unwrap();
        assert!(term.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_definition_type_mismatch() {
        let term = make_defined_term("Company", DefinitionType::QuotedMeans);
        let assertion = DefinedTerm::parse_assertion("definition_type=Parenthetical").unwrap();
        let err = term.check(&assertion).unwrap_err();
        assert_eq!(err.fields.len(), 1);
        assert_eq!(err.fields[0].field, "definition_type");
    }

    #[test]
    fn test_check_multiple_fields() {
        let term = make_defined_term("Company", DefinitionType::QuotedMeans);
        let assertion =
            DefinedTerm::parse_assertion("term_name=Company, definition_type=QuotedMeans").unwrap();
        assert!(term.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_multiple_fields_one_mismatch() {
        let term = make_defined_term("Company", DefinitionType::QuotedMeans);
        let assertion =
            DefinedTerm::parse_assertion("term_name=Company, definition_type=Parenthetical")
                .unwrap();
        let err = term.check(&assertion).unwrap_err();
        assert_eq!(err.fields.len(), 1);
        assert_eq!(err.fields[0].field, "definition_type");
    }

    #[test]
    fn test_describe() {
        let assertion =
            DefinedTerm::parse_assertion("term_name=Company, definition_type=QuotedMeans").unwrap();
        let desc = assertion.describe();
        assert!(desc.contains("DefinedTerm"));
        assert!(desc.contains("term_name"));
    }

    #[test]
    fn test_constrained_fields() {
        let assertion =
            DefinedTerm::parse_assertion("term_name=Company, definition_type=QuotedMeans").unwrap();
        let fields = assertion.constrained_fields();
        assert!(fields.contains(&"term_name"));
        assert!(fields.contains(&"definition_type"));
    }
}
