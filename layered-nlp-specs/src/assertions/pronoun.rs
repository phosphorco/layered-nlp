//! Assertions for PronounReference.

use crate::assertion::{
    AssertionMismatch, AssertionSpec, FieldMismatch, FieldOperator, FieldValue, ParseError,
    SpanAssertion, TypeFieldCheck,
};
use layered_contracts::{PronounReference, PronounType};

#[derive(Debug, Clone)]
pub struct PronounAssertion {
    pub checks: Vec<TypeFieldCheck>,
}

impl AssertionSpec for PronounAssertion {
    fn describe(&self) -> String {
        let fields: Vec<_> = self
            .checks
            .iter()
            .map(|c| format!("{}={:?}", c.field, c.value))
            .collect();
        format!("PronounReference({})", fields.join(", "))
    }

    fn constrained_fields(&self) -> Vec<&'static str> {
        self.checks
            .iter()
            .map(|c| match c.field.as_str() {
                "target" => "candidates",
                "pronoun_type" => "pronoun_type",
                "confidence" => "confidence",
                _ => "unknown",
            })
            .collect()
    }
}

impl SpanAssertion for PronounReference {
    type Assertion = PronounAssertion;

    fn parse_assertion(input: &str) -> Result<Self::Assertion, ParseError> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(PronounAssertion { checks: Vec::new() });
        }

        let mut checks = Vec::new();
        for part in input.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            checks.push(TypeFieldCheck::parse(part)?);
        }

        Ok(PronounAssertion { checks })
    }

    fn check(&self, assertion: &Self::Assertion) -> Result<(), AssertionMismatch> {
        let mut mismatch = AssertionMismatch::new(&self.pronoun, assertion.describe());

        for check in &assertion.checks {
            match check.field.as_str() {
                "target" => {
                    // -> Entity syntax - check first candidate
                    if check.operator == FieldOperator::Arrow {
                        if let FieldValue::EntityRef(entity_id) = &check.value {
                            let actual = self
                                .candidates
                                .first()
                                .map(|c| c.text.clone())
                                .unwrap_or_else(|| "<no candidates>".to_string());

                            if !actual.to_lowercase().contains(&entity_id.to_lowercase()) {
                                mismatch.fields.push(FieldMismatch::hard(
                                    "target",
                                    format!("{}{}", '\u{00A7}', entity_id),
                                    actual,
                                ));
                            }
                        }
                    }
                }
                "pronoun_type" => {
                    if let FieldValue::String(expected) = &check.value {
                        let expected_type = match expected.as_str() {
                            "ThirdSingularNeuter" => Some(PronounType::ThirdSingularNeuter),
                            "ThirdSingularMasculine" => Some(PronounType::ThirdSingularMasculine),
                            "ThirdSingularFeminine" => Some(PronounType::ThirdSingularFeminine),
                            "ThirdPlural" => Some(PronounType::ThirdPlural),
                            "Relative" => Some(PronounType::Relative),
                            "Other" => Some(PronounType::Other),
                            _ => None,
                        };

                        match expected_type {
                            Some(expected) if self.pronoun_type != expected => {
                                mismatch.fields.push(FieldMismatch::hard(
                                    "pronoun_type",
                                    format!("{:?}", expected),
                                    format!("{:?}", self.pronoun_type),
                                ));
                            }
                            None => {
                                mismatch.fields.push(FieldMismatch::hard(
                                    "pronoun_type",
                                    expected.clone(),
                                    "unknown type".to_string(),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                "confidence" => {
                    if let FieldValue::Number(threshold) = &check.value {
                        let actual = self.candidates.first().map(|c| c.confidence).unwrap_or(0.0);

                        let passes = match check.operator {
                            FieldOperator::Gte => actual >= *threshold,
                            FieldOperator::Lte => actual <= *threshold,
                            _ => (actual - *threshold).abs() < 0.001,
                        };

                        if !passes {
                            mismatch.fields.push(FieldMismatch::soft(
                                "confidence",
                                format!("{} {}", operator_str(&check.operator), threshold),
                                format!("{:.2}", actual),
                            ));
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
        "PronounReference"
    }
}

fn operator_str(op: &FieldOperator) -> &'static str {
    match op {
        FieldOperator::Equals => "=",
        FieldOperator::Gte => ">=",
        FieldOperator::Lte => "<=",
        FieldOperator::Arrow => "->",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layered_contracts::AntecedentCandidate;

    fn make_pronoun(
        pronoun: &str,
        pronoun_type: PronounType,
        candidates: Vec<AntecedentCandidate>,
    ) -> PronounReference {
        PronounReference {
            pronoun: pronoun.to_string(),
            pronoun_type,
            candidates,
        }
    }

    fn make_candidate(text: &str, confidence: f64) -> AntecedentCandidate {
        AntecedentCandidate {
            text: text.to_string(),
            is_defined_term: true,
            token_distance: 5,
            confidence,
        }
    }

    #[test]
    fn test_parse_empty_assertion() {
        let assertion = PronounReference::parse_assertion("").unwrap();
        assert!(assertion.checks.is_empty());
    }

    #[test]
    fn test_parse_arrow_target() {
        let assertion = PronounReference::parse_assertion("-> \u{00A7}Tenant").unwrap();
        assert_eq!(assertion.checks.len(), 1);
        assert_eq!(assertion.checks[0].field, "target");
        assert_eq!(assertion.checks[0].operator, FieldOperator::Arrow);
    }

    #[test]
    fn test_check_target_matches() {
        let pronoun = make_pronoun(
            "it",
            PronounType::ThirdSingularNeuter,
            vec![make_candidate("Tenant", 0.85)],
        );

        let assertion = PronounReference::parse_assertion("-> \u{00A7}Tenant").unwrap();
        assert!(pronoun.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_target_mismatch() {
        let pronoun = make_pronoun(
            "it",
            PronounType::ThirdSingularNeuter,
            vec![make_candidate("Landlord", 0.85)],
        );

        let assertion = PronounReference::parse_assertion("-> \u{00A7}Tenant").unwrap();
        let err = pronoun.check(&assertion).unwrap_err();
        assert_eq!(err.fields.len(), 1);
        assert_eq!(err.fields[0].field, "target");
    }

    #[test]
    fn test_check_target_no_candidates() {
        let pronoun = make_pronoun("it", PronounType::ThirdSingularNeuter, vec![]);

        let assertion = PronounReference::parse_assertion("-> \u{00A7}Tenant").unwrap();
        let err = pronoun.check(&assertion).unwrap_err();
        assert_eq!(err.fields.len(), 1);
        assert!(err.fields[0].actual.contains("no candidates"));
    }

    #[test]
    fn test_check_pronoun_type_matches() {
        let pronoun = make_pronoun(
            "it",
            PronounType::ThirdSingularNeuter,
            vec![make_candidate("Company", 0.9)],
        );

        let assertion =
            PronounReference::parse_assertion("pronoun_type=ThirdSingularNeuter").unwrap();
        assert!(pronoun.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_pronoun_type_mismatch() {
        let pronoun = make_pronoun(
            "they",
            PronounType::ThirdPlural,
            vec![make_candidate("Parties", 0.9)],
        );

        let assertion =
            PronounReference::parse_assertion("pronoun_type=ThirdSingularNeuter").unwrap();
        let err = pronoun.check(&assertion).unwrap_err();
        assert_eq!(err.fields.len(), 1);
        assert_eq!(err.fields[0].field, "pronoun_type");
    }

    #[test]
    fn test_check_confidence_gte_passes() {
        let pronoun = make_pronoun(
            "it",
            PronounType::ThirdSingularNeuter,
            vec![make_candidate("Company", 0.85)],
        );

        let assertion = PronounReference::parse_assertion("confidence >= 0.8").unwrap();
        assert!(pronoun.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_confidence_gte_fails() {
        let pronoun = make_pronoun(
            "it",
            PronounType::ThirdSingularNeuter,
            vec![make_candidate("Company", 0.75)],
        );

        let assertion = PronounReference::parse_assertion("confidence >= 0.8").unwrap();
        let err = pronoun.check(&assertion).unwrap_err();
        assert_eq!(err.fields.len(), 1);
        assert_eq!(err.fields[0].field, "confidence");
    }

    #[test]
    fn test_describe() {
        let assertion =
            PronounReference::parse_assertion("-> \u{00A7}Tenant, confidence >= 0.8").unwrap();
        let desc = assertion.describe();
        assert!(desc.contains("PronounReference"));
    }

    #[test]
    fn test_constrained_fields() {
        let assertion =
            PronounReference::parse_assertion("-> \u{00A7}Tenant, pronoun_type=ThirdSingularNeuter")
                .unwrap();
        let fields = assertion.constrained_fields();
        assert!(fields.contains(&"candidates"));
        assert!(fields.contains(&"pronoun_type"));
    }
}
