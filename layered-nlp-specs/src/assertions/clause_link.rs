//! Assertions for ClauseLink.

use crate::assertion::{
    AssertionMismatch, AssertionSpec, FieldMismatch, FieldValue, ParseError, SpanAssertion,
    TypeFieldCheck,
};
use layered_nlp_document::ClauseRole;

/// Minimal view of a clause link for assertions.
#[derive(Debug, Clone)]
pub struct ClauseLinkMatch {
    pub anchor_text: String,
    pub role: ClauseRole,
    pub target_text: String,
}

/// Parsed assertion for ClauseLink.
#[derive(Debug, Clone)]
pub struct ClauseLinkAssertion {
    pub checks: Vec<TypeFieldCheck>,
}

impl AssertionSpec for ClauseLinkAssertion {
    fn describe(&self) -> String {
        let fields: Vec<_> = self
            .checks
            .iter()
            .map(|c| format!("{}={:?}", c.field, c.value))
            .collect();
        format!("ClauseLink({})", fields.join(", "))
    }

    fn constrained_fields(&self) -> Vec<&'static str> {
        self.checks
            .iter()
            .map(|c| match c.field.as_str() {
                "role" => "role",
                "target" => "target",
                _ => "unknown",
            })
            .collect()
    }
}

impl SpanAssertion for ClauseLinkMatch {
    type Assertion = ClauseLinkAssertion;

    fn parse_assertion(input: &str) -> Result<Self::Assertion, ParseError> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(ClauseLinkAssertion { checks: Vec::new() });
        }

        let mut checks = Vec::new();
        for part in input.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            checks.push(TypeFieldCheck::parse(part)?);
        }

        Ok(ClauseLinkAssertion { checks })
    }

    fn check(&self, assertion: &Self::Assertion) -> Result<(), AssertionMismatch> {
        let mut mismatch = AssertionMismatch::new(&self.anchor_text, assertion.describe());

        for check in &assertion.checks {
            match check.field.as_str() {
                "role" => {
                    if let FieldValue::String(expected) = &check.value {
                        match parse_clause_role(expected) {
                            Some(role) => {
                                if self.role != role {
                                    mismatch.fields.push(FieldMismatch::hard(
                                        "role",
                                        expected.clone(),
                                        format!("{:?}", self.role),
                                    ));
                                }
                            }
                            None => mismatch.fields.push(FieldMismatch::hard(
                                "role",
                                expected.clone(),
                                "unknown role".to_string(),
                            )),
                        }
                    }
                }
                "target" => {
                    if let FieldValue::String(expected) = &check.value {
                        if &self.target_text != expected {
                            mismatch.fields.push(FieldMismatch::hard(
                                "target",
                                expected.clone(),
                                self.target_text.clone(),
                            ));
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
        "ClauseLink"
    }
}

fn parse_clause_role(input: &str) -> Option<ClauseRole> {
    let normalized = input
        .trim()
        .to_lowercase()
        .replace('-', "_")
        .replace(' ', "_");

    match normalized.as_str() {
        "parent" => Some(ClauseRole::Parent),
        "child" => Some(ClauseRole::Child),
        "conjunct" => Some(ClauseRole::Conjunct),
        "exception" => Some(ClauseRole::Exception),
        "listitem" | "list_item" => Some(ClauseRole::ListItem),
        "listcontainer" | "list_container" => Some(ClauseRole::ListContainer),
        "crossreference" | "cross_reference" => Some(ClauseRole::CrossReference),
        "self" | "self_" => Some(ClauseRole::Self_),
        "relative" => Some(ClauseRole::Relative),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_assertion() {
        let assertion = ClauseLinkMatch::parse_assertion("").unwrap();
        assert!(assertion.checks.is_empty());
    }

    #[test]
    fn test_parse_role() {
        let assertion = ClauseLinkMatch::parse_assertion("role=parent").unwrap();
        assert_eq!(assertion.checks.len(), 1);
        assert_eq!(assertion.checks[0].field, "role");
    }

    #[test]
    fn test_check_role_match() {
        let link = ClauseLinkMatch {
            anchor_text: "A".to_string(),
            role: ClauseRole::Parent,
            target_text: "B".to_string(),
        };
        let assertion = ClauseLinkMatch::parse_assertion("role=parent").unwrap();
        assert!(link.check(&assertion).is_ok());
    }

    #[test]
    fn test_check_target_match() {
        let link = ClauseLinkMatch {
            anchor_text: "A".to_string(),
            role: ClauseRole::Conjunct,
            target_text: "Target".to_string(),
        };
        let assertion = ClauseLinkMatch::parse_assertion("target=Target").unwrap();
        assert!(link.check(&assertion).is_ok());
    }
}
