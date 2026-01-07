//! Pipeline runner for executing fixtures through the resolver chain.

use crate::config::PipelineConfig;
use crate::context::DocumentContext;
use crate::fixture::NlpFixture;
use crate::matcher::{
    check_defined_term, check_obligation, check_pronoun, format_body, AssertionOutcome,
    MatchResult,
};
use layered_contracts::{
    ContractKeywordResolver, DefinedTerm, DefinedTermResolver, ObligationPhrase,
    ObligationPhraseResolver, ProhibitionResolver, PronounReference, PronounResolver, Scored,
    TermReferenceResolver,
};
use layered_nlp::{create_line_from_string, x};
use layered_part_of_speech::POSTagResolver;

/// Result of running a fixture through the pipeline.
#[derive(Debug, Default)]
pub struct PipelineResult {
    /// Collected obligation spans.
    pub obligations: Vec<(String, ObligationPhrase)>,
    /// Collected pronoun references.
    pub pronouns: Vec<(String, PronounReference)>,
    /// Collected defined terms.
    pub defined_terms: Vec<(String, Scored<DefinedTerm>)>,
}

impl PipelineResult {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Run a fixture through the pipeline and collect detected spans.
///
/// Processes the fixture content through the 7-resolver chain:
/// POSTagResolver -> ContractKeywordResolver -> ProhibitionResolver ->
/// DefinedTermResolver -> TermReferenceResolver -> PronounResolver ->
/// ObligationPhraseResolver
///
/// Returns spans for obligations, pronouns, and defined terms that can be
/// matched against fixture assertions.
pub fn run_fixture(fixture: &NlpFixture, _config: &PipelineConfig) -> PipelineResult {
    let mut result = PipelineResult::new();

    // Process each paragraph through the resolver chain
    for paragraph in &fixture.paragraphs {
        let content = &paragraph.text;

        // Run the resolver chain on the paragraph text
        let ll_line = create_line_from_string(content)
            .run(&POSTagResolver::default())
            .run(&ContractKeywordResolver::default())
            .run(&ProhibitionResolver::default())
            .run(&DefinedTermResolver::default())
            .run(&TermReferenceResolver::default())
            .run(&PronounResolver::default())
            .run(&ObligationPhraseResolver::default());

        // Extract obligations with action span expansion
        // Constructs span text as "modal + first_verb" (e.g., "shall pay")
        for (range, text, attrs_with_assocs) in
            ll_line.query_with_associations::<Scored<ObligationPhrase>>()
        {
            let _ = range; // Unused, but kept for potential debugging
            for (scored_obligation, _associations) in attrs_with_assocs {
                // Use the text directly from query_with_associations (already properly extracted)
                let modal_text = text.clone();

                // Get the first word from the action field
                let first_verb = scored_obligation
                    .value
                    .action
                    .split_whitespace()
                    .next()
                    .map(|s| s.to_string());

                // Construct span text: modal + first verb (e.g., "shall pay")
                let span_text = match first_verb {
                    Some(verb) => format!("{} {}", modal_text, verb),
                    None => modal_text,
                };

                result
                    .obligations
                    .push((span_text, scored_obligation.value.clone()));
            }
        }

        // Extract pronoun references with their span text
        for found in ll_line.find(&x::attr::<Scored<PronounReference>>()) {
            let (start, end) = found.range();
            let span_text = content.get(start..end).unwrap_or("").to_string();
            let scored = found.attr();
            result.pronouns.push((span_text, scored.value.clone()));
        }

        // Extract defined terms with their span text
        for found in ll_line.find(&x::attr::<Scored<DefinedTerm>>()) {
            let (start, end) = found.range();
            let span_text = content.get(start..end).unwrap_or("").to_string();
            let scored = found.attr();
            result.defined_terms.push((span_text, (*scored).clone()));
        }
    }

    result
}

/// Check all assertions in a fixture against detected spans.
///
/// This is the main integration point between the parser (Gate 1),
/// assertions (Gate 2), and the harness (Gate 3).
pub fn check_fixture_assertions(fixture: &NlpFixture, result: &PipelineResult) -> MatchResult {
    let ctx = DocumentContext::new(fixture);
    let mut match_result = MatchResult::new();

    for assertion in &fixture.assertions {
        // Resolve the target to get the expected span text
        let span_text = match ctx.resolve_ref_target(&assertion.target) {
            Some(text) => text,
            None => {
                match_result.add_not_found(
                    assertion.clone(),
                    format!("Failed to resolve target '{:?}'", assertion.target),
                );
                continue;
            }
        };

        // Format the assertion body as a string for checking
        let body_str = format_body(&assertion.body);

        // Dispatch based on span type
        let outcome = match assertion.span_type.as_str() {
            "Obligation" | "ObligationPhrase" => {
                // Find matching obligation in result
                if let Some((_, obl)) = result
                    .obligations
                    .iter()
                    .find(|(text, _)| text == &span_text)
                {
                    match check_obligation(obl, &body_str) {
                        Ok(()) => AssertionOutcome::Passed,
                        Err(mismatch) => AssertionOutcome::Failed(mismatch),
                    }
                } else {
                    AssertionOutcome::NotFound {
                        reason: format!("No ObligationPhrase found for '{}'", span_text),
                    }
                }
            }
            "PronounReference" | "PronounRef" => {
                if let Some((_, pr)) = result.pronouns.iter().find(|(text, _)| text == &span_text) {
                    match check_pronoun(pr, &body_str) {
                        Ok(()) => AssertionOutcome::Passed,
                        Err(mismatch) => AssertionOutcome::Failed(mismatch),
                    }
                } else {
                    AssertionOutcome::NotFound {
                        reason: format!("No PronounReference found for '{}'", span_text),
                    }
                }
            }
            "DefinedTerm" => {
                if let Some((_, scored)) = result
                    .defined_terms
                    .iter()
                    .find(|(text, _)| text == &span_text)
                {
                    match check_defined_term(&scored.value, &body_str) {
                        Ok(()) => AssertionOutcome::Passed,
                        Err(mismatch) => AssertionOutcome::Failed(mismatch),
                    }
                } else {
                    AssertionOutcome::NotFound {
                        reason: format!("No DefinedTerm found for '{}'", span_text),
                    }
                }
            }
            unknown => AssertionOutcome::TypeMismatch {
                expected: unknown.to_string(),
                actual: "unknown type".to_string(),
            },
        };

        match outcome {
            AssertionOutcome::Passed => {
                match_result.add_passed(assertion.clone(), span_text);
            }
            AssertionOutcome::Failed(mismatch) => {
                match_result.add_failed(assertion.clone(), span_text, mismatch);
            }
            AssertionOutcome::NotFound { reason } => {
                match_result.add_not_found(assertion.clone(), reason);
            }
            AssertionOutcome::TypeMismatch { expected, actual } => {
                match_result.add_type_mismatch(assertion.clone(), span_text, expected, actual);
            }
        }
    }

    match_result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load_all_fixtures;
    use crate::parse_fixture;
    use layered_contracts::{DefinitionType, ObligationType, ObligorReference};
    use std::path::Path;

    #[test]
    fn test_run_fixture_detects_duty() {
        let fixture = parse_fixture(
            r#"
# Test
The Company «1:shall pay» rent.
> [1]: Obligation(modal=shall)
"#,
        )
        .unwrap();
        let result = run_fixture(&fixture, &PipelineConfig::standard());
        // Should detect the obligation phrase
        assert!(
            !result.obligations.is_empty(),
            "Expected at least one obligation to be detected"
        );
        // Verify the obligation type is Duty (from "shall")
        let (_, obligation) = &result.obligations[0];
        assert_eq!(obligation.obligation_type, ObligationType::Duty);
    }

    #[test]
    fn test_run_fixture_detects_quoted_means() {
        let fixture = parse_fixture(
            r#"
# Test QuotedMeans
"Tenant" means any individual who leases property.
"#,
        )
        .unwrap();
        let result = run_fixture(&fixture, &PipelineConfig::standard());

        // Verify we detected a defined term
        assert!(
            !result.defined_terms.is_empty(),
            "Should detect defined term"
        );

        // Verify definition_type is QuotedMeans
        let (text, scored_term) = &result.defined_terms[0];
        assert!(
            text.contains("Tenant"),
            "Span text should contain 'Tenant', got: {}",
            text
        );
        assert_eq!(scored_term.value.definition_type, DefinitionType::QuotedMeans);
    }

    #[test]
    fn test_run_fixture_multi_paragraph() {
        let fixture = parse_fixture(
            r#"
# Test Multi-Paragraph
The Landlord shall maintain the property.
---
The Landlord shall also provide utilities.
"#,
        )
        .unwrap();
        let result = run_fixture(&fixture, &PipelineConfig::standard());

        // Both paragraphs should be processed
        // This tests that run_fixture iterates through all paragraphs
        assert!(
            result.obligations.len() >= 2,
            "Should detect obligations from multiple paragraphs, got: {}",
            result.obligations.len()
        );
    }

    #[test]
    fn test_check_fixture_with_injected_spans() {
        let fixture = parse_fixture(
            r#"
# Test
«1:shall pay» rent.
> [1]: Obligation(modal=shall)
"#,
        )
        .unwrap();

        // Inject a matching span
        let mut result = PipelineResult::new();
        result.obligations.push((
            "shall pay".to_string(),
            ObligationPhrase {
                obligor: ObligorReference::NounPhrase {
                    text: "The Tenant".to_string(),
                },
                obligation_type: ObligationType::Duty,
                action: "pay".to_string(),
                conditions: vec![],
            },
        ));

        let match_result = check_fixture_assertions(&fixture, &result);
        assert!(match_result.all_passed());
    }

    #[test]
    fn test_check_fixture_mismatch() {
        let fixture = parse_fixture(
            r#"
# Test
«1:may enter» the premises.
> [1]: Obligation(modal=shall)
"#,
        )
        .unwrap();

        // Inject a permission instead of duty
        let mut result = PipelineResult::new();
        result.obligations.push((
            "may enter".to_string(),
            ObligationPhrase {
                obligor: ObligorReference::NounPhrase {
                    text: "The Landlord".to_string(),
                },
                obligation_type: ObligationType::Permission,
                action: "enter".to_string(),
                conditions: vec![],
            },
        ));

        let match_result = check_fixture_assertions(&fixture, &result);
        assert!(!match_result.all_passed());
        assert_eq!(match_result.failed.len(), 1);
    }

    /// Integration test: load fixtures, run pipeline, check assertions.
    ///
    /// This validates the full pipeline works end-to-end:
    /// 1. Load real fixture files from disk
    /// 2. Run through the 7-resolver chain
    /// 3. Check assertions against detected spans
    ///
    /// Gate 2 requirement: At least 2 fixtures must pass all assertions.
    #[test]
    fn test_full_pipeline_integration() {
        let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let fixtures = load_all_fixtures(&fixtures_dir).unwrap();
        let config = PipelineConfig::standard();

        assert!(
            !fixtures.is_empty(),
            "No fixtures found in {}",
            fixtures_dir.display()
        );

        let mut passed = 0;
        let mut failed_fixtures = Vec::new();

        eprintln!("\n=== FIXTURE RESULTS ===\n");

        for (path, fixture) in &fixtures {
            let result = run_fixture(fixture, &config);
            let match_result = check_fixture_assertions(fixture, &result);

            // Extract fixture name from path string
            let fixture_name = path
                .rsplit('/')
                .next()
                .unwrap_or(path.as_str());
            let total_assertions = match_result.passed.len() + match_result.failed.len();

            // Check if all assertions passed for this fixture
            if match_result.all_passed() {
                passed += 1;
                eprintln!(
                    "[PASS] {} - {}/{} assertions passed",
                    fixture_name,
                    match_result.passed.len(),
                    total_assertions
                );
            } else {
                eprintln!(
                    "[FAIL] {} - {}/{} assertions passed",
                    fixture_name,
                    match_result.passed.len(),
                    total_assertions
                );

                // Print details about failures (all failures are in match_result.failed with different outcomes)
                for f in &match_result.failed {
                    match &f.outcome {
                        AssertionOutcome::Failed(mismatch) => {
                            eprintln!(
                                "       - Failed: span='{}' | {:?}",
                                f.span_text, mismatch
                            );
                        }
                        AssertionOutcome::NotFound { reason } => {
                            eprintln!(
                                "       - NotFound: {:?} | {}",
                                f.assertion.target, reason
                            );
                        }
                        AssertionOutcome::TypeMismatch { expected, actual } => {
                            eprintln!(
                                "       - TypeMismatch: expected={} actual={}",
                                expected, actual
                            );
                        }
                        AssertionOutcome::Passed => {}
                    }
                }

                failed_fixtures.push((
                    path.clone(),
                    match_result.failed.len(),
                    match_result
                        .failed
                        .iter()
                        .map(|f| format!("{:?}", f.outcome))
                        .collect::<Vec<_>>(),
                ));
            }
        }

        eprintln!("\n=== SUMMARY ===");
        eprintln!("Passed: {}/{}", passed, fixtures.len());
        eprintln!("Failed: {}/{}\n", fixtures.len() - passed, fixtures.len());

        // Gate 2 requires at least 2 fixtures pass end-to-end.
        // If this fails, it indicates span text matching needs refinement
        // (e.g., "shall" vs "shall pay" boundary differences).
        assert!(
            passed >= 2,
            "Expected at least 2 fixtures to pass, got {}.\n\
             Total fixtures: {}\n\
             Failed fixtures: {:?}",
            passed,
            fixtures.len(),
            failed_fixtures
        );
    }
}
