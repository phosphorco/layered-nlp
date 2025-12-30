//! Document alignment for contract comparison.
//!
//! This module provides `DocumentAligner` which aligns sections between two versions
//! of a contract document, producing alignment pairs with confidence scores.
//!
//! ## Architecture: Hybrid Analysis Pipeline
//!
//! The Rust layer handles **deterministic extraction** and **scaffolding**, while
//! **semantic analysis** requiring broader context (LLMs, domain experts, external
//! knowledge) happens externally.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     RUST LAYER                              │
//! │  (Deterministic, Fast, Confidence-Scored)                   │
//! ├─────────────────────────────────────────────────────────────┤
//! │  1. Extract structural signals (IDs, titles, positions)     │
//! │  2. Extract semantic signals (obligations, terms, refs)     │
//! │  3. Compute initial alignment with confidence scores        │
//! │  4. Export candidate alignments + signals for review        │
//! └─────────────────────────────────────────────────────────────┘
//!                           │ Export: AlignmentCandidate[]
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   EXTERNAL LAYER                            │
//! │  (LLM, Expert Review, Domain Knowledge)                     │
//! ├─────────────────────────────────────────────────────────────┤
//! │  • "These sections discuss the same topic despite rewording"│
//! │  • "This split was intentional restructuring"               │
//! │  • Confidence adjustments based on legal expertise          │
//! └─────────────────────────────────────────────────────────────┘
//!                           │ Import: AlignmentHint[]
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     RUST LAYER                              │
//! │  (Incorporate External Signals)                             │
//! ├─────────────────────────────────────────────────────────────┤
//! │  5. Apply external hints to refine alignments               │
//! │  6. Recompute confidence with external signals              │
//! │  7. Produce final AlignmentResult                           │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ### Simple (No External Review)
//! ```ignore
//! let aligner = DocumentAligner::new();
//! let result = aligner.align(&orig_struct, &rev_struct, &orig_doc, &rev_doc);
//! ```
//!
//! ### With LLM Review
//! ```ignore
//! let aligner = DocumentAligner::new();
//!
//! // Phase 1: Get candidates
//! let candidates = aligner.compute_candidates(&orig_struct, &rev_struct, &orig_doc, &rev_doc);
//!
//! // Export uncertain candidates for LLM
//! let uncertain: Vec<_> = candidates.candidates.iter()
//!     .filter(|c| c.confidence < 0.75)
//!     .collect();
//! let json = serde_json::to_string(&uncertain)?;
//!
//! // Send to LLM, get hints back
//! let hints: Vec<AlignmentHint> = call_llm_for_alignment_review(json)?;
//!
//! // Phase 2: Apply hints
//! let result = aligner.apply_hints(candidates, &hints);
//! ```

use std::collections::{HashMap, HashSet};

use pathfinding::kuhn_munkres::{kuhn_munkres_min, Weights};
use serde::{Deserialize, Serialize};

use crate::document::ContractDocument;
use crate::document_structure::{DocumentStructure, SectionNode};
use crate::section_header::SectionIdentifier;

/// The type of alignment between sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignmentType {
    /// Same ID, same/similar content
    ExactMatch,
    /// Different ID, same content (section numbering changed)
    Renumbered,
    /// Same content, different hierarchy position
    Moved,
    /// Same section identity, content changed
    Modified,
    /// One original section split into multiple revised sections
    Split,
    /// Multiple original sections merged into one revised section
    Merged,
    /// Section only exists in original (was removed)
    Deleted,
    /// Section only exists in revised (was added)
    Inserted,
}

/// Lightweight reference to a section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionRef {
    /// Canonical identifier (e.g., "ARTICLE:R1", "SECTION:1.1")
    pub canonical_id: String,
    /// Optional title (e.g., "Payment Terms")
    pub title: Option<String>,
    /// Line number where section starts
    pub start_line: usize,
    /// Nesting depth (1 = top-level)
    pub depth: u8,
}

impl SectionRef {
    /// Create a SectionRef from a SectionNode.
    pub fn from_node(node: &SectionNode) -> Self {
        Self {
            canonical_id: node.header.identifier.canonical(),
            title: node.header.title.clone(),
            start_line: node.start_line,
            depth: node.depth(),
        }
    }
}

/// Individual signal contributing to alignment decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentSignal {
    /// Name of the signal (e.g., "canonical_id", "title", "semantic", "text")
    pub name: String,
    /// Signal score (0.0-1.0)
    pub score: f64,
    /// Weight applied to this signal
    pub weight: f64,
}

impl AlignmentSignal {
    fn new(name: impl Into<String>, score: f64, weight: f64) -> Self {
        Self {
            name: name.into(),
            score,
            weight,
        }
    }

    fn weighted_score(&self) -> f64 {
        self.score * self.weight
    }
}

/// A single alignment between section(s).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignedPair {
    /// Sections in the original document (empty for insertions)
    pub original: Vec<SectionRef>,
    /// Sections in the revised document (empty for deletions)
    pub revised: Vec<SectionRef>,
    /// Type of alignment
    pub alignment_type: AlignmentType,
    /// Overall confidence score (0.0-1.0)
    pub confidence: f64,
    /// Individual signals contributing to the alignment
    pub signals: Vec<AlignmentSignal>,
}

/// Statistics about the alignment result.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AlignmentStats {
    pub total_original: usize,
    pub total_revised: usize,
    pub exact_matches: usize,
    pub renumbered: usize,
    pub moved: usize,
    pub modified: usize,
    pub split: usize,
    pub merged: usize,
    pub deleted: usize,
    pub inserted: usize,
}

impl AlignmentStats {
    fn increment(&mut self, alignment_type: AlignmentType) {
        match alignment_type {
            AlignmentType::ExactMatch => self.exact_matches += 1,
            AlignmentType::Renumbered => self.renumbered += 1,
            AlignmentType::Moved => self.moved += 1,
            AlignmentType::Modified => self.modified += 1,
            AlignmentType::Split => self.split += 1,
            AlignmentType::Merged => self.merged += 1,
            AlignmentType::Deleted => self.deleted += 1,
            AlignmentType::Inserted => self.inserted += 1,
        }
    }
}

/// Complete alignment result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentResult {
    /// All aligned pairs
    pub alignments: Vec<AlignedPair>,
    /// Statistics about the alignment
    pub stats: AlignmentStats,
    /// Warnings generated during alignment
    pub warnings: Vec<String>,
}

impl AlignmentResult {
    /// Serialize to JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

// ============ EXTERNAL INTEGRATION TYPES ============

/// Candidate alignment exported for external review.
/// Contains all signals so external system can make informed decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentCandidate {
    /// Unique ID for this candidate
    pub id: String,
    /// Sections in the original document
    pub original: Vec<SectionRef>,
    /// Sections in the revised document
    pub revised: Vec<SectionRef>,
    /// Proposed alignment type
    pub proposed_type: AlignmentType,
    /// Confidence score
    pub confidence: f64,
    /// Signals contributing to this alignment
    pub signals: Vec<AlignmentSignal>,
    /// Why this alignment is uncertain (if confidence < threshold)
    pub uncertainty_reason: Option<String>,
    /// Raw text excerpts for LLM context
    pub original_excerpts: Vec<String>,
    /// Raw text excerpts for LLM context
    pub revised_excerpts: Vec<String>,
}

/// External hint that influences alignment decisions.
/// Provided by LLM, expert, or external system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentHint {
    /// Which candidate this hint applies to (by ID)
    pub candidate_id: Option<String>,
    /// Or specify sections directly by canonical ID
    pub original_ids: Vec<String>,
    /// Or specify sections directly by canonical ID
    pub revised_ids: Vec<String>,
    /// The hint type
    pub hint_type: HintType,
    /// Confidence in this hint (0.0-1.0)
    pub confidence: f64,
    /// Source of the hint ("llm", "expert", "rule")
    pub source: String,
    /// Explanation for audit trail
    pub explanation: Option<String>,
}

/// Types of hints that can be applied to alignments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HintType {
    /// Force these sections to align
    ForceMatch { alignment_type: AlignmentType },
    /// These sections should NOT align
    ForceNoMatch,
    /// Adjust confidence up/down
    AdjustConfidence { delta: f64 },
    /// Override alignment type
    OverrideType { new_type: AlignmentType },
    /// Add semantic context (affects similarity scoring)
    SemanticContext { topics: Vec<String> },
}

/// Intermediate result that can be serialized for external processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentCandidates {
    /// All candidate alignments
    pub candidates: Vec<AlignmentCandidate>,
    /// Sections that couldn't be matched
    pub unmatched_original: Vec<SectionRef>,
    /// Sections that couldn't be matched
    pub unmatched_revised: Vec<SectionRef>,
    /// Total sections in original
    pub original_section_count: usize,
    /// Total sections in revised
    pub revised_section_count: usize,
}

impl AlignmentCandidates {
    /// Export to JSON for external processing.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Import from JSON after external processing.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ============ SIMILARITY CONFIGURATION ============

/// Configuration for similarity scoring weights.
#[derive(Debug, Clone)]
pub struct SimilarityConfig {
    /// Weight for canonical ID matching (default 0.25)
    pub id_weight: f64,
    /// Weight for title matching (default 0.20)
    pub title_weight: f64,
    /// Weight for semantic content matching (default 0.35)
    pub semantic_weight: f64,
    /// Weight for position/depth matching (default 0.10)
    pub position_weight: f64,
    /// Weight for raw text matching (default 0.10)
    pub text_weight: f64,
    /// Minimum score to consider a match (default 0.60)
    pub match_threshold: f64,
    /// Confidence below this triggers export for external review (default 0.75)
    pub review_threshold: f64,
    /// Minimum similarity to consider a section as split/merge candidate (default 0.30)
    pub split_merge_candidate_threshold: f64,
    /// Minimum total similarity to accept a split/merge alignment (default 0.80)
    pub split_merge_accept_threshold: f64,
    /// Default confidence for deletions/insertions with no positive evidence (default 0.60)
    pub unmatched_confidence: f64,
}

impl Default for SimilarityConfig {
    fn default() -> Self {
        Self {
            id_weight: 0.25,
            title_weight: 0.20,
            semantic_weight: 0.35,
            position_weight: 0.10,
            text_weight: 0.10,
            match_threshold: 0.60,
            review_threshold: 0.75,
            split_merge_candidate_threshold: 0.30,
            split_merge_accept_threshold: 0.80,
            unmatched_confidence: 0.60,
        }
    }
}

// ============ SECTION SEMANTICS ============

/// Semantic content extracted from a section for similarity comparison.
#[derive(Debug, Clone, Default)]
struct SectionSemantics {
    /// Terms defined in this section
    defined_terms: HashSet<String>,
    /// Terms referenced in this section
    referenced_terms: HashSet<String>,
    /// Normalized word frequencies for text comparison
    word_frequencies: HashMap<String, usize>,
    /// Total word count
    word_count: usize,
}

impl SectionSemantics {
    /// Compute Jaccard similarity between two sets.
    fn jaccard_set(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
        if a.is_empty() && b.is_empty() {
            return 1.0; // Both empty = identical
        }
        let intersection = a.intersection(b).count();
        let union = a.union(b).count();
        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }

    /// Compute similarity to another section's semantics.
    fn similarity(&self, other: &SectionSemantics) -> f64 {
        let term_sim = Self::jaccard_set(&self.defined_terms, &other.defined_terms);
        let ref_sim = Self::jaccard_set(&self.referenced_terms, &other.referenced_terms);
        let text_sim = self.text_similarity(other);

        // Weighted combination (terms more important than raw text)
        term_sim * 0.4 + ref_sim * 0.3 + text_sim * 0.3
    }

    /// Compute text similarity based on word frequencies.
    fn text_similarity(&self, other: &SectionSemantics) -> f64 {
        let all_words: HashSet<_> = self
            .word_frequencies
            .keys()
            .chain(other.word_frequencies.keys())
            .collect();

        if all_words.is_empty() {
            return 1.0;
        }

        // Cosine similarity of word frequency vectors
        let mut dot_product = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for word in all_words {
            let a = *self.word_frequencies.get(word).unwrap_or(&0) as f64;
            let b = *other.word_frequencies.get(word).unwrap_or(&0) as f64;
            dot_product += a * b;
            norm_a += a * a;
            norm_b += b * b;
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot_product / (norm_a.sqrt() * norm_b.sqrt())
    }
}

/// Macro to extract text from a token (works around LLToken not being public).
macro_rules! token_text {
    ($token:expr) => {{
        use layered_nlp::LToken;
        match $token.get_token() {
            LToken::Text(text, _) => Some(text.as_str()),
            LToken::Value => None,
        }
    }};
}

// ============ DOCUMENT ALIGNER ============

/// Aligns sections between two versions of a contract document.
pub struct DocumentAligner {
    config: SimilarityConfig,
}

impl Default for DocumentAligner {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentAligner {
    /// Create a new aligner with default configuration.
    pub fn new() -> Self {
        Self {
            config: SimilarityConfig::default(),
        }
    }

    /// Create an aligner with custom configuration.
    pub fn with_config(config: SimilarityConfig) -> Self {
        Self { config }
    }

    /// Phase 1: Compute initial alignment with deterministic signals.
    /// Returns candidates that can be exported for external review.
    pub fn compute_candidates(
        &self,
        original: &DocumentStructure,
        revised: &DocumentStructure,
        original_doc: &ContractDocument,
        revised_doc: &ContractDocument,
    ) -> AlignmentCandidates {
        let original_sections = original.flatten();
        let revised_sections = revised.flatten();

        // Track which sections have been matched
        let mut matched_original: HashSet<usize> = HashSet::new();
        let mut matched_revised: HashSet<usize> = HashSet::new();

        let mut candidates: Vec<AlignmentCandidate> = Vec::new();
        let mut candidate_id = 0;

        // Extract semantics for all sections
        let original_semantics: Vec<_> = original_sections
            .iter()
            .map(|node| self.extract_semantics(node, original_doc))
            .collect();
        let revised_semantics: Vec<_> = revised_sections
            .iter()
            .map(|node| self.extract_semantics(node, revised_doc))
            .collect();

        // Pass 1: Exact ID matches
        for (orig_idx, orig_node) in original_sections.iter().enumerate() {
            let orig_canonical = orig_node.header.identifier.canonical();

            for (rev_idx, rev_node) in revised_sections.iter().enumerate() {
                if matched_revised.contains(&rev_idx) {
                    continue;
                }

                let rev_canonical = rev_node.header.identifier.canonical();

                if orig_canonical == rev_canonical {
                    // Same ID - now check content similarity
                    let semantic_score =
                        original_semantics[orig_idx].similarity(&revised_semantics[rev_idx]);

                    let mut signals = vec![
                        AlignmentSignal::new("canonical_id", 1.0, self.config.id_weight),
                        AlignmentSignal::new("semantic", semantic_score, self.config.semantic_weight),
                    ];

                    // Add title signal if both have titles
                    if let (Some(orig_title), Some(rev_title)) =
                        (&orig_node.header.title, &rev_node.header.title)
                    {
                        let title_score = self.title_similarity(orig_title, rev_title);
                        signals.push(AlignmentSignal::new("title", title_score, self.config.title_weight));
                    }

                    let confidence: f64 =
                        signals.iter().map(|s| s.weighted_score()).sum::<f64>()
                            / signals.iter().map(|s| s.weight).sum::<f64>();

                    let alignment_type = if semantic_score >= 0.9 {
                        AlignmentType::ExactMatch
                    } else {
                        AlignmentType::Modified
                    };

                    let uncertainty_reason = if confidence < self.config.review_threshold {
                        Some(format!(
                            "Content similarity {:.2} below threshold despite matching ID",
                            semantic_score
                        ))
                    } else {
                        None
                    };

                    candidate_id += 1;
                    candidates.push(AlignmentCandidate {
                        id: format!("c{}", candidate_id),
                        original: vec![SectionRef::from_node(orig_node)],
                        revised: vec![SectionRef::from_node(rev_node)],
                        proposed_type: alignment_type,
                        confidence,
                        signals,
                        uncertainty_reason,
                        original_excerpts: vec![self.extract_excerpt(orig_node, original_doc)],
                        revised_excerpts: vec![self.extract_excerpt(rev_node, revised_doc)],
                    });

                    matched_original.insert(orig_idx);
                    matched_revised.insert(rev_idx);
                    break;
                }
            }
        }

        // Pass 2: Title + depth match (renumbered sections)
        for (orig_idx, orig_node) in original_sections.iter().enumerate() {
            if matched_original.contains(&orig_idx) {
                continue;
            }

            let orig_title = match &orig_node.header.title {
                Some(t) => t.to_lowercase(),
                None => continue,
            };
            let orig_depth = orig_node.depth();

            for (rev_idx, rev_node) in revised_sections.iter().enumerate() {
                if matched_revised.contains(&rev_idx) {
                    continue;
                }

                let rev_title = match &rev_node.header.title {
                    Some(t) => t.to_lowercase(),
                    None => continue,
                };

                if orig_title == rev_title && orig_depth == rev_node.depth() {
                    let semantic_score =
                        original_semantics[orig_idx].similarity(&revised_semantics[rev_idx]);

                    let signals = vec![
                        AlignmentSignal::new("canonical_id", 0.0, self.config.id_weight),
                        AlignmentSignal::new("title", 1.0, self.config.title_weight),
                        AlignmentSignal::new("semantic", semantic_score, self.config.semantic_weight),
                        AlignmentSignal::new("position", 1.0, self.config.position_weight),
                    ];

                    let confidence: f64 =
                        signals.iter().map(|s| s.weighted_score()).sum::<f64>()
                            / signals.iter().map(|s| s.weight).sum::<f64>();

                    if confidence >= self.config.match_threshold {
                        let uncertainty_reason = if confidence < self.config.review_threshold {
                            Some(format!(
                                "Section renumbered from {} to {}",
                                orig_node.header.identifier.canonical(),
                                rev_node.header.identifier.canonical()
                            ))
                        } else {
                            None
                        };

                        candidate_id += 1;
                        candidates.push(AlignmentCandidate {
                            id: format!("c{}", candidate_id),
                            original: vec![SectionRef::from_node(orig_node)],
                            revised: vec![SectionRef::from_node(rev_node)],
                            proposed_type: AlignmentType::Renumbered,
                            confidence,
                            signals,
                            uncertainty_reason,
                            original_excerpts: vec![self.extract_excerpt(orig_node, original_doc)],
                            revised_excerpts: vec![self.extract_excerpt(rev_node, revised_doc)],
                        });

                        matched_original.insert(orig_idx);
                        matched_revised.insert(rev_idx);
                        break;
                    }
                }
            }
        }

        // Pass 3: Content similarity (Hungarian algorithm for optimal assignment)
        let unmatched_orig: Vec<usize> = (0..original_sections.len())
            .filter(|i| !matched_original.contains(i))
            .collect();
        let unmatched_rev: Vec<usize> = (0..revised_sections.len())
            .filter(|i| !matched_revised.contains(i))
            .collect();

        if !unmatched_orig.is_empty() && !unmatched_rev.is_empty() {
            // Build similarity matrix
            let n = unmatched_orig.len().max(unmatched_rev.len());
            let mut weights = vec![vec![0i64; n]; n];

            for (i, &orig_idx) in unmatched_orig.iter().enumerate() {
                for (j, &rev_idx) in unmatched_rev.iter().enumerate() {
                    let similarity =
                        self.compute_similarity(
                            original_sections[orig_idx],
                            revised_sections[rev_idx],
                            &original_semantics[orig_idx],
                            &revised_semantics[rev_idx],
                        );
                    // Convert to integer cost (higher similarity = lower cost)
                    // Multiply by 1000 for precision, negate for minimization
                    weights[i][j] = (similarity * 1000.0) as i64;
                }
            }

            // Create weights matrix for kuhn_munkres
            let matrix = SquareMatrix::new(weights);
            let (_, assignments) = kuhn_munkres_min(&matrix);

            // Process assignments
            for (i, &j) in assignments.iter().enumerate() {
                if i >= unmatched_orig.len() || j >= unmatched_rev.len() {
                    continue; // Padding assignment
                }

                let orig_idx = unmatched_orig[i];
                let rev_idx = unmatched_rev[j];

                let orig_node = original_sections[orig_idx];
                let rev_node = revised_sections[rev_idx];

                let similarity = self.compute_similarity(
                    orig_node,
                    rev_node,
                    &original_semantics[orig_idx],
                    &revised_semantics[rev_idx],
                );

                if similarity >= self.config.match_threshold {
                    let signals = self.build_signals(
                        orig_node,
                        rev_node,
                        &original_semantics[orig_idx],
                        &revised_semantics[rev_idx],
                    );

                    let alignment_type = self.classify_alignment(orig_node, rev_node, similarity);

                    let uncertainty_reason = if similarity < self.config.review_threshold {
                        Some(format!(
                            "Low similarity {:.2} - possible {} or unrelated sections",
                            similarity,
                            if alignment_type == AlignmentType::Moved {
                                "move"
                            } else {
                                "modification"
                            }
                        ))
                    } else {
                        None
                    };

                    candidate_id += 1;
                    candidates.push(AlignmentCandidate {
                        id: format!("c{}", candidate_id),
                        original: vec![SectionRef::from_node(orig_node)],
                        revised: vec![SectionRef::from_node(rev_node)],
                        proposed_type: alignment_type,
                        confidence: similarity,
                        signals,
                        uncertainty_reason,
                        original_excerpts: vec![self.extract_excerpt(orig_node, original_doc)],
                        revised_excerpts: vec![self.extract_excerpt(rev_node, revised_doc)],
                    });

                    matched_original.insert(orig_idx);
                    matched_revised.insert(rev_idx);
                }
            }
        }

        // Pass 4: Split/Merge detection
        // Check unmatched original sections for potential splits
        for orig_idx in 0..original_sections.len() {
            if matched_original.contains(&orig_idx) {
                continue;
            }

            let orig_node = original_sections[orig_idx];
            let mut split_candidates: Vec<(usize, f64)> = Vec::new();

            for rev_idx in 0..revised_sections.len() {
                if matched_revised.contains(&rev_idx) {
                    continue;
                }

                let similarity =
                    original_semantics[orig_idx].similarity(&revised_semantics[rev_idx]);
                if similarity >= self.config.split_merge_candidate_threshold {
                    split_candidates.push((rev_idx, similarity));
                }
            }

            if split_candidates.len() >= 2 {
                // Calculate combined coverage
                let total_sim: f64 = split_candidates.iter().map(|(_, s)| s).sum();
                if total_sim >= self.config.split_merge_accept_threshold {
                    let rev_nodes: Vec<_> = split_candidates
                        .iter()
                        .map(|(idx, _)| SectionRef::from_node(revised_sections[*idx]))
                        .collect();
                    let rev_excerpts: Vec<_> = split_candidates
                        .iter()
                        .map(|(idx, _)| self.extract_excerpt(revised_sections[*idx], revised_doc))
                        .collect();

                    candidate_id += 1;
                    candidates.push(AlignmentCandidate {
                        id: format!("c{}", candidate_id),
                        original: vec![SectionRef::from_node(orig_node)],
                        revised: rev_nodes,
                        proposed_type: AlignmentType::Split,
                        confidence: (total_sim / split_candidates.len() as f64).min(1.0),
                        signals: vec![AlignmentSignal::new(
                            "split_coverage",
                            total_sim,
                            1.0,
                        )],
                        uncertainty_reason: Some("Section appears to have been split".to_string()),
                        original_excerpts: vec![self.extract_excerpt(orig_node, original_doc)],
                        revised_excerpts: rev_excerpts,
                    });

                    matched_original.insert(orig_idx);
                    for (rev_idx, _) in &split_candidates {
                        matched_revised.insert(*rev_idx);
                    }
                }
            }
        }

        // Check unmatched revised sections for potential merges
        for rev_idx in 0..revised_sections.len() {
            if matched_revised.contains(&rev_idx) {
                continue;
            }

            let rev_node = revised_sections[rev_idx];
            let mut merge_candidates: Vec<(usize, f64)> = Vec::new();

            for orig_idx in 0..original_sections.len() {
                if matched_original.contains(&orig_idx) {
                    continue;
                }

                let similarity =
                    original_semantics[orig_idx].similarity(&revised_semantics[rev_idx]);
                if similarity >= self.config.split_merge_candidate_threshold {
                    merge_candidates.push((orig_idx, similarity));
                }
            }

            if merge_candidates.len() >= 2 {
                let total_sim: f64 = merge_candidates.iter().map(|(_, s)| s).sum();
                if total_sim >= self.config.split_merge_accept_threshold {
                    let orig_nodes: Vec<_> = merge_candidates
                        .iter()
                        .map(|(idx, _)| SectionRef::from_node(original_sections[*idx]))
                        .collect();
                    let orig_excerpts: Vec<_> = merge_candidates
                        .iter()
                        .map(|(idx, _)| self.extract_excerpt(original_sections[*idx], original_doc))
                        .collect();

                    candidate_id += 1;
                    candidates.push(AlignmentCandidate {
                        id: format!("c{}", candidate_id),
                        original: orig_nodes,
                        revised: vec![SectionRef::from_node(rev_node)],
                        proposed_type: AlignmentType::Merged,
                        confidence: (total_sim / merge_candidates.len() as f64).min(1.0),
                        signals: vec![AlignmentSignal::new(
                            "merge_coverage",
                            total_sim,
                            1.0,
                        )],
                        uncertainty_reason: Some("Sections appear to have been merged".to_string()),
                        original_excerpts: orig_excerpts,
                        revised_excerpts: vec![self.extract_excerpt(rev_node, revised_doc)],
                    });

                    matched_revised.insert(rev_idx);
                    for (orig_idx, _) in &merge_candidates {
                        matched_original.insert(*orig_idx);
                    }
                }
            }
        }

        // Pass 5: Mark remaining as deletions/insertions
        for orig_idx in 0..original_sections.len() {
            if matched_original.contains(&orig_idx) {
                continue;
            }

            let orig_node = original_sections[orig_idx];
            candidate_id += 1;
            candidates.push(AlignmentCandidate {
                id: format!("c{}", candidate_id),
                original: vec![SectionRef::from_node(orig_node)],
                revised: vec![],
                proposed_type: AlignmentType::Deleted,
                confidence: self.config.unmatched_confidence,
                signals: vec![AlignmentSignal::new("no_match_found", 0.0, 1.0)],
                uncertainty_reason: Some("No matching section found in revised document".to_string()),
                original_excerpts: vec![self.extract_excerpt(orig_node, original_doc)],
                revised_excerpts: vec![],
            });
        }

        for rev_idx in 0..revised_sections.len() {
            if matched_revised.contains(&rev_idx) {
                continue;
            }

            let rev_node = revised_sections[rev_idx];
            candidate_id += 1;
            candidates.push(AlignmentCandidate {
                id: format!("c{}", candidate_id),
                original: vec![],
                revised: vec![SectionRef::from_node(rev_node)],
                proposed_type: AlignmentType::Inserted,
                confidence: self.config.unmatched_confidence,
                signals: vec![AlignmentSignal::new("no_match_found", 0.0, 1.0)],
                uncertainty_reason: Some("Section does not exist in original document".to_string()),
                original_excerpts: vec![],
                revised_excerpts: vec![self.extract_excerpt(rev_node, revised_doc)],
            });
        }

        // Collect unmatched sections for the result
        let unmatched_original: Vec<_> = (0..original_sections.len())
            .filter(|i| !matched_original.contains(i))
            .map(|i| SectionRef::from_node(original_sections[i]))
            .collect();
        let unmatched_revised: Vec<_> = (0..revised_sections.len())
            .filter(|i| !matched_revised.contains(i))
            .map(|i| SectionRef::from_node(revised_sections[i]))
            .collect();

        AlignmentCandidates {
            candidates,
            unmatched_original,
            unmatched_revised,
            original_section_count: original_sections.len(),
            revised_section_count: revised_sections.len(),
        }
    }

    /// Phase 2: Apply external hints and produce final result.
    pub fn apply_hints(
        &self,
        mut candidates: AlignmentCandidates,
        hints: &[AlignmentHint],
    ) -> AlignmentResult {
        // Apply hints
        for hint in hints {
            // First, collect the target candidate IDs
            let target_ids: Vec<String> = if let Some(ref id) = hint.candidate_id {
                vec![id.clone()]
            } else {
                // Find candidates by section IDs
                candidates
                    .candidates
                    .iter()
                    .filter(|c| {
                        let orig_matches = hint.original_ids.iter().all(|id| {
                            c.original.iter().any(|s| s.canonical_id == *id)
                        });
                        let rev_matches = hint.revised_ids.iter().all(|id| {
                            c.revised.iter().any(|s| s.canonical_id == *id)
                        });
                        orig_matches && rev_matches
                    })
                    .map(|c| c.id.clone())
                    .collect()
            };

            // Now apply the hint to matching candidates
            for candidate in &mut candidates.candidates {
                if !target_ids.contains(&candidate.id) {
                    continue;
                }
                match &hint.hint_type {
                    HintType::ForceMatch { alignment_type } => {
                        candidate.proposed_type = *alignment_type;
                        candidate.confidence = hint.confidence;
                        candidate.uncertainty_reason = None;
                        candidate.signals.push(AlignmentSignal::new(
                            format!("hint:{}", hint.source),
                            hint.confidence,
                            1.0,
                        ));
                    }
                    HintType::ForceNoMatch => {
                        // Mark as very low confidence - will be filtered or split
                        candidate.confidence = 0.0;
                        candidate.uncertainty_reason =
                            Some(format!("Rejected by {}", hint.source));
                    }
                    HintType::AdjustConfidence { delta } => {
                        candidate.confidence = (candidate.confidence + delta).clamp(0.0, 1.0);
                        candidate.signals.push(AlignmentSignal::new(
                            format!("hint:adjust:{}", hint.source),
                            *delta,
                            0.0, // Weight 0 since we already applied it
                        ));
                    }
                    HintType::OverrideType { new_type } => {
                        candidate.proposed_type = *new_type;
                    }
                    HintType::SemanticContext { topics } => {
                        // Add semantic context signal
                        candidate.signals.push(AlignmentSignal::new(
                            format!("semantic_context:{}", topics.join(",")),
                            hint.confidence,
                            self.config.semantic_weight,
                        ));
                    }
                }
            }
        }

        // Build final result
        let mut alignments: Vec<AlignedPair> = Vec::new();
        let mut stats = AlignmentStats {
            total_original: candidates.original_section_count,
            total_revised: candidates.revised_section_count,
            ..Default::default()
        };
        let mut warnings: Vec<String> = Vec::new();

        for candidate in candidates.candidates {
            // Skip rejected candidates
            if candidate.confidence == 0.0 {
                continue;
            }

            stats.increment(candidate.proposed_type);

            alignments.push(AlignedPair {
                original: candidate.original,
                revised: candidate.revised,
                alignment_type: candidate.proposed_type,
                confidence: candidate.confidence,
                signals: candidate.signals,
            });
        }

        // Add warnings for low-confidence alignments
        for alignment in &alignments {
            if alignment.confidence < self.config.review_threshold {
                let desc = match alignment.alignment_type {
                    AlignmentType::ExactMatch | AlignmentType::Modified => {
                        format!(
                            "Low confidence ({:.2}) alignment between {} and {}",
                            alignment.confidence,
                            alignment.original.first().map(|s| s.canonical_id.as_str()).unwrap_or("?"),
                            alignment.revised.first().map(|s| s.canonical_id.as_str()).unwrap_or("?")
                        )
                    }
                    AlignmentType::Deleted => {
                        format!(
                            "Section {} marked as deleted with confidence {:.2}",
                            alignment.original.first().map(|s| s.canonical_id.as_str()).unwrap_or("?"),
                            alignment.confidence
                        )
                    }
                    AlignmentType::Inserted => {
                        format!(
                            "Section {} marked as inserted with confidence {:.2}",
                            alignment.revised.first().map(|s| s.canonical_id.as_str()).unwrap_or("?"),
                            alignment.confidence
                        )
                    }
                    _ => format!(
                        "Alignment {:?} has confidence {:.2}",
                        alignment.alignment_type, alignment.confidence
                    ),
                };
                warnings.push(desc);
            }
        }

        AlignmentResult {
            alignments,
            stats,
            warnings,
        }
    }

    /// Convenience method: Run both phases without external hints.
    pub fn align(
        &self,
        original: &DocumentStructure,
        revised: &DocumentStructure,
        original_doc: &ContractDocument,
        revised_doc: &ContractDocument,
    ) -> AlignmentResult {
        let candidates =
            self.compute_candidates(original, revised, original_doc, revised_doc);
        self.apply_hints(candidates, &[])
    }

    // ============ HELPER METHODS ============

    /// Extract semantic content from a section.
    fn extract_semantics(&self, node: &SectionNode, doc: &ContractDocument) -> SectionSemantics {
        let mut semantics = SectionSemantics::default();

        let start_line = node.start_line;
        let end_line = node.end_line.unwrap_or(doc.line_count());

        for line_idx in start_line..end_line {
            if let Some(line) = doc.get_line(line_idx) {
                // Extract words for frequency analysis
                for token in line.ll_tokens() {
                    if let Some(text) = token_text!(token) {
                        let text_lower = text.to_lowercase();
                        if text_lower.len() > 2 && text_lower.chars().all(|c| c.is_alphabetic()) {
                            *semantics.word_frequencies.entry(text_lower).or_insert(0) += 1;
                            semantics.word_count += 1;
                        }
                    }
                }

                // Try to extract defined terms (quoted terms in definition sections)
                let line_text: String = line
                    .ll_tokens()
                    .iter()
                    .filter_map(|t| token_text!(t))
                    .collect::<Vec<_>>()
                    .join(" ");
                if line_text.contains('"') {
                    // Simple heuristic: extract quoted terms
                    for cap in line_text.split('"').skip(1).step_by(2) {
                        if !cap.is_empty() && cap.len() < 50 {
                            semantics.defined_terms.insert(cap.to_lowercase());
                        }
                    }
                }

                // Extract capitalized terms as potential references
                for token in line.ll_tokens() {
                    if let Some(text) = token_text!(token) {
                        if text.len() > 1
                            && text.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                            && text.chars().skip(1).all(|c| c.is_lowercase() || c.is_whitespace())
                        {
                            semantics.referenced_terms.insert(text.to_lowercase());
                        }
                    }
                }
            }
        }

        semantics
    }

    /// Compute overall similarity between two sections.
    fn compute_similarity(
        &self,
        orig: &SectionNode,
        rev: &SectionNode,
        orig_sem: &SectionSemantics,
        rev_sem: &SectionSemantics,
    ) -> f64 {
        let signals = self.build_signals(orig, rev, orig_sem, rev_sem);
        let total_weight: f64 = signals.iter().map(|s| s.weight).sum();
        if total_weight == 0.0 {
            return 0.0;
        }
        signals.iter().map(|s| s.weighted_score()).sum::<f64>() / total_weight
    }

    /// Build all similarity signals for a pair of sections.
    fn build_signals(
        &self,
        orig: &SectionNode,
        rev: &SectionNode,
        orig_sem: &SectionSemantics,
        rev_sem: &SectionSemantics,
    ) -> Vec<AlignmentSignal> {
        let mut signals = Vec::new();

        // Canonical ID similarity
        let id_score = if orig.header.identifier.canonical() == rev.header.identifier.canonical() {
            1.0
        } else {
            self.id_similarity(&orig.header.identifier, &rev.header.identifier)
        };
        signals.push(AlignmentSignal::new("canonical_id", id_score, self.config.id_weight));

        // Title similarity
        if let (Some(orig_title), Some(rev_title)) = (&orig.header.title, &rev.header.title) {
            let title_score = self.title_similarity(orig_title, rev_title);
            signals.push(AlignmentSignal::new("title", title_score, self.config.title_weight));
        }

        // Semantic similarity
        let semantic_score = orig_sem.similarity(rev_sem);
        signals.push(AlignmentSignal::new(
            "semantic",
            semantic_score,
            self.config.semantic_weight,
        ));

        // Position/depth similarity
        let depth_match = if orig.depth() == rev.depth() { 1.0 } else { 0.5 };
        signals.push(AlignmentSignal::new(
            "position",
            depth_match,
            self.config.position_weight,
        ));

        // Text similarity
        let text_score = orig_sem.text_similarity(rev_sem);
        signals.push(AlignmentSignal::new("text", text_score, self.config.text_weight));

        signals
    }

    /// Compute similarity between section identifiers.
    fn id_similarity(&self, a: &SectionIdentifier, b: &SectionIdentifier) -> f64 {
        let a_str = a.canonical();
        let b_str = b.canonical();

        if a_str == b_str {
            return 1.0;
        }

        // Levenshtein-based similarity
        let max_len = a_str.len().max(b_str.len());
        if max_len == 0 {
            return 1.0;
        }

        let distance = levenshtein(&a_str, &b_str);
        1.0 - (distance as f64 / max_len as f64)
    }

    /// Compute title similarity (case-insensitive).
    fn title_similarity(&self, a: &str, b: &str) -> f64 {
        let a_lower = a.to_lowercase();
        let b_lower = b.to_lowercase();

        if a_lower == b_lower {
            return 1.0;
        }

        let max_len = a_lower.len().max(b_lower.len());
        if max_len == 0 {
            return 1.0;
        }

        let distance = levenshtein(&a_lower, &b_lower);
        1.0 - (distance as f64 / max_len as f64)
    }

    /// Classify the alignment type based on similarity and structural changes.
    fn classify_alignment(
        &self,
        orig: &SectionNode,
        rev: &SectionNode,
        similarity: f64,
    ) -> AlignmentType {
        let same_id = orig.header.identifier.canonical() == rev.header.identifier.canonical();
        let same_depth = orig.depth() == rev.depth();

        if same_id {
            if similarity >= 0.9 {
                AlignmentType::ExactMatch
            } else {
                AlignmentType::Modified
            }
        } else if !same_depth {
            AlignmentType::Moved
        } else {
            AlignmentType::Renumbered
        }
    }

    /// Extract a text excerpt from a section for external review.
    fn extract_excerpt(&self, node: &SectionNode, doc: &ContractDocument) -> String {
        let mut excerpt = String::new();
        let start_line = node.start_line;
        let end_line = node.end_line.unwrap_or(doc.line_count()).min(start_line + 5); // Limit to 5 lines

        for line_idx in start_line..end_line {
            if let Some(line) = doc.get_line(line_idx) {
                if !excerpt.is_empty() {
                    excerpt.push('\n');
                }
                let line_text: String = line
                    .ll_tokens()
                    .iter()
                    .filter_map(|t| token_text!(t))
                    .collect::<Vec<_>>()
                    .join(" ");
                excerpt.push_str(&line_text);
            }
        }

        if end_line < node.end_line.unwrap_or(doc.line_count()) {
            excerpt.push_str("\n...");
        }

        excerpt
    }
}

// ============ UTILITY TYPES AND FUNCTIONS ============

/// Simple Levenshtein distance implementation.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[m][n]
}

/// Wrapper for pathfinding's Weights trait.
struct SquareMatrix {
    data: Vec<Vec<i64>>,
    size: usize,
}

impl SquareMatrix {
    fn new(data: Vec<Vec<i64>>) -> Self {
        let size = data.len();
        Self { data, size }
    }
}

impl Weights<i64> for SquareMatrix {
    fn rows(&self) -> usize {
        self.size
    }

    fn columns(&self) -> usize {
        self.size
    }

    fn at(&self, row: usize, col: usize) -> i64 {
        // Negate to convert from similarity to cost (we want minimum cost)
        -self.data.get(row).and_then(|r| r.get(col)).copied().unwrap_or(0)
    }

    fn neg(&self) -> Self {
        let data = self
            .data
            .iter()
            .map(|row| row.iter().map(|&v| -v).collect())
            .collect();
        Self { data, size: self.size }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("abc", ""), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", "abd"), 1);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn test_section_semantics_jaccard() {
        let set_a: HashSet<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let set_b: HashSet<String> = ["b", "c", "d"].iter().map(|s| s.to_string()).collect();

        // Intersection: {b, c} = 2
        // Union: {a, b, c, d} = 4
        // Jaccard: 2/4 = 0.5
        assert!((SectionSemantics::jaccard_set(&set_a, &set_b) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_alignment_type_serialization() {
        let types = vec![
            AlignmentType::ExactMatch,
            AlignmentType::Renumbered,
            AlignmentType::Moved,
            AlignmentType::Modified,
            AlignmentType::Split,
            AlignmentType::Merged,
            AlignmentType::Deleted,
            AlignmentType::Inserted,
        ];

        for t in types {
            let json = serde_json::to_string(&t).unwrap();
            let parsed: AlignmentType = serde_json::from_str(&json).unwrap();
            assert_eq!(t, parsed);
        }
    }

    #[test]
    fn test_similarity_config_defaults() {
        let config = SimilarityConfig::default();
        let total_weight = config.id_weight
            + config.title_weight
            + config.semantic_weight
            + config.position_weight
            + config.text_weight;
        // Should sum to 1.0
        assert!((total_weight - 1.0).abs() < 0.001);
    }
}
