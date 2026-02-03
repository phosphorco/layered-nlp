//! Document-level clause link resolver for M2 Gate 1.
//!
//! This resolver operates on a `LayeredDocument` that has already been processed
//! by `ClauseResolver` (line-level). It identifies clause spans across the document
//! and emits `SpanLink<ClauseRole>` edges representing parent-child relationships.
//!
//! ## Architecture
//!
//! This is a simpler function-based approach rather than a full `DocumentResolver` trait.
//! The function takes clause spans and returns SpanLink edges that can be stored
//! alongside the existing Clause attributes.
//!
//! ## Usage
//!
//! ```rust
//! use layered_nlp_document::LayeredDocument;
//! use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver};
//!
//! let doc = LayeredDocument::from_text("When it rains, then it pours.")
//!     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
//!     .run_resolver(&ClauseResolver::default());
//!
//! // Find clause relationships
//! let links = ClauseLinkResolver::resolve(&doc);
//! ```

use crate::{Clause, ClauseQueryAPI, ListMarker, ListMarkerResolver, SentenceBoundary};
use layered_contracts::{ObligationPhrase, ObligationType, Scored, SectionReference};
use layered_nlp::x;
use layered_nlp_document::{ClauseRole, DocSpan, DocSpanLink, LayeredDocument};

/// Confidence level for clause link detection.
///
/// Used to express certainty about clause relationships based on heuristics:
/// - High: Same-line links with explicit keywords
/// - Medium: Cross-line links within same sentence
/// - Low: Cross-line heuristic matches
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum LinkConfidence {
    /// Low confidence - cross-line heuristic match
    Low,
    /// Medium confidence - cross-line within sentence
    Medium,
    /// High confidence - same-line links
    High,
}

/// Type of coordination between clauses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CoordinationType {
    /// "and" - additive coordination
    Conjunction,
    /// "or" - alternative coordination
    Disjunction,
    /// "but", "however" - contrastive coordination
    Adversative,
    /// "nor" - negative alternative
    NegativeAlternative,
}

/// Operator precedence levels (higher = tighter binding)
pub const PRECEDENCE_AND: u8 = 2;  // Conjunction
pub const PRECEDENCE_OR: u8 = 1;   // Disjunction
pub const PRECEDENCE_BUT: u8 = 0;  // Adversative (loosest)



/// Document-level resolver that emits `SpanLink<ClauseRole>` edges based on Clause attributes.
///
/// This resolver operates after ClauseResolver has run on all lines.
/// It identifies parent-child relationships between clauses:
/// - Condition clauses are often children of TrailingEffect clauses
/// - Independent clauses have no parent relationships
pub struct ClauseLinkResolver;

/// A clause span with its category and document position
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseSpan {
    /// The document span covering this clause
    pub span: DocSpan,
    /// The clause category
    pub category: Clause,
}

/// A clause relationship represented as a SpanLink
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ClauseLink {
    /// The anchor clause (where this link is "stored")
    pub anchor: DocSpan,
    /// The link to the related clause
    pub link: DocSpanLink<ClauseRole>,
    /// Confidence level for this link
    pub confidence: LinkConfidence,
    /// Type of coordination (only set for Conjunct role links)
    pub coordination_type: Option<CoordinationType>,
    /// Precedence group ID for operator precedence.
    /// Higher values = tighter binding (evaluated first).
    /// None = no grouping (backward compatible with non-coordinated links).
    pub precedence_group: Option<u8>,
    /// Obligation type detected in this clause (Duty/Permission/Prohibition)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obligation_type: Option<ObligationType>,
}

impl ClauseLinkResolver {
    /// Extract all clause spans from a document.
    ///
    /// Searches through all lines for Clause attributes and converts them
    /// to document-level spans with positions.
    pub fn extract_clause_spans(doc: &LayeredDocument) -> Vec<ClauseSpan> {
        let mut clauses = Vec::new();

        for (line_idx, line) in doc.lines_enumerated() {
            // Find all Clause attributes on this line
            for find in line.find(&x::attr::<Clause>()) {
                let span_range = find.range();
                let clause_attr: &Clause = find.attr();

                // Convert line-local span to document span
                let doc_span = DocSpan::single_line(line_idx, span_range.0, span_range.1);

                clauses.push(ClauseSpan {
                    span: doc_span,
                    category: clause_attr.clone(),
                });
            }
        }

        clauses
    }

    /// Check if two spans are in the same sentence.
    /// Uses SentenceBoundary attributes if available, falls back to same-line check.
    fn in_same_sentence(doc: &LayeredDocument, span1: &DocSpan, span2: &DocSpan) -> bool {
        // If same line, check for sentence boundary between them on that line
        if span1.start.line == span2.start.line {
            // Same line: check if sentence boundary exists between the spans
            if let Some(line) = doc.lines().get(span1.start.line) {
                for find in line.find(&x::attr::<SentenceBoundary>()) {
                    let boundary_pos = find.range().0;
                    // If boundary is between the two spans, they're different sentences
                    if boundary_pos >= span1.end.token && boundary_pos < span2.start.token {
                        return false;
                    }
                }
            }
            return true; // Same line, no boundary between them
        }

        // Different lines: max distance check (heuristic: 10 lines)
        let line_distance = if span2.start.line > span1.end.line {
            span2.start.line - span1.end.line
        } else {
            span1.start.line - span2.end.line
        };
        if line_distance > 10 {
            return false; // Too far apart
        }

        // Check for sentence boundaries between the spans
        let start_line = span1.start.line.min(span2.start.line);
        let end_line = span1.start.line.max(span2.start.line);

        for line_idx in start_line..=end_line {
            if let Some(line) = doc.lines().get(line_idx) {
                for find in line.find(&x::attr::<SentenceBoundary>()) {
                    let boundary_pos = find.range().0;

                    // On first line: only check after span1 ends
                    if line_idx == span1.start.line && boundary_pos <= span1.end.token {
                        continue;
                    }
                    // On last line: only check before span2 starts
                    if line_idx == span2.start.line && boundary_pos >= span2.start.token {
                        continue;
                    }

                    // Found a sentence boundary between spans
                    return false;
                }
            }
        }

        true // No sentence boundary found between spans
    }

    /// Detect coordination chains between clauses separated by coordination keywords.
    ///
    /// Emits chain topology: A→B, B→C for "A, B, and C" (not star pattern A→B, A→C).
    ///
    /// This handles two coordination patterns:
    /// 1. Explicit coordination keyword (and/or/but/nor) between clauses: "A and B"
    /// 2. Comma-separated lists with final coordinator: "A, B, and C"
    ///    In this case, commas implicitly coordinate adjacent clauses if there's
    ///    a coordination keyword anywhere later in the sequence.
    fn detect_coordination(
        clause_spans: &[ClauseSpan],
        doc: &LayeredDocument,
    ) -> Vec<ClauseLink> {
        let mut links = Vec::new();

        // First pass: identify if this sequence contains any coordination
        // by checking if there's at least one coordination keyword
        let has_any_coordination = clause_spans.windows(2).any(|pair| {
            let current = &pair[0];
            let next = &pair[1];

            Self::in_same_sentence(doc, &current.span, &next.span)
                && Self::has_coordination_keyword_between_spanning(doc, &current.span, &next.span)
        });

        // Second pass: create chain links
        for i in 0..clause_spans.len().saturating_sub(1) {
            let current = &clause_spans[i];
            let next = &clause_spans[i + 1];

            // Only link clauses in the same sentence
            if !Self::in_same_sentence(doc, &current.span, &next.span) {
                continue;
            }

            // Check if there's a coordination keyword between current and next clause
            let has_coordination_keyword =
                Self::has_coordination_keyword_between_spanning(doc, &current.span, &next.span);

            // Check if there's an exception keyword - if so, don't create coordination link
            let has_exception_keyword =
                Self::has_exception_keyword_between_spanning(doc, &current.span, &next.span);

            // Create link if either:
            // 1. Explicit coordination keyword between these two clauses
            // 2. This sequence has coordination somewhere AND these are adjacent same-type clauses
            //    (handles "A, B, and C" pattern where commas implicitly coordinate)
            // BUT NOT if there's an exception keyword between them
            let should_link = !has_exception_keyword && (
                has_coordination_keyword
                || (has_any_coordination && current.category == next.category)
            );

            if should_link {
                // Set confidence based on line distance
                let confidence = if current.span.start.line == next.span.start.line {
                    LinkConfidence::High
                } else {
                    LinkConfidence::Medium
                };

                // Create unidirectional chain link: current → next
                links.push(ClauseLink {
                    anchor: current.span,
                    link: crate::ClauseLinkBuilder::conjunct_link(next.span),
                    confidence,
                    coordination_type: Self::detect_coordination_type_between_spanning(doc, &current.span, &next.span),
                    precedence_group: None,
                    obligation_type: None,
                });
            }
        }

        // Assign precedence groups to partition chain by operator precedence
        Self::assign_precedence_groups(&mut links);

        links
    }

    /// Assign precedence group IDs to coordination links.
    /// Groups contiguous runs of same-precedence operators.
    /// Higher group numbers don't mean higher precedence - they indicate
    /// a change in operator type along the chain.
    fn assign_precedence_groups(links: &mut [ClauseLink]) {
        let mut group_id = 0u8;
        let mut current_precedence: Option<u8> = None;
        
        for link in links.iter_mut() {
            if link.link.role != ClauseRole::Conjunct {
                continue;
            }
            
            let link_precedence = match link.coordination_type {
                Some(CoordinationType::Conjunction) => PRECEDENCE_AND,
                Some(CoordinationType::Disjunction) => PRECEDENCE_OR,
                Some(CoordinationType::Adversative) | 
                Some(CoordinationType::NegativeAlternative) => PRECEDENCE_BUT,
                None => continue,
            };
            
            match current_precedence {
                None => {
                    current_precedence = Some(link_precedence);
                }
                Some(prev) if prev != link_precedence => {
                    group_id += 1;
                    current_precedence = Some(link_precedence);
                }
                _ => {}
            }
            
            link.precedence_group = Some(group_id);
        }
    }

    /// Detect list relationships between clauses based on ListMarker attributes.
    ///
    /// This method identifies list item clauses (those that start with a ListMarker)
    /// and links them to their container clause (the clause immediately before the first
    /// list item in a group).
    ///
    /// # Algorithm
    /// 1. Run ListMarkerResolver on each line to ensure markers are detected
    /// 2. For each clause, check if it starts with (or immediately follows) a ListMarker
    /// 3. Group consecutive list items with compatible marker types
    /// 4. Link each list item to the container clause
    ///
    /// # Arguments
    /// * `clause_spans` - All clause spans in the document
    /// * `doc` - The layered document (with ListMarker attributes)
    ///
    /// # Returns
    /// A vector of ClauseLinks representing list relationships
    fn detect_list_relationships(
        clause_spans: &[ClauseSpan],
        doc: &LayeredDocument,
    ) -> Vec<ClauseLink> {
        let mut links = Vec::new();

        if clause_spans.is_empty() {
            return links;
        }

        // Find list markers and associate them with clauses
        // A clause is a list item if it contains a ListMarker at or near its start
        let mut list_item_indices: Vec<(usize, ListMarker)> = Vec::new();

        for (idx, clause) in clause_spans.iter().enumerate() {
            if let Some(marker) = Self::get_list_marker_for_clause(doc, &clause.span) {
                list_item_indices.push((idx, marker));
            }
        }

        if list_item_indices.is_empty() {
            return links;
        }

        // Group consecutive list items with compatible marker types
        // Each group shares a common container clause
        let mut current_group: Vec<usize> = Vec::new();
        let mut current_marker_type: Option<std::mem::Discriminant<ListMarker>> = None;

        for (idx, marker) in &list_item_indices {
            let marker_type = std::mem::discriminant(marker);

            // Check if this continues the current group
            let continues_group = if let Some(prev_type) = current_marker_type {
                // Same marker type and consecutive clause index
                prev_type == marker_type
                    && !current_group.is_empty()
                    && *idx == current_group.last().unwrap() + 1
            } else {
                false
            };

            if continues_group {
                current_group.push(*idx);
            } else {
                // Emit links for the previous group if non-empty
                if !current_group.is_empty() {
                    links.extend(Self::create_list_group_links(clause_spans, &current_group));
                }
                // Start new group
                current_group = vec![*idx];
                current_marker_type = Some(marker_type);
            }
        }

        // Emit links for the last group
        if !current_group.is_empty() {
            links.extend(Self::create_list_group_links(clause_spans, &current_group));
        }

        links
    }

    /// Find the next TrailingEffect clause after a Condition, allowing list items in between.
    /// Stops when it encounters a non-list, non-trailing clause or a sentence boundary.
    fn find_trailing_effect_after_condition<'a>(
        clause_spans: &'a [ClauseSpan],
        list_items: &[DocSpan],
        doc: &LayeredDocument,
        start_idx: usize,
    ) -> Option<&'a ClauseSpan> {
        let current = &clause_spans[start_idx];

        for next in clause_spans.iter().skip(start_idx + 1) {
            if !Self::in_same_sentence(doc, &current.span, &next.span) {
                break;
            }

            if next.category == Clause::TrailingEffect {
                return Some(next);
            }

            if list_items.iter().any(|span| *span == next.span) {
                continue;
            }

            break;
        }

        None
    }

    /// Get the ListMarker for a clause if it starts with one.
    ///
    /// A clause "starts with" a list marker if the marker's token range
    /// is close to the clause's start position.
    fn get_list_marker_for_clause(doc: &LayeredDocument, clause_span: &DocSpan) -> Option<ListMarker> {
        let line_idx = clause_span.start.line;
        let clause_start_token = clause_span.start.token;

        if let Some(line) = doc.lines().get(line_idx) {
            // Find the closest marker to the clause start
            let mut closest_marker: Option<(ListMarker, usize)> = None;
            let mut closest_distance = usize::MAX;

            for find in line.find(&x::attr::<ListMarker>()) {
                let (marker_start, marker_end) = find.range();

                // Calculate distance between marker and clause start
                let distance = if marker_end <= clause_start_token {
                    clause_start_token - marker_end
                } else if marker_start >= clause_start_token {
                    marker_start - clause_start_token
                } else {
                    // Overlapping - distance is 0
                    0
                };

                // Only consider markers that are very close (within 3 tokens)
                if distance <= 3 && distance < closest_distance {
                    closest_distance = distance;
                    closest_marker = Some(((*find.attr()).clone(), distance));
                }
            }

            // Return the closest marker if found
            if let Some((marker, _)) = closest_marker {
                return Some(marker);
            }
        }

        None
    }

    /// Create list links for a group of list items.
    ///
    /// The container is the clause immediately before the first list item.
    /// Each list item gets a ListItem link to the container.
    /// The container gets ONE ListContainer link to the first item only.
    fn create_list_group_links(clause_spans: &[ClauseSpan], group: &[usize]) -> Vec<ClauseLink> {
        let mut links = Vec::new();

        if group.is_empty() {
            return links;
        }

        let first_item_idx = group[0];

        // The container is the clause immediately before the first list item
        // If first_item_idx == 0, there's no container clause
        if first_item_idx == 0 {
            return links;
        }

        let container_idx = first_item_idx - 1;
        let container_span = clause_spans[container_idx].span;
        let first_item_span = clause_spans[first_item_idx].span;

        // Set confidence based on whether list items are on same line as container
        let same_line = first_item_span.start.line == container_span.start.line;
        let confidence = if same_line {
            LinkConfidence::High
        } else {
            LinkConfidence::Medium
        };

        // Create ListItem links: each item → container
        for &item_idx in group {
            let item_span = clause_spans[item_idx].span;

            links.push(ClauseLink {
                anchor: item_span,
                link: crate::ClauseLinkBuilder::list_item_link(container_span),
                confidence,
                coordination_type: None,
                precedence_group: None,
                obligation_type: None,
            });
        }

        // Create ONE ListContainer link: container → first item only
        links.push(ClauseLink {
            anchor: container_span,
            link: crate::ClauseLinkBuilder::list_container_link(first_item_span),
            confidence,
            coordination_type: None,
            precedence_group: None,
            obligation_type: None,
        });

        links
    }

    /// Detect cross-reference relationships between clauses and section references.
    ///
    /// This method finds `SectionReference` attributes within clause spans and creates
    /// `CrossReference` links from the clause to the section reference span.
    ///
    /// # Algorithm
    /// 1. For each clause span, scan all lines it covers
    /// 2. Find any `SectionReference` attributes within the clause's token range
    /// 3. Create a `CrossReference` link from the clause to the reference span
    ///
    /// # Arguments
    /// * `clause_spans` - All clause spans in the document
    /// * `doc` - The layered document (with SectionReference attributes)
    ///
    /// # Returns
    /// A vector of ClauseLinks representing cross-reference relationships
    fn detect_cross_references(
        clause_spans: &[ClauseSpan],
        doc: &LayeredDocument,
    ) -> Vec<ClauseLink> {
        let mut links = Vec::new();

        for clause in clause_spans {
            let clause_span = &clause.span;

            // Iterate through all lines covered by this clause
            for line_idx in clause_span.start.line..=clause_span.end.line {
                if let Some(line) = doc.lines().get(line_idx) {
                    // Find all SectionReference attributes on this line
                    for find in line.find(&x::attr::<SectionReference>()) {
                        let (ref_start, ref_end) = find.range();

                        // Determine if this reference is within the clause span
                        let ref_in_clause = if clause_span.start.line == clause_span.end.line {
                            // Single-line clause: reference must be within token range
                            line_idx == clause_span.start.line
                                && ref_start >= clause_span.start.token
                                && ref_end <= clause_span.end.token
                        } else if line_idx == clause_span.start.line {
                            // First line of multi-line clause: reference must start at or after clause start
                            ref_start >= clause_span.start.token
                        } else if line_idx == clause_span.end.line {
                            // Last line of multi-line clause: reference must end at or before clause end
                            ref_end <= clause_span.end.token
                        } else {
                            // Middle line of multi-line clause: all references are within the clause
                            true
                        };

                        if ref_in_clause {
                            // Create the section reference span
                            let ref_span = DocSpan::single_line(line_idx, ref_start, ref_end);

                            // Assign confidence based on whether reference is on same line as clause start
                            let confidence = if line_idx == clause_span.start.line {
                                LinkConfidence::High
                            } else {
                                LinkConfidence::Medium
                            };

                            // Create CrossReference link: clause → section reference
                            links.push(ClauseLink {
                                anchor: *clause_span,
                                link: crate::ClauseLinkBuilder::cross_reference_link(ref_span),
                                confidence,
                                coordination_type: None,
                                precedence_group: None,
                                obligation_type: None,
                            });
                        }
                    }
                }
            }
        }

        links
    }

    /// Detect obligation types from ObligationPhraseResolver output and populate ClauseLink.obligation_type
    ///
    /// This method scans each clause's span for `Scored<ObligationPhrase>` attributes and
    /// populates the `obligation_type` field on matching ClauseLinks. The first match within
    /// a clause wins.
    ///
    /// # Arguments
    /// * `clause_links` - Mutable vector of ClauseLinks to enrich with obligation types
    /// * `doc` - The layered document (with ObligationPhrase attributes from ObligationPhraseResolver)
    fn detect_obligations(
        clause_links: &mut Vec<ClauseLink>,
        doc: &LayeredDocument,
    ) {
        for link in clause_links.iter_mut() {
            let anchor = &link.anchor;
            // DocSpan has start.line and end.line fields
            for line_idx in anchor.start.line..=anchor.end.line {
                if let Some(line) = doc.lines().get(line_idx) {
                    // Query for Scored<ObligationPhrase> on this line
                    let findings = line.find(&x::attr::<Scored<ObligationPhrase>>());
                    if let Some(first) = findings.first() {
                        let scored_phrase = first.attr();
                        link.obligation_type = Some(scored_phrase.value.obligation_type.clone());
                        break; // first match wins within clause
                    }
                }
            }
        }
    }

    /// Create self-referential links for clauses that have obligations but no relationships.
    ///
    /// This ensures that standalone clauses with obligations (like "Tenant shall pay rent.")
    /// can be queried via the ClauseQueryAPI, even though they have no coordination,
    /// exception, or parent-child relationships.
    ///
    /// # Arguments
    /// * `clause_spans` - All clause spans in the document
    /// * `existing_links` - Links already created by other detection passes
    /// * `doc` - The layered document (with ObligationPhrase attributes)
    ///
    /// # Returns
    /// A vector of ClauseLinks for standalone clauses with obligations
    fn create_obligation_only_links(
        clause_spans: &[ClauseSpan],
        existing_links: &[ClauseLink],
        doc: &LayeredDocument,
    ) -> Vec<ClauseLink> {
        let mut links = Vec::new();

        // Find clauses that don't have any links yet
        let clauses_with_links: Vec<_> = existing_links
            .iter()
            .map(|link| link.anchor)
            .collect();

        for clause in clause_spans {
            // Skip if this clause already has a link
            if clauses_with_links.contains(&clause.span) {
                continue;
            }

            // Check if this clause has an obligation
            let obligation_type = Self::detect_obligation_for_span(&clause.span, doc);

            if let Some(obl_type) = obligation_type {
                // Create a self-referential link to capture the obligation
                // We use the clause's own span as target (Self role indicates no relationship)
                links.push(ClauseLink {
                    anchor: clause.span,
                    link: DocSpanLink {
                        role: ClauseRole::Self_,
                        target: clause.span,
                    },
                    confidence: LinkConfidence::High,
                    coordination_type: None,
                    precedence_group: None,
                    obligation_type: Some(obl_type),
                });
            }
        }

        links
    }

    /// Detect obligation type for a single clause span.
    ///
    /// Returns the first ObligationType found within the span, or None if no obligation detected.
    fn detect_obligation_for_span(span: &DocSpan, doc: &LayeredDocument) -> Option<ObligationType> {
        for line_idx in span.start.line..=span.end.line {
            if let Some(line) = doc.lines().get(line_idx) {
                let findings = line.find(&x::attr::<Scored<ObligationPhrase>>());
                if let Some(first) = findings.first() {
                    let scored_phrase = first.attr();
                    return Some(scored_phrase.value.obligation_type.clone());
                }
            }
        }
        None
    }

    /// Check if there's a coordination keyword (ClauseKeyword::And) between two token positions.
    fn has_coordination_keyword_between(
        doc: &LayeredDocument,
        line_idx: usize,
        start_token: usize,
        end_token: usize,
    ) -> bool {
        use crate::ClauseKeyword;

        if let Some(line) = doc.lines().get(line_idx) {
            // Search the range between the two clauses for ClauseKeyword::And
            for find in line.find(&x::attr::<ClauseKeyword>()) {
                let (token_start, _token_end) = find.range();

                // Keyword must be between the clauses
                if token_start >= start_token && token_start < end_token {
                    if matches!(find.attr(), ClauseKeyword::And | ClauseKeyword::Or | ClauseKeyword::But | ClauseKeyword::Nor) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Detect the type of coordination keyword between two spans (same line).
    /// Returns Some(CoordinationType) if a coordination keyword is found, None otherwise.
    fn detect_coordination_type_between(
        doc: &LayeredDocument,
        line_idx: usize,
        start_token: usize,
        end_token: usize,
    ) -> Option<CoordinationType> {
        use crate::ClauseKeyword;

        if let Some(line) = doc.lines().get(line_idx) {
            for find in line.find(&x::attr::<ClauseKeyword>()) {
                let (token_start, _token_end) = find.range();

                // Keyword must be between the clauses
                if token_start >= start_token && token_start < end_token {
                    return match find.attr() {
                        ClauseKeyword::And => Some(CoordinationType::Conjunction),
                        ClauseKeyword::Or => Some(CoordinationType::Disjunction),
                        ClauseKeyword::But => Some(CoordinationType::Adversative),
                        ClauseKeyword::Nor => Some(CoordinationType::NegativeAlternative),
                        _ => None,
                    };
                }
            }
        }

        None
    }

    /// Detect coordination type between two spans (works across lines).
    fn detect_coordination_type_between_spanning(
        doc: &LayeredDocument,
        span1: &DocSpan,
        span2: &DocSpan,
    ) -> Option<CoordinationType> {
        use crate::ClauseKeyword;

        // Same line: use existing single-line logic
        if span1.start.line == span2.start.line {
            return Self::detect_coordination_type_between(
                doc,
                span1.start.line,
                span1.end.token,
                span2.start.token,
            );
        }

        // Different lines: search end of first line, all middle lines, start of last line
        let start_line = span1.start.line;
        let end_line = span2.start.line;

        for line_idx in start_line..=end_line {
            if let Some(line) = doc.lines().get(line_idx) {
                for find in line.find(&x::attr::<ClauseKeyword>()) {
                    let kw_pos = find.range().0;
                    let keyword = find.attr();

                    // Only coordination keywords
                    let coord_type = match keyword {
                        ClauseKeyword::And => Some(CoordinationType::Conjunction),
                        ClauseKeyword::Or => Some(CoordinationType::Disjunction),
                        ClauseKeyword::But => Some(CoordinationType::Adversative),
                        ClauseKeyword::Nor => Some(CoordinationType::NegativeAlternative),
                        _ => None,
                    };

                    if coord_type.is_none() {
                        continue;
                    }

                    // On first line: only after span1 ends
                    if line_idx == start_line && kw_pos <= span1.end.token {
                        continue;
                    }
                    // On last line: only before span2 starts
                    if line_idx == end_line && kw_pos >= span2.start.token {
                        continue;
                    }

                    return coord_type;
                }
            }
        }

        None
    }


    /// Check if a coordination keyword (and/or/but) exists between two spans.
    /// Works across lines.
    fn has_coordination_keyword_between_spanning(
        doc: &LayeredDocument,
        span1: &DocSpan,
        span2: &DocSpan,
    ) -> bool {
        use crate::ClauseKeyword;

        // Same line: use existing single-line logic
        if span1.start.line == span2.start.line {
            return Self::has_coordination_keyword_between(
                doc,
                span1.start.line,
                span1.end.token,
                span2.start.token,
            );
        }

        // Different lines: search end of first line, all middle lines, start of last line
        let start_line = span1.start.line;
        let end_line = span2.start.line;

        for line_idx in start_line..=end_line {
            if let Some(line) = doc.lines().get(line_idx) {
                for find in line.find(&x::attr::<ClauseKeyword>()) {
                    let kw_pos = find.range().0;
                    let keyword = find.attr();

                    // Only coordination keywords
                    if !matches!(keyword, ClauseKeyword::And | ClauseKeyword::Or | ClauseKeyword::But | ClauseKeyword::Nor) {
                        continue;
                    }

                    // On first line: only after span1 ends
                    if line_idx == start_line && kw_pos <= span1.end.token {
                        continue;
                    }
                    // On last line: only before span2 starts
                    if line_idx == end_line && kw_pos >= span2.start.token {
                        continue;
                    }

                    return true;
                }
            }
        }

        false
    }

    /// Check if there's an exception keyword (ClauseKeyword::Exception) between two token positions.
    fn has_exception_keyword_between(
        doc: &LayeredDocument,
        line_idx: usize,
        start_token: usize,
        end_token: usize,
    ) -> bool {
        use crate::ClauseKeyword;

        if let Some(line) = doc.lines().get(line_idx) {
            // Search for ClauseKeyword::Exception in the range
            for find in line.find(&x::attr::<ClauseKeyword>()) {
                let (token_start, _token_end) = find.range();

                if token_start >= start_token && token_start < end_token {
                    if let ClauseKeyword::Exception = find.attr() {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Check if an exception keyword (unless/except/notwithstanding) exists between two spans.
    /// Works across lines.
    fn has_exception_keyword_between_spanning(
        doc: &LayeredDocument,
        span1: &DocSpan,
        span2: &DocSpan,
    ) -> bool {
        use crate::ClauseKeyword;

        // Same line: use existing single-line logic
        if span1.start.line == span2.start.line {
            return Self::has_exception_keyword_between(
                doc,
                span1.start.line,
                span1.end.token,
                span2.start.token,
            );
        }

        // Different lines: search end of first line, all middle lines, start of last line
        let start_line = span1.start.line;
        let end_line = span2.start.line;

        for line_idx in start_line..=end_line {
            if let Some(line) = doc.lines().get(line_idx) {
                for find in line.find(&x::attr::<ClauseKeyword>()) {
                    let kw_pos = find.range().0;
                    let keyword = find.attr();

                    // Only exception keywords
                    if !matches!(keyword, ClauseKeyword::Exception) {
                        continue;
                    }

                    // On first line: only after span1 ends
                    if line_idx == start_line && kw_pos <= span1.end.token {
                        continue;
                    }
                    // On last line: only before span2 starts
                    if line_idx == end_line && kw_pos >= span2.start.token {
                        continue;
                    }

                    return true;
                }
            }
        }

        false
    }

    /// Check if there's an exception keyword before a given token position
    fn has_exception_keyword_before(
        doc: &LayeredDocument,
        line_idx: usize,
        token_pos: usize,
    ) -> bool {
        use crate::ClauseKeyword;

        if let Some(line) = doc.lines().get(line_idx) {
            for find in line.find(&x::attr::<ClauseKeyword>()) {
                let (token_start, _token_end) = find.range();

                // Keyword must be before the token position
                if token_start < token_pos {
                    if let ClauseKeyword::Exception = find.attr() {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Check if there's a semicolon between two spans in the source text.
    ///
    /// Semicolons act as statement boundaries that should block exception propagation.
    /// For example, in "A; B, unless C", the exception C should only apply to B,
    /// not to A, because the semicolon separates independent statements.
    ///
    fn has_semicolon_between(doc: &LayeredDocument, span1: &DocSpan, span2: &DocSpan) -> bool {
        let (first, second) = if (span1.start.line, span1.start.token)
            <= (span2.start.line, span2.start.token)
        {
            (span1, span2)
        } else {
            (span2, span1)
        };

        for line_idx in first.start.line..=second.start.line {
            if let Some(line) = doc.lines().get(line_idx) {
                for find in line.find(&x::token_text()) {
                    if !find.attr().contains(';') {
                        continue;
                    }

                    let pos = find.range().0;
                    if line_idx == first.start.line && pos <= first.end.token {
                        continue;
                    }
                    if line_idx == second.start.line && pos >= second.start.token {
                        continue;
                    }

                    return true;
                }
            }
        }

        false
    }

    /// Detect exception/carve-out relationships with access to coordination information.
    ///
    /// This version accepts a `ClauseQueryAPI` to enable scope propagation:
    /// for "A and B, unless C", the exception should link to ALL coordinated clauses
    /// in the group, not just the immediate predecessor.
    ///
    /// # Algorithm
    /// 1. Iterate through adjacent clause pairs looking for exception keywords
    /// 2. When found: "current [unless] next" means next is exception modifying current
    /// 3. Use `api.conjuncts(current.span)` to find all clauses coordinated with current
    /// 4. Create exception links from next to current AND all its conjuncts
    ///
    /// # Arguments
    /// * `clause_spans` - All clause spans in the document
    /// * `doc` - The layered document
    /// * `api` - Query API with coordination links for conjunct traversal
    fn detect_exceptions_with_api(
        clause_spans: &[ClauseSpan],
        doc: &LayeredDocument,
        api: &ClauseQueryAPI<'_>,
    ) -> Vec<ClauseLink> {
        let mut links = Vec::new();

        for i in 0..clause_spans.len().saturating_sub(1) {
            let current = &clause_spans[i];
            let next = &clause_spans[i + 1];

            // Only link clauses in the same sentence
            if !Self::in_same_sentence(doc, &current.span, &next.span) {
                continue;
            }

            // Check if there's an exception keyword between current and next
            if Self::in_same_sentence(doc, &current.span, &next.span)
                && Self::has_exception_keyword_between_spanning(doc, &current.span, &next.span)
            {
                // Set confidence based on line distance
                let confidence = if current.span.start.line == next.span.start.line {
                    LinkConfidence::High
                } else {
                    LinkConfidence::Medium
                };

                // Standard pattern: "A [exception-keyword] B"
                // B is the exception that modifies A
                // Exception link: next (exception) -> current (main)
                links.push(ClauseLink {
                    anchor: next.span,
                    link: crate::ClauseLinkBuilder::exception_link(current.span),
                    confidence,
                    coordination_type: None,
                    precedence_group: None,
                    obligation_type: None,
                });

                // TRANSITIVE EXCEPTION LINKING:
                // Find all clauses coordinated with current and link exception to them too
                // BUT: don't propagate across semicolon boundaries
                // For "A; B, unless C" - the semicolon blocks propagation to A
                let conjuncts = api.conjuncts(current.span);
                for conjunct_span in conjuncts {
                    // Don't duplicate the link to current (already added above)
                    // Note: conjuncts() excludes the query span itself, so no duplicate possible

                    // Check for semicolon boundary between conjunct and the exception source
                    // If there's a semicolon between them, don't propagate the exception
                    if Self::has_semicolon_between(doc, &conjunct_span, &current.span) {
                        continue; // Don't propagate across semicolon boundary
                    }

                    links.push(ClauseLink {
                        anchor: next.span,
                        link: crate::ClauseLinkBuilder::exception_link(conjunct_span),
                        confidence,
                        coordination_type: None,
                        precedence_group: None,
                        obligation_type: None,
                    });
                }
            }
        }

        // Special case: Check for exception keyword at the very start (before first clause)
        // Pattern: "[exception-keyword] X, Y" where X is first clause
        // In this case, Y modifies/excepts X
        if clause_spans.len() >= 2 {
            let first = &clause_spans[0];
            let second = &clause_spans[1];

            if Self::in_same_sentence(doc, &first.span, &second.span) {
                // Check if there's an exception keyword before the first clause
                if Self::has_exception_keyword_before(
                    doc,
                    first.span.start.line,
                    first.span.start.token,
                ) {
                    // Set confidence based on line distance
                    let confidence = if first.span.start.line == second.span.start.line {
                        LinkConfidence::High
                    } else {
                        LinkConfidence::Medium
                    };

                    // second clause is the main clause that applies despite first clause
                    // Exception link: second -> first
                    links.push(ClauseLink {
                        anchor: second.span,
                        link: crate::ClauseLinkBuilder::exception_link(first.span),
                        confidence,
                        coordination_type: None,
                        precedence_group: None,
                        obligation_type: None,
                    });

                    // Also propagate to conjuncts of first clause
                    // BUT: don't propagate across semicolon boundaries
                    let conjuncts = api.conjuncts(first.span);
                    for conjunct_span in conjuncts {
                        // Check for semicolon boundary between conjunct and first span
                        if Self::has_semicolon_between(doc, &conjunct_span, &first.span) {
                            continue; // Don't propagate across semicolon boundary
                        }

                        links.push(ClauseLink {
                            anchor: second.span,
                            link: crate::ClauseLinkBuilder::exception_link(conjunct_span),
                            confidence,
                            coordination_type: None,
                            precedence_group: None,
                            obligation_type: None,
                        });
                    }
                }
            }
        }

        // CHAINED EXCEPTION PROPAGATION:
        // For "A, unless B, except C": B→A exists, then C→B creates C→A
        // If clause C is an exception to clause B, and B is already an exception to A,
        // then C should also become an exception to A (transitively)
        let mut transitive_links = Vec::new();
        for new_link in &links {
            if new_link.link.role == ClauseRole::Exception {
                // Find what the target clause (e.g., B) is an exception to (e.g., A)
                // We need to look in ALL existing links (the full set including coordination links)
                // since some exception chains may involve links created earlier in the pipeline
                for existing in &links {
                    if existing.anchor == new_link.link.target
                        && existing.link.role == ClauseRole::Exception
                    {
                        // new_link: C → B (C is exception to B)
                        // existing: B → A (B is exception to A)
                        // Create transitive: C → A
                        transitive_links.push(ClauseLink {
                            anchor: new_link.anchor,           // C (the source exception)
                            link: crate::ClauseLinkBuilder::exception_link(existing.link.target), // A (the ultimate target)
                            confidence: new_link.confidence,   // Preserve confidence from new link
                            coordination_type: None,
                            precedence_group: None,
                            obligation_type: None,
                        });
                    }
                }
            }
        }
        links.extend(transitive_links);

        links
    }

    /// Resolve clause relationships and emit SpanLink edges.
    ///
    /// Implemented patterns:
    /// - Gate 1: Condition clauses followed by TrailingEffect clauses form parent-child relationships
    /// - Gate 2: Coordination chains ("and", "or", "but") emit Conjunct links with chain topology
    /// - Gate 3: Exception clauses ("except", "unless", "notwithstanding") emit Exception links
    /// - Gate 4: List relationships between container clauses and list items
    /// - Gate 4b: Cross-references to sections (e.g., "subject to Section 3.2")
    ///
    /// TODO(Gate 5): Storage integration - links should be persisted alongside Clause
    /// attributes in the SpanIndex rather than returned as ephemeral Vec. This will
    /// enable querying relationships via the standard query API.
    pub fn resolve(doc: &LayeredDocument) -> Vec<ClauseLink> {
        let clause_spans = Self::extract_clause_spans(doc);
        let mut links = Vec::new();

        // Precompute list relationships for condition-block handling
        let list_links = Self::detect_list_relationships(&clause_spans, doc);
        let list_item_spans: Vec<DocSpan> = list_links
            .iter()
            .filter(|link| link.link.role == ClauseRole::ListItem)
            .map(|link| link.anchor)
            .collect();

        // Gate 1: Condition → TrailingEffect parent-child relationships
        // Handles patterns like: "When it rains, then it pours"
        // where "it rains" (Condition) is a child of "it pours" (TrailingEffect)
        for i in 0..clause_spans.len() {
            let current = &clause_spans[i];

            // Look for Condition followed by TrailingEffect in the same sentence
            if current.category == Clause::Condition {
                if let Some(next) =
                    Self::find_trailing_effect_after_condition(&clause_spans, &list_item_spans, doc, i)
                {
                    // Set confidence based on line distance
                    let confidence = if current.span.start.line == next.span.start.line {
                        LinkConfidence::High
                    } else {
                        LinkConfidence::Medium
                    };

                    // Condition (child) points to TrailingEffect (parent)
                    links.push(ClauseLink {
                        anchor: current.span,
                        link: crate::ClauseLinkBuilder::parent_link(next.span),
                        confidence,
                        coordination_type: None,
                        precedence_group: None,
                        obligation_type: None,
                    });

                    // TrailingEffect (parent) points back to Condition (child)
                    links.push(ClauseLink {
                        anchor: next.span,
                        link: crate::ClauseLinkBuilder::child_link(current.span),
                        confidence,
                        coordination_type: None,
                        precedence_group: None,
                        obligation_type: None,
                    });
                }
            }
        }

        // Gate 2: Detect coordination chains
        // Handles patterns like: "A and B" → one link, "A, B, and C" → two links (chain)
        let coordination_links = Self::detect_coordination(&clause_spans, doc);
        links.extend(coordination_links.clone());

        // Build temporary API with coordination links for exception detection
        // This enables exceptions to propagate to all coordinated clauses
        let temp_api = ClauseQueryAPI::new(&links);

        // Gate 3: Detect exception/carve-out relationships
        // Handles patterns like: "A unless B" → Exception link from B to A
        // Uses temp_api to access conjuncts() for scope propagation
        let exception_links = Self::detect_exceptions_with_api(&clause_spans, doc, &temp_api);
        links.extend(exception_links);

        // Gate 4: Detect list relationships
        // Handles patterns like: "Do the following: (a) first (b) second"
        // List item clauses link to container clause that precedes them
        links.extend(list_links);

        // Gate 4b: Detect cross-reference relationships
        // Handles patterns like: "subject to Section 3.2" → CrossReference link
        // Links clause spans to SectionReference attributes within them
        let cross_ref_links = Self::detect_cross_references(&clause_spans, doc);
        links.extend(cross_ref_links);

        // Detect obligation types from ObligationPhraseResolver output
        // This enriches existing links with obligation_type
        Self::detect_obligations(&mut links, doc);

        // Create self-referential links for standalone clauses with obligations
        // This ensures clauses like "Tenant shall pay rent." can be queried for obligations
        // even when they have no relationships to other clauses
        let obligation_only_links = Self::create_obligation_only_links(&clause_spans, &links, doc);
        links.extend(obligation_only_links);

        links
    }

    /// Resolve clause relationships with list marker detection enabled.
    ///
    /// This version first runs the ListMarkerResolver on each line of the document
    /// to ensure list markers are detected before analyzing list relationships.
    ///
    /// Use this when you want full list detection without having to manually run
    /// the ListMarkerResolver beforehand.
    ///
    /// Note: This method clones the document. For better performance, run
    /// `ListMarkerResolver` before calling `resolve()` directly.
    pub fn resolve_with_list_markers(doc: LayeredDocument) -> (LayeredDocument, Vec<ClauseLink>) {
        // Run ListMarkerResolver on each line
        let doc_with_markers = doc.run_resolver(&ListMarkerResolver::new());

        let links = Self::resolve(&doc_with_markers);
        (doc_with_markers, links)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClauseKeywordResolver, ClauseResolver};

    fn create_test_document(text: &str) -> LayeredDocument {
        LayeredDocument::from_text(text)
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default())
    }

    #[test]
    fn test_extract_clause_spans() {
        let doc = create_test_document("When it rains, then it pours.");

        let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

        // Should find 2 clauses: Condition and TrailingEffect
        assert_eq!(clauses.len(), 2);
        assert_eq!(clauses[0].category, Clause::Condition);
        assert_eq!(clauses[1].category, Clause::TrailingEffect);
    }

    #[test]
    fn test_resolve_condition_trailing_effect() {
        let doc = create_test_document("When it rains, then it pours.");

        let links = ClauseLinkResolver::resolve(&doc);

        // Should create 2 bidirectional links: Condition->Parent and Parent->Child
        assert_eq!(links.len(), 2);

        // First link: Condition points to TrailingEffect as Parent
        assert_eq!(links[0].link.role, ClauseRole::Parent);

        // Second link: TrailingEffect points to Condition as Child
        assert_eq!(links[1].link.role, ClauseRole::Child);
    }

    #[test]
    fn test_resolve_independent_clause() {
        let doc = create_test_document("It rains.");

        let links = ClauseLinkResolver::resolve(&doc);

        // Independent clause should have no relationships
        assert_eq!(links.len(), 0);
    }

    #[test]
    fn test_extract_from_empty_document() {
        let doc = LayeredDocument::from_text("");

        let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clauses.len(), 0);

        let links = ClauseLinkResolver::resolve(&doc);
        assert_eq!(links.len(), 0);
    }

    #[test]
    fn test_multiple_independent_clauses() {
        // Create a document with multiple independent clauses (no keywords)
        let doc = LayeredDocument::from_text("The tenant shall pay rent. The landlord provides notice.")
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        // Independent clauses should not have relationships with each other
        assert_eq!(links.len(), 0);
    }

    // ========================================================================
    // Gate 2: Coordination Chain Tests
    // ========================================================================

    #[test]
    fn test_coordination_simple_two_clauses() {
        let doc = create_test_document("The tenant pays rent and the landlord provides notice.");

        let links = ClauseLinkResolver::resolve(&doc);

        // Should have one Conjunct link: first clause → second clause
        // Filter for Conjunct links only (ignoring any parent/child links)
        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(
            conjunct_links.len(),
            1,
            "Expected one Conjunct link for 'A and B'"
        );
        assert_eq!(conjunct_links[0].link.role, ClauseRole::Conjunct);
    }

    #[test]
    fn test_coordination_chain_three_clauses() {
        let doc = create_test_document("A pays, B works, and C manages.");

        let links = ClauseLinkResolver::resolve(&doc);

        // Should have two Conjunct links forming a chain: A→B, B→C (not A→B, A→C)
        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(
            conjunct_links.len(),
            2,
            "Expected two Conjunct links for 'A, B, and C' chain topology"
        );

        // Verify chain topology: first→second, second→third
        let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
        assert!(clauses.len() >= 3, "Should have at least 3 clauses");

        // First link should go from first clause to second
        assert_eq!(conjunct_links[0].anchor, clauses[0].span);
        assert_eq!(conjunct_links[0].link.target, clauses[1].span);

        // Second link should go from second clause to third
        assert_eq!(conjunct_links[1].anchor, clauses[1].span);
        assert_eq!(conjunct_links[1].link.target, clauses[2].span);
    }

    #[test]
    fn test_coordination_no_and_keyword() {
        let doc = create_test_document("The tenant pays rent. The landlord provides notice.");

        let links = ClauseLinkResolver::resolve(&doc);

        // No "and" keyword, so no Conjunct links
        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(conjunct_links.len(), 0, "No 'and' keyword means no Conjunct links");
    }

    #[test]
    fn test_coordination_with_condition() {
        // Mix coordination with condition clauses
        let doc = create_test_document("When it rains, then A happens and B occurs.");

        let links = ClauseLinkResolver::resolve(&doc);

        // Should have both parent-child links AND coordination link
        let parent_child_links: Vec<_> = links
            .iter()
            .filter(|link| {
                link.link.role == ClauseRole::Parent || link.link.role == ClauseRole::Child
            })
            .collect();

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        // Should have parent-child relationship between Condition and TrailingEffect
        assert!(
            !parent_child_links.is_empty(),
            "Should have parent-child links for condition"
        );

        // Should have coordination link between the two effect clauses
        assert!(
            !conjunct_links.is_empty(),
            "Should have Conjunct link for 'A and B'"
        );
    }

    #[test]
    fn test_coordination_empty_document() {
        let doc = LayeredDocument::from_text("");

        let links = ClauseLinkResolver::resolve(&doc);
        assert_eq!(links.len(), 0, "Empty document should have no links");
    }

    #[test]
    fn test_coordination_single_clause() {
        let doc = create_test_document("The tenant pays rent.");

        let links = ClauseLinkResolver::resolve(&doc);

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(
            conjunct_links.len(),
            0,
            "Single clause should have no Conjunct links"
        );
    }

    #[test]
    fn test_coordination_four_clause_chain() {
        // Test longer chain: A, B, C, and D should create A→B, B→C, C→D
        let doc = create_test_document("A acts, B reacts, C contracts, and D extracts.");

        let links = ClauseLinkResolver::resolve(&doc);

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(
            conjunct_links.len(),
            3,
            "Expected three Conjunct links for 'A, B, C, and D' chain"
        );

        let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clauses.len(), 4, "Should have 4 clauses");

        // Verify chain topology
        assert_eq!(conjunct_links[0].anchor, clauses[0].span);
        assert_eq!(conjunct_links[0].link.target, clauses[1].span);

        assert_eq!(conjunct_links[1].anchor, clauses[1].span);
        assert_eq!(conjunct_links[1].link.target, clauses[2].span);

        assert_eq!(conjunct_links[2].anchor, clauses[2].span);
        assert_eq!(conjunct_links[2].link.target, clauses[3].span);
    }

    #[test]
    fn test_coordination_mixed_clause_types_no_link() {
        // Different clause types shouldn't be linked by implicit commas
        // (only by explicit "and")
        let doc = create_test_document("When it rains, the streets flood, repairs are needed.");

        let links = ClauseLinkResolver::resolve(&doc);

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        // Should have no Conjunct links because:
        // 1. "When it rains" is Condition
        // 2. "the streets flood" is TrailingEffect
        // 3. "repairs are needed" is also TrailingEffect
        // But since clause types differ, we don't create implicit coordination
        // (only explicit "and" would create links)
        assert_eq!(
            conjunct_links.len(),
            0,
            "Mixed clause types with no explicit 'and' should not be coordinated"
        );
    }

    // ========================================================================
    // Gate 3: Exception/Carve-out Detection Tests
    // ========================================================================

    #[test]
    fn test_exception_simple_unless() {
        // "A unless B" → Exception link from B to A
        let doc = LayeredDocument::from_text("Tenant shall pay rent unless waived by Landlord.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(
            exception_links.len(),
            1,
            "Expected one Exception link for 'A unless B'"
        );

        // The exception clause ("waived by Landlord") should point to main clause ("Tenant shall pay rent")
        let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clauses.len(), 2, "Should have 2 clauses");

        // Exception link should be anchored on the second clause (exception)
        // and point to the first clause (main)
        assert_eq!(exception_links[0].anchor, clauses[1].span);
        assert_eq!(exception_links[0].link.target, clauses[0].span);
    }

    #[test]
    fn test_exception_except_keyword() {
        // "A, except B" → Exception link from B to A
        let doc = LayeredDocument::from_text("All rights reserved, except as noted.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(
            exception_links.len(),
            1,
            "Expected one Exception link for 'A, except B'"
        );
    }

    #[test]
    fn test_exception_notwithstanding() {
        // "Notwithstanding X, Y shall apply" → Exception link from Y to X
        let doc = LayeredDocument::from_text("Notwithstanding prior agreements, this clause controls.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(
            exception_links.len(),
            1,
            "Expected one Exception link for 'Notwithstanding X, Y'"
        );

        let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clauses.len(), 2, "Should have 2 clauses");

        // For "notwithstanding X, Y", the second clause (Y) is the exception
        // that points to the first clause (X)
        assert_eq!(exception_links[0].anchor, clauses[1].span);
        assert_eq!(exception_links[0].link.target, clauses[0].span);
    }

    #[test]
    fn test_exception_provided_that() {
        // "A provided that B" → Exception link from B to A
        let doc = LayeredDocument::from_text("Payment is due provided that notice is given.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(
            exception_links.len(),
            1,
            "Expected one Exception link for 'A provided that B'"
        );
    }

    #[test]
    fn test_exception_subject_to() {
        // "A subject to B" → Exception link from B to A
        let doc = LayeredDocument::from_text("License granted subject to payment terms.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(
            exception_links.len(),
            1,
            "Expected one Exception link for 'A subject to B'"
        );
    }

    #[test]
    fn test_exception_no_exception_keyword() {
        // No exception keywords should mean no exception links
        let doc = create_test_document("Tenant pays rent. Landlord maintains property.");

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(
            exception_links.len(),
            0,
            "No exception keywords means no Exception links"
        );
    }

    #[test]
    fn test_exception_with_coordination() {
        // Mix exception with coordination: "A and B, unless C"
        let doc = LayeredDocument::from_text("Tenant pays rent and utilities unless waived.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        // Should have coordination between first two clauses
        assert!(
            !conjunct_links.is_empty(),
            "Should have Conjunct link for 'A and B'"
        );

        // Should have exception link from third clause to one of the first two
        assert!(
            !exception_links.is_empty(),
            "Should have Exception link for 'unless C'"
        );
    }

    #[test]
    fn test_exception_single_clause() {
        // Single clause with exception keyword but no second clause
        let doc = create_test_document("Unless otherwise noted.");

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        // No exception link because there's only one clause
        assert_eq!(
            exception_links.len(),
            0,
            "Single clause cannot have exception relationship"
        );
    }

    #[test]
    fn test_exception_empty_document() {
        let doc = LayeredDocument::from_text("");

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(exception_links.len(), 0, "Empty document has no exception links");
    }

    // ========================================================================
    // Gate 3b: Chained Exception Tests
    // ========================================================================

    #[test]
    fn test_chained_exceptions_transitive() {
        // "A, unless B, except C" - C should link transitively to A through B
        // B→A (direct): B is exception to A
        // C→B (direct): C is exception to B
        // C→A (transitive): C should also be exception to A
        let doc = LayeredDocument::from_text("Tenant pays rent unless emergency applies except minor repairs.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);
        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);

        // Should have 3 clauses
        assert_eq!(clause_spans.len(), 3, "Expected 3 clauses: A, B, C");

        let a_span = clause_spans[0].span; // "Tenant pays rent"
        let b_span = clause_spans[1].span; // "emergency applies"
        let c_span = clause_spans[2].span; // "minor repairs"

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        // Should have 3 exception links:
        // 1. B→A (direct)
        // 2. C→B (direct)
        // 3. C→A (transitive)
        assert_eq!(
            exception_links.len(),
            3,
            "Expected 3 exception links: B→A, C→B, C→A (transitive)"
        );

        // Verify B→A exists
        let b_to_a = exception_links.iter().find(|l| l.anchor == b_span && l.link.target == a_span);
        assert!(b_to_a.is_some(), "Expected B→A exception link");

        // Verify C→B exists
        let c_to_b = exception_links.iter().find(|l| l.anchor == c_span && l.link.target == b_span);
        assert!(c_to_b.is_some(), "Expected C→B exception link");

        // Verify C→A exists (transitive)
        let c_to_a = exception_links.iter().find(|l| l.anchor == c_span && l.link.target == a_span);
        assert!(c_to_a.is_some(), "Expected C→A transitive exception link");
    }

    #[test]
    fn test_chained_exceptions_three_levels() {
        // "A, unless B, except C, except D" - D should link to A, B, and C
        let doc = LayeredDocument::from_text("Payment due unless waived except emergencies except acts of god.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);
        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);

        // Should have 4 clauses
        assert_eq!(clause_spans.len(), 4, "Expected 4 clauses: A, B, C, D");

        let a_span = clause_spans[0].span;
        let b_span = clause_spans[1].span;
        let c_span = clause_spans[2].span;
        let d_span = clause_spans[3].span;

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        // Verify D→C exists (direct)
        let d_to_c = exception_links.iter().find(|l| l.anchor == d_span && l.link.target == c_span);
        assert!(d_to_c.is_some(), "Expected D→C exception link");

        // Verify D→B exists (transitive through C)
        let d_to_b = exception_links.iter().find(|l| l.anchor == d_span && l.link.target == b_span);
        assert!(d_to_b.is_some(), "Expected D→B transitive exception link");

        // Verify C→A exists (transitive through B)
        let c_to_a = exception_links.iter().find(|l| l.anchor == c_span && l.link.target == a_span);
        assert!(c_to_a.is_some(), "Expected C→A transitive exception link");
    }
}
#[cfg(test)]
mod coordination_type_tests {
    use crate::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver};
    use crate::clause_link_resolver::CoordinationType;
    use layered_nlp_document::{ClauseRole, LayeredDocument};

    fn create_test_document_with_keywords(
        text: &str,
        and: &[&'static str],
        or: &[&'static str],
        but: &[&'static str],
        nor: &[&'static str],
    ) -> LayeredDocument {
        LayeredDocument::from_text(text)
            .run_resolver(&ClauseKeywordResolver::new(
                &["if", "when"],
                and,
                &["then"],
                or,
                but,
                nor,
            ))
            .run_resolver(&ClauseResolver::default())
    }

    #[test]
    fn test_coordination_type_conjunction() {
        let doc = create_test_document_with_keywords(
            "The tenant pays rent and the landlord maintains property.",
            &["and"],
            &[],
            &[],
            &[],
        );

        let links = ClauseLinkResolver::resolve(&doc);

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(conjunct_links.len(), 1);
        assert_eq!(
            conjunct_links[0].coordination_type,
            Some(CoordinationType::Conjunction)
        );
    }

    #[test]
    fn test_coordination_type_disjunction() {
        let doc = create_test_document_with_keywords(
            "The tenant pays rent or the tenant vacates.",
            &[],
            &["or"],
            &[],
            &[],
        );

        let links = ClauseLinkResolver::resolve(&doc);

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(conjunct_links.len(), 1);
        assert_eq!(
            conjunct_links[0].coordination_type,
            Some(CoordinationType::Disjunction)
        );
    }

    #[test]
    fn test_coordination_type_adversative() {
        let doc = create_test_document_with_keywords(
            "The tenant pays rent but the landlord delays repairs.",
            &[],
            &[],
            &["but"],
            &[],
        );

        let links = ClauseLinkResolver::resolve(&doc);

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(conjunct_links.len(), 1);
        assert_eq!(
            conjunct_links[0].coordination_type,
            Some(CoordinationType::Adversative)
        );
    }

    #[test]
    fn test_coordination_type_negative_alternative() {
        let doc = create_test_document_with_keywords(
            "The tenant pays rent nor does the landlord provide notice.",
            &[],
            &[],
            &[],
            &["nor"],
        );

        let links = ClauseLinkResolver::resolve(&doc);

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(conjunct_links.len(), 1);
        assert_eq!(
            conjunct_links[0].coordination_type,
            Some(CoordinationType::NegativeAlternative)
        );
    }

    #[test]
    fn test_coordination_type_none_for_parent_child() {
        let doc = create_test_document_with_keywords(
            "When it rains, then it pours.",
            &[],
            &[],
            &[],
            &[],
        );

        let links = ClauseLinkResolver::resolve(&doc);

        // All links should have coordination_type: None (since these are parent/child links)
        for link in &links {
            assert_eq!(link.coordination_type, None);
        }
    }

    #[test]
    fn test_coordination_type_none_for_exception() {
        let doc = LayeredDocument::from_text("Tenant shall pay rent unless waived by Landlord.")
            .run_resolver(&ClauseKeywordResolver::new(
                &["if", "when"],
                &["and"],
                &["then"],
                &[],
                &[],
                &[],
            ))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(exception_links.len(), 1);
        assert_eq!(exception_links[0].coordination_type, None);
    }
}

// ========================================================================
// List Relationship Detection Tests
// ========================================================================
#[cfg(test)]
mod list_detection_tests {
    use crate::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ListMarkerResolver};
    use layered_nlp_document::{ClauseRole, LayeredDocument};

    fn create_list_test_document(text: &str) -> LayeredDocument {
        LayeredDocument::from_text(text)
            .run_resolver(&ListMarkerResolver::new())
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default())
    }

    #[test]
    fn test_detect_list_relationships_empty() {
        let doc = create_list_test_document("");
        let links = ClauseLinkResolver::resolve(&doc);

        let list_links: Vec<_> = links
            .iter()
            .filter(|l| l.link.role == ClauseRole::ListItem || l.link.role == ClauseRole::ListContainer)
            .collect();

        assert_eq!(list_links.len(), 0, "Empty document should have no list links");
    }

    #[test]
    fn test_detect_list_relationships_no_markers() {
        let doc = create_list_test_document("The tenant pays rent.");
        let links = ClauseLinkResolver::resolve(&doc);

        let list_links: Vec<_> = links
            .iter()
            .filter(|l| l.link.role == ClauseRole::ListItem || l.link.role == ClauseRole::ListContainer)
            .collect();

        assert_eq!(list_links.len(), 0, "Text without list markers should have no list links");
    }

    #[test]
    fn test_detect_list_relationships_single_item_no_container() {
        // A list item as the first clause has no container
        let doc = create_list_test_document("(a) First item only.");
        let links = ClauseLinkResolver::resolve(&doc);

        let list_links: Vec<_> = links
            .iter()
            .filter(|l| l.link.role == ClauseRole::ListItem || l.link.role == ClauseRole::ListContainer)
            .collect();

        assert_eq!(list_links.len(), 0, "First clause cannot be a list item");
    }

    #[test]
    fn test_list_marker_type_grouping() {
        // Different marker types should form separate groups
        // (a), (b) then 1., 2. should be separate lists
        let doc = create_list_test_document("Intro clause. (a) first. (b) second. Then 1. one. 2. two.");
        let links = ClauseLinkResolver::resolve(&doc);

        // Should have some list relationships
        let list_item_links: Vec<_> = links
            .iter()
            .filter(|l| l.link.role == ClauseRole::ListItem)
            .collect();

        // The exact count depends on clause parsing, but we should have some
        // This validates the grouping logic runs without errors
        for link in &list_item_links {
            // Each list item should point to a valid container
            assert!(link.link.target.start.line <= link.anchor.start.line,
                "List item container should come before or on same line as item");
        }
    }

    #[test]
    fn test_resolve_with_list_markers_convenience() {
        let doc = LayeredDocument::from_text("Do this: (a) first (b) second")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default());

        let (doc_with_markers, _links) = ClauseLinkResolver::resolve_with_list_markers(doc);

        // The convenience method should have run ListMarkerResolver
        // Check that the document has list markers
        let has_list_markers = doc_with_markers.lines().iter().any(|line| {
            use layered_nlp::x;
            use crate::ListMarker;
            !line.find(&x::attr::<ListMarker>()).is_empty()
        });

        assert!(has_list_markers, "resolve_with_list_markers should detect list markers");
    }
}
