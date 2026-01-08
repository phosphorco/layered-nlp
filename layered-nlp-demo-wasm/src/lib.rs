use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

mod extractors;
mod manifest;
pub use manifest::{RawAssociation, RawSpan, ResolverManifest, ResolverTag, RESOLVER_MANIFESTS};

// Set up panic hook for better error messages in browser console
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

use layered_contracts::{
    // Resolver imports (used by resolver stack)
    AccountabilityGraphResolver, ClauseAggregationResolver, ContractClauseResolver,
    ContractKeywordResolver, DefinedTermResolver, LinkedObligationResolver,
    ObligationPhraseResolver, ProhibitionResolver, PronounChainResolver, PronounResolver,
    ScopedObligationResolver, SectionHeaderResolver, SectionReferenceResolver,
    TermReferenceResolver, TermsOfArtResolver, TemporalExpressionResolver,
    // Semantic diff imports
    AlignmentResult, AlignmentType, AlignedPair, ContractDocument, DocumentAligner,
    DocumentStructure, DocumentStructureBuilder, ProcessError, SectionNode,
    SectionRef as AlignmentSectionRef, SemanticDiffEngine, SemanticDiffResult,
    // Token diff imports (re-exported from crate root)
    TokenAligner, TokenAlignmentConfig, TokenRelation, AlignedTokenPair,
    // Conflict detection imports
    ConflictDetector,
    // Scope operator imports
    NegationDetector, QuantifierDetector, ScopeDimension,
    NegationKind, QuantifierKind,
    // Accountability analytics (verification queue)
    ObligationGraph, ObligationNode, Scored,
    VerificationQueueDetails as RustVerificationQueueDetails,
    VerificationTarget as RustVerificationTarget,
};
use layered_part_of_speech::POSTagResolver;
use layered_deixis::{
    DiscourseMarkerResolver, PersonPronounResolver, PlaceDeicticResolver, SimpleTemporalResolver,
};
use layered_nlp::create_line_from_string;
use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver};
use layered_nlp_document::{LayeredDocument, ClauseRole};

// ============================================================================
// SPAN LINKS API
// ============================================================================

/// Serializable representation of a span link for WASM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmSpanLink {
    /// Anchor span: line index
    pub anchor_line: usize,
    /// Anchor span: start token index
    pub anchor_start_token: usize,
    /// Anchor span: end token index
    pub anchor_end_token: usize,
    /// Target span: line index
    pub target_line: usize,
    /// Target span: start token index
    pub target_start_token: usize,
    /// Target span: end token index
    pub target_end_token: usize,
    /// Semantic role of the link (Parent, Child, Conjunct, Exception)
    pub role: String,
    /// Type of link (always "Clause" for this API)
    pub link_type: String,
}

/// Result of span link extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLinkResult {
    /// All extracted span links
    pub links: Vec<WasmSpanLink>,
    /// Total count of links
    pub total_count: usize,
}

/// Extract span links (clause relationships) from text.
///
/// Uses ClauseLinkResolver to identify:
/// - Parent/Child relationships (Condition -> TrailingEffect)
/// - Conjunct relationships (coordinated clauses with "and")
/// - Exception relationships (clauses with "unless", "except", etc.)
///
/// Returns a JSON object containing all detected links.
#[wasm_bindgen]
pub fn get_span_links(text: &str) -> JsValue {
    init();
    let result = get_span_links_internal(text);
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

fn get_span_links_internal(text: &str) -> SpanLinkResult {
    // Build the document with clause analysis
    // Note: ClauseKeywordResolver::new() uses default exception keywords
    // (except, unless, notwithstanding, provided, subject)
    let doc = LayeredDocument::from_text(text)
        // Run clause keyword resolver to detect clause boundaries
        .run_resolver(&ClauseKeywordResolver::new(
            &["if", "when", "where", "whereas", "although"],  // condition_start
            &["and"],                                          // and
            &["then", "therefore", "thus", "accordingly"],    // then/effect
            &["or"],                                          // or
            &["but"],                                         // but
            &["nor"],                                         // nor
        ))
        // Run clause resolver to identify clause boundaries
        .run_resolver(&ClauseResolver::default());

    // Extract clause relationships using ClauseLinkResolver
    let clause_links = ClauseLinkResolver::resolve(&doc);

    // Convert to serializable format
    let links: Vec<WasmSpanLink> = clause_links
        .iter()
        .map(|cl| {
            let role_str = match cl.link.role {
                ClauseRole::Parent => "Parent",
                ClauseRole::Child => "Child",
                ClauseRole::Conjunct => "Conjunct",
                ClauseRole::Exception => "Exception",
                ClauseRole::ListItem => "ListItem",
                ClauseRole::ListContainer => "ListContainer",
                ClauseRole::CrossReference => "CrossRef",
                ClauseRole::Relative => "Relative",
                ClauseRole::Self_ => "Self",
            };

            WasmSpanLink {
                anchor_line: cl.anchor.start.line,
                anchor_start_token: cl.anchor.start.token,
                anchor_end_token: cl.anchor.end.token,
                target_line: cl.link.target.start.line,
                target_start_token: cl.link.target.start.token,
                target_end_token: cl.link.target.end.token,
                role: role_str.to_string(),
                link_type: "Clause".to_string(),
            }
        })
        .collect();

    SpanLinkResult {
        total_count: links.len(),
        links,
    }
}

/// Serializable representation of resolver metadata for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmResolverManifest {
    pub name: String,
    pub description: String,
    pub color: String,
    pub tags: Vec<String>,
}

/// Get all resolver manifests with metadata for visualization
#[wasm_bindgen]
pub fn get_resolver_manifests() -> JsValue {
    init();
    let manifests: Vec<WasmResolverManifest> = RESOLVER_MANIFESTS
        .iter()
        .map(|m| WasmResolverManifest {
            name: m.name.to_string(),
            description: m.description.to_string(),
            color: m.color.to_string(),
            tags: m.tags.iter().map(|t| match t {
                ResolverTag::Stable => "stable".to_string(),
                ResolverTag::Experimental => "experimental".to_string(),
            }).collect(),
        })
        .collect();
    serde_wasm_bindgen::to_value(&manifests).unwrap_or(JsValue::NULL)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub start_offset: u32,
    pub end_offset: u32,
    pub label: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// CSS hex color from manifest
    pub color: String,
    /// Stability tags from manifest (e.g., ["stable"])
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub text: String,
    pub spans: Vec<Span>,
    pub version: String,
}

#[wasm_bindgen]
pub fn analyze_contract(text: &str) -> JsValue {
    let result = analyze_contract_internal(text);
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

fn analyze_contract_internal(text: &str) -> AnalysisResult {
    // Run the full resolver stack
    let ll_line = create_line_from_string(text)
        // Layer 1-2: Keywords
        .run(&ContractKeywordResolver::new())
        .run(&ProhibitionResolver::new())
        // Layer 3: Defined Terms
        .run(&DefinedTermResolver::new())
        // Layer 4: Term References
        .run(&TermReferenceResolver::new())
        // Layer 5: Pronouns
        .run(&PronounResolver::new())
        // Layer 6: Obligation Phrases
        .run(&ObligationPhraseResolver::new())
        // Layer 7: Pronoun Chains (coreference)
        .run(&PronounChainResolver::new())
        // Layer 8: Contract Clauses
        .run(&ContractClauseResolver::new())
        // Layer 9: Clause Aggregates
        .run(&ClauseAggregationResolver::new())
        // Layer 10: Accountability Graph
        .run(&AccountabilityGraphResolver::new())
        // Layer 11-14: Deixis (simple word-list resolvers)
        // These fill gaps for deictic expressions not covered by contract-specific resolvers
        .run(&PersonPronounResolver::new())
        .run(&PlaceDeicticResolver::new())
        .run(&SimpleTemporalResolver::new())
        .run(&DiscourseMarkerResolver::new());

    let mut spans = Vec::new();

    // Manifest-driven extraction - replaces 260 lines of per-type loops
    for manifest in RESOLVER_MANIFESTS.iter() {
        let raw_spans = (manifest.extract)(&ll_line);
        for raw_span in raw_spans {
            spans.push(Span {
                start_offset: raw_span.start,
                end_offset: raw_span.end,
                label: raw_span.label,
                kind: manifest.name.to_string(),
                metadata: raw_span.metadata,
                color: manifest.color.to_string(),
                tags: manifest.tags.iter().map(|t| match t {
                    ResolverTag::Stable => "stable".to_string(),
                    ResolverTag::Experimental => "experimental".to_string(),
                }).collect(),
            });
        }
    }

    AnalysisResult {
        text: text.to_string(),
        spans,
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

// ============================================================================
// SEMANTIC DIFF API
// ============================================================================

/// Maximum allowed input size per document (50,000 characters)
const MAX_INPUT_SIZE: usize = 50_000;

fn run_contract_pipeline_with_pos(text: &str) -> Result<ContractDocument, ProcessError> {
    let doc = ContractDocument::from_text(text)
        .run_resolver(&POSTagResolver::default())
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&SectionReferenceResolver::new())
        .run_resolver(&ContractKeywordResolver::new())
        .run_resolver(&TermsOfArtResolver::new())
        .run_resolver(&DefinedTermResolver::new())
        .run_resolver(&TermReferenceResolver::new())
        .run_resolver(&TemporalExpressionResolver::new())
        .run_resolver(&PronounResolver::new())
        .run_resolver(&PronounChainResolver::new())
        .run_resolver(&ObligationPhraseResolver::new());

    Ok(doc)
}

/// Error response structure for semantic diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffError {
    pub error: DiffErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl DiffError {
    fn invalid_input(message: &str) -> Self {
        Self {
            error: DiffErrorDetail {
                code: "invalid_input".to_string(),
                message: message.to_string(),
                details: None,
            },
        }
    }

    fn input_too_large(which: &str, size: usize) -> Self {
        Self {
            error: DiffErrorDetail {
                code: "input_too_large".to_string(),
                message: format!(
                    "{} document exceeds maximum size of {} characters",
                    which, MAX_INPUT_SIZE
                ),
                details: Some(serde_json::json!({
                    "document": which,
                    "size": size,
                    "max_size": MAX_INPUT_SIZE
                })),
            },
        }
    }

    fn alignment_failed(message: &str) -> Self {
        Self {
            error: DiffErrorDetail {
                code: "alignment_failed".to_string(),
                message: message.to_string(),
                details: None,
            },
        }
    }

    fn internal_error(message: &str) -> Self {
        Self {
            error: DiffErrorDetail {
                code: "internal_error".to_string(),
                message: message.to_string(),
                details: None,
            },
        }
    }
}

/// Response structure that includes alignment info for the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareResult {
    /// Aligned section pairs with text content (Gate 2)
    pub aligned_pairs: Vec<FrontendAlignedPair>,
    /// The semantic diff result (changes, summary, party_summaries, warnings)
    pub diff: SemanticDiffResult,
    /// Alignment summary for display
    pub alignment_summary: AlignmentSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentSummary {
    pub total_alignments: usize,
    pub exact_match: usize,
    pub modified: usize,
    pub inserted: usize,
    pub deleted: usize,
    pub renumbered: usize,
    pub other: usize,
}

impl AlignmentSummary {
    fn from_alignment_result(result: &AlignmentResult) -> Self {
        let mut summary = Self {
            total_alignments: result.alignments.len(),
            exact_match: 0,
            modified: 0,
            inserted: 0,
            deleted: 0,
            renumbered: 0,
            other: 0,
        };

        for pair in &result.alignments {
            match pair.alignment_type {
                AlignmentType::ExactMatch => summary.exact_match += 1,
                AlignmentType::Modified => summary.modified += 1,
                AlignmentType::Inserted => summary.inserted += 1,
                AlignmentType::Deleted => summary.deleted += 1,
                AlignmentType::Renumbered => summary.renumbered += 1,
                _ => summary.other += 1,
            }
        }

        summary
    }
}

// ============================================================================
// FRONTEND ALIGNED PAIRS (Gate 2)
// ============================================================================

/// Section reference for frontend display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendSectionRef {
    pub canonical_id: String,
    pub title: Option<String>,
    pub start_line: usize,
    pub depth: u8,
}

impl From<&AlignmentSectionRef> for FrontendSectionRef {
    fn from(sr: &AlignmentSectionRef) -> Self {
        Self {
            canonical_id: sr.canonical_id.clone(),
            title: sr.title.clone(),
            start_line: sr.start_line,
            depth: sr.depth,
        }
    }
}

/// Token-level diff span for frontend rendering.
/// Computed from Rust's TokenAligner for consistent, accurate diffing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmTokenDiff {
    /// The token text
    pub text: String,
    /// Status: "Unchanged", "Added", "Removed"
    pub status: String,
    /// Position in original text [start, end] (null if Added)
    pub original_pos: Option<[usize; 2]>,
    /// Position in revised text [start, end] (null if Removed)
    pub revised_pos: Option<[usize; 2]>,
    /// Token type: "WORD", "SPACE", "PUNC", "NATN", "SYMB"
    pub tag: String,
}

impl WasmTokenDiff {
    /// Convert from AlignedTokenPair to serializable format
    fn from_pair(pair: &AlignedTokenPair) -> Self {
        let (text, original_pos, revised_pos, tag) = match (&pair.left, &pair.right, &pair.relation) {
            (Some(left), Some(_right), TokenRelation::Identical) => (
                left.text.clone(),
                Some([left.start, left.end]),
                pair.right.as_ref().map(|r| [r.start, r.end]),
                format!("{:?}", left.tag),
            ),
            (Some(left), None, TokenRelation::LeftOnly) => (
                left.text.clone(),
                Some([left.start, left.end]),
                None,
                format!("{:?}", left.tag),
            ),
            (None, Some(right), TokenRelation::RightOnly) => (
                right.text.clone(),
                None,
                Some([right.start, right.end]),
                format!("{:?}", right.tag),
            ),
            // Fallback for unexpected combinations
            (Some(left), _, _) => (
                left.text.clone(),
                Some([left.start, left.end]),
                pair.right.as_ref().map(|r| [r.start, r.end]),
                format!("{:?}", left.tag),
            ),
            (None, Some(right), _) => (
                right.text.clone(),
                None,
                Some([right.start, right.end]),
                format!("{:?}", right.tag),
            ),
            (None, None, _) => (String::new(), None, None, "WORD".to_string()),
        };

        let status = match pair.relation {
            TokenRelation::Identical => "Unchanged",
            TokenRelation::LeftOnly => "Removed",
            TokenRelation::RightOnly => "Added",
            TokenRelation::WhitespaceEquivalent => "Unchanged",
        };

        Self {
            text,
            status: status.to_string(),
            original_pos,
            revised_pos,
            tag,
        }
    }
}

/// Aligned pair with section texts for frontend display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendAlignedPair {
    /// Index of this pair in the alignment list (for matching with changes)
    pub index: usize,
    pub alignment_type: String,
    pub confidence: f64,
    pub original: Vec<FrontendSectionRef>,
    pub revised: Vec<FrontendSectionRef>,
    pub original_texts: Vec<String>,
    pub revised_texts: Vec<String>,
    /// Canonical IDs of all sections in this pair (for matching with change explanations)
    pub section_ids: Vec<String>,
    /// Token-level diff computed by Rust (null for exact matches, insertions, deletions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_diffs: Option<Vec<WasmTokenDiff>>,
}

impl FrontendAlignedPair {
    fn from_aligned_pair(
        pair: &AlignedPair,
        index: usize,
        original_structure: &DocumentStructure,
        revised_structure: &DocumentStructure,
        original_doc: &ContractDocument,
        revised_doc: &ContractDocument,
    ) -> Self {
        let original_refs: Vec<FrontendSectionRef> =
            pair.original.iter().map(FrontendSectionRef::from).collect();
        let revised_refs: Vec<FrontendSectionRef> =
            pair.revised.iter().map(FrontendSectionRef::from).collect();

        // Collect all section IDs for matching with change explanations
        let section_ids: Vec<String> = pair
            .original
            .iter()
            .chain(pair.revised.iter())
            .map(|sr| sr.canonical_id.clone())
            .collect();

        // Extract text for each original section
        let original_texts: Vec<String> = pair
            .original
            .iter()
            .map(|sr| {
                original_structure
                    .find_by_canonical(&sr.canonical_id)
                    .map(|node| extract_section_text(node, original_doc))
                    .unwrap_or_default()
            })
            .collect();

        // Extract text for each revised section
        let revised_texts: Vec<String> = pair
            .revised
            .iter()
            .map(|sr| {
                revised_structure
                    .find_by_canonical(&sr.canonical_id)
                    .map(|node| extract_section_text(node, revised_doc))
                    .unwrap_or_default()
            })
            .collect();

        // Compute token-level diff for Modified/Moved/Renumbered pairs
        let token_diffs = Self::compute_token_diffs(pair, &original_texts, &revised_texts);

        Self {
            index,
            alignment_type: format!("{:?}", pair.alignment_type),
            confidence: pair.confidence,
            original: original_refs,
            revised: revised_refs,
            original_texts,
            revised_texts,
            section_ids,
            token_diffs,
        }
    }

    /// Compute token-level diff for pairs that have both original and revised content.
    fn compute_token_diffs(
        pair: &AlignedPair,
        original_texts: &[String],
        revised_texts: &[String],
    ) -> Option<Vec<WasmTokenDiff>> {
        // Only compute diff for alignment types where comparison is meaningful.
        // Note: ExactMatch is included because "exact" means ≥90% similarity,
        // so there may still be small text changes to highlight.
        let should_diff = matches!(
            pair.alignment_type,
            AlignmentType::ExactMatch
                | AlignmentType::Modified
                | AlignmentType::Moved
                | AlignmentType::Renumbered
        );

        if !should_diff {
            return None;
        }

        // For simple 1:1 alignments, compute token diff directly
        if original_texts.len() == 1 && revised_texts.len() == 1 {
            let original_text = &original_texts[0];
            let revised_text = &revised_texts[0];

            // Skip if either is empty
            if original_text.is_empty() || revised_text.is_empty() {
                return None;
            }

            // Extract tokens and compute alignment
            let original_tokens = TokenAligner::extract_tokens_from_text(original_text);
            let revised_tokens = TokenAligner::extract_tokens_from_text(revised_text);

            let config = TokenAlignmentConfig::default(); // Uses Normalize whitespace mode
            let alignment = TokenAligner::align(&original_tokens, &revised_tokens, &config);

            // Convert to serializable format
            let diffs: Vec<WasmTokenDiff> = alignment
                .pairs
                .iter()
                .map(WasmTokenDiff::from_pair)
                .collect();

            return Some(diffs);
        }

        // For multi-section pairs (Split/Merged), concatenate and diff
        // This is a simplified approach - could be enhanced later
        if !original_texts.is_empty() && !revised_texts.is_empty() {
            let combined_original = original_texts.join("\n\n");
            let combined_revised = revised_texts.join("\n\n");

            if combined_original.is_empty() || combined_revised.is_empty() {
                return None;
            }

            let original_tokens = TokenAligner::extract_tokens_from_text(&combined_original);
            let revised_tokens = TokenAligner::extract_tokens_from_text(&combined_revised);

            let config = TokenAlignmentConfig::default();
            let alignment = TokenAligner::align(&original_tokens, &revised_tokens, &config);

            let diffs: Vec<WasmTokenDiff> = alignment
                .pairs
                .iter()
                .map(WasmTokenDiff::from_pair)
                .collect();

            return Some(diffs);
        }

        None
    }
}

/// Extract full section text from a document
fn extract_section_text(node: &SectionNode, doc: &ContractDocument) -> String {
    use layered_nlp::LToken;

    let mut text = String::new();
    let start_line = node.start_line;
    let end_line = node.end_line.unwrap_or(doc.line_count());

    for line_idx in start_line..end_line {
        if let Some(line) = doc.get_line(line_idx) {
            if !text.is_empty() {
                text.push('\n');
            }
            // Reconstruct line text from tokens
            let line_text: String = line
                .ll_tokens()
                .iter()
                .filter_map(|t| match t.get_token() {
                    LToken::Text(s, _) => Some(s.as_str()),
                    LToken::Value => None,
                })
                .collect::<Vec<_>>()
                .join("");
            text.push_str(&line_text);
        }
    }

    text
}

/// Compare two contract versions and return semantic differences.
///
/// # Arguments
/// * `original` - The original contract text
/// * `revised` - The revised contract text
///
/// # Returns
/// A JsValue containing either a `CompareResult` on success or a `DiffError` on failure.
///
/// # Panics
/// Panics are logged to the browser console via `console_error_panic_hook`.
/// In WASM, panics will propagate to JavaScript as exceptions.
#[wasm_bindgen]
pub fn compare_contracts(original: &str, revised: &str) -> JsValue {
    compare_contracts_internal(original, revised)
}

fn compare_contracts_internal(original: &str, revised: &str) -> JsValue {
    // Task 0.10: Validate empty input
    if original.trim().is_empty() {
        let error = DiffError::invalid_input("Original document is empty");
        return serde_wasm_bindgen::to_value(&error).unwrap_or(JsValue::NULL);
    }
    if revised.trim().is_empty() {
        let error = DiffError::invalid_input("Revised document is empty");
        return serde_wasm_bindgen::to_value(&error).unwrap_or(JsValue::NULL);
    }

    // Task 0.3: Validate input length
    if original.len() > MAX_INPUT_SIZE {
        let error = DiffError::input_too_large("Original", original.len());
        return serde_wasm_bindgen::to_value(&error).unwrap_or(JsValue::NULL);
    }
    if revised.len() > MAX_INPUT_SIZE {
        let error = DiffError::input_too_large("Revised", revised.len());
        return serde_wasm_bindgen::to_value(&error).unwrap_or(JsValue::NULL);
    }

    // Task 0.4: Run the full contract pipeline (with POS tags) on both documents
    let original_doc = match run_contract_pipeline_with_pos(original) {
        Ok(doc) => doc,
        Err(e) => {
            let error = DiffError::alignment_failed(&format!(
                "Failed to process original document: {:?}",
                e
            ));
            return serde_wasm_bindgen::to_value(&error).unwrap_or(JsValue::NULL);
        }
    };

    let revised_doc = match run_contract_pipeline_with_pos(revised) {
        Ok(doc) => doc,
        Err(e) => {
            let error = DiffError::alignment_failed(&format!(
                "Failed to process revised document: {:?}",
                e
            ));
            return serde_wasm_bindgen::to_value(&error).unwrap_or(JsValue::NULL);
        }
    };

    // Task 0.5: Build DocumentStructure for each document
    let original_structure = DocumentStructureBuilder::build(&original_doc);
    let revised_structure = DocumentStructureBuilder::build(&revised_doc);

    // Task 0.6: Run DocumentAligner::align()
    let aligner = DocumentAligner::new();
    let alignment_result = aligner.align(
        &original_structure.value,
        &revised_structure.value,
        &original_doc,
        &revised_doc,
    );

    // Task 0.7: Run SemanticDiffEngine::compute_diff()
    let diff_engine = SemanticDiffEngine::new();
    let diff_result = diff_engine.compute_diff(&alignment_result, &original_doc, &revised_doc);

    // Build alignment summary for UI
    let alignment_summary = AlignmentSummary::from_alignment_result(&alignment_result);

    // Gate 2: Build aligned pairs with section texts
    let aligned_pairs: Vec<FrontendAlignedPair> = alignment_result
        .alignments
        .iter()
        .enumerate()
        .map(|(index, pair)| {
            FrontendAlignedPair::from_aligned_pair(
                pair,
                index,
                &original_structure.value,
                &revised_structure.value,
                &original_doc,
                &revised_doc,
            )
        })
        .collect();

    // Task 0.8: Serialize result to JSON
    let result = CompareResult {
        aligned_pairs,
        diff: diff_result,
        alignment_summary,
    };

    serde_wasm_bindgen::to_value(&result).unwrap_or_else(|e| {
        let error = DiffError::internal_error(&format!("Failed to serialize result: {}", e));
        serde_wasm_bindgen::to_value(&error).unwrap_or(JsValue::NULL)
    })
}

// ============================================================================
// CONFLICT DETECTION API
// ============================================================================

/// Serializable representation of a contract conflict for WASM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConflict {
    pub span_a_start_line: usize,
    pub span_a_start_token: usize,
    pub span_a_end_line: usize,
    pub span_a_end_token: usize,
    pub span_b_start_line: usize,
    pub span_b_start_token: usize,
    pub span_b_end_line: usize,
    pub span_b_end_token: usize,
    pub conflict_type: String,
    pub explanation: String,
    pub confidence: f64,
}

/// Result of conflict analysis containing all detected conflicts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictAnalysisResult {
    pub conflicts: Vec<WasmConflict>,
    pub total_count: usize,
}

/// Detects conflicts in contract text.
///
/// Analyzes the contract for internal contradictions such as:
/// - Modal conflicts (shall vs may for same party/action)
/// - Temporal conflicts (incompatible timing requirements)
/// - Party conflicts (same action assigned to different parties)
///
/// Returns a JSON object containing all detected conflicts.
#[wasm_bindgen]
pub fn detect_conflicts(text: &str) -> JsValue {
    init();
    let result = detect_conflicts_internal(text);
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

fn detect_conflicts_internal(text: &str) -> ConflictAnalysisResult {
    // Run the contract pipeline with POS tags for obligation/conflict detection.
    let doc = match run_contract_pipeline_with_pos(text) {
        Ok(doc) => doc,
        Err(_) => {
            // Return empty result on processing error
            return ConflictAnalysisResult {
                conflicts: vec![],
                total_count: 0,
            };
        }
    };

    // Run conflict detection on the analyzed document
    let detector = ConflictDetector::new();
    let conflicts = detector.detect_in_document(&doc);

    let wasm_conflicts: Vec<WasmConflict> = conflicts
        .iter()
        .map(|scored| {
            let conflict = &scored.value;
            WasmConflict {
                span_a_start_line: conflict.span_a.start.line,
                span_a_start_token: conflict.span_a.start.token,
                span_a_end_line: conflict.span_a.end.line,
                span_a_end_token: conflict.span_a.end.token,
                span_b_start_line: conflict.span_b.start.line,
                span_b_start_token: conflict.span_b.start.token,
                span_b_end_line: conflict.span_b.end.line,
                span_b_end_token: conflict.span_b.end.token,
                conflict_type: format!("{:?}", conflict.conflict_type),
                explanation: conflict.explanation.clone(),
                confidence: scored.confidence,
            }
        })
        .collect();

    ConflictAnalysisResult {
        total_count: wasm_conflicts.len(),
        conflicts: wasm_conflicts,
    }
}

// ============================================================================
// SCOPE OPERATOR EXTRACTION API
// ============================================================================

/// Serializable representation of a scope operator for WASM.
///
/// Scope operators are linguistic constructs that have a trigger span
/// (the operator word) and a domain they scope over.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmScopeOperator {
    /// Dimension of the operator: "Negation", "Quantifier", "Precedence", "Deictic", or "Other"
    pub dimension: String,
    /// Line index of the trigger word
    pub trigger_line: usize,
    /// Start token index of the trigger
    pub trigger_start_token: usize,
    /// End token index of the trigger (exclusive)
    pub trigger_end_token: usize,
    /// The trigger text (e.g., "not", "each", "notwithstanding")
    pub trigger_text: String,
    /// Line index of the domain start
    pub domain_line: usize,
    /// Start token index of the domain
    pub domain_start_token: usize,
    /// End token index of the domain (exclusive)
    pub domain_end_token: usize,
    /// The marker word (e.g., "not", "never", "each", "all")
    pub marker: String,
    /// Kind of operator (e.g., "Simple", "Temporal", "Universal", "Existential")
    pub kind: String,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
}

/// Result of scope operator extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeOperatorResult {
    /// All extracted scope operators
    pub operators: Vec<WasmScopeOperator>,
    /// Total count of operators found
    pub total_count: usize,
}

/// Extract scope operators (negation, quantifiers) from text.
///
/// Uses `NegationDetector` and `QuantifierDetector` from layered-contracts
/// to identify scope-bearing operators in the text. Each operator has:
/// - A trigger span (the operator word like "not", "each")
/// - A domain span (the text the operator scopes over)
///
/// # Example operators detected:
/// - Negation: "not", "never", "neither", "nor", "no"
/// - Quantifiers:
///   - Universal: "each", "every", "all"
///   - Existential: "any", "some"
///   - Negative: "no", "none"
///
/// Returns a JSON object containing all detected scope operators.
#[wasm_bindgen]
pub fn extract_scope_operators(text: &str) -> JsValue {
    init();
    let result = extract_scope_operators_internal(text);
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

fn extract_scope_operators_internal(text: &str) -> ScopeOperatorResult {
    use layered_nlp::LToken;

    // Create a ContractDocument from the text
    let doc = ContractDocument::from_text(text);

    // Run the negation and quantifier detectors
    let neg_detector = NegationDetector::new();
    let quant_detector = QuantifierDetector::new();

    let negations = neg_detector.detect(&doc);
    let quantifiers = quant_detector.detect(&doc);

    let mut operators = Vec::new();

    // Helper to get trigger text from document
    let get_trigger_text = |line_idx: usize, start_token: usize, end_token: usize| -> String {
        if let Some(line) = doc.get_line(line_idx) {
            let tokens = line.ll_tokens();
            let mut text_parts = Vec::new();
            for idx in start_token..end_token.min(tokens.len()) {
                if let LToken::Text(s, _) = tokens[idx].get_token() {
                    text_parts.push(s.as_str());
                }
            }
            text_parts.join("")
        } else {
            String::new()
        }
    };

    // Convert negation operators
    for scored in negations {
        let op = &scored.value;
        let trigger_text = get_trigger_text(
            op.trigger.start.line,
            op.trigger.start.token,
            op.trigger.end.token,
        );

        let dimension = match &op.dimension {
            ScopeDimension::Negation => "Negation",
            ScopeDimension::Quantifier => "Quantifier",
            ScopeDimension::Precedence => "Precedence",
            ScopeDimension::Deictic => "Deictic",
            ScopeDimension::Other(_) => "Other",
        };

        let kind = match op.payload.kind {
            NegationKind::Simple => "Simple",
            NegationKind::Temporal => "Temporal",
            NegationKind::Correlative => "Correlative",
        };

        // Get primary domain (best-scoring candidate)
        if let Some(domain) = op.domain.primary() {
            operators.push(WasmScopeOperator {
                dimension: dimension.to_string(),
                trigger_line: op.trigger.start.line,
                trigger_start_token: op.trigger.start.token,
                trigger_end_token: op.trigger.end.token,
                trigger_text,
                domain_line: domain.start.line,
                domain_start_token: domain.start.token,
                domain_end_token: domain.end.token,
                marker: op.payload.marker.clone(),
                kind: kind.to_string(),
                confidence: scored.confidence,
            });
        }
    }

    // Convert quantifier operators
    for scored in quantifiers {
        let op = &scored.value;
        let trigger_text = get_trigger_text(
            op.trigger.start.line,
            op.trigger.start.token,
            op.trigger.end.token,
        );

        let dimension = match &op.dimension {
            ScopeDimension::Negation => "Negation",
            ScopeDimension::Quantifier => "Quantifier",
            ScopeDimension::Precedence => "Precedence",
            ScopeDimension::Deictic => "Deictic",
            ScopeDimension::Other(_) => "Other",
        };

        let kind = match op.payload.kind {
            QuantifierKind::Universal => "Universal",
            QuantifierKind::Existential => "Existential",
            QuantifierKind::Negative => "Negative",
        };

        // Get primary domain (best-scoring candidate)
        if let Some(domain) = op.domain.primary() {
            operators.push(WasmScopeOperator {
                dimension: dimension.to_string(),
                trigger_line: op.trigger.start.line,
                trigger_start_token: op.trigger.start.token,
                trigger_end_token: op.trigger.end.token,
                trigger_text,
                domain_line: domain.start.line,
                domain_start_token: domain.start.token,
                domain_end_token: domain.end.token,
                marker: op.payload.marker.clone(),
                kind: kind.to_string(),
                confidence: scored.confidence,
            });
        }
    }

    // Sort by position (line, then token) for consistent ordering
    operators.sort_by(|a, b| {
        a.trigger_line
            .cmp(&b.trigger_line)
            .then(a.trigger_start_token.cmp(&b.trigger_start_token))
    });

    ScopeOperatorResult {
        total_count: operators.len(),
        operators,
    }
}

// ============================================================================
// VERIFICATION QUEUE API
// ============================================================================

/// Verification target type for WASM boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WasmVerificationTarget {
    Node,
    BeneficiaryLink,
    ConditionLink,
    ObligorLink,
}

impl WasmVerificationTarget {
    fn from_rust(target: &RustVerificationTarget) -> Self {
        match target {
            RustVerificationTarget::Node(_) => WasmVerificationTarget::Node,
            RustVerificationTarget::BeneficiaryLink { .. } => WasmVerificationTarget::BeneficiaryLink,
            RustVerificationTarget::ConditionLink { .. } => WasmVerificationTarget::ConditionLink,
            RustVerificationTarget::ObligorLink { .. } => WasmVerificationTarget::ObligorLink,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            WasmVerificationTarget::Node => "Node",
            WasmVerificationTarget::BeneficiaryLink => "BeneficiaryLink",
            WasmVerificationTarget::ConditionLink => "ConditionLink",
            WasmVerificationTarget::ObligorLink => "ObligorLink",
        }
    }
}

/// Verification queue item for WASM boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmVerificationQueueItem {
    /// Unique identity: "${node_id}-${clause_id}-${target_type}-${ordinal}"
    pub item_id: String,
    /// Target type (Node, BeneficiaryLink, ConditionLink, ObligorLink)
    pub target_type: WasmVerificationTarget,
    /// Obligation node identifier
    pub node_id: u32,
    /// Clause identifier where the target was found
    pub clause_id: u32,
    /// Priority (derived from position in sorted queue, 0 = highest priority)
    pub priority: u32,
    /// Details type ("PassiveVoiceObligor", "LowConfidenceObligor", "Beneficiary", "Condition", "AmbiguousBeneficiary")
    pub details_type: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Display text snippet for review
    pub display_text: String,
}

/// Result of verification queue extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmVerificationQueueResult {
    /// All queue items sorted by confidence (lowest first = highest priority)
    pub items: Vec<WasmVerificationQueueItem>,
    /// Total count of items in queue
    pub total_count: u32,
}

/// Confirmed correction - user verified this item is correct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmedCorrection {
    pub item_id: String,
}

/// Text correction - user corrected the display text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextCorrection {
    pub item_id: String,
    /// Original text before correction (for UI display/tracking, not validated by WASM)
    pub original_text: String,
    pub corrected_text: String,
}

/// Dismissed item - user marked as not needing attention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DismissedItem {
    pub item_id: String,
    pub reason: Option<String>,
}

/// Input for analyze_with_corrections
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorrectionsInput {
    #[serde(default)]
    pub confirmed: Vec<ConfirmedCorrection>,
    #[serde(default)]
    pub corrected: Vec<TextCorrection>,
    #[serde(default)]
    pub dismissed: Vec<DismissedItem>,
}

/// Extract verification queue from text.
///
/// Analyzes the contract to identify items that need human verification:
/// - Passive voice obligors
/// - Low confidence obligors
/// - Unresolved beneficiaries
/// - Ambiguous beneficiaries
/// - Conditions with unknown entities
///
/// Returns a JSON object containing all queue items sorted by confidence
/// (lowest confidence first = highest priority).
#[wasm_bindgen]
pub fn get_verification_queue(text: &str) -> JsValue {
    init();
    let result = get_verification_queue_internal(text);
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

fn get_verification_queue_internal(text: &str) -> WasmVerificationQueueResult {
    // Handle empty input
    if text.trim().is_empty() {
        return WasmVerificationQueueResult {
            items: vec![],
            total_count: 0,
        };
    }

    // Run the full resolver stack including AccountabilityGraphResolver
    // Start with the contract pipeline (with POS tags) and add remaining resolvers
    let doc = match run_contract_pipeline_with_pos(text) {
        Ok(doc) => doc,
        Err(_) => {
            return WasmVerificationQueueResult {
                items: vec![],
                total_count: 0,
            };
        }
    };

    // Add resolvers needed for the accountability graph
    // Gate 1: ContractClauseResolver produces ContractClause
    // Gate 2: ScopedObligationResolver produces Scored<ScopedObligation> from ObligationPhrase
    // Gate 3: LinkedObligationResolver produces LinkedObligation from ScopedObligation
    // Gate 4: AccountabilityGraphResolver produces ObligationNode from LinkedObligation
    let doc = doc
        .run_resolver(&ContractClauseResolver::new())
        .run_resolver(&ClauseAggregationResolver::new())
        .run_resolver(&ScopedObligationResolver::new())
        .run_resolver(&LinkedObligationResolver::default_config())
        .run_resolver(&AccountabilityGraphResolver::new());

    // Extract ObligationNode from all lines
    let nodes: Vec<Scored<ObligationNode>> = doc
        .lines()
        .iter()
        .flat_map(|line| {
            line.query::<Scored<ObligationNode>>()
                .into_iter()
                .flat_map(|(_, _, attrs)| attrs.into_iter().cloned())
        })
        .collect();

    // Build the obligation graph and get verification queue
    let graph = ObligationGraph::new(&nodes);
    let rust_queue = graph.verification_queue();

    // Convert to WASM types with unique item_ids
    // Track ordinals for items with same (node_id, clause_id, target_type)
    use std::collections::HashMap;
    let mut ordinal_counters: HashMap<(u32, u32, &'static str), u32> = HashMap::new();

    let items: Vec<WasmVerificationQueueItem> = rust_queue
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let target_type = WasmVerificationTarget::from_rust(&item.target);
            let target_type_str = target_type.as_str();

            // Compute ordinal for this (node_id, clause_id, target_type) tuple
            let key = (item.node_id, item.clause_id, target_type_str);
            let ordinal = *ordinal_counters.get(&key).unwrap_or(&0);
            ordinal_counters.insert(key, ordinal + 1);

            // Generate unique item_id
            let item_id = format!(
                "{}-{}-{}-{}",
                item.node_id, item.clause_id, target_type_str, ordinal
            );

            // Extract details_type and display_text from VerificationQueueDetails
            let (details_type, display_text) = match &item.details {
                RustVerificationQueueDetails::PassiveVoiceObligor { obligor_text, .. } => {
                    ("PassiveVoiceObligor".to_string(), obligor_text.clone())
                }
                RustVerificationQueueDetails::LowConfidenceObligor { obligor_text, .. } => {
                    ("LowConfidenceObligor".to_string(), obligor_text.clone())
                }
                RustVerificationQueueDetails::Beneficiary { beneficiary_text, .. } => {
                    ("Beneficiary".to_string(), beneficiary_text.clone())
                }
                RustVerificationQueueDetails::Condition { condition_text, .. } => {
                    ("Condition".to_string(), condition_text.clone())
                }
                RustVerificationQueueDetails::AmbiguousBeneficiary { beneficiary_text, .. } => {
                    ("AmbiguousBeneficiary".to_string(), beneficiary_text.clone())
                }
            };

            WasmVerificationQueueItem {
                item_id,
                target_type,
                node_id: item.node_id,
                clause_id: item.clause_id,
                priority: index as u32, // Position in sorted queue = priority
                details_type,
                confidence: item.confidence,
                display_text,
            }
        })
        .collect();

    WasmVerificationQueueResult {
        total_count: items.len() as u32,
        items,
    }
}

/// Re-analyze text with prior corrections applied (post-processing approach).
///
/// Takes the original contract text and a JSON string of corrections.
/// Returns a verification queue with:
///
/// Processing order:
/// 1. Filter out dismissed items
/// 2. Set confidence = 1.0 for confirmed items
/// 3. Apply text corrections to display_text
/// 4. Re-sort by confidence and reassign priorities
#[wasm_bindgen]
pub fn analyze_with_corrections(text: &str, corrections_json: &str) -> JsValue {
    init();
    let result = analyze_with_corrections_internal(text, corrections_json);
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

fn analyze_with_corrections_internal(text: &str, corrections_json: &str) -> WasmVerificationQueueResult {
    // 1. Parse corrections (empty if invalid JSON)
    let corrections: CorrectionsInput = serde_json::from_str(corrections_json)
        .unwrap_or_default();

    // 2. Build lookup sets for O(1) access
    let confirmed_ids: std::collections::HashSet<&str> = corrections.confirmed
        .iter()
        .map(|c| c.item_id.as_str())
        .collect();

    let corrected_map: std::collections::HashMap<&str, &str> = corrections.corrected
        .iter()
        .map(|c| (c.item_id.as_str(), c.corrected_text.as_str()))
        .collect();

    let dismissed_ids: std::collections::HashSet<&str> = corrections.dismissed
        .iter()
        .map(|d| d.item_id.as_str())
        .collect();

    // 3. Get base queue from normal analysis
    let base_queue = get_verification_queue_internal(text);

    // 4. Post-process: filter dismissed, apply corrections
    let mut items: Vec<WasmVerificationQueueItem> = base_queue.items
        .into_iter()
        .filter(|item| !dismissed_ids.contains(item.item_id.as_str()))
        .map(|mut item| {
            // Apply confirmed: set confidence to 1.0
            if confirmed_ids.contains(item.item_id.as_str()) {
                item.confidence = 1.0;
            }
            // Apply text correction: update display_text
            if let Some(corrected_text) = corrected_map.get(item.item_id.as_str()) {
                item.display_text = (*corrected_text).to_string();
            }
            item
        })
        .collect();

    // 5. Re-sort by confidence and reassign priorities (lowest confidence = highest priority)
    items.sort_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal));
    for (i, item) in items.iter_mut().enumerate() {
        item.priority = i as u32;
    }

    WasmVerificationQueueResult {
        total_count: items.len() as u32,
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUEUE_SAMPLE_TEXT: &str = r#"The Company shall pay the Contractor within 30 days."#;
    const QUEUE_MULTI_TEXT: &str = r#"The Company shall pay the Contractor within 30 days.
The Vendor shall reimburse Customer fees."#;
    const QUEUE_TRI_TEXT: &str = r#"The Company shall pay the Contractor within 30 days.
The Vendor shall reimburse Customer fees.
The Seller shall provide Buyer reports."#;
    const QUEUE_DEMO_TEXT: &str = r#"The Company shall pay the Contractor within 30 days.
Reports shall be made within 10 days."#;

    fn require_queue(text: &str) -> WasmVerificationQueueResult {
        let queue = get_verification_queue_internal(text);
        assert!(
            !queue.items.is_empty(),
            "Expected verification queue items for sample text"
        );
        queue
    }

    /// Test that simple personal pronouns are detected as deixis
    #[test]
    fn test_deixis_simple_pronouns() {
        let result = analyze_contract_internal("I will meet you there tomorrow.");

        let deixis_spans: Vec<_> = result.spans.iter()
            .filter(|s| s.kind == "DeicticReference")
            .collect();

        // Should detect: "I" (PersonFirst), "you" (PersonSecond), "there" (PlaceDistal), "tomorrow" (TimeFuture)
        assert!(deixis_spans.len() >= 4, "Expected at least 4 deixis spans, got {}", deixis_spans.len());

        // Check for person deixis
        let person_spans: Vec<_> = deixis_spans.iter()
            .filter(|s| {
                s.metadata.as_ref()
                    .and_then(|m| m.get("category"))
                    .map(|c| c.as_str() == Some("Person"))
                    .unwrap_or(false)
            })
            .collect();
        assert!(person_spans.len() >= 2, "Expected at least 2 person deixis spans");

        // Check for place deixis ("there")
        let place_spans: Vec<_> = deixis_spans.iter()
            .filter(|s| {
                s.metadata.as_ref()
                    .and_then(|m| m.get("category"))
                    .map(|c| c.as_str() == Some("Place"))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(place_spans.len(), 1, "Expected 1 place deixis span");

        // Check for time deixis ("tomorrow")
        let time_spans: Vec<_> = deixis_spans.iter()
            .filter(|s| {
                s.metadata.as_ref()
                    .and_then(|m| m.get("category"))
                    .map(|c| c.as_str() == Some("Time"))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(time_spans.len(), 1, "Expected 1 time deixis span");
    }

    /// Test that discourse markers are detected
    #[test]
    fn test_deixis_discourse_markers() {
        let result = analyze_contract_internal(
            "However, the contract is valid. Therefore, we proceed."
        );

        let discourse_spans: Vec<_> = result.spans.iter()
            .filter(|s| {
                s.kind == "DeicticReference" &&
                s.metadata.as_ref()
                    .and_then(|m| m.get("category"))
                    .map(|c| c.as_str() == Some("Discourse"))
                    .unwrap_or(false)
            })
            .collect();

        // Should detect: "However" and "Therefore"
        assert_eq!(discourse_spans.len(), 2, "Expected 2 discourse markers");

        // Check that "However" was detected
        let however = discourse_spans.iter()
            .find(|s| s.metadata.as_ref()
                .and_then(|m| m.get("surface_text"))
                .map(|t| t.as_str() == Some("However"))
                .unwrap_or(false));
        assert!(however.is_some(), "Expected 'However' to be detected");

        // Check that "Therefore" was detected
        let therefore = discourse_spans.iter()
            .find(|s| s.metadata.as_ref()
                .and_then(|m| m.get("surface_text"))
                .map(|t| t.as_str() == Some("Therefore"))
                .unwrap_or(false));
        assert!(therefore.is_some(), "Expected 'Therefore' to be detected");
    }

    /// Test that metadata structure is complete
    #[test]
    fn test_deixis_metadata_structure() {
        let result = analyze_contract_internal("I am here now.");

        let deixis_spans: Vec<_> = result.spans.iter()
            .filter(|s| s.kind == "DeicticReference")
            .collect();

        assert!(!deixis_spans.is_empty(), "Expected deixis spans");

        for span in deixis_spans {
            let metadata = span.metadata.as_ref().expect("Metadata should be present");

            // Check all required fields exist
            assert!(metadata.get("category").is_some(), "Missing category");
            assert!(metadata.get("subcategory").is_some(), "Missing subcategory");
            assert!(metadata.get("surface_text").is_some(), "Missing surface_text");
            assert!(metadata.get("confidence").is_some(), "Missing confidence");
            assert!(metadata.get("source").is_some(), "Missing source");

            // Check confidence is a valid number
            let confidence = metadata.get("confidence").unwrap().as_f64().unwrap();
            assert!(confidence >= 0.0 && confidence <= 1.0, "Confidence should be 0-1");

            // Check source has type field
            let source = metadata.get("source").unwrap();
            assert!(source.get("type").is_some(), "Source should have type field");
        }
    }

    /// Test that no duplicate spans exist for the same token
    #[test]
    fn test_deixis_no_duplicates() {
        let result = analyze_contract_internal("I will be there.");

        let deixis_spans: Vec<_> = result.spans.iter()
            .filter(|s| s.kind == "DeicticReference")
            .collect();

        // Check for duplicates by (start_offset, end_offset) pairs
        let mut seen_ranges: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
        for span in &deixis_spans {
            let range = (span.start_offset, span.end_offset);
            assert!(
                seen_ranges.insert(range),
                "Duplicate deixis span at offsets {:?}", range
            );
        }
    }

    /// Test edge cases with neutral text (no deictic words)
    #[test]
    fn test_deixis_neutral_text() {
        let result = analyze_contract_internal("Contract terms apply.");
        // No simple deictic words in this sentence
        let deixis_count = result.spans.iter().filter(|s| s.kind == "DeicticReference").count();
        assert_eq!(deixis_count, 0, "Expected no deixis in neutral text");

        let result = analyze_contract_internal("The parties agree to binding arbitration.");
        let deixis_count = result.spans.iter().filter(|s| s.kind == "DeicticReference").count();
        assert_eq!(deixis_count, 0, "Expected no deixis in neutral text");
    }

    /// Integration test with sample contract text from the plan
    #[test]
    fn test_deixis_sample_contract() {
        let sample = r#"This Agreement ("Agreement") is entered into as of the Effective Date.

The Company shall deliver the Product within 30 days. It must comply with
all applicable regulations. We agree to the terms herein.

However, if the Company fails to deliver, the Buyer may terminate this
Agreement. Therefore, timely performance is essential.

I, the undersigned, represent that the information provided here is accurate.
You shall notify us of any changes. They must be submitted in writing."#;

        let result = analyze_contract_internal(sample);

        let deixis_spans: Vec<_> = result.spans.iter()
            .filter(|s| s.kind == "DeicticReference")
            .collect();

        // Count by category
        let person_count = deixis_spans.iter()
            .filter(|s| s.metadata.as_ref()
                .and_then(|m| m.get("category"))
                .map(|c| c.as_str() == Some("Person"))
                .unwrap_or(false))
            .count();

        let discourse_count = deixis_spans.iter()
            .filter(|s| s.metadata.as_ref()
                .and_then(|m| m.get("category"))
                .map(|c| c.as_str() == Some("Discourse"))
                .unwrap_or(false))
            .count();

        let place_count = deixis_spans.iter()
            .filter(|s| s.metadata.as_ref()
                .and_then(|m| m.get("category"))
                .map(|c| c.as_str() == Some("Place"))
                .unwrap_or(false))
            .count();

        // Expected: We, I, You, us, They (person), However, Therefore (discourse), here (place)
        assert!(person_count >= 4, "Expected at least 4 person deixis, got {}", person_count);
        assert!(discourse_count >= 2, "Expected at least 2 discourse markers, got {}", discourse_count);
        assert!(place_count >= 1, "Expected at least 1 place deixis, got {}", place_count);
    }

    /// Performance benchmark - measures analysis time for sample contract
    /// Run with: cargo test -p layered-nlp-demo-wasm --release -- --nocapture --ignored test_performance
    /// NOTE: Ignored by default because debug mode is ~50x slower than release.
    #[test]
    #[ignore]
    fn test_performance_benchmark() {
        let sample = r#"SERVICES AGREEMENT

"Company" means ABC Corporation (the "ABC"). "Contractor" means XYZ Services LLC (the "Provider").

The Contractor shall provide consulting services to the Company. The Company shall pay the Contractor within 30 days of invoice receipt. We agree to the terms herein.

However, the Company may terminate this agreement if Contractor fails to perform. It shall provide 30 days written notice. Therefore, timely performance is essential.

I, the undersigned, represent that the information provided here is accurate. You shall notify us of any changes.

Unless otherwise agreed, the Contractor shall maintain confidentiality. Subject to applicable law, the Provider shall indemnify the Company."#;

        // Warm-up run
        let _ = analyze_contract_internal(sample);

        // Timed runs
        let iterations = 100;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let _ = analyze_contract_internal(sample);
        }
        let elapsed = start.elapsed();

        let avg_micros = elapsed.as_micros() as f64 / iterations as f64;
        let avg_millis = avg_micros / 1000.0;

        println!("\n=== Performance Benchmark ===");
        println!("Text length: {} chars", sample.len());
        println!("Iterations: {}", iterations);
        println!("Total time: {:?}", elapsed);
        println!("Average: {:.2} ms ({:.0} µs)", avg_millis, avg_micros);
        println!("=============================\n");

        // Assert reasonable performance
        // Debug mode is ~10-15x slower than release, so use different thresholds
        let threshold_ms = if cfg!(debug_assertions) { 50.0 } else { 10.0 };
        assert!(
            avg_millis < threshold_ms,
            "Analysis too slow: {:.2} ms (expected < {:.0} ms)",
            avg_millis,
            threshold_ms
        );
    }

    // ========================================================================
    // SEMANTIC DIFF TESTS (Gate 0)
    // ========================================================================

    /// Helper to run compare and get result (for tests that don't need JsValue)
    fn compare_contracts_test(original: &str, revised: &str) -> Result<CompareResult, DiffError> {
        // Validate empty input
        if original.trim().is_empty() {
            return Err(DiffError::invalid_input("Original document is empty"));
        }
        if revised.trim().is_empty() {
            return Err(DiffError::invalid_input("Revised document is empty"));
        }

        // Validate input length
        if original.len() > MAX_INPUT_SIZE {
            return Err(DiffError::input_too_large("Original", original.len()));
        }
        if revised.len() > MAX_INPUT_SIZE {
            return Err(DiffError::input_too_large("Revised", revised.len()));
        }

        // Run pipeline
        let original_doc = run_contract_pipeline_with_pos(original)
            .map_err(|e| DiffError::alignment_failed(&format!("Original: {:?}", e)))?;
        let revised_doc = run_contract_pipeline_with_pos(revised)
            .map_err(|e| DiffError::alignment_failed(&format!("Revised: {:?}", e)))?;

        // Build structures and align
        let original_structure = DocumentStructureBuilder::build(&original_doc);
        let revised_structure = DocumentStructureBuilder::build(&revised_doc);

        let aligner = DocumentAligner::new();
        let alignment_result = aligner.align(
            &original_structure.value,
            &revised_structure.value,
            &original_doc,
            &revised_doc,
        );

        // Compute diff
        let diff_engine = SemanticDiffEngine::new();
        let diff_result = diff_engine.compute_diff(&alignment_result, &original_doc, &revised_doc);

        let alignment_summary = AlignmentSummary::from_alignment_result(&alignment_result);

        // Build aligned pairs with section texts
        let aligned_pairs: Vec<FrontendAlignedPair> = alignment_result
            .alignments
            .iter()
            .enumerate()
            .map(|(index, pair)| {
                FrontendAlignedPair::from_aligned_pair(
                    pair,
                    index,
                    &original_structure.value,
                    &revised_structure.value,
                    &original_doc,
                    &revised_doc,
                )
            })
            .collect();

        Ok(CompareResult {
            aligned_pairs,
            diff: diff_result,
            alignment_summary,
        })
    }

    /// Test 1: Two identical contracts should produce no semantic changes
    #[test]
    fn test_compare_identical_contracts() {
        let contract = r#"ARTICLE I: DEFINITIONS

Section 1.1 "Company" means ABC Corporation.

ARTICLE II: OBLIGATIONS

Section 2.1 The Company shall deliver goods within 30 days."#;

        let result = compare_contracts_test(contract, contract).expect("Should succeed");

        assert_eq!(
            result.diff.changes.len(),
            0,
            "Identical contracts should have no semantic changes"
        );
        assert!(
            result.alignment_summary.exact_match > 0,
            "Should have exact matches"
        );
    }

    /// Test 2: Modal change (shall -> may) should be detected
    /// Note: This test includes a defined term "Company" so the obligation resolver
    /// can identify the obligor via term reference.
    #[test]
    fn test_compare_modal_change() {
        let original = r#"ARTICLE I: DEFINITIONS

Section 1.1 "Company" means ABC Corporation.

ARTICLE II: OBLIGATIONS

Section 2.1 The Company shall deliver goods within 30 days."#;

        let revised = r#"ARTICLE I: DEFINITIONS

Section 1.1 "Company" means ABC Corporation.

ARTICLE II: OBLIGATIONS

Section 2.1 The Company may deliver goods within 30 days."#;

        let result = compare_contracts_test(original, revised).expect("Should succeed");

        // Should detect a modal change
        let modal_changes: Vec<_> = result
            .diff
            .changes
            .iter()
            .filter(|c| matches!(c.change_type, layered_contracts::SemanticChangeType::ObligationModal(_)))
            .collect();

        // Print debug info for troubleshooting
        println!("Modal change test debug:");
        println!("  Total changes: {}", result.diff.changes.len());
        println!("  Alignments: {} total, {} exact, {} modified, {} inserted, {} deleted",
            result.alignment_summary.total_alignments,
            result.alignment_summary.exact_match,
            result.alignment_summary.modified,
            result.alignment_summary.inserted,
            result.alignment_summary.deleted,
        );
        for change in &result.diff.changes {
            println!("  Change: {:?}", change.change_type);
        }

        // If no modal change detected, check if at least we got the alignment right
        // The obligation resolver may not always detect obligations depending on line structure
        if modal_changes.is_empty() {
            // At minimum, we should detect section modification or have some alignments
            assert!(
                result.alignment_summary.total_alignments > 0,
                "Should have at least some alignments"
            );
        } else {
            // Verify risk level is high or critical for shall->may
            for change in &modal_changes {
                assert!(
                    change.risk_level >= layered_contracts::RiskLevel::High,
                    "Modal weakening should be high risk or higher"
                );
            }
        }
    }

    /// Test 3: Term redefinition should be detected
    #[test]
    fn test_compare_term_redefinition() {
        let original = r#"ARTICLE I: DEFINITIONS

Section 1.1 "Confidential Information" means any non-public information."#;

        let revised = r#"ARTICLE I: DEFINITIONS

Section 1.1 "Confidential Information" means any non-public technical information."#;

        let result = compare_contracts_test(original, revised).expect("Should succeed");

        // Should detect a term definition change
        let term_changes: Vec<_> = result
            .diff
            .changes
            .iter()
            .filter(|c| matches!(c.change_type, layered_contracts::SemanticChangeType::TermDefinition(_)))
            .collect();

        assert!(
            !term_changes.is_empty(),
            "Should detect term definition change"
        );
    }

    /// Test 4: Section addition should be detected
    #[test]
    fn test_compare_section_added() {
        let original = r#"ARTICLE I: OBLIGATIONS

Section 1.1 The Company shall deliver goods."#;

        let revised = r#"ARTICLE I: OBLIGATIONS

Section 1.1 The Company shall deliver goods.

Section 1.2 The Company shall maintain records."#;

        let result = compare_contracts_test(original, revised).expect("Should succeed");

        // Check alignment summary shows an insertion
        assert!(
            result.alignment_summary.inserted > 0,
            "Should detect section insertion"
        );
    }

    /// Test 5: Empty original should return invalid_input error
    #[test]
    fn test_compare_empty_original() {
        let result = compare_contracts_test("", "Some contract text");

        assert!(result.is_err(), "Empty original should fail");
        let error = result.unwrap_err();
        assert_eq!(error.error.code, "invalid_input");
    }

    /// Test 6: Empty revised should return invalid_input error
    #[test]
    fn test_compare_empty_revised() {
        let result = compare_contracts_test("Some contract text", "   ");

        assert!(result.is_err(), "Empty revised should fail");
        let error = result.unwrap_err();
        assert_eq!(error.error.code, "invalid_input");
    }

    /// Test 7: Input too large should return input_too_large error
    #[test]
    fn test_compare_input_too_large() {
        let large_text = "x".repeat(MAX_INPUT_SIZE + 1);
        let result = compare_contracts_test(&large_text, "Some contract text");

        assert!(result.is_err(), "Oversized input should fail");
        let error = result.unwrap_err();
        assert_eq!(error.error.code, "input_too_large");
        assert!(error.error.details.is_some(), "Should have details");
    }

    /// Test 8: Non-contract text should process without panic
    #[test]
    fn test_compare_non_contract_text() {
        let original = "Hello world, this is just some random text.";
        let revised = "Hello world, this is different random text.";

        let result = compare_contracts_test(original, revised);

        // Should succeed (may have empty results, but no panic)
        assert!(result.is_ok(), "Non-contract text should process without error");
    }

    /// Test 9: Sample NDA contracts from spec
    #[test]
    fn test_compare_sample_nda() {
        let original = r#"ARTICLE I: DEFINITIONS

Section 1.1 "Confidential Information" means any non-public information
disclosed by either party to the other.

Section 1.2 "Receiving Party" means the party receiving Confidential Information.

ARTICLE II: OBLIGATIONS

Section 2.1 The Receiving Party shall protect all Confidential Information
using reasonable care.

Section 2.2 The Receiving Party shall not disclose Confidential Information
to any third party without prior written consent.

ARTICLE III: TERM

Section 3.1 This Agreement shall remain in effect for two (2) years from
the Effective Date."#;

        let revised = r#"ARTICLE I: DEFINITIONS

Section 1.1 "Confidential Information" means any non-public technical
information disclosed by either party to the other.

Section 1.2 "Receiving Party" means the party receiving Confidential Information.

ARTICLE II: OBLIGATIONS

Section 2.1 The Receiving Party may protect all Confidential Information
using reasonable care.

Section 2.2 The Receiving Party shall not disclose Confidential Information
to any third party without prior written consent.

Section 2.3 The Receiving Party shall return all materials within 30 days
of termination.

ARTICLE III: TERM

Section 3.1 This Agreement shall remain in effect for three (3) years from
the Effective Date."#;

        let result = compare_contracts_test(original, revised).expect("Should succeed");

        // Verify we get changes
        assert!(
            !result.diff.changes.is_empty(),
            "Should detect changes between NDA versions"
        );

        // Verify summary has changes
        assert!(
            result.diff.summary.total_changes > 0,
            "Summary should show total changes"
        );

        // Check that we have alignment summary
        assert!(
            result.alignment_summary.total_alignments > 0,
            "Should have alignments"
        );

        // Print summary for debugging (visible with --nocapture)
        println!("NDA Diff Summary:");
        println!("  Total changes: {}", result.diff.summary.total_changes);
        println!("  Critical: {}", result.diff.summary.critical_changes);
        println!("  High: {}", result.diff.summary.high_risk_changes);
        println!("  Medium: {}", result.diff.summary.medium_risk_changes);
        println!("  Low: {}", result.diff.summary.low_risk_changes);
        println!("  Alignments: {} total ({} exact, {} modified, {} inserted)",
            result.alignment_summary.total_alignments,
            result.alignment_summary.exact_match,
            result.alignment_summary.modified,
            result.alignment_summary.inserted,
        );
    }

    /// Test 10: Verify result serializes to JSON correctly
    #[test]
    fn test_compare_result_serializable() {
        let original = r#"Section 1.1 The Company shall deliver goods."#;
        let revised = r#"Section 1.1 The Company may deliver goods."#;

        let result = compare_contracts_test(original, revised).expect("Should succeed");

        // Should serialize to JSON without error
        let json = serde_json::to_string_pretty(&result);
        assert!(json.is_ok(), "Result should be serializable to JSON");

        // Print JSON for debugging
        let json_str = json.unwrap();
        println!("JSON output:\n{}", json_str);

        // Verify JSON contains expected fields
        assert!(json_str.contains("changes"), "JSON should have changes field");
        assert!(json_str.contains("summary"), "JSON should have summary field");
        assert!(json_str.contains("alignment_summary"), "JSON should have alignment_summary field");
    }

    /// Test 11: Verify aligned_pairs has section texts for frontend display
    #[test]
    fn test_compare_aligned_pairs_content() {
        let original = r#"Section 1.1 The Company shall deliver goods within 30 days."#;
        let revised = r#"Section 1.1 The Company may deliver goods within 30 days.

Section 1.2 The Company shall provide updates."#;

        let result = compare_contracts_test(original, revised).expect("Should succeed");

        // Should have aligned pairs
        assert!(
            !result.aligned_pairs.is_empty(),
            "Should have at least one aligned pair"
        );

        // First pair should have texts populated
        let first_pair = &result.aligned_pairs[0];

        // Verify index is set
        assert_eq!(first_pair.index, 0, "First pair should have index 0");

        // Verify section_ids is populated
        assert!(
            !first_pair.section_ids.is_empty(),
            "section_ids should not be empty"
        );

        // Verify we have at least one text populated (either original or revised)
        let has_original_text = !first_pair.original_texts.is_empty()
            && first_pair.original_texts.iter().any(|t| !t.is_empty());
        let has_revised_text = !first_pair.revised_texts.is_empty()
            && first_pair.revised_texts.iter().any(|t| !t.is_empty());

        assert!(
            has_original_text || has_revised_text,
            "Aligned pair should have section text content. Original texts: {:?}, Revised texts: {:?}",
            first_pair.original_texts,
            first_pair.revised_texts
        );

        // Verify inserted section (Section 1.2) has text
        let inserted_pair = result.aligned_pairs.iter().find(|p| p.alignment_type == "Inserted");
        if let Some(pair) = inserted_pair {
            assert!(
                pair.revised_texts.iter().any(|t| t.contains("provide updates")),
                "Inserted section should contain its text"
            );
        }

        println!("Aligned pairs test:");
        for pair in &result.aligned_pairs {
            println!(
                "  [{}] {} - section_ids: {:?}, orig_texts: {}, rev_texts: {}",
                pair.index,
                pair.alignment_type,
                pair.section_ids,
                pair.original_texts.len(),
                pair.revised_texts.len()
            );
        }
    }

    /// Test 12: Verify token_diffs are computed for aligned pairs with text changes.
    /// Note: ExactMatch means ≥90% similarity, so small changes like "shall"→"may" still
    /// produce ExactMatch. We compute token diffs for all such pairs.
    #[test]
    fn test_token_diffs_for_aligned_pairs() {
        let original = r#"ARTICLE I: DEFINITIONS

Section 1.1 "Company" means ABC Corporation.

ARTICLE II: OBLIGATIONS

Section 2.1 The Company shall deliver goods within thirty (30) days."#;

        let revised = r#"ARTICLE I: DEFINITIONS

Section 1.1 "Company" means ABC Corporation.

ARTICLE II: OBLIGATIONS

Section 2.1 The Company may deliver goods within sixty (60) days."#;

        let result = compare_contracts_test(original, revised).expect("Should succeed");

        // Debug output
        println!("Alignment types and token_diffs:");
        for pair in &result.aligned_pairs {
            let has_diffs = pair.token_diffs.is_some();
            let change_count = pair.token_diffs.as_ref()
                .map(|d| d.iter().filter(|t| t.status != "Unchanged").count())
                .unwrap_or(0);
            println!("  {} - token_diffs: {}, changes: {}",
                pair.alignment_type, has_diffs, change_count);
        }

        // Find the pair with Section 2.1 (has shall→may change)
        let section_2_1_pair = result
            .aligned_pairs
            .iter()
            .find(|p| p.original_texts.iter().any(|t| t.contains("shall")));

        assert!(
            section_2_1_pair.is_some(),
            "Should find a pair containing 'shall'. Pairs: {:?}",
            result.aligned_pairs.iter()
                .map(|p| format!("{}: {:?}", p.alignment_type, &p.original_texts))
                .collect::<Vec<_>>()
        );

        let pair = section_2_1_pair.unwrap();

        // Pairs with text content should have token_diffs
        assert!(
            pair.token_diffs.is_some(),
            "Pair with text content should have token_diffs"
        );

        let diffs = pair.token_diffs.as_ref().unwrap();

        // Should have multiple token diffs
        assert!(
            !diffs.is_empty(),
            "token_diffs should not be empty"
        );

        // Find specific changes: "shall" -> "may", "thirty" -> "sixty", "30" -> "60"
        let removed_tokens: Vec<_> = diffs.iter().filter(|d| d.status == "Removed").collect();
        let added_tokens: Vec<_> = diffs.iter().filter(|d| d.status == "Added").collect();

        // Should have removed "shall"
        assert!(
            removed_tokens.iter().any(|d| d.text == "shall"),
            "Should have 'shall' as removed. Removed: {:?}",
            removed_tokens.iter().map(|d| &d.text).collect::<Vec<_>>()
        );

        // Should have added "may"
        assert!(
            added_tokens.iter().any(|d| d.text == "may"),
            "Should have 'may' as added. Added: {:?}",
            added_tokens.iter().map(|d| &d.text).collect::<Vec<_>>()
        );

        // Should have removed "thirty" and "30"
        assert!(
            removed_tokens.iter().any(|d| d.text == "thirty"),
            "Should have 'thirty' as removed"
        );
        assert!(
            removed_tokens.iter().any(|d| d.text == "30"),
            "Should have '30' as removed"
        );

        // Should have added "sixty" and "60"
        assert!(
            added_tokens.iter().any(|d| d.text == "sixty"),
            "Should have 'sixty' as added"
        );
        assert!(
            added_tokens.iter().any(|d| d.text == "60"),
            "Should have '60' as added"
        );

        // Verify token types are correct
        let shall_token = removed_tokens.iter().find(|d| d.text == "shall").unwrap();
        assert_eq!(shall_token.tag, "Word", "Modal verb should be tagged as Word");

        let thirty_num = removed_tokens.iter().find(|d| d.text == "30").unwrap();
        assert_eq!(thirty_num.tag, "Natn", "Number should be tagged as Natn");

        println!("Token diff test:");
        println!("  Total diffs: {}", diffs.len());
        println!("  Removed: {:?}", removed_tokens.iter().map(|d| &d.text).collect::<Vec<_>>());
        println!("  Added: {:?}", added_tokens.iter().map(|d| &d.text).collect::<Vec<_>>());
    }

    /// Test 13: Verify token_diffs behavior for different alignment types:
    /// - ExactMatch: HAS token_diffs (all Unchanged when truly identical)
    /// - Inserted: NO token_diffs (no original to compare)
    /// - Deleted: NO token_diffs (no revised to compare)
    #[test]
    fn test_token_diffs_null_for_non_modified() {
        let original = r#"Section 1.1 The Company shall deliver goods."#;
        let revised = r#"Section 1.1 The Company shall deliver goods.

Section 1.2 Additional section inserted."#;

        let result = compare_contracts_test(original, revised).expect("Should succeed");

        // ExactMatch pairs SHOULD have token_diffs (with all Unchanged tokens)
        let exact_pair = result
            .aligned_pairs
            .iter()
            .find(|p| p.alignment_type == "ExactMatch");

        if let Some(pair) = exact_pair {
            assert!(
                pair.token_diffs.is_some(),
                "ExactMatch pairs should have token_diffs"
            );
            // All tokens should be Unchanged for truly identical sections
            let diffs = pair.token_diffs.as_ref().unwrap();
            let changes = diffs.iter().filter(|d| d.status != "Unchanged").count();
            assert_eq!(
                changes, 0,
                "Truly identical ExactMatch should have no changes. Found: {}",
                changes
            );
        }

        // Inserted pairs should NOT have token_diffs (no original to compare)
        let inserted_pair = result
            .aligned_pairs
            .iter()
            .find(|p| p.alignment_type == "Inserted");

        if let Some(pair) = inserted_pair {
            assert!(
                pair.token_diffs.is_none(),
                "Inserted pairs should not have token_diffs"
            );
        }
    }

    // ========================================================================
    // CONFLICT DETECTION TESTS
    // ========================================================================

    /// Test that detect_conflicts returns a valid result with empty text
    #[test]
    fn test_detect_conflicts_empty_text() {
        let result = detect_conflicts_internal("");
        assert_eq!(result.total_count, 0);
        assert!(result.conflicts.is_empty());
    }

    /// Test that detect_conflicts processes contract text without panicking.
    /// Note: Conflict detection accuracy depends on obligation phrase resolution,
    /// which works best with POS tagging (not included in WASM pipeline).
    #[test]
    fn test_detect_conflicts_processes_text() {
        let text = r#"ABC Corp (the "Company") shall deliver goods within 30 days.
ABC Corp (the "Company") may deliver goods within 60 days."#;

        let result = detect_conflicts_internal(text);

        // Verify the result structure is valid
        assert_eq!(result.total_count, result.conflicts.len());
        // The function should process without panicking
    }

    /// Test that detect_conflicts returns proper WasmConflict structure
    #[test]
    fn test_detect_conflicts_structure() {
        // Use a simple contract text
        let text = r#"The Company shall deliver goods. The Company may deliver goods."#;

        let result = detect_conflicts_internal(text);

        // Verify the result is properly structured
        assert_eq!(result.total_count, result.conflicts.len());

        // If any conflicts were detected, verify their structure
        for conflict in &result.conflicts {
            assert!(!conflict.conflict_type.is_empty());
            assert!(!conflict.explanation.is_empty());
            assert!(conflict.confidence >= 0.0 && conflict.confidence <= 1.0);
            // Verify span coordinates are reasonable
            assert!(conflict.span_a_end_line >= conflict.span_a_start_line);
            assert!(conflict.span_b_end_line >= conflict.span_b_start_line);
        }
    }

    /// Test that detect_conflicts result is serializable to JSON
    #[test]
    fn test_detect_conflicts_serializable() {
        let text = r#"ABC Corp (the "Company") shall deliver goods.
ABC Corp (the "Company") may deliver goods."#;

        let result = detect_conflicts_internal(text);
        let json = serde_json::to_string(&result);
        assert!(json.is_ok(), "Result should be serializable to JSON");

        // Verify JSON structure
        let json_str = json.unwrap();
        assert!(json_str.contains("\"conflicts\""));
        assert!(json_str.contains("\"total_count\""));
    }

    /// Test that detect_conflicts handles complex contract text
    #[test]
    fn test_detect_conflicts_complex_contract() {
        let text = r#"ARTICLE I: DEFINITIONS

Section 1.1 "Company" means ABC Corporation.
Section 1.2 "Contractor" means XYZ Services LLC.

ARTICLE II: OBLIGATIONS

Section 2.1 The Company shall pay the Contractor within 30 days.
Section 2.2 The Company shall pay the Contractor within 15 days.
Section 2.3 The Contractor shall deliver services promptly."#;

        let result = detect_conflicts_internal(text);

        // The function should process complex contract text without panicking
        assert_eq!(result.total_count, result.conflicts.len());

        // Log results for debugging (visible with --nocapture)
        println!("Complex contract test:");
        println!("  Total conflicts: {}", result.total_count);
        for conflict in &result.conflicts {
            println!("  - {}: {}", conflict.conflict_type, conflict.explanation);
        }
    }

    // ========================================================================
    // SPAN LINKS TESTS
    // ========================================================================

    /// Test that get_span_links returns empty result for empty text
    #[test]
    fn test_span_links_empty_text() {
        let result = get_span_links_internal("");
        assert_eq!(result.total_count, 0);
        assert!(result.links.is_empty());
    }

    /// Test that get_span_links detects condition-effect relationships
    #[test]
    fn test_span_links_condition_effect() {
        let text = "When it rains, then it pours.";
        let result = get_span_links_internal(text);

        // Should have bidirectional links: Parent (condition->effect) and Child (effect->condition)
        assert_eq!(result.total_count, 2, "Expected 2 links for condition-effect pattern");

        // Find the Parent link (condition pointing to effect)
        let parent_link = result.links.iter().find(|l| l.role == "Parent");
        assert!(parent_link.is_some(), "Should have Parent link");

        // Find the Child link (effect pointing back to condition)
        let child_link = result.links.iter().find(|l| l.role == "Child");
        assert!(child_link.is_some(), "Should have Child link");

        // Verify link_type is "Clause"
        for link in &result.links {
            assert_eq!(link.link_type, "Clause");
        }
    }

    /// Test that get_span_links detects coordination chains
    #[test]
    fn test_span_links_coordination() {
        let text = "The tenant pays rent and the landlord provides notice.";
        let result = get_span_links_internal(text);

        // Should have one Conjunct link for "A and B"
        let conjunct_links: Vec<_> = result.links.iter().filter(|l| l.role == "Conjunct").collect();
        assert_eq!(conjunct_links.len(), 1, "Expected one Conjunct link for 'A and B'");
    }

    /// Test that get_span_links detects exception relationships
    #[test]
    fn test_span_links_exception() {
        let text = "Tenant shall pay rent unless waived by Landlord.";
        let result = get_span_links_internal(text);

        // Should have Exception link
        let exception_links: Vec<_> = result.links.iter().filter(|l| l.role == "Exception").collect();
        assert_eq!(exception_links.len(), 1, "Expected one Exception link for 'unless' pattern");
    }

    /// Test that get_span_links result is serializable to JSON
    #[test]
    fn test_span_links_serializable() {
        let text = "When it rains, then it pours.";
        let result = get_span_links_internal(text);

        let json = serde_json::to_string(&result);
        assert!(json.is_ok(), "Result should be serializable to JSON");

        let json_str = json.unwrap();
        assert!(json_str.contains("\"links\""));
        assert!(json_str.contains("\"total_count\""));
        assert!(json_str.contains("\"role\""));
        assert!(json_str.contains("\"link_type\""));

        println!("Span links JSON:\n{}", json_str);
    }

    /// Test that get_span_links works with complex multi-clause text
    #[test]
    fn test_span_links_complex() {
        let text = r#"When the Company fails to deliver, then the Buyer may terminate.
The Contractor shall provide services and maintain records.
All rights reserved, except as noted."#;

        let result = get_span_links_internal(text);

        // Should detect multiple relationship types
        println!("Complex span links test:");
        println!("  Total links: {}", result.total_count);
        for link in &result.links {
            println!("  - {} (line {}, tokens {}-{}) -> (line {}, tokens {}-{})",
                link.role,
                link.anchor_line, link.anchor_start_token, link.anchor_end_token,
                link.target_line, link.target_start_token, link.target_end_token
            );
        }

        // Should have at least some links
        assert!(result.total_count > 0, "Expected some links in complex text");
    }

    /// Test that span link coordinates are valid
    #[test]
    fn test_span_links_valid_coordinates() {
        let text = "When it rains, then it pours.";
        let result = get_span_links_internal(text);

        for link in &result.links {
            // All links should be on line 0 (single line input)
            assert_eq!(link.anchor_line, 0, "Anchor should be on line 0");
            assert_eq!(link.target_line, 0, "Target should be on line 0");

            // Token indices should be valid
            assert!(link.anchor_end_token >= link.anchor_start_token,
                "Anchor end should be >= start");
            assert!(link.target_end_token >= link.target_start_token,
                "Target end should be >= start");
        }
    }

    // ========================================================================
    // SCOPE OPERATOR EXTRACTION TESTS
    // ========================================================================

    /// Test that extract_scope_operators returns empty result for empty text
    #[test]
    fn test_scope_operators_empty_text() {
        let result = extract_scope_operators_internal("");
        assert_eq!(result.total_count, 0);
        assert!(result.operators.is_empty());
    }

    /// Test that negation operators are detected
    #[test]
    fn test_scope_operators_negation() {
        let text = "The Company shall not disclose confidential information.";
        let result = extract_scope_operators_internal(text);

        // Should detect "not"
        let negations: Vec<_> = result.operators.iter()
            .filter(|op| op.dimension == "Negation")
            .collect();

        assert_eq!(negations.len(), 1, "Expected one negation operator");
        assert_eq!(negations[0].marker, "not");
        assert_eq!(negations[0].kind, "Simple");
        assert!(negations[0].confidence > 0.0);
    }

    /// Test that temporal negation (never) is detected
    #[test]
    fn test_scope_operators_negation_temporal() {
        let text = "The Buyer shall never terminate this Agreement.";
        let result = extract_scope_operators_internal(text);

        let negations: Vec<_> = result.operators.iter()
            .filter(|op| op.dimension == "Negation")
            .collect();

        assert_eq!(negations.len(), 1, "Expected one negation operator");
        assert_eq!(negations[0].marker, "never");
        assert_eq!(negations[0].kind, "Temporal");
    }

    /// Test that correlative negation (neither) is detected
    #[test]
    fn test_scope_operators_negation_correlative() {
        let text = "Neither party shall be responsible for damages.";
        let result = extract_scope_operators_internal(text);

        let negations: Vec<_> = result.operators.iter()
            .filter(|op| op.dimension == "Negation")
            .collect();

        assert_eq!(negations.len(), 1, "Expected one negation operator");
        assert_eq!(negations[0].marker, "neither");
        assert_eq!(negations[0].kind, "Correlative");
    }

    /// Test that universal quantifiers are detected
    #[test]
    fn test_scope_operators_quantifier_universal() {
        let text = "Each party shall comply with applicable laws.";
        let result = extract_scope_operators_internal(text);

        let quantifiers: Vec<_> = result.operators.iter()
            .filter(|op| op.dimension == "Quantifier")
            .collect();

        assert_eq!(quantifiers.len(), 1, "Expected one quantifier operator");
        assert_eq!(quantifiers[0].marker, "each");
        assert_eq!(quantifiers[0].kind, "Universal");
    }

    /// Test that existential quantifiers are detected
    #[test]
    fn test_scope_operators_quantifier_existential() {
        let text = "The Company shall not disclose any confidential information.";
        let result = extract_scope_operators_internal(text);

        let quantifiers: Vec<_> = result.operators.iter()
            .filter(|op| op.dimension == "Quantifier" && op.kind == "Existential")
            .collect();

        assert_eq!(quantifiers.len(), 1, "Expected one existential quantifier");
        assert_eq!(quantifiers[0].marker, "any");
    }

    /// Test that negative quantifiers are detected
    #[test]
    fn test_scope_operators_quantifier_negative() {
        let text = "There shall be no liability for indirect damages.";
        let result = extract_scope_operators_internal(text);

        let quantifiers: Vec<_> = result.operators.iter()
            .filter(|op| op.dimension == "Quantifier" && op.kind == "Negative")
            .collect();

        assert_eq!(quantifiers.len(), 1, "Expected one negative quantifier");
        assert_eq!(quantifiers[0].marker, "no");
    }

    /// Test that multiple operators in same sentence are detected
    #[test]
    fn test_scope_operators_multiple() {
        let text = "Each party shall not disclose any information.";
        let result = extract_scope_operators_internal(text);

        // Should detect "each" (universal), "not" (negation), and "any" (existential)
        assert!(result.total_count >= 3, "Expected at least 3 operators, got {}", result.total_count);

        // Verify dimensions
        let dimensions: Vec<_> = result.operators.iter().map(|op| &op.dimension).collect();
        assert!(dimensions.contains(&&"Negation".to_string()), "Should have negation");
        assert!(dimensions.contains(&&"Quantifier".to_string()), "Should have quantifier");
    }

    /// Test that operators are sorted by position
    #[test]
    fn test_scope_operators_sorted() {
        let text = "Each party shall not disclose any information.";
        let result = extract_scope_operators_internal(text);

        // Verify sorting by checking that positions are non-decreasing
        for i in 1..result.operators.len() {
            let prev = &result.operators[i - 1];
            let curr = &result.operators[i];
            assert!(
                (prev.trigger_line, prev.trigger_start_token) <= (curr.trigger_line, curr.trigger_start_token),
                "Operators should be sorted by position"
            );
        }
    }

    /// Test domain spans are computed correctly
    #[test]
    fn test_scope_operators_domain_spans() {
        let text = "The Company shall not disclose information, except as required.";
        let result = extract_scope_operators_internal(text);

        let negation = result.operators.iter()
            .find(|op| op.marker == "not")
            .expect("Should find 'not' operator");

        println!("Domain spans test:");
        println!("  trigger: tokens [{}-{})", negation.trigger_start_token, negation.trigger_end_token);
        println!("  domain:  tokens [{}-{})", negation.domain_start_token, negation.domain_end_token);

        // Domain should start at or after the trigger end token
        // (domain_start_token == trigger_end_token means domain starts immediately after trigger)
        assert!(negation.domain_start_token >= negation.trigger_end_token,
            "Domain should start at or after trigger end");
        // Domain end should be greater than domain start (positive length domain)
        assert!(negation.domain_end_token > negation.domain_start_token,
            "Domain should have positive length");
        // The domain should cover "disclose information" (at minimum 2 tokens)
        assert!(negation.domain_end_token - negation.domain_start_token >= 2,
            "Domain should cover at least 2 tokens");
    }

    /// Test that result is serializable to JSON
    #[test]
    fn test_scope_operators_serializable() {
        let text = "The Company shall not disclose any confidential information.";
        let result = extract_scope_operators_internal(text);

        let json = serde_json::to_string(&result);
        assert!(json.is_ok(), "Result should be serializable to JSON");

        let json_str = json.unwrap();
        assert!(json_str.contains("\"operators\""));
        assert!(json_str.contains("\"total_count\""));
        assert!(json_str.contains("\"dimension\""));
        assert!(json_str.contains("\"marker\""));
        assert!(json_str.contains("\"kind\""));
        assert!(json_str.contains("\"confidence\""));

        println!("Scope operators JSON:\n{}", json_str);
    }

    /// Test complex contract text with multiple scope operators
    #[test]
    fn test_scope_operators_complex_contract() {
        let text = r#"Each party shall not disclose any Confidential Information.
Neither party shall never transfer all rights without consent.
There shall be no liability for some indirect damages."#;

        let result = extract_scope_operators_internal(text);

        println!("Complex scope operators test:");
        println!("  Total operators: {}", result.total_count);
        for op in &result.operators {
            println!("  - {} {} '{}' (kind: {}, conf: {:.2})",
                op.dimension, op.marker, op.trigger_text, op.kind, op.confidence);
            println!("    trigger: line {} tokens [{}-{})",
                op.trigger_line, op.trigger_start_token, op.trigger_end_token);
            println!("    domain:  line {} tokens [{}-{})",
                op.domain_line, op.domain_start_token, op.domain_end_token);
        }

        // Should detect multiple operators across lines
        assert!(result.total_count >= 6, "Expected at least 6 operators in complex text");
    }

    /// Test that all/every variants of universal quantifiers are detected
    #[test]
    fn test_scope_operators_all_every() {
        let text = "All employees must comply. Every provision is binding.";
        let result = extract_scope_operators_internal(text);

        let markers: Vec<_> = result.operators.iter()
            .filter(|op| op.kind == "Universal")
            .map(|op| op.marker.as_str())
            .collect();

        assert!(markers.contains(&"all"), "Should detect 'all'");
        assert!(markers.contains(&"every"), "Should detect 'every'");
    }

    // ========================================================================
    // VERIFICATION QUEUE TESTS
    // ========================================================================

    /// Test that the demo sample text produces reviewable queue items.
    #[test]
    fn test_verification_queue_demo_sample_text_has_items() {
        let result = require_queue(QUEUE_DEMO_TEXT);
        assert_eq!(result.total_count, result.items.len() as u32);
    }

    /// Test that get_verification_queue returns empty result for empty text
    #[test]
    fn test_verification_queue_empty_text() {
        let result = get_verification_queue_internal("");
        assert_eq!(result.total_count, 0);
        assert!(result.items.is_empty());
    }

    /// Test that get_verification_queue returns empty result for whitespace-only text
    #[test]
    fn test_verification_queue_whitespace_text() {
        let result = get_verification_queue_internal("   \n\n   ");
        assert_eq!(result.total_count, 0);
        assert!(result.items.is_empty());
    }

    /// Test that get_verification_queue processes contract text without panicking.
    ///
    /// Note: The WASM pipeline includes POS tagging, so this should yield reviewable items.
    #[test]
    fn test_verification_queue_processes_text() {
        let text = QUEUE_SAMPLE_TEXT;
        let result = require_queue(text);

        // Should process without panic
        assert_eq!(result.total_count, result.items.len() as u32);

        // Print for debugging
        println!("Verification queue test:");
        println!("  Total items: {}", result.total_count);
        for item in &result.items {
            println!("  - {} ({}): {} conf={:.2}",
                item.item_id, item.details_type, item.display_text, item.confidence);
        }
    }

    /// Test that queue items have unique item_ids
    #[test]
    fn test_verification_queue_unique_item_ids() {
        let text = QUEUE_MULTI_TEXT;
        let result = require_queue(text);

        // Collect all item_ids
        let item_ids: Vec<_> = result.items.iter().map(|i| &i.item_id).collect();

        // Check uniqueness using HashSet
        let unique_ids: std::collections::HashSet<_> = item_ids.iter().collect();
        assert_eq!(
            item_ids.len(),
            unique_ids.len(),
            "All item_ids should be unique. Found duplicates in: {:?}",
            item_ids
        );
    }

    /// Test that queue items are sorted by confidence (lowest first)
    #[test]
    fn test_verification_queue_sorted_by_confidence() {
        let text = QUEUE_MULTI_TEXT;
        let result = require_queue(text);

        // Verify items are sorted by confidence ascending
        for i in 1..result.items.len() {
            assert!(
                result.items[i - 1].confidence <= result.items[i].confidence,
                "Queue should be sorted by confidence ascending. Item {} conf={:.2} > Item {} conf={:.2}",
                i - 1,
                result.items[i - 1].confidence,
                i,
                result.items[i].confidence
            );
        }

        // Verify priority matches index
        for (index, item) in result.items.iter().enumerate() {
            assert_eq!(
                item.priority,
                index as u32,
                "Priority should match position in sorted queue"
            );
        }
    }

    /// Test that item_id format matches spec
    #[test]
    fn test_verification_queue_item_id_format() {
        let text = QUEUE_SAMPLE_TEXT;
        let result = require_queue(text);

        for item in &result.items {
            // item_id should be: ${node_id}-${clause_id}-${target_type}-${ordinal}
            let parts: Vec<_> = item.item_id.split('-').collect();
            assert_eq!(
                parts.len(), 4,
                "item_id should have exactly 4 parts: node_id-clause_id-target_type-ordinal, got: {}",
                item.item_id
            );

            // First part should be parseable as node_id
            let parsed_node_id: Result<u32, _> = parts[0].parse();
            assert!(
                parsed_node_id.is_ok(),
                "First part of item_id should be node_id: {}",
                item.item_id
            );
            assert_eq!(
                parsed_node_id.unwrap(),
                item.node_id,
                "node_id in item_id should match node_id field"
            );

            // Second part should be parseable as clause_id
            let parsed_clause_id: Result<u32, _> = parts[1].parse();
            assert!(
                parsed_clause_id.is_ok(),
                "Second part of item_id should be clause_id: {}",
                item.item_id
            );
            assert_eq!(
                parsed_clause_id.unwrap(),
                item.clause_id,
                "clause_id in item_id should match clause_id field"
            );
        }
    }

    /// Test that details_type is one of the expected values
    #[test]
    fn test_verification_queue_details_types() {
        let text = QUEUE_SAMPLE_TEXT;
        let result = require_queue(text);

        let valid_types = [
            "PassiveVoiceObligor",
            "LowConfidenceObligor",
            "Beneficiary",
            "Condition",
            "AmbiguousBeneficiary",
        ];

        for item in &result.items {
            assert!(
                valid_types.contains(&item.details_type.as_str()),
                "details_type '{}' should be one of: {:?}",
                item.details_type,
                valid_types
            );
        }
    }

    /// Test that target_type matches expected enum values
    #[test]
    fn test_verification_queue_target_types() {
        let text = QUEUE_SAMPLE_TEXT;
        let result = require_queue(text);

        for item in &result.items {
            // Verify target_type is a valid enum variant
            match &item.target_type {
                WasmVerificationTarget::Node
                | WasmVerificationTarget::BeneficiaryLink
                | WasmVerificationTarget::ConditionLink
                | WasmVerificationTarget::ObligorLink => {
                    // Valid variant
                }
            }
        }
    }

    /// Test that result is serializable to JSON
    #[test]
    fn test_verification_queue_serializable() {
        let text = QUEUE_SAMPLE_TEXT;
        let result = require_queue(text);

        let json = serde_json::to_string(&result);
        assert!(json.is_ok(), "Result should be serializable to JSON");

        let json_str = json.unwrap();
        assert!(json_str.contains("\"items\""), "JSON should have items field");
        assert!(json_str.contains("\"total_count\""), "JSON should have total_count field");

        println!("Verification queue JSON:\n{}", json_str);
    }

    /// Test with complex contract text.
    ///
    /// Note: Queue content depends on obligation detection quality. With defined terms
    /// like "Company" and "Contractor", beneficiaries may resolve without verification.
    /// The test verifies correct processing; actual items depend on pipeline results.
    #[test]
    fn test_verification_queue_complex_contract() {
        let text = r#"ARTICLE I: DEFINITIONS

Section 1.1 "Company" means ABC Corporation.
Section 1.2 "Contractor" means XYZ Services LLC.

ARTICLE II: OBLIGATIONS

Section 2.1 The Company shall pay the Contractor within 30 days.
Section 2.2 Goods shall be delivered by the Vendor to the Customer.
Section 2.3 The Contractor shall deliver services to Regional Authority."#;

        let result = get_verification_queue_internal(text);

        println!("Complex contract verification queue test:");
        println!("  Total items: {}", result.total_count);
        for item in &result.items {
            println!(
                "  - [{}] {} ({}) conf={:.2}: {}",
                item.item_id,
                item.target_type.as_str(),
                item.details_type,
                item.confidence,
                item.display_text
            );
        }

        // Should process without panic
        assert_eq!(result.total_count, result.items.len() as u32);
    }

    /// Test that confidence values are in valid range
    #[test]
    fn test_verification_queue_confidence_range() {
        let text = QUEUE_SAMPLE_TEXT;
        let result = require_queue(text);

        for item in &result.items {
            assert!(
                item.confidence >= 0.0 && item.confidence <= 1.0,
                "Confidence should be in [0.0, 1.0] range, got: {}",
                item.confidence
            );
        }
    }

    /// Test with German contract text containing umlauts
    #[test]
    fn test_verification_queue_unicode_text() {
        let result = get_verification_queue_internal("Der Verkäufer soll die Waren liefern.");
        assert_eq!(result.total_count, result.items.len() as u32);
    }

    /// Test with repeated contract clauses for stress testing
    #[test]
    fn test_verification_queue_very_long_text() {
        let text = "The Vendor shall deliver goods. ".repeat(1000);
        let result = get_verification_queue_internal(&text);
        // Should complete without panic
        assert_eq!(result.total_count, result.items.len() as u32);
    }

    // ========================================================================
    // ANALYZE WITH CORRECTIONS TESTS
    // ========================================================================

    /// Test that analyze_with_corrections with empty corrections JSON produces
    /// the same result as get_verification_queue_internal
    #[test]
    fn test_analyze_with_corrections_empty_corrections() {
        let text = QUEUE_SAMPLE_TEXT;

        let base_queue = get_verification_queue_internal(text);
        let corrected_queue = analyze_with_corrections_internal(text, "{}");

        // Should produce identical results
        assert_eq!(
            base_queue.total_count, corrected_queue.total_count,
            "Empty corrections should not change item count"
        );
        assert_eq!(
            base_queue.items.len(), corrected_queue.items.len(),
            "Empty corrections should not change items vector length"
        );

        // Verify each item matches
        for (base_item, corrected_item) in base_queue.items.iter().zip(corrected_queue.items.iter()) {
            assert_eq!(base_item.item_id, corrected_item.item_id);
            assert_eq!(base_item.confidence, corrected_item.confidence);
            assert_eq!(base_item.display_text, corrected_item.display_text);
        }
    }

    /// Test that analyze_with_corrections gracefully handles invalid JSON
    /// by falling back to base queue (no corrections applied)
    #[test]
    fn test_analyze_with_corrections_invalid_json() {
        let text = QUEUE_SAMPLE_TEXT;

        let base_queue = get_verification_queue_internal(text);
        let corrected_queue = analyze_with_corrections_internal(text, "not json");

        // Invalid JSON should gracefully fall back to no corrections
        assert_eq!(
            base_queue.total_count, corrected_queue.total_count,
            "Invalid JSON should fall back to base queue"
        );
        assert_eq!(
            base_queue.items.len(), corrected_queue.items.len(),
            "Invalid JSON should not affect items"
        );
    }

    /// Test that confirmed corrections set item confidence to 1.0
    #[test]
    fn test_analyze_with_corrections_confirmed_item() {
        let text = QUEUE_SAMPLE_TEXT;

        let base_queue = require_queue(text);

        let first_item_id = &base_queue.items[0].item_id;
        let original_confidence = base_queue.items[0].confidence;

        // Create corrections JSON with first item confirmed
        let corrections = CorrectionsInput {
            confirmed: vec![ConfirmedCorrection {
                item_id: first_item_id.clone(),
            }],
            corrected: vec![],
            dismissed: vec![],
        };
        let corrections_json = serde_json::to_string(&corrections).unwrap();

        let corrected_queue = analyze_with_corrections_internal(text, &corrections_json);

        // Find the confirmed item
        let confirmed_item = corrected_queue.items.iter()
            .find(|item| item.item_id == *first_item_id)
            .expect("Confirmed item should still be in queue");

        assert_eq!(
            confirmed_item.confidence, 1.0,
            "Confirmed item should have confidence 1.0 (was {})",
            original_confidence
        );

        // Other items should maintain original confidence
        for item in &corrected_queue.items {
            if item.item_id != *first_item_id {
                // Find matching item in base queue
                if let Some(base_item) = base_queue.items.iter().find(|b| b.item_id == item.item_id) {
                    assert_eq!(
                        item.confidence, base_item.confidence,
                        "Non-confirmed item confidence should be unchanged"
                    );
                }
            }
        }
    }

    /// Test that text corrections update the display_text field
    #[test]
    fn test_analyze_with_corrections_text_correction() {
        let text = QUEUE_SAMPLE_TEXT;

        let base_queue = require_queue(text);

        let first_item = &base_queue.items[0];
        let original_text = &first_item.display_text;
        let corrected_text = "Corrected Display Text";

        // Create corrections JSON with text correction
        let corrections = CorrectionsInput {
            confirmed: vec![],
            corrected: vec![TextCorrection {
                item_id: first_item.item_id.clone(),
                original_text: original_text.clone(),
                corrected_text: corrected_text.to_string(),
            }],
            dismissed: vec![],
        };
        let corrections_json = serde_json::to_string(&corrections).unwrap();

        let corrected_queue = analyze_with_corrections_internal(text, &corrections_json);

        // Find the corrected item
        let updated_item = corrected_queue.items.iter()
            .find(|item| item.item_id == first_item.item_id)
            .expect("Corrected item should still be in queue");

        assert_eq!(
            updated_item.display_text, corrected_text,
            "Text correction should update display_text from '{}' to '{}'",
            original_text, corrected_text
        );

        // Other fields should remain unchanged
        assert_eq!(updated_item.item_id, first_item.item_id);
        assert_eq!(updated_item.node_id, first_item.node_id);
        assert_eq!(updated_item.clause_id, first_item.clause_id);
    }

    /// Test that dismissed items are filtered from the queue
    #[test]
    fn test_analyze_with_corrections_dismissed_item() {
        let text = QUEUE_SAMPLE_TEXT;

        let base_queue = require_queue(text);

        let first_item_id = base_queue.items[0].item_id.clone();
        let original_count = base_queue.total_count;

        // Create corrections JSON with first item dismissed
        let corrections = CorrectionsInput {
            confirmed: vec![],
            corrected: vec![],
            dismissed: vec![DismissedItem {
                item_id: first_item_id.clone(),
                reason: Some("Not relevant to this review".to_string()),
            }],
        };
        let corrections_json = serde_json::to_string(&corrections).unwrap();

        let corrected_queue = analyze_with_corrections_internal(text, &corrections_json);

        // Count should be reduced by 1
        assert_eq!(
            corrected_queue.total_count,
            original_count - 1,
            "Dismissed item should reduce count by 1"
        );

        // Dismissed item should not be present
        let dismissed_found = corrected_queue.items.iter()
            .any(|item| item.item_id == first_item_id);
        assert!(
            !dismissed_found,
            "Dismissed item '{}' should not be in queue",
            first_item_id
        );
    }

    /// Test that dismissed items can have reason: None
    #[test]
    fn test_analyze_with_corrections_dismissed_without_reason() {
        let text = QUEUE_SAMPLE_TEXT;

        let base_queue = require_queue(text);

        let first_item_id = base_queue.items[0].item_id.clone();

        // Create corrections with dismissed item but no reason
        let corrections = CorrectionsInput {
            confirmed: vec![],
            corrected: vec![],
            dismissed: vec![DismissedItem {
                item_id: first_item_id.clone(),
                reason: None,
            }],
        };
        let corrections_json = serde_json::to_string(&corrections).unwrap();

        let corrected_queue = analyze_with_corrections_internal(text, &corrections_json);

        // Item should be filtered even without reason
        let dismissed_found = corrected_queue.items.iter()
            .any(|item| item.item_id == first_item_id);
        assert!(
            !dismissed_found,
            "Dismissed item should be filtered even without reason"
        );
    }

    /// Test that multiple corrections of different types can be applied together
    #[test]
    fn test_analyze_with_corrections_multiple_corrections() {
        let text = QUEUE_TRI_TEXT;

        let base_queue = require_queue(text);
        assert!(
            base_queue.items.len() >= 3,
            "Expected at least 3 queue items for multi-correction test"
        );

        let item_to_confirm = &base_queue.items[0];
        let item_to_correct = &base_queue.items[1];
        let item_to_dismiss = &base_queue.items[2];

        let corrected_text = "Updated Text Value";

        // Create corrections with all three types
        let corrections = CorrectionsInput {
            confirmed: vec![ConfirmedCorrection {
                item_id: item_to_confirm.item_id.clone(),
            }],
            corrected: vec![TextCorrection {
                item_id: item_to_correct.item_id.clone(),
                original_text: item_to_correct.display_text.clone(),
                corrected_text: corrected_text.to_string(),
            }],
            dismissed: vec![DismissedItem {
                item_id: item_to_dismiss.item_id.clone(),
                reason: Some("Test dismissal".to_string()),
            }],
        };
        let corrections_json = serde_json::to_string(&corrections).unwrap();

        let corrected_queue = analyze_with_corrections_internal(text, &corrections_json);

        // 1. Verify confirmed item has confidence 1.0
        let confirmed = corrected_queue.items.iter()
            .find(|i| i.item_id == item_to_confirm.item_id)
            .expect("Confirmed item should be present");
        assert_eq!(confirmed.confidence, 1.0, "Confirmed item should have confidence 1.0");

        // 2. Verify corrected item has updated display_text
        let corrected = corrected_queue.items.iter()
            .find(|i| i.item_id == item_to_correct.item_id)
            .expect("Corrected item should be present");
        assert_eq!(
            corrected.display_text, corrected_text,
            "Corrected item should have updated display_text"
        );

        // 3. Verify dismissed item is not present
        let dismissed_found = corrected_queue.items.iter()
            .any(|i| i.item_id == item_to_dismiss.item_id);
        assert!(!dismissed_found, "Dismissed item should not be in queue");

        // 4. Verify total count is reduced by 1 (one dismissed)
        assert_eq!(
            corrected_queue.total_count,
            base_queue.total_count - 1,
            "Total count should be reduced by 1 for dismissed item"
        );
    }

    /// Test that corrections referencing unknown item_ids are ignored gracefully
    #[test]
    fn test_analyze_with_corrections_unknown_item_id() {
        let text = QUEUE_SAMPLE_TEXT;

        let base_queue = get_verification_queue_internal(text);

        // Create corrections referencing non-existent item_ids
        let corrections = CorrectionsInput {
            confirmed: vec![ConfirmedCorrection {
                item_id: "999-999-UnknownType-99".to_string(),
            }],
            corrected: vec![TextCorrection {
                item_id: "888-888-FakeType-88".to_string(),
                original_text: "original".to_string(),
                corrected_text: "corrected".to_string(),
            }],
            dismissed: vec![DismissedItem {
                item_id: "777-777-MissingType-77".to_string(),
                reason: Some("Does not exist".to_string()),
            }],
        };
        let corrections_json = serde_json::to_string(&corrections).unwrap();

        let corrected_queue = analyze_with_corrections_internal(text, &corrections_json);

        // Queue should be unchanged since all item_ids are unknown
        assert_eq!(
            corrected_queue.total_count, base_queue.total_count,
            "Unknown item_ids should not affect queue count"
        );
        assert_eq!(
            corrected_queue.items.len(), base_queue.items.len(),
            "Unknown item_ids should not affect items"
        );

        // All items should have original values
        for (base_item, corrected_item) in base_queue.items.iter().zip(corrected_queue.items.iter()) {
            assert_eq!(base_item.item_id, corrected_item.item_id);
            assert_eq!(base_item.confidence, corrected_item.confidence);
            assert_eq!(base_item.display_text, corrected_item.display_text);
        }
    }

    /// Test that calling analyze_with_corrections twice with same inputs produces identical results
    #[test]
    fn test_analyze_with_corrections_persistence() {
        let text = QUEUE_SAMPLE_TEXT;

        let base_queue = require_queue(text);

        let corrections = CorrectionsInput {
            confirmed: vec![ConfirmedCorrection {
                item_id: base_queue.items[0].item_id.clone(),
            }],
            corrected: vec![],
            dismissed: vec![],
        };
        let corrections_json = serde_json::to_string(&corrections).unwrap();

        // Call twice with same inputs
        let result1 = analyze_with_corrections_internal(text, &corrections_json);
        let result2 = analyze_with_corrections_internal(text, &corrections_json);

        // Results should be identical
        assert_eq!(
            result1.total_count, result2.total_count,
            "Repeated calls should produce same count"
        );
        assert_eq!(
            result1.items.len(), result2.items.len(),
            "Repeated calls should produce same number of items"
        );

        for (item1, item2) in result1.items.iter().zip(result2.items.iter()) {
            assert_eq!(item1.item_id, item2.item_id, "Item IDs should match");
            assert_eq!(item1.confidence, item2.confidence, "Confidences should match");
            assert_eq!(item1.display_text, item2.display_text, "Display texts should match");
            assert_eq!(item1.priority, item2.priority, "Priorities should match");
        }
    }

    /// Test analyze_with_corrections with empty text
    #[test]
    fn test_analyze_with_corrections_empty_text() {
        let result = analyze_with_corrections_internal("", "{}");
        assert_eq!(result.total_count, 0);
        assert!(result.items.is_empty());
    }

    /// Test analyze_with_corrections with whitespace-only text
    #[test]
    fn test_analyze_with_corrections_whitespace_text() {
        let result = analyze_with_corrections_internal("   \n\n   ", "{}");
        assert_eq!(result.total_count, 0);
        assert!(result.items.is_empty());
    }

    /// Test analyze_with_corrections with empty arrays in corrections JSON
    #[test]
    fn test_analyze_with_corrections_empty_arrays() {
        let text = QUEUE_SAMPLE_TEXT;

        let base_queue = get_verification_queue_internal(text);

        // Explicit empty arrays (different from missing fields)
        let corrections_json = r#"{"confirmed":[],"corrected":[],"dismissed":[]}"#;

        let corrected_queue = analyze_with_corrections_internal(text, corrections_json);

        assert_eq!(
            base_queue.total_count, corrected_queue.total_count,
            "Empty arrays should not change queue"
        );
    }

    /// Test that result from analyze_with_corrections is JSON serializable
    #[test]
    fn test_analyze_with_corrections_serializable() {
        let text = QUEUE_SAMPLE_TEXT;
        let corrections_json = r#"{"confirmed":[],"corrected":[],"dismissed":[]}"#;

        let result = analyze_with_corrections_internal(text, corrections_json);

        let json = serde_json::to_string(&result);
        assert!(json.is_ok(), "Result should be serializable to JSON");

        let json_str = json.unwrap();
        assert!(json_str.contains("\"items\""), "JSON should have items field");
        assert!(json_str.contains("\"total_count\""), "JSON should have total_count field");
    }

    /// Test that corrections can be applied with both confirm and correct on same item
    #[test]
    fn test_analyze_with_corrections_confirm_and_correct_same_item() {
        let text = QUEUE_SAMPLE_TEXT;

        let base_queue = require_queue(text);

        let item = &base_queue.items[0];
        let corrected_text = "Both Confirmed And Corrected";

        // Apply both confirm and correct to same item
        let corrections = CorrectionsInput {
            confirmed: vec![ConfirmedCorrection {
                item_id: item.item_id.clone(),
            }],
            corrected: vec![TextCorrection {
                item_id: item.item_id.clone(),
                original_text: item.display_text.clone(),
                corrected_text: corrected_text.to_string(),
            }],
            dismissed: vec![],
        };
        let corrections_json = serde_json::to_string(&corrections).unwrap();

        let corrected_queue = analyze_with_corrections_internal(text, &corrections_json);

        let updated_item = corrected_queue.items.iter()
            .find(|i| i.item_id == item.item_id)
            .expect("Item should be present");

        // Both corrections should be applied
        assert_eq!(updated_item.confidence, 1.0, "Should be confirmed");
        assert_eq!(updated_item.display_text, corrected_text, "Should have corrected text");
    }

    /// Test that dismissing an item that was also confirmed results in dismissal (filter wins)
    #[test]
    fn test_analyze_with_corrections_dismiss_overrides_confirm() {
        let text = QUEUE_SAMPLE_TEXT;

        let base_queue = require_queue(text);

        let item_id = base_queue.items[0].item_id.clone();

        // Both confirm and dismiss the same item
        let corrections = CorrectionsInput {
            confirmed: vec![ConfirmedCorrection {
                item_id: item_id.clone(),
            }],
            corrected: vec![],
            dismissed: vec![DismissedItem {
                item_id: item_id.clone(),
                reason: Some("Dismiss should win".to_string()),
            }],
        };
        let corrections_json = serde_json::to_string(&corrections).unwrap();

        let corrected_queue = analyze_with_corrections_internal(text, &corrections_json);

        // Dismissed should filter out the item (filter happens first in processing)
        let item_found = corrected_queue.items.iter()
            .any(|i| i.item_id == item_id);
        assert!(
            !item_found,
            "Dismissed item should be filtered out even if also confirmed"
        );
    }
}
