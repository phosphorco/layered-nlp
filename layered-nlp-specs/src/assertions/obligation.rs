//! Assertions for ObligationPhrase.

use crate::assertion::{
    AssertionMismatch, AssertionSpec, FieldMismatch, FieldValue, ParseError, SpanAssertion,
    TypeFieldCheck,
};
use layered_contracts::{ObligationPhrase, ObligationType, ObligorReference};

/// Parsed assertion for ObligationPhrase.
#[derive(Debug, Clone)]
pub struct ObligationAssertion {
    pub checks: Vec<TypeFieldCheck>,
}

impl AssertionSpec for ObligationAssertion {
    fn describe(&self) -> String {
        let fields: Vec<_> = self
            .checks
            .iter()
            .map(|c| format!("{}={:?}", c.field, c.value))
            .collect();
        format!("Obligation({})", fields.join(", "))
    }

    fn constrained_fields(&self) -> Vec<&'static str> {
        self.checks
            .iter()
            .map(|c| match c.field.as_str() {
                "modal" => "obligation_type",
                "bearer" => "obligor",
                "action" => "action",
                _ => "unknown",
            })
            .collect()
    }
}

impl SpanAssertion for ObligationPhrase {
    type Assertion = ObligationAssertion;

    fn parse_assertion(input: &str) -> Result<Self::Assertion, ParseError> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(ObligationAssertion { checks: Vec::new() });
        }

        let mut checks = Vec::new();
        for part in input.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            checks.push(TypeFieldCheck::parse(part)?);
        }

        Ok(ObligationAssertion { checks })
    }

    fn check(&self, assertion: &Self::Assertion) -> Result<(), AssertionMismatch> {
        let mut mismatch = AssertionMismatch::new(&self.action, assertion.describe());

        for check in &assertion.checks {
            match check.field.as_str() {
                "modal" => {
                    if let FieldValue::String(expected) = &check.value {
                        let expected_type = match expected.as_str() {
                            "shall" | "must" => ObligationType::Duty,
                            "may" | "can" => ObligationType::Permission,
                            "shall not" | "must not" => ObligationType::Prohibition,
                            _ => {
                                mismatch.fields.push(FieldMismatch::hard(
                                    "modal",
                                    expected.clone(),
                                    format!("unknown modal: {}", expected),
                                ));
                                continue;
                            }
                        };

                        if self.obligation_type != expected_type {
                            mismatch.fields.push(FieldMismatch::hard(
                                "modal",
                                expected.clone(),
                                format!("{:?}", self.obligation_type),
                            ));
                        }
                    }
                }
                "bearer" => {
                    if let FieldValue::EntityRef(entity_id) = &check.value {
                        let obligor_text = match &self.obligor {
                            ObligorReference::TermRef { term_name, .. } => term_name.clone(),
                            ObligorReference::PronounRef { resolved_to, .. } => resolved_to.clone(),
                            ObligorReference::NounPhrase { text } => text.clone(),
                        };

                        // Check if the entity ID appears in the obligor text
                        if !obligor_text
                            .to_lowercase()
                            .contains(&entity_id.to_lowercase())
                        {
                            mismatch.fields.push(FieldMismatch::hard(
                                "bearer",
                                format!("{}{}", '\u{00A7}', entity_id),
                                obligor_text,
                            ));
                        }
                    }
                }
                "action" => {
                    if let FieldValue::String(expected) = &check.value {
                        if !self.action.contains(expected.as_str()) {
                            mismatch.fields.push(FieldMismatch::hard(
                                "action",
                                expected.clone(),
                                self.action.clone(),
                            ));
                        }
                    }
                }
                _ => {
                    // Unknown field - report as info
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
        "ObligationPhrase"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_obligation(
        obligation_type: ObligationType,
        obligor: ObligorReference,
        action: &str,
    ) -> ObligationPhrase {
        ObligationPhrase {
            obligor,
            obligation_type,
            action: action.to_string(),
            conditions: Vec::new(),
        }
    }

    #[test]
    fn test_parse_empty_assertion() {
        let assertion = ObligationPhrase::parse_assertion("").unwrap();
        assert!(assertion.checks.is_empty());
    }

    #[test]
    fn test_parse_modal_shall() {
        let assertion = ObligationPhrase::parse_assertion("modal=shall").unwrap();
        assert_eq!(assertion.checks.len(), 1);
        assert_eq!(assertion.checks[0].field, "modal");
    }

    #[test]
    fn test_parse_multiple_checks() {
        let assertion = ObligationPhrase::parse_assertion("modal=shall, action=deliver").unwrap();
        assert_eq!(assertion.checks.len(), 2);
    }

    #[test]
    fn test_check_modal_shall_matches() {
        let obligation = make_obligation(
            ObligationType::Duty,
            ObligorReference::TermRef {
                term_name: "Company".to_string(),
                confidence: 0.9,
            },
            "deliver goods",
        );

        let assertion = ObligationPhrase::parse_assertion("modal=shall").unwrap();
        assert!(obligation.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_modal_may_matches() {
        let obligation = make_obligation(
            ObligationType::Permission,
            ObligorReference::TermRef {
                term_name: "Company".to_string(),
                confidence: 0.9,
            },
            "terminate early",
        );

        let assertion = ObligationPhrase::parse_assertion("modal=may").unwrap();
        assert!(obligation.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_modal_mismatch() {
        let obligation = make_obligation(
            ObligationType::Duty,
            ObligorReference::TermRef {
                term_name: "Company".to_string(),
                confidence: 0.9,
            },
            "deliver goods",
        );

        let assertion = ObligationPhrase::parse_assertion("modal=may").unwrap();
        let err = obligation.check(&assertion).unwrap_err();
        assert_eq!(err.fields.len(), 1);
        assert_eq!(err.fields[0].field, "modal");
    }

    #[test]
    fn test_check_bearer_matches() {
        let obligation = make_obligation(
            ObligationType::Duty,
            ObligorReference::TermRef {
                term_name: "Tenant".to_string(),
                confidence: 0.9,
            },
            "pay rent",
        );

        let assertion =
            ObligationPhrase::parse_assertion("bearer=\u{00A7}Tenant").unwrap();
        assert!(obligation.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_bearer_mismatch() {
        let obligation = make_obligation(
            ObligationType::Duty,
            ObligorReference::TermRef {
                term_name: "Tenant".to_string(),
                confidence: 0.9,
            },
            "pay rent",
        );

        let assertion =
            ObligationPhrase::parse_assertion("bearer=\u{00A7}Landlord").unwrap();
        let err = obligation.check(&assertion).unwrap_err();
        assert_eq!(err.fields.len(), 1);
        assert_eq!(err.fields[0].field, "bearer");
    }

    #[test]
    fn test_check_action_contains() {
        let obligation = make_obligation(
            ObligationType::Duty,
            ObligorReference::TermRef {
                term_name: "Company".to_string(),
                confidence: 0.9,
            },
            "deliver goods within 30 days",
        );

        let assertion = ObligationPhrase::parse_assertion("action=deliver").unwrap();
        assert!(obligation.check(&assertion).is_ok());
    }

    #[test]
    fn test_describe() {
        let assertion =
            ObligationPhrase::parse_assertion("modal=shall, bearer=\u{00A7}T").unwrap();
        let desc = assertion.describe();
        assert!(desc.contains("Obligation"));
        assert!(desc.contains("modal"));
    }

    #[test]
    fn test_constrained_fields() {
        let assertion = ObligationPhrase::parse_assertion("modal=shall, action=deliver").unwrap();
        let fields = assertion.constrained_fields();
        assert!(fields.contains(&"obligation_type"));
        assert!(fields.contains(&"action"));
    }
}
