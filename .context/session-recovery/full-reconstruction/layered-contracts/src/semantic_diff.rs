//! Semantic diff engine for contract comparison.
//!
//! This module transforms structural alignments from `DocumentAligner` into
//! semantic changes—understanding not just "what text changed" but "what the
//! change means legally."
//!
//! ## Architecture
//!
//! ```text
//! AlignmentResult (section pairs)
//!        │
//!        ▼
//! ┌─────────────────────────────────────┐
//! │     SemanticDiffEngine              │
//! ├─────────────────────────────────────┤
//! │ Section-level:                      │
//! │   • Added/removed/modified sections │
//! │   • Obligation modal changes        │
//! │   • Temporal expression changes     │
//! │                                     │
//! │ Document-level:                     │
//! │   • Term definition changes         │
//! │   • Blast radius computation        │
//! └─────────────────────────────────────┘
//!        │
//!        ▼
//! SemanticDiffResult
//!   • SemanticChange[] with risk levels
//!   • Two-sided party impacts (obligor + beneficiary)
//!   • Confidence scores
//! ```
//!
//! ## Extraction Levels
//!
//! - **Section**: Structural changes detected via `DocumentAligner`
//! - **Obligation**: Modal changes (shall→may) detected via `ObligationPhrase`
//! - **Term**: Definition changes with blast radius at document level
//!
//! Note: Clause-level extraction is not currently implemented.
//!
//! ## Usage
//!
//! ```ignore
//! let diff_engine = SemanticDiffEngine::new();
//! let diff = diff_engine.compute_diff(&alignment, &original_doc, &revised_doc);
//!
//! for change in &diff.changes {
//!     if change.risk_level >= RiskLevel::High {
//!         println!("{:?}: {}", change.change_type, change.explanation);
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use layered_nlp::x;
use serde::{Deserialize, Serialize};

use crate::document::ContractDocument;
use crate::document_aligner::{AlignedPair, AlignmentResult, AlignmentType};
use crate::obligation::ObligationType;

// ============================================================================
// CORE CHANGE TYPES
// ============================================================================

/// A semantic change detected between document versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticChange {
    /// Unique identifier for this change
    pub change_id: String,
    /// The type of semantic change
    pub change_type: SemanticChangeType,
    /// Risk level for this change
    pub risk_level: RiskLevel,
    /// Party-specific impacts
    pub party_impacts: Vec<PartyImpact>,
    /// Confidence in this change detection (0.0-1.0)
    pub confidence: f64,
    /// Source alignment that produced this change
    pub source_alignment_id: Option<String>,
    /// Human-readable explanation
    pub explanation: String,
    /// Signals contributing to this change
    pub signals: Vec<ChangeSignal>,
}

/// The type of semantic change detected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SemanticChangeType {
    /// Obligation modal changed (shall→may, may→shall, etc.)
    ObligationModal(ObligationModalChange),
    /// Condition added or removed from obligation
    ObligationCondition(ConditionChange),
    /// Obligor (who has the duty) changed
    ObligorChange(PartyChange),
    /// Beneficiary (who receives the benefit) changed
    BeneficiaryChange(PartyChange),
    /// Term definition modified
    TermDefinition(TermChange),
    /// Temporal expression changed (deadlines, durations)
    Temporal(TemporalChange),
    /// Section added (new content)
    SectionAdded {
        section_id: String,
        title: Option<String>,
    },
    /// Section removed
    SectionRemoved {
        section_id: String,
        title: Option<String>,
    },
    /// Section renumbered (ID changed, content same)
    SectionRenumbered { old_id: String, new_id: String },
}

// ============================================================================
// OBLIGATION CHANGE TYPES
// ============================================================================

/// Modal transformation in obligation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationModalChange {
    /// Original obligation type
    pub from: ObligationType,
    /// New obligation type
    pub to: ObligationType,
    /// Who has the obligation
    pub obligor: String,
    /// What action is involved
    pub action: String,
    /// Original text excerpt
    pub original_text: String,
    /// Revised text excerpt
    pub revised_text: String,
}

/// Condition change on obligation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionChange {
    /// New condition added
    Added {
        condition_type: String,
        condition_text: String,
        obligation_action: String,
    },
    /// Existing condition removed
    Removed {
        condition_type: String,
        condition_text: String,
        obligation_action: String,
    },
    /// Condition text modified
    Modified {
        condition_type: String,
        original_text: String,
        revised_text: String,
        obligation_action: String,
    },
}

/// Party reference change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyChange {
    /// Original party (None if added)
    pub from: Option<String>,
    /// New party (None if removed)
    pub to: Option<String>,
    /// Context of this change
    pub context: String,
}

// ============================================================================
// TERM CHANGE TYPES
// ============================================================================

/// Term definition change with blast radius.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TermChange {
    /// The term name
    pub term_name: String,
    /// Original definition text (None if new term)
    pub original_definition: Option<String>,
    /// Revised definition text (None if removed)
    pub revised_definition: Option<String>,
    /// Change classification
    pub change_class: TermChangeClass,
    /// Downstream references affected ("blast radius")
    pub affected_references: Vec<AffectedReference>,
}

/// Classification of how a term definition changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TermChangeClass {
    /// Definition scope expanded
    Expanded,
    /// Definition scope narrowed
    Narrowed,
    /// Definition restructured (unclear if expanded/narrowed)
    Restructured,
    /// New term added
    Added,
    /// Term removed
    Removed,
}

/// A reference affected by a term definition change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AffectedReference {
    /// Section where reference appears
    pub section_id: String,
    /// Line number
    pub line: usize,
    /// Snippet of text containing the reference
    pub context_snippet: String,
    /// Type of usage (obligation, condition, etc.)
    pub usage_type: ReferenceUsageType,
}

/// How a term is used at a reference site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceUsageType {
    /// Used in an obligation phrase
    Obligation,
    /// Used in a condition
    Condition,
    /// Used in a definition of another term
    Definition,
    /// General reference
    General,
}

// ============================================================================
// TEMPORAL CHANGE TYPES
// ============================================================================

/// Temporal expression change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemporalChange {
    /// Original temporal expression
    pub from: TemporalSnapshot,
    /// Revised temporal expression
    pub to: TemporalSnapshot,
    /// Context (what obligation this affects)
    pub context: String,
}

/// Snapshot of a temporal expression for comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemporalSnapshot {
    /// Raw text
    pub text: String,
    /// Numeric value if applicable
    pub value: Option<u32>,
    /// Unit if applicable
    pub unit: Option<String>,
}

// ============================================================================
// RISK AND IMPACT TYPES
// ============================================================================

/// Risk level for a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Low risk - minor or cosmetic change
    Low,
    /// Medium risk - substantive but expected
    Medium,
    /// High risk - significant legal/financial impact
    High,
    /// Critical - requires immediate attention
    Critical,
}

/// Party-specific impact of a change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartyImpact {
    /// Party name (normalized)
    pub party_name: String,
    /// Direction of impact for this party
    pub impact: ImpactDirection,
    /// Explanation of why this impacts the party
    pub reason: String,
}

/// Direction of impact for a party.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImpactDirection {
    /// Change is favorable to this party
    Favorable,
    /// Change is neutral
    Neutral,
    /// Change is unfavorable to this party
    Unfavorable,
}

/// Signal contributing to change detection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChangeSignal {
    pub name: String,
    pub value: String,
    pub weight: f64,
}

impl ChangeSignal {
    pub fn new(name: impl Into<String>, value: impl Into<String>, weight: f64) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            weight,
        }
    }
}

// ============================================================================
// RESULT TYPES
// ============================================================================

/// Complete semantic diff result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticDiffResult {
    /// All detected semantic changes
    pub changes: Vec<SemanticChange>,
    /// Summary statistics
    pub summary: DiffSummary,
    /// Party-level impact summaries
    pub party_summaries: Vec<PartySummaryDiff>,
    /// Warnings about uncertain detections
    pub warnings: Vec<String>,
}

impl SemanticDiffResult {
    /// Export to JSON for external processing.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Import from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Summary statistics for the diff.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffSummary {
    pub total_changes: usize,
    pub critical_changes: usize,
    pub high_risk_changes: usize,
    pub medium_risk_changes: usize,
    pub low_risk_changes: usize,
    pub obligation_changes: usize,
    pub term_changes: usize,
    pub temporal_changes: usize,
    pub structural_changes: usize,
}

impl DiffSummary {
    fn increment_risk(&mut self, risk: RiskLevel) {
        match risk {
            RiskLevel::Critical => self.critical_changes += 1,
            RiskLevel::High => self.high_risk_changes += 1,
            RiskLevel::Medium => self.medium_risk_changes += 1,
            RiskLevel::Low => self.low_risk_changes += 1,
        }
    }

    fn increment_type(&mut self, change_type: &SemanticChangeType) {
        match change_type {
            SemanticChangeType::ObligationModal(_)
            | SemanticChangeType::ObligationCondition(_)
            | SemanticChangeType::ObligorChange(_)
            | SemanticChangeType::BeneficiaryChange(_) => {
                self.obligation_changes += 1;
            }
            SemanticChangeType::TermDefinition(_) => {
                self.term_changes += 1;
            }
            SemanticChangeType::Temporal(_) => {
                self.temporal_changes += 1;
            }
            SemanticChangeType::SectionAdded { .. }
            | SemanticChangeType::SectionRemoved { .. }
            | SemanticChangeType::SectionRenumbered { .. } => {
                self.structural_changes += 1;
            }
        }
    }
}

/// Summary of impacts for a specific party.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartySummaryDiff {
    pub party_name: String,
    pub favorable_changes: usize,
    pub unfavorable_changes: usize,
    pub neutral_changes: usize,
    /// Net change in duties (positive = more duties)
    pub net_duty_change: i32,
    /// Net change in rights/permissions (positive = more rights)
    pub net_permission_change: i32,
    /// High-risk changes affecting this party
    pub high_risk_items: Vec<String>,
}

// ============================================================================
// EXTERNAL REVIEW INTEGRATION
// ============================================================================

/// Candidates for external review (low confidence changes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffReviewCandidates {
    pub candidates: Vec<SemanticChange>,
    pub total_changes: usize,
    pub review_threshold: f64,
}

/// Hint from external reviewer to adjust change detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHint {
    pub change_id: String,
    pub hint_type: DiffHintType,
    pub confidence: f64,
    pub source: String,
    pub explanation: Option<String>,
}

/// Type of external hint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffHintType {
    /// Confirm this change is real
    Confirm,
    /// Reject this change (false positive)
    Reject,
    /// Adjust risk level
    AdjustRisk(RiskLevel),
    /// Adjust party impact
    AdjustImpact {
        party: String,
        impact: ImpactDirection,
    },
    /// Classify term change as expanded/narrowed
    ClassifyTermChange(TermChangeClass),
}

// ============================================================================
// CONFIGURATION
// ============================================================================

/// Configuration for semantic diff.
#[derive(Debug, Clone)]
pub struct DiffConfig {
    /// Minimum confidence to include a change (default 0.5)
    pub min_confidence: f64,
    /// Confidence below this triggers review flag (default 0.75)
    pub review_threshold: f64,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.5,
            review_threshold: 0.75,
        }
    }
}

// ============================================================================
// INTERNAL EXTRACTION TYPES
// ============================================================================

/// Extracted semantic content from a section.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
struct SectionSemantics {
    obligations: Vec<ExtractedObligation>,
    term_definitions: Vec<ExtractedTerm>,
    term_references: Vec<ExtractedReference>,
    temporals: Vec<ExtractedTemporal>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ExtractedObligation {
    obligor: String,
    obligation_type: ObligationType,
    action: String,
    conditions: Vec<String>,
    beneficiary: Option<String>,
    source_line: usize,
    confidence: f64,
    full_text: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ExtractedTerm {
    term_name: String,
    definition_text: String,
    source_line: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ExtractedReference {
    term_name: String,
    usage_context: String,
    source_line: usize,
    in_obligation: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ExtractedTemporal {
    text: String,
    value: Option<u32>,
    unit: Option<String>,
    associated_action: Option<String>,
    source_line: usize,
}

// ============================================================================
// MAIN ENGINE
// ============================================================================

/// Engine for computing semantic diffs between document versions.
pub struct SemanticDiffEngine {
    config: DiffConfig,
    next_change_id: AtomicU32,
}

impl Default for SemanticDiffEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticDiffEngine {
    /// Create a new engine with default configuration.
    pub fn new() -> Self {
        Self {
            config: DiffConfig::default(),
            next_change_id: AtomicU32::new(1),
        }
    }

    /// Create an engine with custom configuration.
    pub fn with_config(config: DiffConfig) -> Self {
        Self {
            config,
            next_change_id: AtomicU32::new(1),
        }
    }

    /// Generate a unique change ID (thread-safe).
    fn generate_change_id(&self) -> String {
        let id = self.next_change_id.fetch_add(1, Ordering::Relaxed);
        format!("chg_{}", id)
    }

    /// Compute semantic diff from alignment result.
    pub fn compute_diff(
        &self,
        alignment: &AlignmentResult,
        original_doc: &ContractDocument,
        revised_doc: &ContractDocument,
    ) -> SemanticDiffResult {
        let mut changes = Vec::new();
        let mut warnings = Vec::new();

        // Extract document-wide term definitions for blast radius computation
        let original_terms = self.extract_document_terms(original_doc);
        let revised_terms = self.extract_document_terms(revised_doc);
        let revised_references = self.extract_document_references(revised_doc);

        // Process each alignment for section-level changes
        for pair in &alignment.alignments {
            let pair_changes = self.process_alignment_pair(
                pair,
                original_doc,
                revised_doc,
                &mut warnings,
            );
            changes.extend(pair_changes);
        }

        // Detect term changes at document level
        let term_changes =
            self.detect_term_changes(&original_terms, &revised_terms, &revised_references);
        changes.extend(term_changes);

        // Filter by minimum confidence
        changes.retain(|c| c.confidence >= self.config.min_confidence);

        // Build summary and party summaries
        let summary = self.build_summary(&changes);
        let party_summaries = self.build_party_summaries(&changes);

        SemanticDiffResult {
            changes,
            summary,
            party_summaries,
            warnings,
        }
    }

    /// Extract term definitions from entire document.
    fn extract_document_terms(&self, doc: &ContractDocument) -> HashMap<String, ExtractedTerm> {
        use crate::defined_term::DefinedTerm;
        use crate::scored::Scored;

        let mut terms = HashMap::new();

        for (idx, line) in doc.lines().iter().enumerate() {
            let source_line = doc.source_line_number(idx).unwrap_or(idx + 1);

            for found in line.find(&x::attr::<Scored<DefinedTerm>>()) {
                let scored = found.attr();
                let term = &scored.value;
                // Get definition text from the line
                let def_text = self.extract_line_text(line);

                terms.insert(
                    term.term_name.clone(),
                    ExtractedTerm {
                        term_name: term.term_name.clone(),
                        definition_text: def_text,
                        source_line,
                    },
                );
            }
        }

        terms
    }

    /// Extract term references from entire document.
    fn extract_document_references(&self, doc: &ContractDocument) -> Vec<ExtractedReference> {
        use crate::obligation::ObligationPhrase;
        use crate::scored::Scored;
        use crate::term_reference::TermReference;

        let mut references = Vec::new();

        for (idx, line) in doc.lines().iter().enumerate() {
            let source_line = doc.source_line_number(idx).unwrap_or(idx + 1);

            // Check if this line has obligations
            let has_obligation = !line
                .find(&x::attr::<Scored<ObligationPhrase>>())
                .is_empty();

            // Find term references
            for found in line.find(&x::attr::<Scored<TermReference>>()) {
                let scored = found.attr();
                let term_ref = &scored.value;

                // Get context snippet
                let context = self.extract_line_text(line);
                let context_snippet = if context.len() > 100 {
                    format!("{}...", &context[..100])
                } else {
                    context
                };

                references.push(ExtractedReference {
                    term_name: term_ref.term_name.clone(),
                    usage_context: context_snippet,
                    source_line,
                    in_obligation: has_obligation,
                });
            }
        }

        references
    }

    /// Process a single alignment pair to detect section-level changes.
    fn process_alignment_pair(
        &self,
        pair: &AlignedPair,
        original_doc: &ContractDocument,
        revised_doc: &ContractDocument,
        warnings: &mut Vec<String>,
    ) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        match pair.alignment_type {
            AlignmentType::Deleted => {
                // Section was removed
                if let Some(orig) = pair.original.first() {
                    changes.push(SemanticChange {
                        change_id: self.generate_change_id(),
                        change_type: SemanticChangeType::SectionRemoved {
                            section_id: orig.canonical_id.clone(),
                            title: orig.title.clone(),
                        },
                        risk_level: RiskLevel::High,
                        party_impacts: vec![],
                        confidence: pair.confidence,
                        source_alignment_id: None,
                        explanation: format!(
                            "Section {} \"{}\" was removed",
                            orig.canonical_id,
                            orig.title.as_deref().unwrap_or("untitled")
                        ),
                        signals: vec![ChangeSignal::new("alignment_type", "deleted", 1.0)],
                    });
                }
            }
            AlignmentType::Inserted => {
                // Section was added
                if let Some(rev) = pair.revised.first() {
                    changes.push(SemanticChange {
                        change_id: self.generate_change_id(),
                        change_type: SemanticChangeType::SectionAdded {
                            section_id: rev.canonical_id.clone(),
                            title: rev.title.clone(),
                        },
                        risk_level: RiskLevel::Medium,
                        party_impacts: vec![],
                        confidence: pair.confidence,
                        source_alignment_id: None,
                        explanation: format!(
                            "Section {} \"{}\" was added",
                            rev.canonical_id,
                            rev.title.as_deref().unwrap_or("untitled")
                        ),
                        signals: vec![ChangeSignal::new("alignment_type", "inserted", 1.0)],
                    });
                }
            }
            AlignmentType::Renumbered => {
                // Section was renumbered
                if let (Some(orig), Some(rev)) = (pair.original.first(), pair.revised.first()) {
                    changes.push(SemanticChange {
                        change_id: self.generate_change_id(),
                        change_type: SemanticChangeType::SectionRenumbered {
                            old_id: orig.canonical_id.clone(),
                            new_id: rev.canonical_id.clone(),
                        },
                        risk_level: RiskLevel::Low,
                        party_impacts: vec![],
                        confidence: pair.confidence,
                        source_alignment_id: None,
                        explanation: format!(
                            "Section renumbered from {} to {}",
                            orig.canonical_id, rev.canonical_id
                        ),
                        signals: vec![ChangeSignal::new("alignment_type", "renumbered", 1.0)],
                    });
                }
            }
            AlignmentType::Modified | AlignmentType::ExactMatch => {
                // Compare semantic content within aligned sections
                if let (Some(orig), Some(rev)) = (pair.original.first(), pair.revised.first()) {
                    let orig_semantics =
                        self.extract_section_semantics(orig, original_doc, warnings);
                    let rev_semantics =
                        self.extract_section_semantics(rev, revised_doc, warnings);

                    // Compare obligations
                    let obligation_changes =
                        self.compare_obligations(&orig_semantics, &rev_semantics, pair.confidence);
                    changes.extend(obligation_changes);

                    // Compare temporals
                    let temporal_changes =
                        self.compare_temporals(&orig_semantics, &rev_semantics, pair.confidence);
                    changes.extend(temporal_changes);
                }
            }
            AlignmentType::Split | AlignmentType::Merged | AlignmentType::Moved => {
                // For now, just note the structural change
                if !pair.original.is_empty() && !pair.revised.is_empty() {
                    let explanation = match pair.alignment_type {
                        AlignmentType::Split => format!(
                            "Section {} split into {} sections",
                            pair.original[0].canonical_id,
                            pair.revised.len()
                        ),
                        AlignmentType::Merged => format!(
                            "{} sections merged into {}",
                            pair.original.len(),
                            pair.revised[0].canonical_id
                        ),
                        AlignmentType::Moved => format!(
                            "Section {} moved to {}",
                            pair.original[0].canonical_id, pair.revised[0].canonical_id
                        ),
                        _ => String::new(),
                    };

                    warnings.push(explanation);
                }
            }
        }

        changes
    }

    /// Extract semantic content from a section.
    fn extract_section_semantics(
        &self,
        section_ref: &crate::document_aligner::SectionRef,
        doc: &ContractDocument,
        _warnings: &mut Vec<String>,
    ) -> SectionSemantics {
        use crate::obligation::ObligationPhrase;
        use crate::scored::Scored;
        use crate::temporal::TemporalExpression;

        let mut semantics = SectionSemantics::default();

        // Determine section boundaries: start_line to start_line + reasonable limit
        // Use a maximum of 50 source lines for a single section
        let section_start = section_ref.start_line;
        let section_end = section_start + 50;

        // Iterate through document lines, checking source line numbers
        for (idx, line) in doc.lines().iter().enumerate() {
            let source_line = doc.source_line_number(idx).unwrap_or(idx + 1);

            // Skip lines before section start
            if source_line < section_start {
                continue;
            }

            // Stop when we've passed the section end
            if source_line > section_end {
                break;
            }

            // Extract obligations
            for found in line.find(&x::attr::<Scored<ObligationPhrase>>()) {
                let scored = found.attr();
                let obl = &scored.value;
                let obligor_text = match &obl.obligor {
                    crate::obligation::ObligorReference::TermRef { term_name, .. } => {
                        term_name.clone()
                    }
                    crate::obligation::ObligorReference::PronounRef { resolved_to, .. } => {
                        resolved_to.clone()
                    }
                    crate::obligation::ObligorReference::NounPhrase { text } => text.clone(),
                };

                semantics.obligations.push(ExtractedObligation {
                    obligor: obligor_text,
                    obligation_type: obl.obligation_type,
                    action: obl.action.clone(),
                    conditions: obl
                        .conditions
                        .iter()
                        .map(|c| c.text_preview.clone())
                        .collect(),
                    beneficiary: None,
                    source_line,
                    confidence: scored.confidence,
                    full_text: self.extract_line_text(line),
                });
            }

            // Extract temporals
            for found in line.find(&x::attr::<TemporalExpression>()) {
                let temporal = found.attr();
                let (value, unit) = match &temporal.temporal_type {
                    crate::temporal::TemporalType::Duration {
                        value, unit, ..
                    } => {
                        let unit_str = match unit {
                            crate::temporal::DurationUnit::Days => "days",
                            crate::temporal::DurationUnit::Weeks => "weeks",
                            crate::temporal::DurationUnit::Months => "months",
                            crate::temporal::DurationUnit::Years => "years",
                            crate::temporal::DurationUnit::BusinessDays => "business days",
                        };
                        (Some(*value), Some(unit_str.to_string()))
                    }
                    _ => (None, None),
                };

                semantics.temporals.push(ExtractedTemporal {
                    text: temporal.text.clone(),
                    value,
                    unit,
                    associated_action: None,
                    source_line,
                });
            }
        }

        semantics
    }

    /// Extract text from an LLLine by concatenating token texts.
    fn extract_line_text(&self, line: &layered_nlp::LLLine) -> String {
        use layered_nlp::LToken;
        line.ll_tokens()
            .iter()
            .filter_map(|t| match t.get_token() {
                LToken::Text(text, _) => Some(text.as_str()),
                LToken::Value => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Compare obligations between two sections.
    fn compare_obligations(
        &self,
        original: &SectionSemantics,
        revised: &SectionSemantics,
        base_confidence: f64,
    ) -> Vec<SemanticChange> {
        let mut changes = Vec::new();
        let mut matched_revised: Vec<bool> = vec![false; revised.obligations.len()];

        for orig_obl in &original.obligations {
            // Find matching obligation in revised by obligor + similar action
            let mut best_match: Option<(usize, f64)> = None;

            for (idx, rev_obl) in revised.obligations.iter().enumerate() {
                if matched_revised[idx] {
                    continue;
                }

                // Match by obligor
                if self.normalize_party(&orig_obl.obligor)
                    != self.normalize_party(&rev_obl.obligor)
                {
                    continue;
                }

                // Compute action similarity (threshold: 0.7)
                const ACTION_SIMILARITY_THRESHOLD: f64 = 0.7;
                let similarity = self.action_similarity(&orig_obl.action, &rev_obl.action);
                if similarity >= ACTION_SIMILARITY_THRESHOLD {
                    if best_match.is_none() || similarity > best_match.unwrap().1 {
                        best_match = Some((idx, similarity));
                    }
                }
            }

            if let Some((idx, _similarity)) = best_match {
                matched_revised[idx] = true;
                let rev_obl = &revised.obligations[idx];

                // Check for modal change
                if orig_obl.obligation_type != rev_obl.obligation_type {
                    let (risk_level, party_impacts) = self.score_modal_change(
                        orig_obl.obligation_type,
                        rev_obl.obligation_type,
                        &orig_obl.obligor,
                        orig_obl.beneficiary.as_deref(),
                    );

                    changes.push(SemanticChange {
                        change_id: self.generate_change_id(),
                        change_type: SemanticChangeType::ObligationModal(ObligationModalChange {
                            from: orig_obl.obligation_type,
                            to: rev_obl.obligation_type,
                            obligor: orig_obl.obligor.clone(),
                            action: orig_obl.action.clone(),
                            original_text: orig_obl.full_text.clone(),
                            revised_text: rev_obl.full_text.clone(),
                        }),
                        risk_level,
                        party_impacts,
                        confidence: base_confidence * orig_obl.confidence * rev_obl.confidence,
                        source_alignment_id: None,
                        explanation: format!(
                            "{}'s obligation \"{}\" changed from {:?} to {:?}",
                            orig_obl.obligor,
                            orig_obl.action,
                            orig_obl.obligation_type,
                            rev_obl.obligation_type
                        ),
                        signals: vec![
                            ChangeSignal::new(
                                "from_modal",
                                format!("{:?}", orig_obl.obligation_type),
                                1.0,
                            ),
                            ChangeSignal::new(
                                "to_modal",
                                format!("{:?}", rev_obl.obligation_type),
                                1.0,
                            ),
                        ],
                    });
                }

                // Check for condition changes
                let condition_changes = self.compare_conditions(orig_obl, rev_obl, base_confidence);
                changes.extend(condition_changes);
            }
            // Note: We don't report "removed obligations" within a section here
            // because that's covered by section-level deletion
        }

        changes
    }

    /// Compare conditions between two obligations.
    fn compare_conditions(
        &self,
        original: &ExtractedObligation,
        revised: &ExtractedObligation,
        base_confidence: f64,
    ) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        // Find added conditions
        for rev_cond in &revised.conditions {
            if !original
                .conditions
                .iter()
                .any(|c| self.normalize_text(c) == self.normalize_text(rev_cond))
            {
                changes.push(SemanticChange {
                    change_id: self.generate_change_id(),
                    change_type: SemanticChangeType::ObligationCondition(ConditionChange::Added {
                        condition_type: "condition".to_string(),
                        condition_text: rev_cond.clone(),
                        obligation_action: original.action.clone(),
                    }),
                    risk_level: RiskLevel::Medium,
                    party_impacts: vec![PartyImpact {
                        party_name: original.obligor.clone(),
                        impact: ImpactDirection::Favorable,
                        reason: "Added condition may limit when obligation applies".to_string(),
                    }],
                    confidence: base_confidence * 0.9,
                    source_alignment_id: None,
                    explanation: format!(
                        "Condition \"{}\" added to {}'s obligation",
                        rev_cond, original.obligor
                    ),
                    signals: vec![ChangeSignal::new("condition_added", rev_cond.clone(), 1.0)],
                });
            }
        }

        // Find removed conditions
        for orig_cond in &original.conditions {
            if !revised
                .conditions
                .iter()
                .any(|c| self.normalize_text(c) == self.normalize_text(orig_cond))
            {
                changes.push(SemanticChange {
                    change_id: self.generate_change_id(),
                    change_type: SemanticChangeType::ObligationCondition(
                        ConditionChange::Removed {
                            condition_type: "condition".to_string(),
                            condition_text: orig_cond.clone(),
                            obligation_action: original.action.clone(),
                        },
                    ),
                    risk_level: RiskLevel::Medium,
                    party_impacts: vec![PartyImpact {
                        party_name: original.obligor.clone(),
                        impact: ImpactDirection::Unfavorable,
                        reason: "Removed condition makes obligation unconditional".to_string(),
                    }],
                    confidence: base_confidence * 0.9,
                    source_alignment_id: None,
                    explanation: format!(
                        "Condition \"{}\" removed from {}'s obligation",
                        orig_cond, original.obligor
                    ),
                    signals: vec![ChangeSignal::new("condition_removed", orig_cond.clone(), 1.0)],
                });
            }
        }

        changes
    }

    /// Compare temporal expressions.
    /// Uses greedy matching by unit type to pair related temporals.
    fn compare_temporals(
        &self,
        original: &SectionSemantics,
        revised: &SectionSemantics,
        base_confidence: f64,
    ) -> Vec<SemanticChange> {
        let mut changes = Vec::new();
        let mut matched_revised: Vec<bool> = vec![false; revised.temporals.len()];

        // Match original temporals to revised temporals by unit
        // Each original can match at most one revised (greedy: first match wins)
        for orig_temp in &original.temporals {
            // Find best matching revised temporal (same unit, not yet matched)
            let mut best_match: Option<(usize, u32)> = None;

            for (rev_idx, rev_temp) in revised.temporals.iter().enumerate() {
                if matched_revised[rev_idx] {
                    continue;
                }

                // Must have same unit to be comparable
                if orig_temp.unit != rev_temp.unit {
                    continue;
                }

                if let (Some(orig_val), Some(rev_val)) = (orig_temp.value, rev_temp.value) {
                    // Prefer closer values as better matches
                    let diff = (orig_val as i64 - rev_val as i64).unsigned_abs() as u32;
                    match best_match {
                        None => best_match = Some((rev_idx, diff)),
                        Some((_, prev_diff)) if diff < prev_diff => {
                            best_match = Some((rev_idx, diff));
                        }
                        _ => {}
                    }
                }
            }

            // If we found a match, create change if values differ
            if let Some((rev_idx, _)) = best_match {
                matched_revised[rev_idx] = true;
                let rev_temp = &revised.temporals[rev_idx];

                if let (Some(orig_val), Some(rev_val)) = (orig_temp.value, rev_temp.value) {
                    if orig_val != rev_val {
                        let risk = if rev_val < orig_val {
                            RiskLevel::Medium // Shorter deadline = more pressure
                        } else {
                            RiskLevel::Low // Longer deadline = more lenient
                        };

                        changes.push(SemanticChange {
                            change_id: self.generate_change_id(),
                            change_type: SemanticChangeType::Temporal(TemporalChange {
                                from: TemporalSnapshot {
                                    text: orig_temp.text.clone(),
                                    value: Some(orig_val),
                                    unit: orig_temp.unit.clone(),
                                },
                                to: TemporalSnapshot {
                                    text: rev_temp.text.clone(),
                                    value: Some(rev_val),
                                    unit: rev_temp.unit.clone(),
                                },
                                context: orig_temp
                                    .associated_action
                                    .clone()
                                    .unwrap_or_default(),
                            }),
                            risk_level: risk,
                            party_impacts: vec![],
                            confidence: base_confidence * 0.85,
                            source_alignment_id: None,
                            explanation: format!(
                                "Duration changed from {} to {}",
                                orig_temp.text, rev_temp.text
                            ),
                            signals: vec![
                                ChangeSignal::new(
                                    "from_duration",
                                    orig_temp.text.clone(),
                                    1.0,
                                ),
                                ChangeSignal::new(
                                    "to_duration",
                                    rev_temp.text.clone(),
                                    1.0,
                                ),
                            ],
                        });
                    }
                }
            }
        }

        changes
    }

    /// Detect term definition changes at document level.
    fn detect_term_changes(
        &self,
        original_terms: &HashMap<String, ExtractedTerm>,
        revised_terms: &HashMap<String, ExtractedTerm>,
        revised_references: &[ExtractedReference],
    ) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        // Find modified and removed terms
        for (term_name, orig_term) in original_terms {
            if let Some(rev_term) = revised_terms.get(term_name) {
                // Term exists in both - check if definition changed
                if self.normalize_text(&orig_term.definition_text)
                    != self.normalize_text(&rev_term.definition_text)
                {
                    let change_class = self.classify_term_change(
                        &orig_term.definition_text,
                        &rev_term.definition_text,
                    );

                    // Compute blast radius (find all references affected by this term change)
                    let affected = self.compute_blast_radius(term_name, revised_references);

                    let risk = if affected.len() > 5 {
                        RiskLevel::High
                    } else if affected.is_empty() {
                        RiskLevel::Low
                    } else {
                        RiskLevel::Medium
                    };

                    changes.push(SemanticChange {
                        change_id: self.generate_change_id(),
                        change_type: SemanticChangeType::TermDefinition(TermChange {
                            term_name: term_name.clone(),
                            original_definition: Some(orig_term.definition_text.clone()),
                            revised_definition: Some(rev_term.definition_text.clone()),
                            change_class,
                            affected_references: affected.clone(),
                        }),
                        risk_level: risk,
                        party_impacts: vec![],
                        confidence: 0.9,
                        source_alignment_id: None,
                        explanation: format!(
                            "Definition of \"{}\" was {:?} ({} references affected)",
                            term_name,
                            change_class,
                            affected.len()
                        ),
                        signals: vec![
                            ChangeSignal::new("term_name", term_name.clone(), 1.0),
                            ChangeSignal::new(
                                "blast_radius",
                                affected.len().to_string(),
                                0.5,
                            ),
                        ],
                    });
                }
            } else {
                // Term was removed
                changes.push(SemanticChange {
                    change_id: self.generate_change_id(),
                    change_type: SemanticChangeType::TermDefinition(TermChange {
                        term_name: term_name.clone(),
                        original_definition: Some(orig_term.definition_text.clone()),
                        revised_definition: None,
                        change_class: TermChangeClass::Removed,
                        affected_references: vec![],
                    }),
                    risk_level: RiskLevel::High,
                    party_impacts: vec![],
                    confidence: 0.95,
                    source_alignment_id: None,
                    explanation: format!("Defined term \"{}\" was removed", term_name),
                    signals: vec![ChangeSignal::new("term_removed", term_name.clone(), 1.0)],
                });
            }
        }

        // Find added terms
        for (term_name, rev_term) in revised_terms {
            if !original_terms.contains_key(term_name) {
                let affected = self.compute_blast_radius(term_name, revised_references);

                changes.push(SemanticChange {
                    change_id: self.generate_change_id(),
                    change_type: SemanticChangeType::TermDefinition(TermChange {
                        term_name: term_name.clone(),
                        original_definition: None,
                        revised_definition: Some(rev_term.definition_text.clone()),
                        change_class: TermChangeClass::Added,
                        affected_references: affected.clone(),
                    }),
                    risk_level: RiskLevel::Low,
                    party_impacts: vec![],
                    confidence: 0.95,
                    source_alignment_id: None,
                    explanation: format!(
                        "New defined term \"{}\" ({} references)",
                        term_name,
                        affected.len()
                    ),
                    signals: vec![ChangeSignal::new("term_added", term_name.clone(), 1.0)],
                });
            }
        }

        changes
    }

    /// Classify how a term definition changed.
    fn classify_term_change(&self, original: &str, revised: &str) -> TermChangeClass {
        let orig_words: std::collections::HashSet<_> = original
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect();
        let rev_words: std::collections::HashSet<_> = revised
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect();

        let orig_only = orig_words.difference(&rev_words).count();
        let rev_only = rev_words.difference(&orig_words).count();

        if orig_only == 0 && rev_only > 0 {
            TermChangeClass::Expanded
        } else if rev_only == 0 && orig_only > 0 {
            TermChangeClass::Narrowed
        } else {
            TermChangeClass::Restructured
        }
    }

    /// Compute blast radius for a term change.
    fn compute_blast_radius(
        &self,
        term_name: &str,
        references: &[ExtractedReference],
    ) -> Vec<AffectedReference> {
        const MAX_REFS: usize = 20;
        let normalized = term_name.to_lowercase();

        references
            .iter()
            .filter(|r| r.term_name.to_lowercase() == normalized)
            .take(MAX_REFS)
            .map(|r| {
                let usage_type = if r.in_obligation {
                    ReferenceUsageType::Obligation
                } else {
                    ReferenceUsageType::General
                };

                AffectedReference {
                    section_id: String::new(),
                    line: r.source_line,
                    context_snippet: r.usage_context.clone(),
                    usage_type,
                }
            })
            .collect()
    }

    /// Score a modal change and compute party impacts for both obligor and beneficiary.
    /// Returns impacts for both parties - modal changes affect parties in opposite directions.
    fn score_modal_change(
        &self,
        from: ObligationType,
        to: ObligationType,
        obligor: &str,
        beneficiary: Option<&str>,
    ) -> (RiskLevel, Vec<PartyImpact>) {
        // Determine impact on obligor (the party with the duty/permission/prohibition)
        let (risk, obligor_impact, obligor_reason, beneficiary_reason) = match (from, to) {
            // Duty → Permission: obligor gains discretion, beneficiary loses certainty
            (ObligationType::Duty, ObligationType::Permission) => (
                RiskLevel::Critical,
                ImpactDirection::Favorable,
                "Mandatory obligation became discretionary",
                "Lost certainty of receiving promised action",
            ),
            // Duty → Prohibition: obligor now prohibited, beneficiary loses benefit entirely
            (ObligationType::Duty, ObligationType::Prohibition) => (
                RiskLevel::High,
                ImpactDirection::Unfavorable,
                "Required action is now prohibited",
                "Action that was owed is now forbidden",
            ),
            // Permission → Duty: obligor now required, beneficiary gains certainty
            (ObligationType::Permission, ObligationType::Duty) => (
                RiskLevel::Medium,
                ImpactDirection::Unfavorable,
                "Optional action became mandatory",
                "Gained certainty of receiving action",
            ),
            // Permission → Prohibition: obligor loses option, beneficiary loses potential benefit
            (ObligationType::Permission, ObligationType::Prohibition) => (
                RiskLevel::High,
                ImpactDirection::Unfavorable,
                "Permitted action is now prohibited",
                "Potential benefit is now forbidden",
            ),
            // Prohibition → Permission: obligor gains option, beneficiary may receive action
            (ObligationType::Prohibition, ObligationType::Permission) => (
                RiskLevel::Medium,
                ImpactDirection::Favorable,
                "Prohibited action is now permitted",
                "May now receive previously forbidden action",
            ),
            // Prohibition → Duty: obligor now required, beneficiary gains certainty
            (ObligationType::Prohibition, ObligationType::Duty) => (
                RiskLevel::High,
                ImpactDirection::Unfavorable,
                "Prohibited action is now required",
                "Gained certainty of receiving action",
            ),
            // Same type (shouldn't happen)
            _ => (
                RiskLevel::Low,
                ImpactDirection::Neutral,
                "Modal unchanged",
                "No change in expected action",
            ),
        };

        // Beneficiary impact is generally opposite of obligor impact for duty changes
        let beneficiary_impact = match obligor_impact {
            ImpactDirection::Favorable => ImpactDirection::Unfavorable,
            ImpactDirection::Unfavorable => ImpactDirection::Favorable,
            ImpactDirection::Neutral => ImpactDirection::Neutral,
        };

        let mut impacts = vec![PartyImpact {
            party_name: obligor.to_string(),
            impact: obligor_impact,
            reason: obligor_reason.to_string(),
        }];

        // Add beneficiary impact if known
        if let Some(beneficiary_name) = beneficiary {
            if !beneficiary_name.is_empty() {
                impacts.push(PartyImpact {
                    party_name: beneficiary_name.to_string(),
                    impact: beneficiary_impact,
                    reason: beneficiary_reason.to_string(),
                });
            }
        }

        (risk, impacts)
    }

    /// Normalize party name for comparison.
    fn normalize_party(&self, name: &str) -> String {
        name.to_lowercase()
            .trim()
            .replace("the ", "")
            .replace("  ", " ")
    }

    /// Normalize text for comparison.
    fn normalize_text(&self, text: &str) -> String {
        text.to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Compute action similarity using word overlap.
    fn action_similarity(&self, a: &str, b: &str) -> f64 {
        let a_words: std::collections::HashSet<_> =
            a.split_whitespace().map(|w| w.to_lowercase()).collect();
        let b_words: std::collections::HashSet<_> =
            b.split_whitespace().map(|w| w.to_lowercase()).collect();

        if a_words.is_empty() && b_words.is_empty() {
            return 1.0;
        }

        let intersection = a_words.intersection(&b_words).count();
        let union = a_words.union(&b_words).count();

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }

    /// Build summary statistics.
    fn build_summary(&self, changes: &[SemanticChange]) -> DiffSummary {
        let mut summary = DiffSummary {
            total_changes: changes.len(),
            ..Default::default()
        };

        for change in changes {
            summary.increment_risk(change.risk_level);
            summary.increment_type(&change.change_type);
        }

        summary
    }

    /// Build party-level summaries.
    fn build_party_summaries(&self, changes: &[SemanticChange]) -> Vec<PartySummaryDiff> {
        let mut party_stats: HashMap<String, PartySummaryDiff> = HashMap::new();

        for change in changes {
            for impact in &change.party_impacts {
                let entry = party_stats
                    .entry(self.normalize_party(&impact.party_name))
                    .or_insert_with(|| PartySummaryDiff {
                        party_name: impact.party_name.clone(),
                        favorable_changes: 0,
                        unfavorable_changes: 0,
                        neutral_changes: 0,
                        net_duty_change: 0,
                        net_permission_change: 0,
                        high_risk_items: vec![],
                    });

                match impact.impact {
                    ImpactDirection::Favorable => entry.favorable_changes += 1,
                    ImpactDirection::Unfavorable => entry.unfavorable_changes += 1,
                    ImpactDirection::Neutral => entry.neutral_changes += 1,
                }

                if change.risk_level >= RiskLevel::High {
                    entry.high_risk_items.push(change.explanation.clone());
                }

                // Track duty/permission changes
                if let SemanticChangeType::ObligationModal(modal) = &change.change_type {
                    if self.normalize_party(&modal.obligor)
                        == self.normalize_party(&impact.party_name)
                    {
                        match (modal.from, modal.to) {
                            (ObligationType::Permission, ObligationType::Duty) => {
                                entry.net_duty_change += 1;
                                entry.net_permission_change -= 1;
                            }
                            (ObligationType::Duty, ObligationType::Permission) => {
                                entry.net_duty_change -= 1;
                                entry.net_permission_change += 1;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        party_stats.into_values().collect()
    }

    /// Get low-confidence changes for external review.
    pub fn get_review_candidates(
        &self,
        result: &SemanticDiffResult,
        threshold: f64,
    ) -> DiffReviewCandidates {
        let candidates: Vec<_> = result
            .changes
            .iter()
            .filter(|c| c.confidence < threshold)
            .cloned()
            .collect();

        DiffReviewCandidates {
            candidates,
            total_changes: result.changes.len(),
            review_threshold: threshold,
        }
    }

    /// Apply external hints to refine diff result.
    pub fn apply_hints(&self, mut result: SemanticDiffResult, hints: &[DiffHint]) -> SemanticDiffResult {
        for hint in hints {
            if let Some(change) = result.changes.iter_mut().find(|c| c.change_id == hint.change_id)
            {
                match &hint.hint_type {
                    DiffHintType::Confirm => {
                        // Boost confidence: take weighted average with hint confidence
                        change.confidence = (change.confidence + hint.confidence) / 2.0;
                    }
                    DiffHintType::Reject => {
                        change.confidence = 0.0;
                    }
                    DiffHintType::AdjustRisk(new_risk) => {
                        change.risk_level = *new_risk;
                    }
                    DiffHintType::AdjustImpact { party, impact } => {
                        if let Some(pi) = change
                            .party_impacts
                            .iter_mut()
                            .find(|p| self.normalize_party(&p.party_name) == self.normalize_party(party))
                        {
                            pi.impact = *impact;
                        } else {
                            change.party_impacts.push(PartyImpact {
                                party_name: party.clone(),
                                impact: *impact,
                                reason: hint.explanation.clone().unwrap_or_default(),
                            });
                        }
                    }
                    DiffHintType::ClassifyTermChange(new_class) => {
                        if let SemanticChangeType::TermDefinition(ref mut term_change) =
                            change.change_type
                        {
                            term_change.change_class = *new_class;
                        }
                    }
                }

                if let Some(ref explanation) = hint.explanation {
                    change.signals.push(ChangeSignal::new(
                        format!("hint_{}", hint.source),
                        explanation.clone(),
                        hint.confidence,
                    ));
                }
            }
        }

        // Filter out rejected changes
        result.changes.retain(|c| c.confidence > 0.0);

        // Rebuild summaries
        result.summary = self.build_summary(&result.changes);
        result.party_summaries = self.build_party_summaries(&result.changes);

        result
    }
}
