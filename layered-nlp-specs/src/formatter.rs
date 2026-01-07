//! Rich error formatting for assertion failures.

use crate::assertion::MismatchSeverity;
use crate::fixture::Assertion;
use crate::matcher::AssertionOutcome;
use std::fmt::Write;

/// Result of a single assertion check (for formatting purposes).
pub struct FormattableResult<'a> {
    pub assertion: &'a Assertion,
    pub span_text: &'a str,
    pub outcome: &'a AssertionOutcome,
}

/// Format a failed assertion with rich context.
pub fn format_failure(
    fixture_name: &str,
    assertion: &Assertion,
    span_text: &str,
    outcome: &AssertionOutcome,
    text_context: Option<&str>,
) -> String {
    let mut output = String::new();

    // Header
    writeln!(output, "\nFAIL: {}:{}", fixture_name, assertion.source_line).unwrap();
    writeln!(output).unwrap();

    // Text context with underline
    if let Some(context) = text_context {
        writeln!(output, "  {}", context).unwrap();
        if !span_text.is_empty() {
            // Underline the span text if we can find it
            if let Some(pos) = context.find(span_text) {
                let spaces = " ".repeat(pos + 2);
                let carets = "^".repeat(span_text.len());
                writeln!(output, "{}{}", spaces, carets).unwrap();
            }
        }
        writeln!(output).unwrap();
    }

    // Main failure message
    match outcome {
        AssertionOutcome::Failed(mismatch) => {
            writeln!(
                output,
                "  assertion failed for {} span: \"{}\"",
                assertion.span_type, span_text
            )
            .unwrap();

            // Field-level details
            for field in &mismatch.fields {
                let marker = match field.severity {
                    MismatchSeverity::Hard => "\u{2717}", // cross mark
                    MismatchSeverity::Soft => "~",
                    MismatchSeverity::Info => "\u{2139}", // info
                };
                writeln!(
                    output,
                    "    {} {}: expected `{}`, found `{}`",
                    marker, field.field, field.expected, field.actual
                )
                .unwrap();
            }
        }
        AssertionOutcome::NotFound { reason } => {
            writeln!(output, "  span not found: {}", reason).unwrap();
        }
        AssertionOutcome::TypeMismatch { expected, actual } => {
            writeln!(
                output,
                "  type mismatch: expected {}, found {}",
                expected, actual
            )
            .unwrap();
        }
        AssertionOutcome::Passed => {
            // Should not happen in format_failure
            writeln!(output, "  (passed)").unwrap();
        }
    }

    // Assertion source
    writeln!(output).unwrap();
    writeln!(
        output,
        "  Assertion was: {}({})",
        assertion.span_type,
        format_assertion_body(assertion)
    )
    .unwrap();

    // Hints based on patterns
    if let Some(hint) = generate_hint(assertion, span_text, outcome) {
        writeln!(output).unwrap();
        writeln!(output, "  hint: {}", hint).unwrap();
    }

    output
}

/// Format a summary of all results.
pub fn format_summary(
    fixture_name: &str,
    passed: usize,
    failed: usize,
    expected_failures: usize,
    regressions: usize,
) -> String {
    let mut output = String::new();

    let status = if regressions > 0 { "FAIL" } else { "PASS" };

    writeln!(output, "\n{}: {}", status, fixture_name).unwrap();
    writeln!(
        output,
        "  {} passed, {} failed ({} expected, {} regressions)",
        passed, failed, expected_failures, regressions
    )
    .unwrap();

    output
}

fn format_assertion_body(assertion: &Assertion) -> String {
    assertion
        .body
        .field_checks
        .iter()
        .map(|fc| format!("{}{}{}", fc.field, fc.operator, fc.expected))
        .collect::<Vec<_>>()
        .join(", ")
}

fn generate_hint(assertion: &Assertion, span_text: &str, outcome: &AssertionOutcome) -> Option<String> {
    // Generate contextual hints based on mismatch patterns
    if let AssertionOutcome::Failed(mismatch) = outcome {
        for field in &mismatch.fields {
            // Hint for modal mismatches
            if field.field == "modal"
                && field.actual.contains("Permission")
                && field.expected == "shall"
            {
                return Some(
                    "'may' indicates permission, not obligation - consider using modal=may"
                        .to_string(),
                );
            }
            if field.field == "modal" && span_text.contains("fail") {
                return Some(
                    "'fails to' may be a negative condition, not an obligation".to_string(),
                );
            }
            // Hint for confidence threshold
            if field.field == "confidence" {
                return Some(
                    "Confidence thresholds are soft failures - the span was detected but below threshold"
                        .to_string(),
                );
            }
        }
    }

    if let AssertionOutcome::NotFound { .. } = outcome {
        return Some(format!(
            "No {} was detected at this position - check the resolver is enabled",
            assertion.span_type
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assertion::{AssertionMismatch, FieldMismatch};
    use crate::fixture::{AssertionBody, RefTarget};

    fn make_assertion() -> Assertion {
        Assertion {
            target: RefTarget::Span(1),
            span_type: "Obligation".to_string(),
            body: AssertionBody::default(),
            source_line: 5,
        }
    }

    #[test]
    fn test_format_failure_not_found() {
        let assertion = make_assertion();
        let outcome = AssertionOutcome::NotFound {
            reason: "No ObligationPhrase found".to_string(),
        };

        let output = format_failure(
            "test.nlp",
            &assertion,
            "shall pay",
            &outcome,
            Some("The tenant shall pay rent."),
        );
        assert!(output.contains("FAIL: test.nlp:5"));
        assert!(output.contains("span not found"));
        assert!(output.contains("hint:"));
    }

    #[test]
    fn test_format_failure_with_mismatch() {
        let assertion = make_assertion();
        let mismatch = AssertionMismatch {
            span_text: "may enter".to_string(),
            assertion_source: "Obligation(modal=shall)".to_string(),
            fields: vec![FieldMismatch::hard("modal", "shall", "Permission")],
        };
        let outcome = AssertionOutcome::Failed(mismatch);

        let output = format_failure("test.nlp", &assertion, "may enter", &outcome, None);
        assert!(output.contains("modal"));
        assert!(output.contains("expected `shall`"));
    }

    #[test]
    fn test_format_failure_with_text_context() {
        let assertion = make_assertion();
        let outcome = AssertionOutcome::NotFound {
            reason: "No ObligationPhrase found".to_string(),
        };

        let output = format_failure(
            "test.nlp",
            &assertion,
            "shall pay",
            &outcome,
            Some("The tenant shall pay rent."),
        );
        // Should have underline carets
        assert!(output.contains("^^^"));
    }

    #[test]
    fn test_format_summary_pass() {
        let output = format_summary("contract.nlp", 10, 0, 0, 0);
        assert!(output.contains("PASS: contract.nlp"));
        assert!(output.contains("10 passed"));
    }

    #[test]
    fn test_format_summary_with_regressions() {
        let output = format_summary("contract.nlp", 10, 2, 1, 1);
        assert!(output.contains("FAIL: contract.nlp"));
        assert!(output.contains("10 passed"));
        assert!(output.contains("1 regressions"));
    }

    #[test]
    fn test_format_type_mismatch() {
        let assertion = make_assertion();
        let outcome = AssertionOutcome::TypeMismatch {
            expected: "Obligation".to_string(),
            actual: "DefinedTerm".to_string(),
        };

        let output = format_failure("test.nlp", &assertion, "the term", &outcome, None);
        assert!(output.contains("type mismatch"));
        assert!(output.contains("expected Obligation"));
        assert!(output.contains("found DefinedTerm"));
    }

    #[test]
    fn test_hint_for_modal_permission() {
        let assertion = make_assertion();
        let mismatch = AssertionMismatch {
            span_text: "may enter".to_string(),
            assertion_source: "Obligation(modal=shall)".to_string(),
            fields: vec![FieldMismatch::hard("modal", "shall", "Permission")],
        };
        let outcome = AssertionOutcome::Failed(mismatch);

        let output = format_failure("test.nlp", &assertion, "may enter", &outcome, None);
        assert!(output.contains("hint:"));
        assert!(output.contains("'may' indicates permission"));
    }

    #[test]
    fn test_hint_for_fails_to() {
        let assertion = make_assertion();
        let mismatch = AssertionMismatch {
            span_text: "fails to pay".to_string(),
            assertion_source: "Obligation(modal=shall)".to_string(),
            fields: vec![FieldMismatch::hard("modal", "shall", "implicit")],
        };
        let outcome = AssertionOutcome::Failed(mismatch);

        let output = format_failure("test.nlp", &assertion, "fails to pay", &outcome, None);
        assert!(output.contains("hint:"));
        assert!(output.contains("negative condition"));
    }
}
