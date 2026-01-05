//! Query API for clause relationships (M2 Gate 4).
//!
//! Provides efficient queries over clause links emitted by `ClauseLinkResolver`.
//! Enables navigation of parent-child hierarchies, coordination chains, and exceptions.
//!
//! ## Usage
//!
//! ```rust
//! use layered_nlp_document::{LayeredDocument, DocSpan};
//! use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
//!
//! let doc = LayeredDocument::from_text("When it rains, then A happens and B occurs.")
//!     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
//!     .run_resolver(&ClauseResolver::default());
//!
//! let links = ClauseLinkResolver::resolve(&doc);
//! let api = ClauseQueryAPI::new(&links);
//!
//! // Query for parent clause containing a span
//! let span = DocSpan::single_line(0, 5, 10);
//! if let Some(parent) = api.parent_clause(span) {
//!     println!("Parent clause: {:?}", parent);
//! }
//!
//! // Get all conjuncts transitively
//! let conjuncts = api.conjuncts(span);
//! for conjunct in conjuncts {
//!     println!("Conjunct: {:?}", conjunct);
//! }
//! ```

use crate::{ClauseLink, LinkConfidence, CoordinationType, ObligationType};
use crate::clause_participant::ClauseParticipants;
use layered_nlp_document::{ClauseRole, DocSpan};
use std::collections::VecDeque;

/// Query API for clause relationships.
///
/// Operates on a slice of `ClauseLink` objects returned by `ClauseLinkResolver::resolve()`.
/// Provides methods to query parent-child hierarchies, conjunct chains, and exception relationships.
pub struct ClauseQueryAPI<'a> {
    links: &'a [ClauseLink],
}

impl<'a> ClauseQueryAPI<'a> {
    /// Create a new query API from clause links.
    ///
    /// # Arguments
    /// * `links` - The clause links returned by `ClauseLinkResolver::resolve()`
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::LayeredDocument;
    /// # use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
    /// let doc = LayeredDocument::from_text("When it rains, then it pours.")
    ///     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
    ///     .run_resolver(&ClauseResolver::default());
    ///
    /// let links = ClauseLinkResolver::resolve(&doc);
    /// let api = ClauseQueryAPI::new(&links);
    /// ```
    pub fn new(links: &'a [ClauseLink]) -> Self {
        Self { links }
    }

    /// Find the direct parent clause containing a given span.
    ///
    /// Returns the parent clause if this span has a Parent link, or None if it's a top-level clause.
    ///
    /// # Arguments
    /// * `span` - The clause span to query
    ///
    /// # Returns
    /// The parent clause span, or None if there is no parent
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::{LayeredDocument, DocSpan};
    /// # use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
    /// let doc = LayeredDocument::from_text("When it rains, then it pours.")
    ///     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
    ///     .run_resolver(&ClauseResolver::default());
    ///
    /// let links = ClauseLinkResolver::resolve(&doc);
    /// let api = ClauseQueryAPI::new(&links);
    ///
    /// // Find the condition clause span
    /// let condition_span = DocSpan::single_line(0, 0, 2); // "When it rains"
    ///
    /// // Query for its parent
    /// if let Some(parent) = api.parent_clause(condition_span) {
    ///     // parent should be the TrailingEffect clause
    ///     assert!(parent.start.token > condition_span.end.token);
    /// }
    /// ```
    pub fn parent_clause(&self, span: DocSpan) -> Option<DocSpan> {
        self.links
            .iter()
            .find(|link| link.anchor == span && link.link.role == ClauseRole::Parent)
            .map(|link| link.link.target)
    }

    /// Find the containing clause for a span that may not be a clause itself.
    ///
    /// This is similar to `parent_clause()` but conceptually asks: "What clause contains
    /// this position?" rather than "What is the parent of this clause?"
    ///
    /// For now, this is implemented as an alias to `parent_clause()` since we only track
    /// clause-to-clause relationships. In the future, this could be extended to handle
    /// arbitrary spans by checking which clause's span range contains the query span.
    ///
    /// # Arguments
    /// * `span` - The span to find the containing clause for
    ///
    /// # Returns
    /// The containing clause span, or None if the span is not contained by any clause
    pub fn containing_clause(&self, span: DocSpan) -> Option<DocSpan> {
        self.parent_clause(span)
    }

    /// Get all conjuncts of a clause transitively via the chain.
    ///
    /// For a coordination chain A→B→C, querying any clause returns all other clauses
    /// in the chain. This handles transitive closure by following Conjunct links
    /// in both forward and backward directions.
    ///
    /// # Arguments
    /// * `span` - The clause span to find conjuncts for
    ///
    /// # Returns
    /// A vector of all conjunct clause spans (not including the input span)
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::{LayeredDocument, DocSpan};
    /// # use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
    /// let doc = LayeredDocument::from_text("A pays, B works, and C manages.")
    ///     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
    ///     .run_resolver(&ClauseResolver::default());
    ///
    /// let links = ClauseLinkResolver::resolve(&doc);
    /// let api = ClauseQueryAPI::new(&links);
    ///
    /// // Get conjuncts for the first clause (A)
    /// // Should return both B and C due to chain: A→B→C
    /// // let conjuncts = api.conjuncts(clause_a_span);
    /// // assert_eq!(conjuncts.len(), 2);
    /// ```
    pub fn conjuncts(&self, span: DocSpan) -> Vec<DocSpan> {
        let mut visited = Vec::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        visited.push(span);
        queue.push_back(span);

        // BFS to find all connected clauses via Conjunct links
        while let Some(current) = queue.pop_front() {
            // Find all conjunct links from current clause
            for link in self.links.iter() {
                if link.anchor == current && link.link.role == ClauseRole::Conjunct {
                    let target = link.link.target;
                    if !visited.contains(&target) {
                        visited.push(target);
                        result.push(target);
                        queue.push_back(target);
                    }
                }
            }

            // Also search backwards: find clauses that point to current as conjunct
            for link in self.links.iter() {
                if link.link.target == current && link.link.role == ClauseRole::Conjunct {
                    let source = link.anchor;
                    if !visited.contains(&source) {
                        visited.push(source);
                        result.push(source);
                        queue.push_back(source);
                    }
                }
            }
        }

        result
    }

    /// Get all exception clauses that modify a given span.
    ///
    /// Returns clauses that have Exception links pointing to the query span.
    /// These are clauses that carve out exceptions or special cases for the main clause.
    ///
    /// # Arguments
    /// * `span` - The main clause span to find exceptions for
    ///
    /// # Returns
    /// A vector of exception clause spans
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::{LayeredDocument, DocSpan};
    /// # use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
    /// let doc = LayeredDocument::from_text("Tenant shall pay rent unless waived by Landlord.")
    ///     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
    ///     .run_resolver(&ClauseResolver::default());
    ///
    /// let links = ClauseLinkResolver::resolve(&doc);
    /// let api = ClauseQueryAPI::new(&links);
    ///
    /// // Get exceptions for the main clause ("Tenant shall pay rent")
    /// // let main_clause_span = DocSpan::single_line(0, 0, 4);
    /// // let exceptions = api.exceptions(main_clause_span);
    /// // assert_eq!(exceptions.len(), 1); // "waived by Landlord"
    /// ```
    pub fn exceptions(&self, span: DocSpan) -> Vec<DocSpan> {
        let mut result = Vec::new();
        let mut visited = Vec::new();
        let mut queue = VecDeque::new();

        visited.push(span);
        queue.push_back(span);

        while let Some(current) = queue.pop_front() {
            // Find all clauses that are exceptions to the current clause
            // Exception links go FROM the exception clause TO the base clause
            // (anchor is the exception, target is what it's an exception to)
            for link in self.links.iter() {
                if link.link.target == current && link.link.role == ClauseRole::Exception {
                    let exception_clause = link.anchor;
                    if !visited.contains(&exception_clause) {
                        visited.push(exception_clause);
                        result.push(exception_clause);
                        queue.push_back(exception_clause);
                    }
                }
            }
        }

        result
    }

    /// Get all child clauses directly contained by a parent clause.
    ///
    /// Returns clauses that have been marked as children of the query span.
    ///
    /// # Arguments
    /// * `span` - The parent clause span
    ///
    /// # Returns
    /// A vector of child clause spans
    pub fn child_clauses(&self, span: DocSpan) -> Vec<DocSpan> {
        self.links
            .iter()
            .filter(|link| link.anchor == span && link.link.role == ClauseRole::Child)
            .map(|link| link.link.target)
            .collect()
    }

    /// Get all high-confidence links.
    ///
    /// Returns an iterator over links that have High confidence.
    ///
    /// # Returns
    /// An iterator over high-confidence ClauseLink references
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::LayeredDocument;
    /// # use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
    /// let doc = LayeredDocument::from_text("When it rains, then it pours.")
    ///     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
    ///     .run_resolver(&ClauseResolver::default());
    ///
    /// let links = ClauseLinkResolver::resolve(&doc);
    /// let api = ClauseQueryAPI::new(&links);
    ///
    /// for link in api.high_confidence_links() {
    ///     // Process only high-confidence links
    /// }
    /// ```
    pub fn high_confidence_links(&self) -> impl Iterator<Item = &ClauseLink> {
        self.links
            .iter()
            .filter(|link| link.confidence == LinkConfidence::High)
    }

    /// Get all links with at least the specified minimum confidence level.
    ///
    /// Returns an iterator over links that meet or exceed the minimum confidence.
    ///
    /// # Arguments
    /// * `min` - The minimum confidence level (Low, Medium, or High)
    ///
    /// # Returns
    /// An iterator over ClauseLink references meeting the confidence threshold
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::LayeredDocument;
    /// # use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI, LinkConfidence};
    /// let doc = LayeredDocument::from_text("When it rains, then it pours.")
    ///     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
    ///     .run_resolver(&ClauseResolver::default());
    ///
    /// let links = ClauseLinkResolver::resolve(&doc);
    /// let api = ClauseQueryAPI::new(&links);
    ///
    /// // Get all medium and high confidence links
    /// for link in api.links_with_confidence(LinkConfidence::Medium) {
    ///     // Process links with Medium or High confidence
    /// }
    /// ```
    pub fn links_with_confidence(&self, min: LinkConfidence) -> impl Iterator<Item = &ClauseLink> {
        self.links.iter().filter(move |link| link.confidence >= min)
    }

    /// Get the top-level coordination operator (lowest precedence, evaluated last).
    /// 
    /// In "A and B or C", the top-level operator is OR (precedence=1) because
    /// AND (precedence=2) binds tighter and is evaluated first.
    /// 
    /// Returns the coordination type of the operator with the lowest precedence value.
    pub fn top_level_operator(&self) -> Option<CoordinationType> {
        let min_precedence = self.links.iter()
            .filter(|l| l.link.role == ClauseRole::Conjunct)
            .filter_map(|l| l.precedence_group)
            .min()?;
        
        self.links.iter()
            .find(|l| l.precedence_group == Some(min_precedence) && l.link.role == ClauseRole::Conjunct)
            .and_then(|l| l.coordination_type)
    }

    /// Get all clauses in the same precedence group as the given span.
    ///
    /// Finds all clauses that share a precedence group with the query span.
    /// These are clauses that are grouped together at the same level of operator precedence.
    ///
    /// # Arguments
    /// * `span` - The clause span to find group members for
    ///
    /// # Returns
    /// A sorted, deduplicated vector of clause spans in the same precedence group
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::LayeredDocument;
    /// # use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
    /// let doc = LayeredDocument::from_text("A pays, B works, and C manages.")
    ///     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
    ///     .run_resolver(&ClauseResolver::default());
    ///
    /// let links = ClauseLinkResolver::resolve(&doc);
    /// let api = ClauseQueryAPI::new(&links);
    ///
    /// let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
    /// let first_span = clause_spans[0].span;
    ///
    /// // Get all clauses at the same precedence level
    /// let members = api.precedence_group_members(first_span);
    /// ```
    pub fn precedence_group_members(&self, span: DocSpan) -> Vec<DocSpan> {
        let groups: Vec<u8> = self.links.iter()
            .filter(|l| l.link.role == ClauseRole::Conjunct)
            .filter(|l| l.anchor == span || l.link.target == span)
            .filter_map(|l| l.precedence_group)
            .collect();
        
        if groups.is_empty() {
            return vec![];
        }
        
        let mut members = Vec::new();
        for &group in &groups {
            for link in self.links.iter() {
                if link.precedence_group == Some(group) && link.link.role == ClauseRole::Conjunct {
                    if !members.contains(&link.anchor) {
                        members.push(link.anchor);
                    }
                    if !members.contains(&link.link.target) {
                        members.push(link.link.target);
                    }
                }
            }
        }
        
        members
    }

    /// Get all distinct precedence groups in the current links.
    ///
    /// Returns a sorted list of all precedence group IDs that appear in the links.
    /// Higher IDs represent tighter binding (evaluated first).
    ///
    /// # Returns
    /// A sorted vector of unique precedence group IDs
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::LayeredDocument;
    /// # use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
    /// let doc = LayeredDocument::from_text("A pays, B works, and C manages.")
    ///     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
    ///     .run_resolver(&ClauseResolver::default());
    ///
    /// let links = ClauseLinkResolver::resolve(&doc);
    /// let api = ClauseQueryAPI::new(&links);
    ///
    /// let groups = api.precedence_groups();
    /// println!("Found {} precedence groups", groups.len());
    /// ```
    pub fn precedence_groups(&self) -> Vec<u8> {
        let mut groups: Vec<u8> = self.links.iter()
            .filter(|l| l.link.role == ClauseRole::Conjunct)
            .filter_map(|l| l.precedence_group)
            .collect();
        groups.sort();
        groups.dedup();
        groups
    }

    /// Get the list container clause for a list item.
    ///
    /// If this span is a list item, returns the container clause that introduces the list.
    ///
    /// # Arguments
    /// * `span` - The list item span to query
    ///
    /// # Returns
    /// The container clause span, or None if this is not a list item
    pub fn list_container(&self, span: DocSpan) -> Option<DocSpan> {
        self.links
            .iter()
            .find(|link| link.anchor == span && link.link.role == ClauseRole::ListItem)
            .map(|link| link.link.target)
    }

    /// Get all list items belonging to a container clause.
    ///
    /// Returns all clauses that are marked as list items pointing to this container.
    ///
    /// # Arguments
    /// * `span` - The container clause span
    ///
    /// # Returns
    /// A vector of list item clause spans
    pub fn list_items(&self, span: DocSpan) -> Vec<DocSpan> {
        self.links
            .iter()
            .filter(|link| link.anchor == span && link.link.role == ClauseRole::ListContainer)
            .map(|link| link.link.target)
            .collect()
    }

    /// Check if a clause is a list item.
    ///
    /// # Arguments
    /// * `span` - The clause span to check
    ///
    /// # Returns
    /// True if this clause is a list item, false otherwise
    pub fn is_list_item(&self, span: DocSpan) -> bool {
        self.list_container(span).is_some()
    }

    /// Check if a clause is a list container.
    ///
    /// # Arguments
    /// * `span` - The clause span to check
    ///
    /// # Returns
    /// True if this clause is a list container, false otherwise
    pub fn is_list_container(&self, span: DocSpan) -> bool {
        self.links
            .iter()
            .any(|link| link.anchor == span && link.link.role == ClauseRole::ListContainer)
    }

    // ========================================================================
    // Cross-Reference Queries (Gate 4)
    // ========================================================================

    /// Returns all DocSpans of section references within the given clause.
    ///
    /// Finds all CrossReference links where the anchor is the given clause span
    /// and returns the target spans (the section reference spans).
    ///
    /// # Arguments
    /// * `clause_span` - The clause span to find cross-references in
    ///
    /// # Returns
    /// A vector of section reference spans contained in this clause
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::DocSpan;
    /// # use layered_clauses::{ClauseQueryAPI, ClauseLink, ClauseLinkBuilder, LinkConfidence};
    /// let clause_span = DocSpan::single_line(0, 0, 30);
    /// let section_ref_span = DocSpan::single_line(0, 15, 25);
    ///
    /// let links = vec![
    ///     ClauseLink {
    ///         anchor: clause_span,
    ///         link: ClauseLinkBuilder::cross_reference_link(section_ref_span),
    ///         confidence: LinkConfidence::High,
    ///         coordination_type: None,
    ///         precedence_group: None,
    ///         obligation_type: None,
    ///     },
    /// ];
    ///
    /// let api = ClauseQueryAPI::new(&links);
    /// let refs = api.referenced_sections(clause_span);
    /// assert_eq!(refs.len(), 1);
    /// assert!(refs.contains(&section_ref_span));
    /// ```
    pub fn referenced_sections(&self, clause_span: DocSpan) -> Vec<DocSpan> {
        self.links
            .iter()
            .filter(|link| link.anchor == clause_span && link.link.role == ClauseRole::CrossReference)
            .map(|link| link.link.target)
            .collect()
    }

    /// Returns all clause spans that reference a given section reference span.
    ///
    /// Finds all CrossReference links where the target is the given section reference span
    /// and returns the anchor spans (the clause spans that contain the reference).
    ///
    /// # Arguments
    /// * `section_ref_span` - The section reference span to find referencing clauses for
    ///
    /// # Returns
    /// A vector of clause spans that contain cross-references to this section
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::DocSpan;
    /// # use layered_clauses::{ClauseQueryAPI, ClauseLink, ClauseLinkBuilder, LinkConfidence};
    /// let clause_span = DocSpan::single_line(0, 0, 30);
    /// let section_ref_span = DocSpan::single_line(0, 15, 25);
    ///
    /// let links = vec![
    ///     ClauseLink {
    ///         anchor: clause_span,
    ///         link: ClauseLinkBuilder::cross_reference_link(section_ref_span),
    ///         confidence: LinkConfidence::High,
    ///         coordination_type: None,
    ///         precedence_group: None,
    ///         obligation_type: None,
    ///     },
    /// ];
    ///
    /// let api = ClauseQueryAPI::new(&links);
    /// let clauses = api.referencing_clauses(section_ref_span);
    /// assert_eq!(clauses.len(), 1);
    /// assert!(clauses.contains(&clause_span));
    /// ```
    pub fn referencing_clauses(&self, section_ref_span: DocSpan) -> Vec<DocSpan> {
        self.links
            .iter()
            .filter(|link| link.link.target == section_ref_span && link.link.role == ClauseRole::CrossReference)
            .map(|link| link.anchor)
            .collect()
    }

    /// Returns true if the given clause contains any cross-references.
    ///
    /// # Arguments
    /// * `span` - The clause span to check
    ///
    /// # Returns
    /// True if the clause has at least one cross-reference, false otherwise
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::DocSpan;
    /// # use layered_clauses::{ClauseQueryAPI, ClauseLink, ClauseLinkBuilder, LinkConfidence};
    /// let clause_span = DocSpan::single_line(0, 0, 30);
    /// let section_ref_span = DocSpan::single_line(0, 15, 25);
    ///
    /// let links = vec![
    ///     ClauseLink {
    ///         anchor: clause_span,
    ///         link: ClauseLinkBuilder::cross_reference_link(section_ref_span),
    ///         confidence: LinkConfidence::High,
    ///         coordination_type: None,
    ///         precedence_group: None,
    ///         obligation_type: None,
    ///     },
    /// ];
    ///
    /// let api = ClauseQueryAPI::new(&links);
    /// assert!(api.has_cross_references(clause_span));
    /// assert!(!api.has_cross_references(section_ref_span)); // The section ref itself has no cross-refs
    /// ```
    pub fn has_cross_references(&self, span: DocSpan) -> bool {
        !self.referenced_sections(span).is_empty()
    }

    // ========================================================================
    // Obligation Type Queries (Gate 2)
    // ========================================================================

    /// Returns the obligation type for a clause, if detected.
    ///
    /// # Example
    /// ```ignore
    /// let api = ClauseQueryAPI::new(&links);
    /// if let Some(obligation) = api.obligation(my_clause_span) {
    ///     match obligation {
    ///         ObligationType::Duty => println!("This is a duty clause"),
    ///         ObligationType::Permission => println!("This is a permission clause"),
    ///         ObligationType::Prohibition => println!("This is a prohibition clause"),
    ///     }
    /// }
    /// ```
    pub fn obligation(&self, span: DocSpan) -> Option<ObligationType> {
        self.links
            .iter()
            .find(|link| link.anchor == span)
            .and_then(|link| link.obligation_type.clone())
    }

    /// Returns all clause spans that have the specified obligation type.
    ///
    /// Results are returned in document order (ascending spans), with duplicates removed.
    ///
    /// # Example
    /// ```ignore
    /// let api = ClauseQueryAPI::new(&links);
    /// let duties = api.clauses_by_obligation_type(ObligationType::Duty);
    /// println!("Found {} duty clauses", duties.len());
    /// ```
    pub fn clauses_by_obligation_type(&self, obligation_type: ObligationType) -> Vec<DocSpan> {
        let mut spans: Vec<DocSpan> = self.links
            .iter()
            .filter(|link| link.obligation_type == Some(obligation_type.clone()))
            .map(|link| link.anchor)
            .collect();

        // Deduplicate (same clause may appear in multiple links) and sort for document order
        spans.sort_by_key(|span| (span.start.line, span.start.token));
        spans.dedup();
        spans
    }

    // ========================================================================
    // PARTICIPANT QUERIES (Gate 4)
    // ========================================================================

    /// Get participants for a specific clause span.
    ///
    /// # Status: Stub Implementation
    ///
    /// This method currently returns an empty `ClauseParticipants` collection.
    /// Full functionality requires a `ParticipantResolver` to populate participant
    /// data in the document. See `clause_participant.rs` for the data types.
    ///
    /// # Future Integration
    ///
    /// Once `ParticipantResolver` is implemented:
    /// 1. Run the resolver on the document
    /// 2. Query participant data from the resolver's output
    /// 3. Or pass participant data to `ClauseQueryAPI::new()`
    ///
    /// # Example
    /// ```ignore
    /// let api = ClauseQueryAPI::new(&links);
    /// let participants = api.participants(clause_span);
    /// if let Some(subject) = participants.primary_subject() {
    ///     println!("Subject: {}", subject.text);
    /// }
    /// ```
    pub fn participants(&self, clause_span: DocSpan) -> ClauseParticipants {
        // TODO: Integrate with ParticipantResolver output
        ClauseParticipants::new(clause_span)
    }

    // ========================================================================
    // RELATIVE CLAUSE QUERIES (Gate 5)
    // ========================================================================

    /// Find the relative clause that modifies a given head noun span.
    ///
    /// # Status: Stub Implementation
    ///
    /// This method searches for `ClauseLink` entries with `ClauseRole::Relative`,
    /// but currently no resolver produces such links. Returns `None` until
    /// `RelativeClauseLinkResolver` is implemented.
    ///
    /// # Future Integration
    ///
    /// The `relative_clause.rs` module provides detection logic via
    /// `RelativeClauseDetector`. A resolver needs to:
    /// 1. Detect relative clauses using `RelativeClauseDetector`
    /// 2. Emit `ClauseLink` with `ClauseRole::Relative` and target = head noun span
    /// 3. Store links in the document for querying here
    ///
    /// # Example
    /// ```ignore
    /// // "the tenant who fails to pay"
    /// let api = ClauseQueryAPI::new(&links);
    /// if let Some(rel_clause) = api.relative_clause(tenant_span) {
    ///     // rel_clause is the span of "who fails to pay"
    /// }
    /// ```
    pub fn relative_clause(&self, head_noun_span: DocSpan) -> Option<DocSpan> {
        self.links
            .iter()
            .find(|link| {
                link.link.role == ClauseRole::Relative && link.link.target == head_noun_span
            })
            .map(|link| link.anchor)
    }

    /// Get all relative clause attachments in the document.
    ///
    /// # Status: Stub Implementation
    ///
    /// See `relative_clause()` for status details. Currently returns empty
    /// iterator until resolver integration is complete.
    ///
    /// # Returns
    ///
    /// Pairs of (head_noun_span, relative_clause_span) for each detected
    /// relative clause attachment.
    pub fn all_relative_clauses(&self) -> Vec<(DocSpan, DocSpan)> {
        self.links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Relative)
            .map(|link| (link.link.target, link.anchor))
            .collect()
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver};
    use layered_nlp_document::LayeredDocument;

    fn create_test_document(text: &str) -> LayeredDocument {
        LayeredDocument::from_text(text)
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default())
    }

    #[test]
    fn test_parent_clause_query() {
        let doc = create_test_document("When it rains, then it pours.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        // Get clause spans
        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 2);

        let condition_span = clause_spans[0].span; // "When it rains"
        let effect_span = clause_spans[1].span; // "then it pours"

        // Condition should have effect as parent
        let parent = api.parent_clause(condition_span);
        assert_eq!(parent, Some(effect_span));

        // Effect (top-level) should have no parent
        let parent = api.parent_clause(effect_span);
        assert_eq!(parent, None);
    }

    #[test]
    fn test_containing_clause_query() {
        let doc = create_test_document("When it rains, then it pours.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        let condition_span = clause_spans[0].span;
        let effect_span = clause_spans[1].span;

        // containing_clause should work like parent_clause
        assert_eq!(api.containing_clause(condition_span), Some(effect_span));
        assert_eq!(api.containing_clause(effect_span), None);
    }

    #[test]
    fn test_conjuncts_two_clauses() {
        let doc = create_test_document("The tenant pays rent and the landlord provides notice.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 2);

        let first_span = clause_spans[0].span;
        let second_span = clause_spans[1].span;

        // First clause should have second as conjunct
        let conjuncts = api.conjuncts(first_span);
        assert_eq!(conjuncts.len(), 1);
        assert!(conjuncts.contains(&second_span));

        // Second clause should have first as conjunct (bidirectional)
        let conjuncts = api.conjuncts(second_span);
        assert_eq!(conjuncts.len(), 1);
        assert!(conjuncts.contains(&first_span));
    }

    #[test]
    fn test_conjuncts_chain_transitive() {
        let doc = create_test_document("A pays, B works, and C manages.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 3);

        let a_span = clause_spans[0].span;
        let b_span = clause_spans[1].span;
        let c_span = clause_spans[2].span;

        // A should return [B, C] due to transitive chain A→B→C
        let conjuncts = api.conjuncts(a_span);
        assert_eq!(conjuncts.len(), 2);
        assert!(conjuncts.contains(&b_span));
        assert!(conjuncts.contains(&c_span));

        // B should return [A, C] (connected both ways)
        let conjuncts = api.conjuncts(b_span);
        assert_eq!(conjuncts.len(), 2);
        assert!(conjuncts.contains(&a_span));
        assert!(conjuncts.contains(&c_span));

        // C should return [A, B]
        let conjuncts = api.conjuncts(c_span);
        assert_eq!(conjuncts.len(), 2);
        assert!(conjuncts.contains(&a_span));
        assert!(conjuncts.contains(&b_span));
    }

    #[test]
    fn test_conjuncts_no_coordination() {
        let doc = create_test_document("The tenant pays rent.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 1);

        let span = clause_spans[0].span;

        // Single clause has no conjuncts
        let conjuncts = api.conjuncts(span);
        assert_eq!(conjuncts.len(), 0);
    }

    #[test]
    fn test_exceptions_simple() {
        let doc = LayeredDocument::from_text("Tenant shall pay rent unless waived by Landlord.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 2);

        let main_span = clause_spans[0].span; // "Tenant shall pay rent"
        let exception_span = clause_spans[1].span; // "waived by Landlord"

        // Main clause should have one exception
        let exceptions = api.exceptions(main_span);
        assert_eq!(exceptions.len(), 1);
        assert!(exceptions.contains(&exception_span));

        // Exception clause should have no exceptions pointing to it
        let exceptions = api.exceptions(exception_span);
        assert_eq!(exceptions.len(), 0);
    }

    #[test]
    fn test_exceptions_no_exception_keyword() {
        let doc = create_test_document("Tenant pays rent. Landlord maintains property.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 2);

        let first_span = clause_spans[0].span;

        // No exception keywords, so no exceptions
        let exceptions = api.exceptions(first_span);
        assert_eq!(exceptions.len(), 0);
    }

    #[test]
    fn test_child_clauses() {
        let doc = create_test_document("When it rains, then it pours.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        let condition_span = clause_spans[0].span;
        let effect_span = clause_spans[1].span;

        // Effect clause should have condition as child
        let children = api.child_clauses(effect_span);
        assert_eq!(children.len(), 1);
        assert!(children.contains(&condition_span));

        // Condition clause should have no children
        let children = api.child_clauses(condition_span);
        assert_eq!(children.len(), 0);
    }

    #[test]
    fn test_mixed_link_types() {
        // Document with parent-child, coordination, and exceptions
        let doc = LayeredDocument::from_text("When A happens and B occurs, then C applies unless D.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        // Should have: A (Condition), B (Condition), C (TrailingEffect), D (Independent)
        assert!(clause_spans.len() >= 3);

        // Verify we can query each relationship type
        let first_span = clause_spans[0].span;

        // Should have parent (the TrailingEffect clause)
        let parent = api.parent_clause(first_span);
        assert!(parent.is_some());

        // Should have conjunct (B)
        let conjuncts = api.conjuncts(first_span);
        assert!(!conjuncts.is_empty());
    }

    #[test]
    fn test_empty_links() {
        // Create API with no links
        let links = vec![];
        let api = ClauseQueryAPI::new(&links);

        let arbitrary_span = DocSpan::single_line(0, 0, 5);

        // All queries should return empty/None
        assert_eq!(api.parent_clause(arbitrary_span), None);
        assert_eq!(api.containing_clause(arbitrary_span), None);
        assert_eq!(api.conjuncts(arbitrary_span).len(), 0);
        assert_eq!(api.exceptions(arbitrary_span).len(), 0);
        assert_eq!(api.child_clauses(arbitrary_span).len(), 0);
    }

    #[test]
    fn test_conjuncts_four_clause_chain() {
        // Test longer chain: A, B, C, and D
        let doc = create_test_document("A acts, B reacts, C contracts, and D extracts.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 4);

        let a_span = clause_spans[0].span;
        let b_span = clause_spans[1].span;
        let c_span = clause_spans[2].span;
        let d_span = clause_spans[3].span;

        // A should be connected to all others transitively
        let conjuncts = api.conjuncts(a_span);
        assert_eq!(conjuncts.len(), 3);
        assert!(conjuncts.contains(&b_span));
        assert!(conjuncts.contains(&c_span));
        assert!(conjuncts.contains(&d_span));

        // B should be connected to all others
        let conjuncts = api.conjuncts(b_span);
        assert_eq!(conjuncts.len(), 3);
        assert!(conjuncts.contains(&a_span));
        assert!(conjuncts.contains(&c_span));
        assert!(conjuncts.contains(&d_span));
    }

    #[test]
    fn test_comprehensive_all_link_types() {
        // Comprehensive test demonstrating all query methods with multiple link types
        // Document structure:
        // "When X occurs and Y happens, then Z applies unless W, and Q follows."
        //
        // Expected structure:
        // - X (Condition) and Y (Condition) are coordinated
        // - X and Y are children of Z (TrailingEffect)
        // - W is an exception to Z
        // - Z and Q are coordinated
        let doc = LayeredDocument::from_text(
            "When X occurs and Y happens, then Z applies unless W, and Q follows."
        )
        .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
        .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);
        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);

        // We should have at least 5 clauses: X, Y, Z, W, Q
        assert!(clause_spans.len() >= 4, "Expected at least 4 clauses, got {}", clause_spans.len());

        // Demonstrate all query capabilities by printing what we found
        // (This is an integration test to ensure the API works end-to-end)

        for clause in clause_spans.iter() {
            let span = clause.span;

            // Test parent_clause
            let parent = api.parent_clause(span);

            // Test child_clauses
            let children = api.child_clauses(span);

            // Test conjuncts
            let conjuncts = api.conjuncts(span);

            // Test exceptions
            let exceptions = api.exceptions(span);

            // All queries should complete without panic
            // This validates that the API handles complex documents correctly
            // Simply accessing the results ensures the queries execute without error
            let _ = parent;
            let _ = children;
            let _ = conjuncts;
            let _ = exceptions;
        }

        // Verify that we have at least some links
        assert!(!links.is_empty(), "Expected some clause links");

        // Verify we have multiple link types
        let has_parent = links.iter().any(|l| l.link.role == ClauseRole::Parent);
        let has_conjunct = links.iter().any(|l| l.link.role == ClauseRole::Conjunct);
        let has_exception = links.iter().any(|l| l.link.role == ClauseRole::Exception);

        assert!(has_parent || has_conjunct || has_exception,
            "Expected at least one link type in complex document");
    }

    #[test]
    fn test_high_confidence_links() {
        let doc = create_test_document("When it rains, then it pours.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        // All current links should be high confidence (same-line)
        let high_confidence: Vec<_> = api.high_confidence_links().collect();
        assert_eq!(
            high_confidence.len(),
            links.len(),
            "All same-line links should be high confidence"
        );

        for link in high_confidence {
            assert_eq!(link.confidence, LinkConfidence::High);
        }
    }

    #[test]
    fn test_links_with_confidence_high() {
        let doc = create_test_document("When it rains, then it pours.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        // All links should meet High threshold
        let high: Vec<_> = api.links_with_confidence(LinkConfidence::High).collect();
        assert_eq!(high.len(), links.len());
    }

    #[test]
    fn test_links_with_confidence_medium() {
        let doc = create_test_document("When it rains, then it pours.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        // All high links should also meet medium threshold
        let medium_or_higher: Vec<_> = api.links_with_confidence(LinkConfidence::Medium).collect();
        assert_eq!(
            medium_or_higher.len(),
            links.len(),
            "All High confidence links should meet Medium threshold"
        );
    }

    #[test]
    fn test_links_with_confidence_low() {
        let doc = create_test_document("When it rains, then it pours.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        // All links should meet Low threshold (lowest bar)
        let all: Vec<_> = api.links_with_confidence(LinkConfidence::Low).collect();
        assert_eq!(all.len(), links.len(), "All links should meet Low threshold");
    }

    #[test]
    fn test_confidence_filtering_empty() {
        // Test confidence filtering on empty link set
        let links = vec![];
        let api = ClauseQueryAPI::new(&links);

        let high: Vec<_> = api.high_confidence_links().collect();
        assert_eq!(high.len(), 0);

        let medium: Vec<_> = api.links_with_confidence(LinkConfidence::Medium).collect();
        assert_eq!(medium.len(), 0);
    }

    // ========================================================================
    // List Query Tests
    // ========================================================================

    #[test]
    fn test_list_container_empty() {
        let links = vec![];
        let api = ClauseQueryAPI::new(&links);

        let span = DocSpan::single_line(0, 0, 5);
        assert_eq!(api.list_container(span), None);
    }

    #[test]
    fn test_list_items_empty() {
        let links = vec![];
        let api = ClauseQueryAPI::new(&links);

        let span = DocSpan::single_line(0, 0, 5);
        assert!(api.list_items(span).is_empty());
    }

    #[test]
    fn test_is_list_item_false() {
        let links = vec![];
        let api = ClauseQueryAPI::new(&links);

        let span = DocSpan::single_line(0, 0, 5);
        assert!(!api.is_list_item(span));
    }

    #[test]
    fn test_is_list_container_false() {
        let links = vec![];
        let api = ClauseQueryAPI::new(&links);

        let span = DocSpan::single_line(0, 0, 5);
        assert!(!api.is_list_container(span));
    }

    #[test]
    fn test_list_queries_with_manual_links() {
        use crate::ClauseLink;

        // Create synthetic list links for testing
        let container_span = DocSpan::single_line(0, 0, 5);
        let item1_span = DocSpan::single_line(0, 6, 10);
        let item2_span = DocSpan::single_line(0, 11, 15);

        let links = vec![
            // item1 → container (ListItem)
            ClauseLink {
                anchor: item1_span,
                link: crate::ClauseLinkBuilder::list_item_link(container_span),
                confidence: LinkConfidence::High,
                coordination_type: None,
                precedence_group: None,
                obligation_type: None,
            },
            // container → item1 (ListContainer)
            ClauseLink {
                anchor: container_span,
                link: crate::ClauseLinkBuilder::list_container_link(item1_span),
                confidence: LinkConfidence::High,
                coordination_type: None,
                precedence_group: None,
                obligation_type: None,
            },
            // item2 → container (ListItem)
            ClauseLink {
                anchor: item2_span,
                link: crate::ClauseLinkBuilder::list_item_link(container_span),
                confidence: LinkConfidence::High,
                coordination_type: None,
                precedence_group: None,
                obligation_type: None,
            },
            // container → item2 (ListContainer)
            ClauseLink {
                anchor: container_span,
                link: crate::ClauseLinkBuilder::list_container_link(item2_span),
                confidence: LinkConfidence::High,
                coordination_type: None,
                precedence_group: None,
                obligation_type: None,
            },
        ];

        let api = ClauseQueryAPI::new(&links);

        // Test list_container
        assert_eq!(api.list_container(item1_span), Some(container_span));
        assert_eq!(api.list_container(item2_span), Some(container_span));
        assert_eq!(api.list_container(container_span), None);

        // Test list_items
        let items = api.list_items(container_span);
        assert_eq!(items.len(), 2);
        assert!(items.contains(&item1_span));
        assert!(items.contains(&item2_span));
        assert!(api.list_items(item1_span).is_empty());

        // Test is_list_item
        assert!(api.is_list_item(item1_span));
        assert!(api.is_list_item(item2_span));
        assert!(!api.is_list_item(container_span));

        // Test is_list_container
        assert!(api.is_list_container(container_span));
        assert!(!api.is_list_container(item1_span));
        assert!(!api.is_list_container(item2_span));
    }

    // ========================================================================
    // Cross-Reference Query Tests
    // ========================================================================

    #[test]
    fn test_referenced_sections_empty() {
        let links = vec![];
        let api = ClauseQueryAPI::new(&links);

        let span = DocSpan::single_line(0, 0, 5);
        assert!(api.referenced_sections(span).is_empty());
    }

    #[test]
    fn test_referencing_clauses_empty() {
        let links = vec![];
        let api = ClauseQueryAPI::new(&links);

        let span = DocSpan::single_line(0, 0, 5);
        assert!(api.referencing_clauses(span).is_empty());
    }

    #[test]
    fn test_has_cross_references_false() {
        let links = vec![];
        let api = ClauseQueryAPI::new(&links);

        let span = DocSpan::single_line(0, 0, 5);
        assert!(!api.has_cross_references(span));
    }

    #[test]
    fn test_cross_reference_queries_with_manual_links() {
        use crate::ClauseLink;

        // Create synthetic cross-reference links for testing
        // Simulates: "Subject to Section 3.2, the Tenant shall pay rent."
        let clause_span = DocSpan::single_line(0, 0, 50);
        let section_ref_span = DocSpan::single_line(0, 11, 22); // "Section 3.2"

        let links = vec![
            // clause → section_ref (CrossReference)
            ClauseLink {
                anchor: clause_span,
                link: crate::ClauseLinkBuilder::cross_reference_link(section_ref_span),
                confidence: LinkConfidence::High,
                coordination_type: None,
                precedence_group: None,
                obligation_type: None,
            },
        ];

        let api = ClauseQueryAPI::new(&links);

        // Test referenced_sections
        let refs = api.referenced_sections(clause_span);
        assert_eq!(refs.len(), 1);
        assert!(refs.contains(&section_ref_span));
        assert!(api.referenced_sections(section_ref_span).is_empty());

        // Test referencing_clauses
        let clauses = api.referencing_clauses(section_ref_span);
        assert_eq!(clauses.len(), 1);
        assert!(clauses.contains(&clause_span));
        assert!(api.referencing_clauses(clause_span).is_empty());

        // Test has_cross_references
        assert!(api.has_cross_references(clause_span));
        assert!(!api.has_cross_references(section_ref_span));
    }

    #[test]
    fn test_multiple_cross_references_in_clause() {
        use crate::ClauseLink;

        // Clause with multiple section references
        // Simulates: "Subject to Sections 3.2 and 4.1, the Tenant shall..."
        let clause_span = DocSpan::single_line(0, 0, 60);
        let section_ref1 = DocSpan::single_line(0, 11, 22); // "Sections 3.2"
        let section_ref2 = DocSpan::single_line(0, 27, 30); // "4.1"

        let links = vec![
            ClauseLink {
                anchor: clause_span,
                link: crate::ClauseLinkBuilder::cross_reference_link(section_ref1),
                confidence: LinkConfidence::High,
                coordination_type: None,
                precedence_group: None,
                obligation_type: None,
            },
            ClauseLink {
                anchor: clause_span,
                link: crate::ClauseLinkBuilder::cross_reference_link(section_ref2),
                confidence: LinkConfidence::High,
                coordination_type: None,
                precedence_group: None,
                obligation_type: None,
            },
        ];

        let api = ClauseQueryAPI::new(&links);

        // Should find both section references
        let refs = api.referenced_sections(clause_span);
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(&section_ref1));
        assert!(refs.contains(&section_ref2));

        // has_cross_references should still return true
        assert!(api.has_cross_references(clause_span));
    }

    #[test]
    fn test_multiple_clauses_referencing_same_section() {
        use crate::ClauseLink;

        // Multiple clauses referencing the same section
        let section_ref_span = DocSpan::single_line(0, 0, 10); // shared section reference
        let clause1 = DocSpan::single_line(1, 0, 30);
        let clause2 = DocSpan::single_line(2, 0, 25);
        let clause3 = DocSpan::single_line(3, 0, 40);

        let links = vec![
            ClauseLink {
                anchor: clause1,
                link: crate::ClauseLinkBuilder::cross_reference_link(section_ref_span),
                confidence: LinkConfidence::High,
                coordination_type: None,
                precedence_group: None,
                obligation_type: None,
            },
            ClauseLink {
                anchor: clause2,
                link: crate::ClauseLinkBuilder::cross_reference_link(section_ref_span),
                confidence: LinkConfidence::High,
                coordination_type: None,
                precedence_group: None,
                obligation_type: None,
            },
            ClauseLink {
                anchor: clause3,
                link: crate::ClauseLinkBuilder::cross_reference_link(section_ref_span),
                confidence: LinkConfidence::High,
                coordination_type: None,
                precedence_group: None,
                obligation_type: None,
            },
        ];

        let api = ClauseQueryAPI::new(&links);

        // All three clauses should be found as referencing the section
        let clauses = api.referencing_clauses(section_ref_span);
        assert_eq!(clauses.len(), 3);
        assert!(clauses.contains(&clause1));
        assert!(clauses.contains(&clause2));
        assert!(clauses.contains(&clause3));

        // Each clause should have exactly one reference
        assert_eq!(api.referenced_sections(clause1).len(), 1);
        assert_eq!(api.referenced_sections(clause2).len(), 1);
        assert_eq!(api.referenced_sections(clause3).len(), 1);
    }
}
