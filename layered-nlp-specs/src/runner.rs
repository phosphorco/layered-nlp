//! Pipeline runner for executing fixtures through the resolver chain.

use crate::config::PipelineConfig;
use crate::context::DocumentContext;
use crate::fixture::NlpFixture;
use crate::matcher::{
    check_clause, check_clause_link, check_defined_term, check_obligation, check_pronoun,
    check_term_reference, format_body, AssertionOutcome, MatchResult,
};
use crate::assertions::ClauseLinkMatch;
use layered_clauses::{Clause, ClauseKeywordResolver, ClauseLinkResolver, ClauseResolver, SentenceBoundaryResolver};
use layered_contracts::{
    ContractKeywordResolver, DefinedTerm, DefinedTermResolver, ObligationPhrase,
    ObligationPhraseResolver, ProhibitionResolver, PronounReference, PronounResolver, Scored,
    SectionReferenceResolver, TermReference, TermReferenceResolver,
};
use layered_nlp::{create_line_from_string, x};
use layered_part_of_speech::POSTagResolver;
use layered_nlp_document::LayeredDocument;

/// Result of running a fixture through the pipeline.
#[derive(Debug, Default)]
pub struct PipelineResult {
    /// Collected obligation spans.
    pub obligations: Vec<(usize, String, ObligationPhrase)>,
    /// Collected pronoun references.
    pub pronouns: Vec<(usize, String, PronounReference)>,
    /// Collected defined terms.
    pub defined_terms: Vec<(usize, String, Scored<DefinedTerm>)>,
    /// Collected term references.
    pub term_references: Vec<(usize, String, Scored<TermReference>)>,
    /// Collected clause spans.
    pub clauses: Vec<(usize, String, Clause)>,
    /// Collected clause links.
    pub clause_links: Vec<(usize, String, ClauseLinkMatch)>,
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

    let normalized_text = fixture.normalized_text();
    let line_texts: Vec<&str> = normalized_text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();

    let mut line_to_paragraph = Vec::new();
    for paragraph in &fixture.paragraphs {
        for line in paragraph.text.lines() {
            if !line.trim().is_empty() {
                line_to_paragraph.push(paragraph.index);
            }
        }
    }

    // Process each paragraph through the resolver chain
    for paragraph in &fixture.paragraphs {
        let content = &paragraph.text;
        let paragraph_idx = paragraph.index;

        // Run the resolver chain on the paragraph text
        let ll_line = create_line_from_string(content)
            .run(&POSTagResolver::default())
            .run(&ContractKeywordResolver::default())
            .run(&ProhibitionResolver::default())
            .run(&DefinedTermResolver::default())
            .run(&TermReferenceResolver::default())
            .run(&PronounResolver::default())
            .run(&ObligationPhraseResolver::default())
            .run(&ClauseKeywordResolver::new(
                &["if", "when"],
                &["and"],
                &["then"],
                &["or"],
                &["but", "however"],
                &["nor"],
            ))
            .run(&ClauseResolver::default());

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
                    .push((paragraph_idx, span_text, scored_obligation.value.clone()));
            }
        }

        // Extract pronoun references with their span text
        for found in ll_line.find(&x::attr::<Scored<PronounReference>>()) {
            let (start, end) = found.range();
            let span_text = content.get(start..end).unwrap_or("").to_string();
            let scored = found.attr();
            result
                .pronouns
                .push((paragraph_idx, span_text, scored.value.clone()));
        }

        // Extract defined terms with their span text
        for found in ll_line.find(&x::attr::<Scored<DefinedTerm>>()) {
            let scored = found.attr();

            // Use the term_name as the span text for matching, since that's what fixtures expect
            result
                .defined_terms
                .push((paragraph_idx, scored.value.term_name.clone(), (*scored).clone()));
        }

        // Extract term references with their span text
        for found in ll_line.find(&x::attr::<Scored<TermReference>>()) {
            let (start, end) = found.range();
            let span_text = content.get(start..end).unwrap_or("").to_string();
            let scored = found.attr();
            result
                .term_references
                .push((paragraph_idx, span_text, (*scored).clone()));
        }

        // Extract clauses with their span text
        for found in ll_line.find(&x::attr::<Clause>()) {
            let (start, end) = found.range();
            let span_text = content.get(start..end).unwrap_or("").to_string();
            let clause = (*found.attr()).clone();
            result
                .clauses
                .push((paragraph_idx, span_text, clause));
        }
    }

    // Document-level clause links
    let doc = LayeredDocument::from_text(&normalized_text)
        .run_resolver(&POSTagResolver::default())
        .run_resolver(&ContractKeywordResolver::default())
        .run_resolver(&ProhibitionResolver::default())
        .run_resolver(&ObligationPhraseResolver::default())
        .run_resolver(&SentenceBoundaryResolver::new())
        .run_resolver(&SectionReferenceResolver::new())
        .run_resolver(&ClauseKeywordResolver::new(
            &["if", "when"],
            &["and"],
            &["then"],
            &["or"],
            &["but", "however"],
            &["nor"],
        ))
        .run_resolver(&ClauseResolver::default());

    let (_doc_with_markers, links) = ClauseLinkResolver::resolve_with_list_markers(doc);

    for link in links {
        let anchor_text = span_text_for_docspan(&line_texts, &link.anchor);
        let target_text = span_text_for_docspan(&line_texts, &link.link.target);
        if anchor_text.is_empty() || target_text.is_empty() {
            continue;
        }

        let paragraph_idx = line_to_paragraph
            .get(link.anchor.start.line)
            .copied()
            .unwrap_or(0);

        let anchor_text_for_tuple = anchor_text.clone();
        result.clause_links.push((
            paragraph_idx,
            anchor_text_for_tuple,
            ClauseLinkMatch {
                anchor_text,
                role: link.link.role,
                target_text,
            },
        ));
    }

    result
}

fn span_text_for_docspan(line_texts: &[&str], span: &layered_nlp_document::DocSpan) -> String {
    if span.start.line != span.end.line {
        return String::new();
    }
    let line_idx = span.start.line;
    let line_text = line_texts.get(line_idx).copied().unwrap_or("");
    let start = span.start.token;
    let end = span.end.token;
    if start >= end || end > line_text.len() {
        return String::new();
    }
    line_text.get(start..end).unwrap_or("").to_string()
}

/// Check all assertions in a fixture against detected spans.
///
/// This is the main integration point between the parser (Gate 1),
/// assertions (Gate 2), and the harness (Gate 3).
pub fn check_fixture_assertions(fixture: &NlpFixture, result: &PipelineResult) -> MatchResult {
    let ctx = DocumentContext::new(fixture);
    let mut match_result = MatchResult::new();

    for assertion in &fixture.assertions {
        let match_paragraph_idx = match &assertion.target {
            crate::fixture::RefTarget::Span(id) => {
                fixture.span_by_numeric_id(*id).map(|(para, _)| para.index)
            }
            _ => None,
        };

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

        let matches_text_and_para = |paragraph_idx: usize, text: &str| {
            if let Some(target_idx) = match_paragraph_idx {
                paragraph_idx == target_idx && text == &span_text
            } else {
                text == &span_text
            }
        };

        // Dispatch based on span type
        let outcome = match assertion.span_type.as_str() {
            "Obligation" | "ObligationPhrase" => {
                // Find matching obligation in result
                if let Some((_, _, obl)) = result
                    .obligations
                    .iter()
                    .find(|(paragraph_idx, text, _)| matches_text_and_para(*paragraph_idx, text))
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
                if let Some((_, _, pr)) = result
                    .pronouns
                    .iter()
                    .find(|(paragraph_idx, text, _)| matches_text_and_para(*paragraph_idx, text))
                {
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
                if let Some((_, _, scored)) = result
                    .defined_terms
                    .iter()
                    .find(|(paragraph_idx, text, _)| matches_text_and_para(*paragraph_idx, text))
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
            "TermReference" => {
                if let Some((_, _, scored)) = result
                    .term_references
                    .iter()
                    .find(|(paragraph_idx, text, _)| matches_text_and_para(*paragraph_idx, text))
                {
                    match check_term_reference(&scored.value, &body_str) {
                        Ok(()) => AssertionOutcome::Passed,
                        Err(mismatch) => AssertionOutcome::Failed(mismatch),
                    }
                } else {
                    AssertionOutcome::NotFound {
                        reason: format!("No TermReference found for '{}'", span_text),
                    }
                }
            }
            "Clause" => {
                if let Some((_, _, clause)) = result
                    .clauses
                    .iter()
                    .find(|(paragraph_idx, text, _)| matches_text_and_para(*paragraph_idx, text))
                {
                    match check_clause(clause, &body_str) {
                        Ok(()) => AssertionOutcome::Passed,
                        Err(mismatch) => AssertionOutcome::Failed(mismatch),
                    }
                } else {
                    AssertionOutcome::NotFound {
                        reason: format!("No Clause found for '{}'", span_text),
                    }
                }
            }
            "ClauseLink" => {
                let candidates: Vec<_> = result
                    .clause_links
                    .iter()
                    .filter(|(paragraph_idx, text, _)| matches_text_and_para(*paragraph_idx, text))
                    .collect();

                if candidates.is_empty() {
                    AssertionOutcome::NotFound {
                        reason: format!("No ClauseLink found for '{}'", span_text),
                    }
                } else {
                    let mut first_mismatch = None;
                    let mut passed = false;

                    for (_, _, clause_link) in candidates {
                        match check_clause_link(clause_link, &body_str) {
                            Ok(()) => {
                                passed = true;
                                break;
                            }
                            Err(mismatch) => {
                                if first_mismatch.is_none() {
                                    first_mismatch = Some(mismatch);
                                }
                            }
                        }
                    }

                    if passed {
                        AssertionOutcome::Passed
                    } else {
                        AssertionOutcome::Failed(
                            first_mismatch.expect("ClauseLink candidates should yield mismatches"),
                        )
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
    use layered_clauses::Clause;
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
        let (_, _, obligation) = &result.obligations[0];
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
        let (_, text, scored_term) = &result.defined_terms[0];
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
    fn test_run_fixture_detects_clauses() {
        let fixture = parse_fixture(
            r#"
# Test Clauses
If payment is late, then the Company shall charge a fee.
"#,
        )
        .unwrap();

        let result = run_fixture(&fixture, &PipelineConfig::standard());
        assert!(
            result.clauses.len() >= 2,
            "Expected at least 2 clauses, got {}",
            result.clauses.len()
        );

        let clause_types: Vec<_> = result.clauses.iter().map(|(_, _, c)| c).collect();
        assert!(
            clause_types.contains(&&Clause::Condition),
            "Expected a Condition clause"
        );
        assert!(
            clause_types.contains(&&Clause::TrailingEffect),
            "Expected a TrailingEffect clause"
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
            0,
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
            0,
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
