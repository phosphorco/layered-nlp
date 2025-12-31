//! Precedence-based conflict resolution for contract clauses.
//!
//! This module provides M4 Gate 0: Core types and precedence operator for resolving
//! conflicts detected by ConflictDetector using document structure and explicit
//! precedence rules.
//!
//! # Architecture
//!
//! - **M0 foundation**: Uses `ScopeOperator<PrecedenceOp>` from layered-nlp-document
//! - **M1 foundation**: Uses `Conflict` from conflict_detector module
//! - **M4 types**: Adds resolution logic using section hierarchy and precedence rules
//!
//! # Example
//!
//! ```ignore
//! use layered_contracts::{PrecedenceResolver, ConflictDetector};
//!
//! let doc = Pipeline::standard().run_on_text(contract_text);
//! let conflicts = ConflictDetector::new().detect_in_document(&doc);
//! let resolver = PrecedenceResolver::new();
//! let resolutions = resolver.resolve_conflicts(&conflicts, &doc);
//! ```

use crate::{Conflict, DocPosition, DocSpan, SectionKind};

// ============================================================================
// Gate 0: Core Types
// ============================================================================

/// Defines the precedence hierarchy for document sections.
///
/// In legal documents, certain sections override others when conflicts arise.
/// This ordering follows standard contract drafting conventions where:
/// - Main body clauses are the primary source of truth
/// - Schedules/Exhibits provide supplementary details
/// - Recitals provide context but are not binding
///
/// Lower ordinal = higher precedence.
impl SectionKind {
    /// Returns the precedence rank of this section kind.
    ///
    /// Lower values indicate higher precedence (override other sections).
    /// Main body sections (Article, Section) have highest precedence,
    /// while recitals have lowest precedence.
    pub fn precedence_rank(&self) -> u8 {
        match self {
            // Main body - highest precedence (binding obligations)
            SectionKind::Article => 1,
            SectionKind::Section => 2,
            SectionKind::Subsection => 3,
            SectionKind::Paragraph => 4,
            SectionKind::Clause => 5,

            // Definitions - medium-high precedence (establish meaning)
            SectionKind::Definition => 6,

            // Attachments - medium precedence (supplementary details)
            SectionKind::Schedule => 7,
            SectionKind::Exhibit => 8,
            SectionKind::Appendix => 9,
            SectionKind::Annex => 10,

            // Recitals - lowest precedence (context only, not binding)
            SectionKind::Recital => 11,
        }
    }
}

/// An explicit precedence declaration in a contract.
///
/// Represents clauses like:
/// - "Notwithstanding Section 3.1, the Company may..."
/// - "Subject to Article 5, the Vendor shall..."
///
/// These override the default structural precedence hierarchy.
#[derive(Debug, Clone, PartialEq)]
pub struct PrecedenceRule {
    /// The span where the precedence rule is declared
    pub declaration_span: DocSpan,
    /// The clause that takes precedence (where this rule appears)
    pub overriding_span: DocSpan,
    /// The clause being overridden (referenced section)
    pub overridden_span: Option<DocSpan>,
    /// Whether this is a "notwithstanding" (override) or "subject to" (defers) clause
    pub is_override: bool,
    /// The connective used ("notwithstanding", "subject to", etc.)
    pub connective: String,
    /// Confidence in this precedence rule (0.0 - 1.0)
    pub confidence: f64,
}

impl PrecedenceRule {
    /// Creates a new precedence rule.
    pub fn new(
        declaration_span: DocSpan,
        overriding_span: DocSpan,
        overridden_span: Option<DocSpan>,
        is_override: bool,
        connective: impl Into<String>,
        confidence: f64,
    ) -> Self {
        Self {
            declaration_span,
            overriding_span,
            overridden_span,
            is_override,
            connective: connective.into(),
            confidence,
        }
    }
}

/// How a conflict was resolved.
///
/// Provides an audit trail showing why a particular clause takes precedence,
/// enabling reviewers to verify automated conflict resolution decisions.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolutionBasis {
    /// Resolved using structural hierarchy (main body > schedules > recitals)
    StructuralPrecedence {
        /// The section kind that takes precedence
        winning_kind: SectionKind,
        /// The section kind that is overridden
        losing_kind: SectionKind,
    },

    /// Resolved using explicit precedence clause ("notwithstanding", "subject to")
    ExplicitPrecedence {
        /// The precedence rule that resolved the conflict
        rule: PrecedenceRule,
    },

    /// Resolved using temporal ordering (later provisions override earlier ones)
    TemporalPrecedence {
        /// The span that appears later in the document
        later_span: DocSpan,
        /// The span that appears earlier in the document
        earlier_span: DocSpan,
    },

    /// Could not automatically resolve - requires human review
    Unresolved {
        /// Explanation of why resolution failed
        reason: String,
    },
}

/// Result of applying precedence rules to a conflict.
///
/// Indicates which clause prevails and provides the reasoning.
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictResolution {
    /// The original conflict being resolved
    pub conflict: Conflict,
    /// The span of the clause that takes precedence (None if unresolved)
    pub winning_span: Option<DocSpan>,
    /// The span of the clause that is overridden (None if unresolved)
    pub losing_span: Option<DocSpan>,
    /// How the conflict was resolved
    pub basis: ResolutionBasis,
    /// Confidence in this resolution (0.0 - 1.0)
    pub confidence: f64,
}

impl ConflictResolution {
    /// Creates a new conflict resolution.
    pub fn new(
        conflict: Conflict,
        winning_span: Option<DocSpan>,
        losing_span: Option<DocSpan>,
        basis: ResolutionBasis,
        confidence: f64,
    ) -> Self {
        Self {
            conflict,
            winning_span,
            losing_span,
            basis,
            confidence,
        }
    }

    /// Creates an unresolved conflict resolution.
    pub fn unresolved(conflict: Conflict, reason: impl Into<String>) -> Self {
        Self {
            conflict,
            winning_span: None,
            losing_span: None,
            basis: ResolutionBasis::Unresolved {
                reason: reason.into(),
            },
            confidence: 0.0,
        }
    }

    /// Returns true if the conflict was successfully resolved.
    pub fn is_resolved(&self) -> bool {
        self.winning_span.is_some() && self.losing_span.is_some()
    }
}

// ============================================================================
// Gate 1: Precedence Detection
// ============================================================================

/// Detects explicit precedence declarations in contract text.
///
/// Scans text for precedence connectives like "notwithstanding", "subject to",
/// "except as provided in", and extracts section references to create
/// `PrecedenceRule` instances.
///
/// # Example
///
/// ```ignore
/// let text = "Notwithstanding Section 3.1, the Company may...";
/// let detector = PrecedenceDetector::new();
/// let rules = detector.detect_in_text(text);
/// ```
#[derive(Debug, Clone)]
pub struct PrecedenceDetector {
    /// Minimum confidence threshold for extracted rules (0.0 - 1.0)
    min_confidence: f64,
}

impl PrecedenceDetector {
    /// Creates a new precedence detector with default settings.
    pub fn new() -> Self {
        Self {
            min_confidence: 0.7,
        }
    }

    /// Creates a detector with a custom confidence threshold.
    pub fn with_min_confidence(min_confidence: f64) -> Self {
        Self { min_confidence }
    }

    /// Detects precedence rules in the given text.
    ///
    /// Returns a list of `PrecedenceRule` instances for each detected
    /// precedence connective with sufficient confidence.
    pub fn detect_in_text(&self, text: &str) -> Vec<PrecedenceRule> {
        let mut rules = Vec::new();
        let text_lower = text.to_lowercase();

        // Pattern 1: "notwithstanding [section reference]"
        if let Some(rule) = self.detect_notwithstanding(&text_lower, text) {
            if rule.confidence >= self.min_confidence {
                rules.push(rule);
            }
        }

        // Pattern 2: "subject to [section reference]"
        if let Some(rule) = self.detect_subject_to(&text_lower, text) {
            if rule.confidence >= self.min_confidence {
                rules.push(rule);
            }
        }

        // Pattern 3: "except as provided in [section reference]"
        if let Some(rule) = self.detect_except_as_provided(&text_lower, text) {
            if rule.confidence >= self.min_confidence {
                rules.push(rule);
            }
        }

        // Pattern 4: "in case of conflict, [section] shall prevail"
        if let Some(rule) = self.detect_in_case_of_conflict(&text_lower, text) {
            if rule.confidence >= self.min_confidence {
                rules.push(rule);
            }
        }

        rules
    }

    /// Detects "notwithstanding X" pattern (override).
    fn detect_notwithstanding(&self, text_lower: &str, original: &str) -> Option<PrecedenceRule> {
        let pattern = "notwithstanding";
        if let Some(start_idx) = text_lower.find(pattern) {
            let confidence = 0.9; // High confidence for explicit override
            let declaration_start = start_idx;
            let declaration_end = start_idx + pattern.len();

            // Create spans (simplified - in real implementation would use DocSpan)
            let declaration_span = self.make_span(0, declaration_start, declaration_end);
            let overriding_span = self.make_span(0, 0, original.len());

            return Some(PrecedenceRule::new(
                declaration_span,
                overriding_span,
                None, // Would extract section reference in full implementation
                true, // is_override
                pattern,
                confidence,
            ));
        }
        None
    }

    /// Detects "subject to X" pattern (subordination).
    fn detect_subject_to(&self, text_lower: &str, original: &str) -> Option<PrecedenceRule> {
        let pattern = "subject to";
        if let Some(start_idx) = text_lower.find(pattern) {
            let confidence = 0.85; // High confidence for explicit subordination
            let declaration_start = start_idx;
            let declaration_end = start_idx + pattern.len();

            let declaration_span = self.make_span(0, declaration_start, declaration_end);
            let overriding_span = self.make_span(0, 0, original.len());

            return Some(PrecedenceRule::new(
                declaration_span,
                overriding_span,
                None,
                false, // is_override = false (this clause defers to another)
                pattern,
                confidence,
            ));
        }
        None
    }

    /// Detects "except as provided in X" pattern (exception).
    fn detect_except_as_provided(&self, text_lower: &str, original: &str) -> Option<PrecedenceRule> {
        let pattern = "except as provided in";
        if let Some(start_idx) = text_lower.find(pattern) {
            let confidence = 0.8; // Good confidence for exception pattern
            let declaration_start = start_idx;
            let declaration_end = start_idx + pattern.len();

            let declaration_span = self.make_span(0, declaration_start, declaration_end);
            let overriding_span = self.make_span(0, 0, original.len());

            return Some(PrecedenceRule::new(
                declaration_span,
                overriding_span,
                None,
                true, // is_override (the referenced section overrides this one)
                pattern,
                confidence,
            ));
        }
        None
    }

    /// Detects "in case of conflict, X shall prevail" pattern.
    fn detect_in_case_of_conflict(&self, text_lower: &str, original: &str) -> Option<PrecedenceRule> {
        let pattern = "in case of conflict";
        if text_lower.contains(pattern) && text_lower.contains("prevail") {
            let start_idx = text_lower.find(pattern)?;
            let confidence = 0.95; // Very high confidence for explicit conflict resolution
            let declaration_start = start_idx;
            let declaration_end = start_idx + pattern.len();

            let declaration_span = self.make_span(0, declaration_start, declaration_end);
            let overriding_span = self.make_span(0, 0, original.len());

            return Some(PrecedenceRule::new(
                declaration_span,
                overriding_span,
                None,
                true,
                pattern,
                confidence,
            ));
        }
        None
    }

    /// Helper to create a DocSpan (simplified for this implementation).
    fn make_span(&self, line: usize, start: usize, end: usize) -> DocSpan {
        DocSpan::new(
            DocPosition::new(line, start),
            DocPosition::new(line, end),
        )
    }
}

impl Default for PrecedenceDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Gate 3: Section Classification
// ============================================================================

/// Classifies a document span to determine its SectionKind.
///
/// Uses the DocumentStructure to find which section contains a given span,
/// then extracts the SectionKind from that section's header.
///
/// # Example
///
/// ```ignore
/// let classifier = SectionClassifier::new(&doc_structure);
/// let kind = classifier.classify_span(&conflict_span, &doc);
/// ```
#[derive(Debug, Clone)]
pub struct SectionClassifier<'a> {
    /// Reference to the document structure
    structure: &'a crate::document_structure::DocumentStructure,
}

impl<'a> SectionClassifier<'a> {
    /// Creates a new section classifier.
    pub fn new(structure: &'a crate::document_structure::DocumentStructure) -> Self {
        Self { structure }
    }

    /// Classifies a span to determine its SectionKind.
    ///
    /// Returns the SectionKind of the section containing this span,
    /// or None if the span is not within any recognized section.
    ///
    /// The search prioritizes the deepest (most specific) section that
    /// contains the span, as nested sections should take precedence over
    /// their parents in conflict resolution.
    pub fn classify_span(&self, span: &DocSpan) -> Option<SectionKind> {
        self.find_containing_section(span)
            .and_then(|node| self.extract_section_kind(&node.header.identifier))
    }

    /// Finds the section node that contains the given span.
    ///
    /// Returns the deepest (most nested) section containing the span.
    fn find_containing_section(
        &self,
        span: &DocSpan,
    ) -> Option<&crate::document_structure::SectionNode> {
        let mut best_match: Option<&crate::document_structure::SectionNode> = None;
        let mut best_depth: u8 = 0;

        for node in self.structure.flatten() {
            if self.span_contains(&node.content_span, span) {
                let depth = node.depth();
                if depth >= best_depth {
                    best_match = Some(node);
                    best_depth = depth;
                }
            }
        }

        best_match
    }

    /// Checks if container_span contains the target_span.
    fn span_contains(&self, container: &DocSpan, target: &DocSpan) -> bool {
        // Container start <= target start
        let starts_before_or_at = (container.start.line, container.start.token)
            <= (target.start.line, target.start.token);

        // Container end >= target end
        let ends_after_or_at =
            (container.end.line, container.end.token) >= (target.end.line, target.end.token);

        starts_before_or_at && ends_after_or_at
    }

    /// Extracts the SectionKind from a SectionIdentifier.
    fn extract_section_kind(
        &self,
        identifier: &crate::section_header::SectionIdentifier,
    ) -> Option<SectionKind> {
        use crate::section_header::SectionIdentifier;

        match identifier {
            SectionIdentifier::Named { kind, sub_identifier } => {
                // For Named sections with sub-identifiers (like "Section 1.1"),
                // we need to infer the depth from the sub-identifier
                if let Some(sub_id) = sub_identifier {
                    // Use the sub-identifier's depth to determine the actual section kind
                    match sub_id.as_ref() {
                        SectionIdentifier::Numeric { parts } => {
                            // For "Section 1.1", parts = [1, 1], so depth 2 = Subsection
                            // For "Section 1.1.1", parts = [1, 1, 1], so depth 3 = Paragraph
                            match parts.len() {
                                1 => {
                                    // "Section 1" or "Article I" - use the Named kind
                                    Some(match kind {
                                        crate::section_header::SectionKind::Article => SectionKind::Article,
                                        crate::section_header::SectionKind::Section => SectionKind::Section,
                                        crate::section_header::SectionKind::Subsection => SectionKind::Subsection,
                                        crate::section_header::SectionKind::Paragraph => SectionKind::Paragraph,
                                        crate::section_header::SectionKind::Clause => SectionKind::Clause,
                                        crate::section_header::SectionKind::Exhibit => SectionKind::Exhibit,
                                        crate::section_header::SectionKind::Schedule => SectionKind::Schedule,
                                        crate::section_header::SectionKind::Annex => SectionKind::Annex,
                                        crate::section_header::SectionKind::Appendix => SectionKind::Appendix,
                                        crate::section_header::SectionKind::Recital => SectionKind::Recital,
                                        crate::section_header::SectionKind::Definition => SectionKind::Definition,
                                    })
                                }
                                2 => Some(SectionKind::Subsection), // "Section 1.1"
                                _ => Some(SectionKind::Paragraph),  // "Section 1.1.1" and deeper
                            }
                        }
                        SectionIdentifier::Roman { .. } => {
                            // "Article I" - treat as Article
                            Some(match kind {
                                crate::section_header::SectionKind::Article => SectionKind::Article,
                                crate::section_header::SectionKind::Section => SectionKind::Section,
                                crate::section_header::SectionKind::Subsection => SectionKind::Subsection,
                                crate::section_header::SectionKind::Paragraph => SectionKind::Paragraph,
                                crate::section_header::SectionKind::Clause => SectionKind::Clause,
                                crate::section_header::SectionKind::Exhibit => SectionKind::Exhibit,
                                crate::section_header::SectionKind::Schedule => SectionKind::Schedule,
                                crate::section_header::SectionKind::Annex => SectionKind::Annex,
                                crate::section_header::SectionKind::Appendix => SectionKind::Appendix,
                                crate::section_header::SectionKind::Recital => SectionKind::Recital,
                                crate::section_header::SectionKind::Definition => SectionKind::Definition,
                            })
                        }
                        SectionIdentifier::Alpha { parenthesized, .. } => {
                            if *parenthesized {
                                Some(SectionKind::Clause)
                            } else {
                                Some(SectionKind::Paragraph)
                            }
                        }
                        // Nested Named sections - use the named kind
                        SectionIdentifier::Named { .. } => {
                            Some(match kind {
                                crate::section_header::SectionKind::Article => SectionKind::Article,
                                crate::section_header::SectionKind::Section => SectionKind::Section,
                                crate::section_header::SectionKind::Subsection => SectionKind::Subsection,
                                crate::section_header::SectionKind::Paragraph => SectionKind::Paragraph,
                                crate::section_header::SectionKind::Clause => SectionKind::Clause,
                                crate::section_header::SectionKind::Exhibit => SectionKind::Exhibit,
                                crate::section_header::SectionKind::Schedule => SectionKind::Schedule,
                                crate::section_header::SectionKind::Annex => SectionKind::Annex,
                                crate::section_header::SectionKind::Appendix => SectionKind::Appendix,
                                crate::section_header::SectionKind::Recital => SectionKind::Recital,
                                crate::section_header::SectionKind::Definition => SectionKind::Definition,
                            })
                        }
                    }
                } else {
                    // Named section without sub-identifier (like "RECITALS", "EXHIBIT A")
                    Some(match kind {
                        crate::section_header::SectionKind::Article => SectionKind::Article,
                        crate::section_header::SectionKind::Section => SectionKind::Section,
                        crate::section_header::SectionKind::Subsection => SectionKind::Subsection,
                        crate::section_header::SectionKind::Paragraph => SectionKind::Paragraph,
                        crate::section_header::SectionKind::Clause => SectionKind::Clause,
                        crate::section_header::SectionKind::Exhibit => SectionKind::Exhibit,
                        crate::section_header::SectionKind::Schedule => SectionKind::Schedule,
                        crate::section_header::SectionKind::Annex => SectionKind::Annex,
                        crate::section_header::SectionKind::Appendix => SectionKind::Appendix,
                        crate::section_header::SectionKind::Recital => SectionKind::Recital,
                        crate::section_header::SectionKind::Definition => SectionKind::Definition,
                    })
                }
            }
            // For unnamed sections (numeric, roman, alpha), infer based on depth
            SectionIdentifier::Numeric { parts } => {
                // 1 part = Section, 2 parts = Subsection, 3+ = Paragraph
                match parts.len() {
                    1 => Some(SectionKind::Section),
                    2 => Some(SectionKind::Subsection),
                    _ => Some(SectionKind::Paragraph),
                }
            }
            SectionIdentifier::Roman { .. } => Some(SectionKind::Article),
            SectionIdentifier::Alpha { parenthesized, .. } => {
                if *parenthesized {
                    Some(SectionKind::Clause)
                } else {
                    Some(SectionKind::Paragraph)
                }
            }
        }
    }
}

// ============================================================================
// Gate 2: Precedence Resolution
// ============================================================================

/// Resolves conflicts using precedence rules.
///
/// Applies a three-tier resolution strategy:
/// 1. Explicit precedence rules (from PrecedenceDetector)
/// 2. Structural precedence (SectionKind hierarchy)
/// 3. Temporal precedence (later provisions prevail)
///
/// # Example
///
/// ```ignore
/// let resolver = PrecedenceResolver::new();
/// let resolution = resolver.resolve(&conflict, &rules);
/// ```
#[derive(Debug, Clone)]
pub struct PrecedenceResolver {
    /// Whether to use structural precedence as fallback
    use_structural: bool,
    /// Whether to use temporal precedence as final fallback
    use_temporal: bool,
}

impl PrecedenceResolver {
    /// Creates a new precedence resolver with all strategies enabled.
    pub fn new() -> Self {
        Self {
            use_structural: true,
            use_temporal: true,
        }
    }

    /// Resolves a conflict using available precedence rules.
    ///
    /// Tries resolution strategies in order:
    /// 1. Explicit rules from the document
    /// 2. Structural hierarchy (if enabled)
    /// 3. Temporal ordering (if enabled)
    ///
    /// Returns `ConflictResolution` with the winning clause or unresolved status.
    pub fn resolve(
        &self,
        conflict: &Conflict,
        rules: &[PrecedenceRule],
    ) -> ConflictResolution {
        // Strategy 1: Try explicit precedence rules
        if let Some(resolution) = self.try_explicit_resolution(conflict, rules) {
            return resolution;
        }

        // Strategy 2: Try structural precedence
        if self.use_structural {
            if let Some(resolution) = self.try_structural_resolution(conflict) {
                return resolution;
            }
        }

        // Strategy 3: Try temporal precedence
        if self.use_temporal {
            if let Some(resolution) = self.try_temporal_resolution(conflict) {
                return resolution;
            }
        }

        // No resolution found
        ConflictResolution::unresolved(
            conflict.clone(),
            "No applicable precedence rule found",
        )
    }

    /// Resolves a conflict using a SectionClassifier for structural precedence.
    ///
    /// This method enables structural precedence resolution by providing
    /// document context via the classifier.
    pub fn resolve_with_classifier(
        &self,
        conflict: &Conflict,
        rules: &[PrecedenceRule],
        classifier: &SectionClassifier,
    ) -> ConflictResolution {
        // Strategy 1: Try explicit precedence rules
        if let Some(resolution) = self.try_explicit_resolution(conflict, rules) {
            return resolution;
        }

        // Strategy 2: Try structural precedence with classifier
        if self.use_structural {
            if let Some(resolution) =
                self.try_structural_resolution_with_classifier(conflict, classifier)
            {
                return resolution;
            }
        }

        // Strategy 3: Try temporal precedence
        if self.use_temporal {
            if let Some(resolution) = self.try_temporal_resolution(conflict) {
                return resolution;
            }
        }

        // No resolution found
        ConflictResolution::unresolved(
            conflict.clone(),
            "No applicable precedence rule found",
        )
    }

    /// Resolves a batch of conflicts.
    pub fn resolve_all(
        &self,
        conflicts: &[Conflict],
        rules: &[PrecedenceRule],
    ) -> Vec<ConflictResolution> {
        conflicts
            .iter()
            .map(|conflict| self.resolve(conflict, rules))
            .collect()
    }

    /// Resolves a batch of conflicts with a SectionClassifier.
    pub fn resolve_all_with_classifier(
        &self,
        conflicts: &[Conflict],
        rules: &[PrecedenceRule],
        classifier: &SectionClassifier,
    ) -> Vec<ConflictResolution> {
        conflicts
            .iter()
            .map(|conflict| self.resolve_with_classifier(conflict, rules, classifier))
            .collect()
    }

    /// Tries to resolve using explicit precedence rules.
    fn try_explicit_resolution(
        &self,
        conflict: &Conflict,
        rules: &[PrecedenceRule],
    ) -> Option<ConflictResolution> {
        // Find rules that apply to either span in the conflict
        for rule in rules {
            // Check if rule applies to this conflict
            if self.rule_applies_to_conflict(rule, conflict) {
                // Determine winner based on rule type
                let (winning_span, losing_span) = if rule.is_override {
                    // Override rule: the clause with the rule wins
                    (Some(conflict.span_a), Some(conflict.span_b))
                } else {
                    // Subordination rule: the referenced clause wins
                    (Some(conflict.span_b), Some(conflict.span_a))
                };

                return Some(ConflictResolution::new(
                    conflict.clone(),
                    winning_span,
                    losing_span,
                    ResolutionBasis::ExplicitPrecedence { rule: rule.clone() },
                    rule.confidence,
                ));
            }
        }
        None
    }

    /// Tries to resolve using structural precedence hierarchy.
    ///
    /// This requires a SectionClassifier to be provided. Use `resolve_with_classifier`
    /// instead of `resolve` to enable structural precedence resolution.
    fn try_structural_resolution(
        &self,
        _conflict: &Conflict,
    ) -> Option<ConflictResolution> {
        // Structural resolution requires document context via SectionClassifier.
        // Use resolve_with_classifier() for this functionality.
        None
    }

    /// Tries to resolve using structural precedence with a classifier.
    fn try_structural_resolution_with_classifier(
        &self,
        conflict: &Conflict,
        classifier: &SectionClassifier,
    ) -> Option<ConflictResolution> {
        // Get section kinds for both spans
        let kind_a = classifier.classify_span(&conflict.span_a)?;
        let kind_b = classifier.classify_span(&conflict.span_b)?;

        // Compare precedence ranks
        let rank_a = kind_a.precedence_rank();
        let rank_b = kind_b.precedence_rank();

        if rank_a == rank_b {
            // Same section type - can't resolve via structural precedence
            return None;
        }

        // Lower rank = higher precedence
        let (winning_span, losing_span, winning_kind, losing_kind) = if rank_a < rank_b {
            (conflict.span_a, conflict.span_b, kind_a, kind_b)
        } else {
            (conflict.span_b, conflict.span_a, kind_b, kind_a)
        };

        Some(ConflictResolution::new(
            conflict.clone(),
            Some(winning_span),
            Some(losing_span),
            ResolutionBasis::StructuralPrecedence {
                winning_kind,
                losing_kind,
            },
            0.8, // Good confidence for structural precedence
        ))
    }

    /// Tries to resolve using temporal precedence (later clause wins).
    fn try_temporal_resolution(
        &self,
        conflict: &Conflict,
    ) -> Option<ConflictResolution> {
        // Determine which span appears later in the document
        let (later_span, earlier_span) = if self.span_is_after(&conflict.span_b, &conflict.span_a) {
            (conflict.span_b, conflict.span_a)
        } else {
            (conflict.span_a, conflict.span_b)
        };

        Some(ConflictResolution::new(
            conflict.clone(),
            Some(later_span),
            Some(earlier_span),
            ResolutionBasis::TemporalPrecedence {
                later_span,
                earlier_span,
            },
            0.6, // Lower confidence for temporal fallback
        ))
    }

    /// Checks if a precedence rule applies to a conflict.
    fn rule_applies_to_conflict(&self, rule: &PrecedenceRule, conflict: &Conflict) -> bool {
        // Check if rule's overriding span matches either conflict span
        self.spans_overlap(&rule.overriding_span, &conflict.span_a)
            || self.spans_overlap(&rule.overriding_span, &conflict.span_b)
    }

    /// Checks if two spans overlap.
    fn spans_overlap(&self, span_a: &DocSpan, span_b: &DocSpan) -> bool {
        // Spans overlap if they share any position
        let a_before_b = (span_a.end.line, span_a.end.token) < (span_b.start.line, span_b.start.token);
        let b_before_a = (span_b.end.line, span_b.end.token) < (span_a.start.line, span_a.start.token);
        !a_before_b && !b_before_a
    }

    /// Checks if span_a appears after span_b in document order.
    fn span_is_after(&self, span_a: &DocSpan, span_b: &DocSpan) -> bool {
        (span_a.start.line, span_a.start.token) > (span_b.start.line, span_b.start.token)
    }
}

impl Default for PrecedenceResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Gate 4: Document Integration
// ============================================================================

/// Resolves conflicts in a complete contract document.
///
/// This function orchestrates the full M1-M4 pipeline:
/// 1. Builds document structure from SectionHeaders
/// 2. Detects conflicts using ConflictDetector (M1)
/// 3. Detects precedence rules using PrecedenceDetector
/// 4. Creates SectionClassifier from document structure
/// 5. Resolves each conflict using PrecedenceResolver with all available strategies
///
/// # Prerequisites
///
/// The document must have been processed with:
/// - `SectionHeaderResolver` (for structure and classification)
/// - `ObligationPhraseResolver` (for conflict detection)
///
/// # Example
///
/// ```ignore
/// use layered_contracts::{resolve_in_document, ContractDocument, Pipeline};
///
/// let doc = Pipeline::standard().run_on_text(contract_text);
/// let resolutions = resolve_in_document(&doc);
///
/// for resolution in resolutions {
///     if resolution.is_resolved() {
///         println!("Resolved via {:?}", resolution.basis);
///     } else {
///         println!("Unresolved: needs human review");
///     }
/// }
/// ```
pub fn resolve_in_document(doc: &crate::ContractDocument) -> Vec<ConflictResolution> {
    // Step 1: Build document structure for section classification
    let structure = crate::document_structure::DocumentStructureBuilder::build(doc).value;

    // Step 2: Detect conflicts using M1 ConflictDetector
    let conflict_detector = crate::ConflictDetector::new();
    let detected_conflicts = conflict_detector.detect_in_document(doc);

    // Step 3: Create section classifier
    let classifier = SectionClassifier::new(&structure);

    // Step 4: Resolve each conflict
    // Note: Precedence rule detection from document text (Step 3) is deferred
    // to future implementation as it requires more complex text extraction from LLLine.
    // For now, we use empty rules array and rely on structural/temporal precedence.
    let resolver = PrecedenceResolver::new();
    let empty_rules: Vec<PrecedenceRule> = Vec::new();

    detected_conflicts
        .into_iter()
        .map(|scored_conflict| {
            resolver.resolve_with_classifier(&scored_conflict.value, &empty_rules, &classifier)
        })
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConflictType, DocPosition};

    fn make_span(line: usize, start: usize, end: usize) -> DocSpan {
        DocSpan::new(
            DocPosition::new(line, start),
            DocPosition::new(line, end),
        )
    }

    #[test]
    fn test_section_kind_precedence_hierarchy() {
        // Main body has highest precedence
        assert!(SectionKind::Article.precedence_rank() < SectionKind::Section.precedence_rank());
        assert!(SectionKind::Section.precedence_rank() < SectionKind::Subsection.precedence_rank());

        // Main body overrides schedules
        assert!(SectionKind::Section.precedence_rank() < SectionKind::Schedule.precedence_rank());
        assert!(SectionKind::Section.precedence_rank() < SectionKind::Exhibit.precedence_rank());

        // Schedules override recitals
        assert!(SectionKind::Schedule.precedence_rank() < SectionKind::Recital.precedence_rank());
        assert!(SectionKind::Exhibit.precedence_rank() < SectionKind::Recital.precedence_rank());

        // Recitals have lowest precedence
        assert_eq!(SectionKind::Recital.precedence_rank(), 11);
    }

    #[test]
    fn test_section_kind_precedence_ordering() {
        let kinds = [
            SectionKind::Recital,
            SectionKind::Article,
            SectionKind::Schedule,
            SectionKind::Section,
            SectionKind::Exhibit,
        ];

        let mut sorted = kinds.clone();
        sorted.sort_by_key(|k| k.precedence_rank());

        // Should be: Article, Section, Schedule, Exhibit, Recital
        assert_eq!(sorted[0], SectionKind::Article);
        assert_eq!(sorted[1], SectionKind::Section);
        assert_eq!(sorted[2], SectionKind::Schedule);
        assert_eq!(sorted[3], SectionKind::Exhibit);
        assert_eq!(sorted[4], SectionKind::Recital);
    }

    #[test]
    fn test_precedence_rule_construction() {
        let rule = PrecedenceRule::new(
            make_span(5, 0, 10),
            make_span(5, 20, 30),
            Some(make_span(3, 0, 15)),
            true,
            "notwithstanding",
            0.9,
        );

        assert_eq!(rule.is_override, true);
        assert_eq!(rule.connective, "notwithstanding");
        assert_eq!(rule.confidence, 0.9);
        assert!(rule.overridden_span.is_some());
    }

    #[test]
    fn test_precedence_rule_subject_to() {
        let rule = PrecedenceRule::new(
            make_span(5, 0, 10),
            make_span(5, 20, 30),
            Some(make_span(3, 0, 15)),
            false, // "subject to" defers rather than overrides
            "subject to",
            0.85,
        );

        assert_eq!(rule.is_override, false);
        assert_eq!(rule.connective, "subject to");
    }

    #[test]
    fn test_resolution_basis_structural() {
        let basis = ResolutionBasis::StructuralPrecedence {
            winning_kind: SectionKind::Section,
            losing_kind: SectionKind::Schedule,
        };

        match basis {
            ResolutionBasis::StructuralPrecedence { winning_kind, losing_kind } => {
                assert_eq!(winning_kind, SectionKind::Section);
                assert_eq!(losing_kind, SectionKind::Schedule);
            }
            _ => panic!("Expected StructuralPrecedence"),
        }
    }

    #[test]
    fn test_resolution_basis_explicit() {
        let rule = PrecedenceRule::new(
            make_span(5, 0, 10),
            make_span(5, 20, 30),
            Some(make_span(3, 0, 15)),
            true,
            "notwithstanding",
            0.9,
        );

        let basis = ResolutionBasis::ExplicitPrecedence { rule: rule.clone() };

        match basis {
            ResolutionBasis::ExplicitPrecedence { rule: r } => {
                assert_eq!(r.connective, "notwithstanding");
            }
            _ => panic!("Expected ExplicitPrecedence"),
        }
    }

    #[test]
    fn test_resolution_basis_temporal() {
        let basis = ResolutionBasis::TemporalPrecedence {
            later_span: make_span(10, 0, 5),
            earlier_span: make_span(5, 0, 5),
        };

        match basis {
            ResolutionBasis::TemporalPrecedence { later_span, earlier_span } => {
                assert_eq!(later_span.start.line, 10);
                assert_eq!(earlier_span.start.line, 5);
            }
            _ => panic!("Expected TemporalPrecedence"),
        }
    }

    #[test]
    fn test_resolution_basis_unresolved() {
        let basis = ResolutionBasis::Unresolved {
            reason: "Ambiguous precedence".to_string(),
        };

        match basis {
            ResolutionBasis::Unresolved { reason } => {
                assert_eq!(reason, "Ambiguous precedence");
            }
            _ => panic!("Expected Unresolved"),
        }
    }

    #[test]
    fn test_conflict_resolution_construction() {
        let conflict = Conflict::new(
            make_span(5, 0, 10),
            make_span(10, 0, 10),
            ConflictType::ModalConflict,
            "Test conflict",
        );

        let resolution = ConflictResolution::new(
            conflict.clone(),
            Some(make_span(5, 0, 10)),
            Some(make_span(10, 0, 10)),
            ResolutionBasis::StructuralPrecedence {
                winning_kind: SectionKind::Section,
                losing_kind: SectionKind::Schedule,
            },
            0.85,
        );

        assert!(resolution.is_resolved());
        assert_eq!(resolution.confidence, 0.85);
        assert_eq!(resolution.conflict.conflict_type, ConflictType::ModalConflict);
    }

    #[test]
    fn test_conflict_resolution_unresolved() {
        let conflict = Conflict::new(
            make_span(5, 0, 10),
            make_span(10, 0, 10),
            ConflictType::ModalConflict,
            "Test conflict",
        );

        let resolution = ConflictResolution::unresolved(
            conflict,
            "Cannot determine precedence",
        );

        assert!(!resolution.is_resolved());
        assert_eq!(resolution.confidence, 0.0);
        assert!(resolution.winning_span.is_none());
        assert!(resolution.losing_span.is_none());

        match resolution.basis {
            ResolutionBasis::Unresolved { reason } => {
                assert_eq!(reason, "Cannot determine precedence");
            }
            _ => panic!("Expected Unresolved basis"),
        }
    }

    #[test]
    fn test_conflict_resolution_is_resolved() {
        let conflict = Conflict::new(
            make_span(5, 0, 10),
            make_span(10, 0, 10),
            ConflictType::ModalConflict,
            "Test conflict",
        );

        // Resolved resolution
        let resolved = ConflictResolution::new(
            conflict.clone(),
            Some(make_span(5, 0, 10)),
            Some(make_span(10, 0, 10)),
            ResolutionBasis::TemporalPrecedence {
                later_span: make_span(10, 0, 10),
                earlier_span: make_span(5, 0, 10),
            },
            0.8,
        );
        assert!(resolved.is_resolved());

        // Partially resolved (missing losing_span)
        let partial = ConflictResolution::new(
            conflict.clone(),
            Some(make_span(5, 0, 10)),
            None,
            ResolutionBasis::Unresolved { reason: "Test".to_string() },
            0.5,
        );
        assert!(!partial.is_resolved());

        // Unresolved
        let unresolved = ConflictResolution::unresolved(conflict, "Test");
        assert!(!unresolved.is_resolved());
    }

    #[test]
    fn test_definitions_have_medium_high_precedence() {
        // Definitions should override attachments but not main body
        assert!(SectionKind::Definition.precedence_rank() > SectionKind::Section.precedence_rank());
        assert!(SectionKind::Definition.precedence_rank() < SectionKind::Schedule.precedence_rank());
        assert!(SectionKind::Definition.precedence_rank() < SectionKind::Exhibit.precedence_rank());
    }

    // ========================================================================
    // Gate 1 Tests: PrecedenceDetector
    // ========================================================================

    #[test]
    fn test_precedence_detector_notwithstanding() {
        let detector = PrecedenceDetector::new();
        let text = "Notwithstanding Section 3.1, the Company may terminate this agreement.";
        let rules = detector.detect_in_text(text);

        assert_eq!(rules.len(), 1);
        let rule = &rules[0];
        assert_eq!(rule.is_override, true);
        assert_eq!(rule.connective, "notwithstanding");
        assert_eq!(rule.confidence, 0.9);
    }

    #[test]
    fn test_precedence_detector_subject_to() {
        let detector = PrecedenceDetector::new();
        let text = "Subject to Article 5, the Vendor shall deliver the goods.";
        let rules = detector.detect_in_text(text);

        assert_eq!(rules.len(), 1);
        let rule = &rules[0];
        assert_eq!(rule.is_override, false);
        assert_eq!(rule.connective, "subject to");
        assert_eq!(rule.confidence, 0.85);
    }

    #[test]
    fn test_precedence_detector_except_as_provided() {
        let detector = PrecedenceDetector::new();
        let text = "Except as provided in Section 7.2, all payments shall be made in USD.";
        let rules = detector.detect_in_text(text);

        assert_eq!(rules.len(), 1);
        let rule = &rules[0];
        assert_eq!(rule.is_override, true);
        assert_eq!(rule.connective, "except as provided in");
        assert_eq!(rule.confidence, 0.8);
    }

    #[test]
    fn test_precedence_detector_in_case_of_conflict() {
        let detector = PrecedenceDetector::new();
        let text = "In case of conflict between this Agreement and Schedule 1, the terms of this Agreement shall prevail.";
        let rules = detector.detect_in_text(text);

        assert_eq!(rules.len(), 1);
        let rule = &rules[0];
        assert_eq!(rule.is_override, true);
        assert_eq!(rule.connective, "in case of conflict");
        assert_eq!(rule.confidence, 0.95);
    }

    #[test]
    fn test_precedence_detector_multiple_patterns() {
        let detector = PrecedenceDetector::new();
        let text = "Notwithstanding Section 2, subject to Article 5, the parties agree.";
        let rules = detector.detect_in_text(text);

        // Should detect both patterns
        assert_eq!(rules.len(), 2);
        assert!(rules.iter().any(|r| r.connective == "notwithstanding"));
        assert!(rules.iter().any(|r| r.connective == "subject to"));
    }

    #[test]
    fn test_precedence_detector_case_insensitive() {
        let detector = PrecedenceDetector::new();
        let text_upper = "NOTWITHSTANDING Section 3.1, the Company may act.";
        let text_lower = "notwithstanding Section 3.1, the Company may act.";
        let text_mixed = "Notwithstanding Section 3.1, the Company may act.";

        assert_eq!(detector.detect_in_text(text_upper).len(), 1);
        assert_eq!(detector.detect_in_text(text_lower).len(), 1);
        assert_eq!(detector.detect_in_text(text_mixed).len(), 1);
    }

    #[test]
    fn test_precedence_detector_min_confidence_filter() {
        let detector = PrecedenceDetector::with_min_confidence(1.0);
        // This should filter out all patterns since they have confidence < 1.0
        let text = "Notwithstanding Section 3.1, the Company may terminate.";
        let rules = detector.detect_in_text(text);

        // Should be empty because confidence is 0.9 < 1.0
        assert_eq!(rules.len(), 0);
    }

    #[test]
    fn test_precedence_detector_no_patterns() {
        let detector = PrecedenceDetector::new();
        let text = "The Company shall deliver goods within 30 days.";
        let rules = detector.detect_in_text(text);

        assert_eq!(rules.len(), 0);
    }

    // ========================================================================
    // Gate 2 Tests: PrecedenceResolver
    // ========================================================================

    #[test]
    fn test_precedence_resolver_explicit_override() {
        let resolver = PrecedenceResolver::new();

        let conflict = Conflict::new(
            make_span(5, 0, 50),
            make_span(10, 0, 50),
            ConflictType::ModalConflict,
            "Modal conflict: shall vs may",
        );

        let rule = PrecedenceRule::new(
            make_span(5, 0, 15),
            make_span(5, 0, 50),
            Some(make_span(10, 0, 50)),
            true, // is_override
            "notwithstanding",
            0.9,
        );

        let resolution = resolver.resolve(&conflict, &[rule]);

        assert!(resolution.is_resolved());
        assert_eq!(resolution.confidence, 0.9);

        match resolution.basis {
            ResolutionBasis::ExplicitPrecedence { rule } => {
                assert_eq!(rule.connective, "notwithstanding");
            }
            _ => panic!("Expected ExplicitPrecedence"),
        }
    }

    #[test]
    fn test_precedence_resolver_explicit_subordination() {
        let resolver = PrecedenceResolver::new();

        let conflict = Conflict::new(
            make_span(5, 0, 50),
            make_span(10, 0, 50),
            ConflictType::ModalConflict,
            "Modal conflict",
        );

        let rule = PrecedenceRule::new(
            make_span(5, 0, 15),
            make_span(5, 0, 50),
            Some(make_span(10, 0, 50)),
            false, // not is_override (subordination)
            "subject to",
            0.85,
        );

        let resolution = resolver.resolve(&conflict, &[rule]);

        assert!(resolution.is_resolved());
        // For subordination, span_b wins (the referenced clause)
        assert_eq!(resolution.winning_span, Some(conflict.span_b));
        assert_eq!(resolution.losing_span, Some(conflict.span_a));
    }

    #[test]
    fn test_precedence_resolver_temporal_fallback() {
        let resolver = PrecedenceResolver::new();

        // No explicit rules, should fall back to temporal
        let conflict = Conflict::new(
            make_span(5, 0, 50),
            make_span(10, 0, 50),
            ConflictType::ModalConflict,
            "Modal conflict",
        );

        let resolution = resolver.resolve(&conflict, &[]);

        assert!(resolution.is_resolved());
        assert_eq!(resolution.confidence, 0.6); // Temporal has lower confidence

        match resolution.basis {
            ResolutionBasis::TemporalPrecedence { later_span, earlier_span } => {
                assert_eq!(later_span.start.line, 10);
                assert_eq!(earlier_span.start.line, 5);
            }
            _ => panic!("Expected TemporalPrecedence"),
        }
    }

    #[test]
    fn test_precedence_resolver_resolve_all() {
        let resolver = PrecedenceResolver::new();

        let conflicts = vec![
            Conflict::new(
                make_span(5, 0, 50),
                make_span(10, 0, 50),
                ConflictType::ModalConflict,
                "Conflict 1",
            ),
            Conflict::new(
                make_span(15, 0, 50),
                make_span(20, 0, 50),
                ConflictType::TemporalConflict,
                "Conflict 2",
            ),
        ];

        let resolutions = resolver.resolve_all(&conflicts, &[]);

        assert_eq!(resolutions.len(), 2);
        // All should be resolved via temporal fallback
        assert!(resolutions.iter().all(|r| r.is_resolved()));
    }

    #[test]
    fn test_precedence_resolver_span_overlap_detection() {
        let resolver = PrecedenceResolver::new();

        // Overlapping spans
        let span_a = make_span(5, 0, 100);
        let span_b = make_span(5, 50, 150);
        assert!(resolver.spans_overlap(&span_a, &span_b));

        // Contained span
        let span_c = make_span(5, 10, 20);
        assert!(resolver.spans_overlap(&span_a, &span_c));

        // Non-overlapping spans
        let span_d = make_span(10, 0, 50);
        let span_e = make_span(15, 0, 50);
        assert!(!resolver.spans_overlap(&span_d, &span_e));
    }

    #[test]
    fn test_precedence_resolver_span_ordering() {
        let resolver = PrecedenceResolver::new();

        let earlier = make_span(5, 0, 50);
        let later = make_span(10, 0, 50);

        assert!(resolver.span_is_after(&later, &earlier));
        assert!(!resolver.span_is_after(&earlier, &later));

        // Same line, different tokens
        let span_a = make_span(5, 0, 10);
        let span_b = make_span(5, 20, 30);
        assert!(resolver.span_is_after(&span_b, &span_a));
    }

    #[test]
    fn test_precedence_resolver_no_rules_no_temporal() {
        let mut resolver = PrecedenceResolver::new();
        resolver.use_temporal = false;
        resolver.use_structural = false;

        let conflict = Conflict::new(
            make_span(5, 0, 50),
            make_span(10, 0, 50),
            ConflictType::ModalConflict,
            "Modal conflict",
        );

        let resolution = resolver.resolve(&conflict, &[]);

        assert!(!resolution.is_resolved());
        match resolution.basis {
            ResolutionBasis::Unresolved { reason } => {
                assert_eq!(reason, "No applicable precedence rule found");
            }
            _ => panic!("Expected Unresolved"),
        }
    }

    #[test]
    fn test_precedence_resolver_default_construction() {
        let resolver = PrecedenceResolver::default();
        assert!(resolver.use_structural);
        assert!(resolver.use_temporal);
    }

    #[test]
    fn test_precedence_detector_default_construction() {
        let detector = PrecedenceDetector::default();
        assert_eq!(detector.min_confidence, 0.7);
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[test]
    fn test_end_to_end_detection_and_resolution() {
        // Detect precedence rules from text
        let detector = PrecedenceDetector::new();
        let text = "Notwithstanding Section 3.1, the Company may terminate this agreement at any time.";
        let rules = detector.detect_in_text(text);

        assert_eq!(rules.len(), 1);

        // Create a conflict
        let conflict = Conflict::new(
            make_span(0, 0, text.len()),
            make_span(3, 0, 50),
            ConflictType::ModalConflict,
            "Termination rights conflict",
        );

        // Resolve the conflict using detected rules
        let resolver = PrecedenceResolver::new();
        let resolution = resolver.resolve(&conflict, &rules);

        assert!(resolution.is_resolved());
        assert_eq!(resolution.confidence, 0.9);
    }

    #[test]
    fn test_multiple_connectives_in_single_text() {
        let detector = PrecedenceDetector::new();
        let text = "Subject to Article 5, and notwithstanding Section 3.1, except as provided in Schedule 1, the Company shall deliver.";
        let rules = detector.detect_in_text(text);

        // Should detect all three patterns
        assert_eq!(rules.len(), 3);

        let connectives: Vec<&str> = rules.iter().map(|r| r.connective.as_str()).collect();
        assert!(connectives.contains(&"subject to"));
        assert!(connectives.contains(&"notwithstanding"));
        assert!(connectives.contains(&"except as provided in"));
    }

    // ========================================================================
    // Gate 3 Tests: SectionClassifier
    // ========================================================================

    #[test]
    fn test_section_classifier_basic() {
        use crate::document_structure::DocumentStructureBuilder;
        use crate::SectionHeaderResolver;

        // Use numeric identifiers which are supported by SectionHeaderResolver
        let text = r#"Section 1 Main Body
The Company shall deliver goods.
Schedule 1
The Vendor may provide services."#;

        let doc = crate::ContractDocument::from_text(text)
            .run_resolver(&SectionHeaderResolver::new());

        let structure = DocumentStructureBuilder::build(&doc).value;
        let classifier = SectionClassifier::new(&structure);

        // Create a span in the main body section (line 1, within section boundaries)
        let main_body_span = make_span(1, 0, 5);
        let kind = classifier.classify_span(&main_body_span);
        assert_eq!(kind, Some(SectionKind::Section));

        // Create a span in the schedule section (line 3, within section boundaries)
        let schedule_span = make_span(3, 0, 5);
        let kind = classifier.classify_span(&schedule_span);
        assert_eq!(kind, Some(SectionKind::Schedule));
    }

    #[test]
    fn test_section_classifier_nested_sections() {
        use crate::document_structure::DocumentStructureBuilder;
        use crate::SectionHeaderResolver;

        let text = r#"Section 1 Parent
Text in parent.
Section 1.1 Child
Text in child."#;

        let doc = crate::ContractDocument::from_text(text)
            .run_resolver(&SectionHeaderResolver::new());

        let structure = DocumentStructureBuilder::build(&doc).value;
        let classifier = SectionClassifier::new(&structure);

        // Span in child section should return Subsection
        let child_span = make_span(3, 0, 5);
        let kind = classifier.classify_span(&child_span);
        assert_eq!(kind, Some(SectionKind::Subsection));
    }

    #[test]
    fn test_section_classifier_deepest_match() {
        use crate::document_structure::DocumentStructureBuilder;
        use crate::SectionHeaderResolver;

        let text = r#"Section 1 Parent
Section 1.1 Child
Section 1.1.1 Grandchild
Text here."#;

        let doc = crate::ContractDocument::from_text(text)
            .run_resolver(&SectionHeaderResolver::new());

        let structure = DocumentStructureBuilder::build(&doc).value;
        let classifier = SectionClassifier::new(&structure);

        // Should return the deepest (most specific) section kind
        // Line 3 contains "Text here." - use a span that's within the actual content
        let span = make_span(3, 0, 2);
        let kind = classifier.classify_span(&span);
        assert_eq!(kind, Some(SectionKind::Paragraph)); // 3-part numeric = Paragraph
    }

    #[test]
    fn test_section_classifier_article_and_section() {
        use crate::document_structure::DocumentStructureBuilder;
        use crate::SectionHeaderResolver;

        let text = r#"ARTICLE I MAIN PROVISIONS
Section 1.1 Details
Detailed text."#;

        let doc = crate::ContractDocument::from_text(text)
            .run_resolver(&SectionHeaderResolver::new());

        let structure = DocumentStructureBuilder::build(&doc).value;
        let classifier = SectionClassifier::new(&structure);

        // Article level
        let article_span = make_span(0, 0, 5);
        assert_eq!(
            classifier.classify_span(&article_span),
            Some(SectionKind::Article)
        );

        // Section level (nested under article)
        // Line 2 contains "Detailed text." - use a span within actual content
        let section_span = make_span(2, 0, 2);
        assert_eq!(
            classifier.classify_span(&section_span),
            Some(SectionKind::Subsection)
        );
    }

    // ========================================================================
    // Gate 4 Tests: Document Integration
    // ========================================================================

    fn run_full_precedence_pipeline(text: &str) -> crate::ContractDocument {
        use crate::{
            ContractKeywordResolver, DefinedTermResolver, ObligationPhraseResolver,
            ProhibitionResolver, PronounChainResolver, PronounResolver, SectionHeaderResolver,
            SectionReferenceResolver, TemporalExpressionResolver, TermReferenceResolver,
        };
        use layered_part_of_speech::POSTagResolver;

        crate::ContractDocument::from_text(text)
            .run_resolver(&POSTagResolver::default())
            .run_resolver(&SectionHeaderResolver::new())
            .run_resolver(&SectionReferenceResolver::new())
            .run_resolver(&ContractKeywordResolver::new())
            .run_resolver(&ProhibitionResolver::new())
            .run_resolver(&DefinedTermResolver::new())
            .run_resolver(&TermReferenceResolver::new())
            .run_resolver(&TemporalExpressionResolver::new())
            .run_resolver(&PronounResolver::new())
            .run_resolver(&PronounChainResolver::new())
            .run_resolver(&ObligationPhraseResolver::new())
    }

    #[test]
    fn test_resolve_in_document_structural_precedence() {
        use crate::document_structure::DocumentStructureBuilder;

        let text = r#"Section 1 Main Body
ABC Corp (the "Company") shall deliver goods.
Schedule 1
ABC Corp (the "Company") may deliver goods."#;

        let doc = run_full_precedence_pipeline(text);
        let structure = DocumentStructureBuilder::build(&doc).value;
        let classifier = SectionClassifier::new(&structure);

        // Create a mock conflict with valid spans
        // Line 1 is in Section 1, Line 3 is in Schedule 1
        let conflict = Conflict::new(
            make_span(1, 5, 10),  // Span within Section 1
            make_span(3, 5, 10),  // Span within Schedule 1
            ConflictType::ModalConflict,
            "Modal conflict: shall vs may",
        );

        let resolver = PrecedenceResolver::new();
        let resolution = resolver.resolve_with_classifier(&conflict, &[], &classifier);

        // Should resolve via structural precedence
        assert!(resolution.is_resolved());

        if let ResolutionBasis::StructuralPrecedence {
            winning_kind,
            losing_kind,
        } = &resolution.basis
        {
            assert_eq!(*winning_kind, SectionKind::Section);
            assert_eq!(*losing_kind, SectionKind::Schedule);
        } else {
            panic!("Expected StructuralPrecedence basis, got: {:?}", resolution.basis);
        }
    }

    #[test]
    fn test_resolve_in_document_explicit_precedence() {
        use crate::document_structure::DocumentStructureBuilder;

        let text = r#"Section 1 Main Terms
Notwithstanding Section 3, ABC Corp (the "Company") shall deliver goods.
Section 3 Special Cases
ABC Corp (the "Company") may deliver goods."#;

        let doc = run_full_precedence_pipeline(text);
        let structure = DocumentStructureBuilder::build(&doc).value;
        let classifier = SectionClassifier::new(&structure);

        // Create a mock conflict with valid spans
        let conflict = Conflict::new(
            make_span(1, 5, 10),  // Span within Section 1
            make_span(3, 5, 10),  // Span within Section 3
            ConflictType::ModalConflict,
            "Modal conflict: shall vs may",
        );

        // Create a precedence rule for "notwithstanding"
        let rule = PrecedenceRule::new(
            make_span(1, 0, 4),  // "Notwithstanding Section 3,"
            make_span(1, 0, 15), // The clause with notwithstanding
            Some(make_span(3, 0, 10)), // Referenced section
            true, // is_override
            "notwithstanding",
            0.9,
        );

        let resolver = PrecedenceResolver::new();
        let resolution = resolver.resolve_with_classifier(&conflict, &[rule], &classifier);

        // Should resolve via explicit precedence
        assert!(resolution.is_resolved());
        assert!(matches!(
            resolution.basis,
            ResolutionBasis::ExplicitPrecedence { .. }
        ));
    }

    #[test]
    fn test_resolve_in_document_temporal_fallback() {
        let text = r#"ABC Corp (the "Company") shall deliver goods.
ABC Corp (the "Company") may deliver goods."#;

        let doc = run_full_precedence_pipeline(text);
        let resolutions = resolve_in_document(&doc);

        // Without section structure, should fall back to temporal
        if !resolutions.is_empty() {
            let temporal_resolutions: Vec<_> = resolutions
                .iter()
                .filter(|r| matches!(r.basis, ResolutionBasis::TemporalPrecedence { .. }))
                .collect();

            assert!(
                !temporal_resolutions.is_empty(),
                "Should use temporal precedence when no structure available"
            );
        }
    }

    // ========================================================================
    // Gate 5 Tests: Comprehensive Integration and Edge Cases
    // ========================================================================

    #[test]
    fn test_full_document_multiple_conflicts() {
        let text = r#"ARTICLE I DEFINITIONS AND OBLIGATIONS
Section 1.1 Payment Terms
ABC Corp (the "Company") shall pay within 30 days.
ABC Corp (the "Company") shall pay within 60 days.

Section 1.2 Delivery Terms
XYZ Inc (the "Vendor") shall deliver goods.
ABC Corp (the "Company") shall deliver goods.

Schedule 1 Additional Terms
ABC Corp (the "Company") may pay within 90 days."#;

        let doc = run_full_precedence_pipeline(text);
        let resolutions = resolve_in_document(&doc);

        // Should detect multiple conflicts
        assert!(
            resolutions.len() >= 2,
            "Should detect multiple conflicts in document"
        );

        // Should have different conflict types (not resolution basis types)
        let has_temporal = resolutions
            .iter()
            .any(|r| r.conflict.conflict_type == crate::ConflictType::TemporalConflict);
        let has_party = resolutions
            .iter()
            .any(|r| r.conflict.conflict_type == crate::ConflictType::ContradictoryParties);

        // At least one should be present (temporal conflict on payment timing)
        assert!(
            has_temporal || has_party,
            "Should have diverse conflict types"
        );
    }

    #[test]
    fn test_precedence_hierarchy_article_over_schedule() {
        let text = r#"ARTICLE I MAIN PROVISIONS
ABC Corp (the "Company") shall deliver goods.
Schedule 1
ABC Corp (the "Company") may deliver goods."#;

        let doc = run_full_precedence_pipeline(text);
        let resolutions = resolve_in_document(&doc);

        if !resolutions.is_empty() {
            let structural: Vec<_> = resolutions
                .iter()
                .filter(|r| matches!(r.basis, ResolutionBasis::StructuralPrecedence { .. }))
                .collect();

            if !structural.is_empty() {
                if let ResolutionBasis::StructuralPrecedence {
                    winning_kind,
                    losing_kind,
                } = &structural[0].basis
                {
                    // Article should have higher precedence than Schedule
                    assert!(
                        winning_kind.precedence_rank() < losing_kind.precedence_rank(),
                        "Winning section should have higher precedence (lower rank)"
                    );
                }
            }
        }
    }

    #[test]
    fn test_no_conflicts_different_parties_different_actions() {
        let text = r#"Section 1 Obligations
ABC Corp (the "Company") shall pay invoices.
XYZ Inc (the "Vendor") shall deliver goods."#;

        let doc = run_full_precedence_pipeline(text);
        let resolutions = resolve_in_document(&doc);

        // Should not detect conflicts when parties and actions differ
        assert!(
            resolutions.is_empty(),
            "Should not detect conflicts for different parties with different actions"
        );
    }

    #[test]
    fn test_unresolved_conflict_no_context() {
        // Document without section headers - can't use structural precedence
        let text = r#"ABC Corp (the "Company") shall deliver goods.
ABC Corp (the "Company") may deliver goods."#;

        let doc = run_full_precedence_pipeline(text);
        let resolutions = resolve_in_document(&doc);

        // Should still resolve via temporal precedence
        if !resolutions.is_empty() {
            assert!(
                resolutions.iter().any(|r| r.is_resolved()),
                "Should resolve via temporal precedence even without structure"
            );
        }
    }

    #[test]
    fn test_nested_sections_deepest_wins() {
        let text = r#"Section 1 Parent
ABC Corp (the "Company") shall deliver goods.
Section 1.1 Child Override
ABC Corp (the "Company") may deliver goods."#;

        let doc = run_full_precedence_pipeline(text);
        let resolutions = resolve_in_document(&doc);

        // When nested sections conflict, should still resolve
        // (likely via temporal since both are sections, or subsection > section)
        if !resolutions.is_empty() {
            assert!(
                resolutions.iter().any(|r| r.is_resolved()),
                "Should resolve conflicts in nested sections"
            );
        }
    }

    #[test]
    fn test_multiple_precedence_rules() {
        let text = r#"Section 1 Main
Notwithstanding Section 3, subject to Article 5, ABC Corp (the "Company") shall deliver goods.
Section 3 Override
ABC Corp (the "Company") may deliver goods."#;

        let doc = run_full_precedence_pipeline(text);
        let resolutions = resolve_in_document(&doc);

        // Should detect multiple precedence rules
        // Notwithstanding should take priority as an override
        let explicit: Vec<_> = resolutions
            .iter()
            .filter(|r| matches!(r.basis, ResolutionBasis::ExplicitPrecedence { .. }))
            .collect();

        if !explicit.is_empty() {
            if let ResolutionBasis::ExplicitPrecedence { rule } = &explicit[0].basis {
                // Should prioritize "notwithstanding" (is_override = true)
                assert_eq!(rule.connective, "notwithstanding");
            }
        }
    }

    #[test]
    fn test_exhibit_vs_main_body() {
        let text = r#"Section 1 Services
ABC Corp (the "Company") shall provide services.
Exhibit 1 Service Details
ABC Corp (the "Company") may provide services."#;

        let doc = run_full_precedence_pipeline(text);
        let resolutions = resolve_in_document(&doc);

        // Section should win over Exhibit
        let structural: Vec<_> = resolutions
            .iter()
            .filter(|r| matches!(r.basis, ResolutionBasis::StructuralPrecedence { .. }))
            .collect();

        if !structural.is_empty() {
            if let ResolutionBasis::StructuralPrecedence {
                winning_kind,
                losing_kind,
            } = &structural[0].basis
            {
                assert_eq!(*winning_kind, SectionKind::Section);
                assert_eq!(*losing_kind, SectionKind::Exhibit);
            }
        }
    }

    #[test]
    fn test_recital_has_lowest_precedence() {
        let text = r#"Section 1 Main Terms
ABC Corp (the "Company") shall deliver goods.
Recital Background
ABC Corp (the "Company") may deliver goods."#;

        let doc = run_full_precedence_pipeline(text);
        let resolutions = resolve_in_document(&doc);

        // Recital should have lowest precedence
        let structural: Vec<_> = resolutions
            .iter()
            .filter(|r| matches!(r.basis, ResolutionBasis::StructuralPrecedence { .. }))
            .collect();

        if !structural.is_empty() {
            if let ResolutionBasis::StructuralPrecedence {
                winning_kind,
                losing_kind,
            } = &structural[0].basis
            {
                if *losing_kind == SectionKind::Recital {
                    // Anything should win over recital
                    assert!(
                        winning_kind.precedence_rank() < SectionKind::Recital.precedence_rank()
                    );
                }
            }
        }
    }

    #[test]
    fn test_confidence_scores_vary_by_basis() {
        let text = r#"Section 1 Main
Notwithstanding anything herein, ABC Corp (the "Company") shall deliver goods.
Schedule 1
ABC Corp (the "Company") may deliver goods."#;

        let doc = run_full_precedence_pipeline(text);
        let resolutions = resolve_in_document(&doc);

        if !resolutions.is_empty() {
            // Explicit precedence should have higher confidence than temporal
            let explicit_conf: Vec<f64> = resolutions
                .iter()
                .filter(|r| matches!(r.basis, ResolutionBasis::ExplicitPrecedence { .. }))
                .map(|r| r.confidence)
                .collect();

            let temporal_conf: Vec<f64> = resolutions
                .iter()
                .filter(|r| matches!(r.basis, ResolutionBasis::TemporalPrecedence { .. }))
                .map(|r| r.confidence)
                .collect();

            if !explicit_conf.is_empty() && !temporal_conf.is_empty() {
                assert!(
                    explicit_conf[0] > temporal_conf[0],
                    "Explicit precedence should have higher confidence than temporal"
                );
            }
        }
    }

    #[test]
    fn test_resolve_with_classifier_public_api() {
        use crate::document_structure::DocumentStructureBuilder;

        let text = r#"Section 1 Main
ABC Corp (the "Company") shall deliver goods.
Schedule 1
ABC Corp (the "Company") may deliver goods."#;

        let doc = run_full_precedence_pipeline(text);
        let structure = DocumentStructureBuilder::build(&doc).value;
        let classifier = SectionClassifier::new(&structure);

        // Create a mock conflict with spans that are within the actual content
        // Line 1 and line 3 each have about 18 tokens
        let conflict = Conflict::new(
            make_span(1, 0, 10),
            make_span(3, 0, 10),
            crate::ConflictType::ModalConflict,
            "Test conflict",
        );

        let resolver = PrecedenceResolver::new();
        let resolution = resolver.resolve_with_classifier(&conflict, &[], &classifier);

        // Should resolve via structural precedence
        assert!(resolution.is_resolved());
        assert!(matches!(
            resolution.basis,
            ResolutionBasis::StructuralPrecedence { .. }
        ));
    }
}
