//! Conflict detection for contract obligations.
//!
//! This module provides the `ConflictDetector` which identifies contradictory
//! provisions within a single contract document:
//!
//! - **Modal conflicts**: Same party, same action, different obligation type (shall vs may)
//! - **Temporal conflicts**: Same obligation with incompatible timing requirements
//! - **Contradictory parties**: Same action assigned to different parties
//! - **Scope overlap**: Obligations that partially overlap with conflicting requirements
//!
//! # Example
//!
//! ```ignore
//! use layered_contracts::{ConflictDetector, ContractDocument, Pipeline};
//!
//! let doc = Pipeline::standard().run_on_text(contract_text);
//! let detector = ConflictDetector::new();
//! let conflicts = detector.detect_in_document(&doc);
//! ```

use std::collections::HashMap;

use crate::document::{DocPosition, DocSpan};
use crate::obligation::{ObligorReference, ObligationType};
use crate::scored::Scored;
use crate::temporal::{NormalizedTiming, TimeUnit};

// ============================================================================
// Gate 0: Core Types
// ============================================================================

/// A detected conflict between two obligations in a contract.
#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    /// The span of the first conflicting obligation.
    pub span_a: DocSpan,
    /// The span of the second conflicting obligation.
    pub span_b: DocSpan,
    /// The type of conflict detected.
    pub conflict_type: ConflictType,
    /// Human-readable explanation of the conflict.
    pub explanation: String,
}

impl Conflict {
    /// Creates a new conflict.
    ///
    /// Spans are automatically ordered by start position (earliest first)
    /// to ensure deterministic behavior when comparing or deduplicating conflicts.
    pub fn new(
        span_a: DocSpan,
        span_b: DocSpan,
        conflict_type: ConflictType,
        explanation: impl Into<String>,
    ) -> Self {
        // Canonicalize ordering by earliest start position for determinism
        let (span_a, span_b) = if (span_a.start.line, span_a.start.token)
            <= (span_b.start.line, span_b.start.token)
        {
            (span_a, span_b)
        } else {
            (span_b, span_a)
        };

        Self {
            span_a,
            span_b,
            conflict_type,
            explanation: explanation.into(),
        }
    }

    /// Returns a span that covers both conflicting obligations.
    ///
    /// Ordering uses `(line, token)` tuple comparison since `DocPosition`
    /// does not implement `Ord`. This matches document's natural reading order.
    pub fn combined_span(&self) -> DocSpan {
        // Since span_a is canonically before span_b (by start position),
        // we still need to find the true min start and max end for overlapping spans
        let start = if (self.span_a.start.line, self.span_a.start.token)
            <= (self.span_b.start.line, self.span_b.start.token)
        {
            self.span_a.start
        } else {
            self.span_b.start
        };

        let end = if (self.span_a.end.line, self.span_a.end.token)
            >= (self.span_b.end.line, self.span_b.end.token)
        {
            self.span_a.end
        } else {
            self.span_b.end
        };

        DocSpan::new(start, end)
    }
}

// Note: SnapshotKind impl is in snapshot/types.rs when snapshot module is enabled

/// The type of conflict between two obligations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConflictType {
    /// Same party, same action, but different modal (shall vs may).
    ModalConflict,
    /// Same obligation with incompatible timing requirements.
    TemporalConflict,
    /// Same action assigned to different parties.
    ContradictoryParties,
    /// Obligations that partially overlap with conflicting requirements.
    ScopeOverlap,
}

impl ConflictType {
    /// Returns a human-readable description of the conflict type.
    pub fn description(&self) -> &'static str {
        match self {
            ConflictType::ModalConflict => "Modal conflict (shall vs may)",
            ConflictType::TemporalConflict => "Temporal conflict (incompatible timing)",
            ConflictType::ContradictoryParties => "Contradictory parties (same action, different obligors)",
            ConflictType::ScopeOverlap => "Scope overlap (conflicting requirements)",
        }
    }
}

/// Topic classification for grouping related obligations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObligationTopic {
    /// Payment-related obligations (pay, remit, compensate).
    Payment,
    /// Delivery-related obligations (deliver, ship, provide).
    Delivery,
    /// Confidentiality obligations (keep confidential, not disclose).
    Confidentiality,
    /// Termination-related obligations (terminate, cancel, end).
    Termination,
    /// Indemnification obligations (indemnify, hold harmless).
    Indemnification,
    /// Notice obligations (notify, inform, give notice).
    Notice,
    /// Other obligations not fitting the above categories.
    Other,
}

/// A normalized representation of an obligation for comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedObligation {
    /// Normalized party name (lowercase, articles stripped).
    pub obligor: String,
    /// The type of obligation (Duty, Permission, Prohibition).
    pub obligation_type: ObligationType,
    /// Normalized/lemmatized action text.
    pub action: String,
    /// Parsed timing information, if present.
    pub timing: Option<NormalizedTiming>,
    /// The original span in the document.
    pub original_span: DocSpan,
    /// The line index where this obligation appears.
    pub line_index: usize,
    /// The classified topic of this obligation.
    pub topic: ObligationTopic,
}

impl NormalizedObligation {
    /// Creates a new normalized obligation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        obligor: String,
        obligation_type: ObligationType,
        action: String,
        timing: Option<NormalizedTiming>,
        original_span: DocSpan,
        line_index: usize,
        topic: ObligationTopic,
    ) -> Self {
        Self {
            obligor,
            obligation_type,
            action,
            timing,
            original_span,
            line_index,
            topic,
        }
    }
}

// ============================================================================
// Gate 1: Obligation Normalization
// ============================================================================

/// Normalizes obligation phrases for comparison.
///
/// Handles:
/// - Party name normalization (strips articles, lowercases)
/// - Action verb lemmatization
/// - Timing expression parsing
#[derive(Debug, Clone)]
pub struct ObligationNormalizer {
    /// Mapping from inflected verb forms to base lemma
    lemma_table: HashMap<String, String>,
}

impl Default for ObligationNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl ObligationNormalizer {
    /// Creates a new normalizer with standard contract verb lemmas.
    pub fn new() -> Self {
        let mut lemma_table = HashMap::new();

        // Standard contract verbs with their inflected forms
        // Format: (base, [inflections...])
        let verbs = [
            ("deliver", &["delivers", "delivered", "delivering", "delivery"][..]),
            ("pay", &["pays", "paid", "paying", "payment"]),
            ("provide", &["provides", "provided", "providing", "provision"]),
            ("perform", &["performs", "performed", "performing", "performance"]),
            ("notify", &["notifies", "notified", "notifying", "notification"]),
            ("terminate", &["terminates", "terminated", "terminating", "termination"]),
            ("indemnify", &["indemnifies", "indemnified", "indemnifying", "indemnification"]),
            ("warrant", &["warrants", "warranted", "warranting", "warranty"]),
            ("represent", &["represents", "represented", "representing", "representation"]),
            ("agree", &["agrees", "agreed", "agreeing", "agreement"]),
            ("comply", &["complies", "complied", "complying", "compliance"]),
            ("submit", &["submits", "submitted", "submitting", "submission"]),
            ("maintain", &["maintains", "maintained", "maintaining", "maintenance"]),
            ("obtain", &["obtains", "obtained", "obtaining"]),
            ("ensure", &["ensures", "ensured", "ensuring"]),
            ("require", &["requires", "required", "requiring", "requirement"]),
            ("complete", &["completes", "completed", "completing", "completion"]),
            ("execute", &["executes", "executed", "executing", "execution"]),
            ("disclose", &["discloses", "disclosed", "disclosing", "disclosure"]),
            ("reimburse", &["reimburses", "reimbursed", "reimbursing", "reimbursement"]),
            ("remit", &["remits", "remitted", "remitting", "remittance"]),
            ("transfer", &["transfers", "transferred", "transferring"]),
        ];

        for (base, inflections) in verbs {
            // Map base to itself
            lemma_table.insert(base.to_string(), base.to_string());
            // Map each inflection to base
            for inflection in inflections {
                lemma_table.insert(inflection.to_string(), base.to_string());
            }
        }

        Self { lemma_table }
    }

    /// Extracts the obligor name from an ObligorReference.
    pub fn extract_obligor_name(&self, obligor: &ObligorReference) -> String {
        match obligor {
            ObligorReference::TermRef { term_name, .. } => self.normalize_party(term_name),
            ObligorReference::PronounRef { resolved_to, .. } => self.normalize_party(resolved_to),
            ObligorReference::NounPhrase { text } => self.normalize_party(text),
        }
    }

    /// Normalizes a party name by stripping articles and lowercasing.
    pub fn normalize_party(&self, name: &str) -> String {
        let lower = name.to_lowercase();
        let trimmed = lower.trim();

        // Strip leading articles
        let without_article = if trimmed.starts_with("the ") {
            &trimmed[4..]
        } else if trimmed.starts_with("a ") {
            &trimmed[2..]
        } else if trimmed.starts_with("an ") {
            &trimmed[3..]
        } else {
            trimmed
        };

        without_article.trim().to_string()
    }

    /// Normalizes an action string by lemmatizing verbs.
    pub fn normalize_action(&self, action: &str) -> String {
        let words: Vec<&str> = action.split_whitespace().collect();
        let normalized_words: Vec<String> = words
            .iter()
            .map(|word| {
                let lower = word.to_lowercase();
                // Remove trailing punctuation for lookup
                let clean = lower.trim_end_matches(|c: char| c.is_ascii_punctuation());
                self.lemma_table
                    .get(clean)
                    .cloned()
                    .unwrap_or_else(|| clean.to_string())
            })
            .collect();

        normalized_words.join(" ")
    }

    /// Parses timing information from an action string.
    ///
    /// Handles patterns like:
    /// - "within 30 days"
    /// - "within thirty (30) days"
    /// - "15 business days"
    pub fn normalize_timing(&self, text: &str) -> Option<NormalizedTiming> {
        let lower = text.to_lowercase();

        // Try to find numeric patterns first (prefer digits over written)
        if let Some(timing) = self.parse_numeric_timing(&lower) {
            return Some(timing);
        }

        // Try written numbers if no numeric match
        if let Some(timing) = self.parse_written_timing(&lower) {
            return Some(timing);
        }

        None
    }

    /// Parses numeric timing expressions like "30 days" or "within 15 business days".
    fn parse_numeric_timing(&self, text: &str) -> Option<NormalizedTiming> {
        // Find a number in the text
        let mut num_start = None;
        let mut num_end = 0;

        for (i, c) in text.char_indices() {
            if c.is_ascii_digit() {
                if num_start.is_none() {
                    num_start = Some(i);
                }
                num_end = i + 1;
            } else if num_start.is_some() {
                break;
            }
        }

        let num_start = num_start?;
        let value: f64 = text[num_start..num_end].parse().ok()?;

        // Look for unit after the number
        let after_num = &text[num_end..];

        let unit = if after_num.contains("business day") || after_num.contains("working day") {
            TimeUnit::BusinessDays
        } else if after_num.contains("day") {
            TimeUnit::Days
        } else if after_num.contains("week") {
            TimeUnit::Weeks
        } else if after_num.contains("month") {
            TimeUnit::Months
        } else if after_num.contains("year") {
            TimeUnit::Years
        } else {
            return None;
        };

        Some(NormalizedTiming::new(value, unit, false))
    }

    /// Parses written number timing expressions.
    fn parse_written_timing(&self, text: &str) -> Option<NormalizedTiming> {
        // Written number patterns - order matters (longer matches first)
        let written_numbers = [
            ("twenty-five", 25), ("twenty-four", 24), ("twenty-three", 23),
            ("twenty-two", 22), ("twenty-one", 21), ("twenty", 20),
            ("nineteen", 19), ("eighteen", 18), ("seventeen", 17),
            ("sixteen", 16), ("fifteen", 15), ("fourteen", 14),
            ("thirteen", 13), ("twelve", 12), ("eleven", 11),
            ("ten", 10), ("nine", 9), ("eight", 8), ("seven", 7),
            ("six", 6), ("five", 5), ("four", 4), ("three", 3),
            ("two", 2), ("one", 1),
            ("thirty", 30), ("forty", 40), ("fifty", 50),
            ("sixty", 60), ("ninety", 90),
        ];

        for (word, value) in written_numbers {
            if text.contains(word) {
                // Check what unit follows
                let after_num = text.split(word).nth(1)?;
                let unit = if after_num.contains("business day") || after_num.contains("working day") {
                    TimeUnit::BusinessDays
                } else if after_num.contains("day") {
                    TimeUnit::Days
                } else if after_num.contains("week") {
                    TimeUnit::Weeks
                } else if after_num.contains("month") {
                    TimeUnit::Months
                } else if after_num.contains("year") {
                    TimeUnit::Years
                } else {
                    continue;
                };

                return Some(NormalizedTiming::new(value as f64, unit, false));
            }
        }

        None
    }

    /// Normalizes a scored obligation phrase into a NormalizedObligation.
    pub fn normalize(
        &self,
        scored: &Scored<crate::obligation::ObligationPhrase>,
        line_index: usize,
        start_token: usize,
        end_token: usize,
    ) -> NormalizedObligation {
        let phrase = &scored.value;

        let obligor = self.extract_obligor_name(&phrase.obligor);
        let action = self.normalize_action(&phrase.action);
        let timing = self.normalize_timing(&phrase.action);
        let original_span = DocSpan::new(
            DocPosition::new(line_index, start_token),
            DocPosition::new(line_index, end_token),
        );

        // Topic classification will be done in Gate 2
        let topic = ObligationTopic::Other;

        NormalizedObligation::new(
            obligor,
            phrase.obligation_type,
            action,
            timing,
            original_span,
            line_index,
            topic,
        )
    }
}

// ============================================================================
// Gate 2: Topic Classification
// ============================================================================

/// Classifies obligations into topic categories for grouping related obligations.
///
/// Uses keyword matching with word boundary detection to avoid false positives
/// like "repayment" matching "payment" when searching for Payment topics.
#[derive(Debug, Clone)]
pub struct TopicClassifier {
    /// Keywords for Payment topic
    payment_keywords: Vec<&'static str>,
    /// Keywords for Delivery topic
    delivery_keywords: Vec<&'static str>,
    /// Keywords for Confidentiality topic
    confidentiality_keywords: Vec<&'static str>,
    /// Keywords for Termination topic
    termination_keywords: Vec<&'static str>,
    /// Keywords for Indemnification topic
    indemnification_keywords: Vec<&'static str>,
    /// Keywords for Notice topic
    notice_keywords: Vec<&'static str>,
}

impl Default for TopicClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl TopicClassifier {
    /// Creates a new classifier with standard contract topic keywords.
    pub fn new() -> Self {
        Self {
            payment_keywords: vec![
                "pay", "paid", "pays", "paying", "payment", "payments",
                "remit", "remits", "remitted", "remitting", "remittance",
                "compensate", "compensates", "compensated", "compensation",
                "reimburse", "reimburses", "reimbursed", "reimbursement",
                "fee", "fees", "invoice", "invoices", "amount", "amounts",
                "price", "prices", "cost", "costs", "sum", "sums",
            ],
            delivery_keywords: vec![
                "deliver", "delivers", "delivered", "delivering", "delivery",
                "ship", "ships", "shipped", "shipping", "shipment",
                "provide", "provides", "provided", "providing", "provision",
                "supply", "supplies", "supplied", "supplying",
                "furnish", "furnishes", "furnished", "furnishing",
                "goods", "products", "materials", "items",
            ],
            confidentiality_keywords: vec![
                "confidential", "confidentiality", "confidentially",
                "secret", "secrets", "secrecy",
                "proprietary", "non-disclosure", "nondisclosure",
                "disclose", "discloses", "disclosed", "disclosing", "disclosure",
                "trade secret", "trade secrets",
            ],
            termination_keywords: vec![
                "terminate", "terminates", "terminated", "terminating", "termination",
                "cancel", "cancels", "cancelled", "canceling", "cancellation",
                "end", "ends", "ended", "ending",
                "expire", "expires", "expired", "expiring", "expiration",
                "revoke", "revokes", "revoked", "revoking", "revocation",
            ],
            indemnification_keywords: vec![
                "indemnify", "indemnifies", "indemnified", "indemnifying", "indemnification",
                "hold harmless", "held harmless", "holds harmless",
                "defend", "defends", "defended", "defending", "defense",
                "liability", "liabilities", "liable",
                "damages", "losses", "claims",
            ],
            notice_keywords: vec![
                "notify", "notifies", "notified", "notifying", "notification",
                "notice", "notices",
                "inform", "informs", "informed", "informing",
                "advise", "advises", "advised", "advising",
                "written notice", "prior notice", "advance notice",
            ],
        }
    }

    /// Classifies an obligation into a topic category.
    ///
    /// Searches the normalized action text for topic keywords with word
    /// boundary detection to avoid false positives.
    pub fn classify(&self, obligation: &NormalizedObligation) -> ObligationTopic {
        let action = &obligation.action;

        // Check topics in order of specificity (more specific first)
        if self.matches_any(action, &self.indemnification_keywords) {
            ObligationTopic::Indemnification
        } else if self.matches_any(action, &self.confidentiality_keywords) {
            ObligationTopic::Confidentiality
        } else if self.matches_any(action, &self.termination_keywords) {
            ObligationTopic::Termination
        } else if self.matches_any(action, &self.notice_keywords) {
            ObligationTopic::Notice
        } else if self.matches_any(action, &self.payment_keywords) {
            ObligationTopic::Payment
        } else if self.matches_any(action, &self.delivery_keywords) {
            ObligationTopic::Delivery
        } else {
            ObligationTopic::Other
        }
    }

    /// Checks if text contains any of the keywords with word boundary detection.
    fn matches_any(&self, text: &str, keywords: &[&str]) -> bool {
        let lower = text.to_lowercase();
        keywords.iter().any(|kw| self.has_word_boundary_match(&lower, kw))
    }

    /// Checks if a keyword appears in text with word boundaries.
    ///
    /// A word boundary is defined as:
    /// - Start of string or non-alphanumeric character before the keyword
    /// - End of string or non-alphanumeric character after the keyword
    ///
    /// This prevents "payment" from matching inside "repayment".
    fn has_word_boundary_match(&self, text: &str, keyword: &str) -> bool {
        let mut search_start = 0;

        while let Some(pos) = text[search_start..].find(keyword) {
            let abs_pos = search_start + pos;
            let end_pos = abs_pos + keyword.len();

            // Check preceding character (word boundary before)
            let has_boundary_before = if abs_pos == 0 {
                true
            } else {
                text[..abs_pos]
                    .chars()
                    .next_back()
                    .map(|c| !c.is_alphanumeric())
                    .unwrap_or(true)
            };

            // Check following character (word boundary after)
            let has_boundary_after = if end_pos >= text.len() {
                true
            } else {
                text[end_pos..]
                    .chars()
                    .next()
                    .map(|c| !c.is_alphanumeric())
                    .unwrap_or(true)
            };

            if has_boundary_before && has_boundary_after {
                return true;
            }

            // Move past this match and continue searching
            search_start = abs_pos + 1;
        }

        false
    }
}

/// Groups obligations by their topic.
///
/// Returns a map from topic to all obligations with that topic.
/// Uses the provided classifier to categorize each obligation.
pub fn group_by_topic(
    obligations: Vec<NormalizedObligation>,
    classifier: &TopicClassifier,
) -> HashMap<ObligationTopic, Vec<NormalizedObligation>> {
    let mut groups: HashMap<ObligationTopic, Vec<NormalizedObligation>> = HashMap::new();

    for mut obligation in obligations {
        let topic = classifier.classify(&obligation);
        obligation.topic = topic;
        groups.entry(topic).or_default().push(obligation);
    }

    groups
}

// ============================================================================
// Gate 3: Conflict Detection Logic
// ============================================================================

/// Detects conflicts between obligations in a contract document.
///
/// Conflicts include:
/// - **Modal conflicts**: Same party, same action, different obligation type (shall vs may)
/// - **Temporal conflicts**: Same obligation with incompatible timing requirements
/// - **Contradictory parties**: Same action assigned to different parties
#[derive(Debug, Clone)]
pub struct ConflictDetector {
    /// Threshold for action similarity (Jaccard) to consider them "same action"
    pub similarity_threshold: f64,
    /// Minimum confidence for obligations to be considered for conflict detection
    pub confidence_threshold: f64,
    /// Tolerance ratio for temporal conflicts (e.g., 0.5 means 50% difference is a conflict)
    pub temporal_tolerance: f64,
    /// Topic classifier
    classifier: TopicClassifier,
    /// Obligation normalizer
    normalizer: ObligationNormalizer,
}

impl Default for ConflictDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ConflictDetector {
    /// Creates a new conflict detector with default settings.
    pub fn new() -> Self {
        Self {
            similarity_threshold: 0.7,
            confidence_threshold: 0.5,
            temporal_tolerance: 0.5,
            classifier: TopicClassifier::new(),
            normalizer: ObligationNormalizer::new(),
        }
    }

    /// Creates a conflict detector with custom thresholds.
    pub fn with_thresholds(
        similarity_threshold: f64,
        confidence_threshold: f64,
        temporal_tolerance: f64,
    ) -> Self {
        Self {
            similarity_threshold,
            confidence_threshold,
            temporal_tolerance,
            classifier: TopicClassifier::new(),
            normalizer: ObligationNormalizer::new(),
        }
    }

    /// Returns a reference to the topic classifier.
    pub fn classifier(&self) -> &TopicClassifier {
        &self.classifier
    }

    /// Returns a reference to the obligation normalizer.
    pub fn normalizer(&self) -> &ObligationNormalizer {
        &self.normalizer
    }

    /// Computes Jaccard similarity between two action strings.
    ///
    /// The Jaccard coefficient is defined as |A ∩ B| / |A ∪ B| where
    /// A and B are the sets of words in each action.
    ///
    /// Returns 1.0 for identical actions, 0.0 for completely different actions.
    pub fn action_similarity(&self, a: &str, b: &str) -> f64 {
        let words_a: std::collections::HashSet<&str> = a.split_whitespace().collect();
        let words_b: std::collections::HashSet<&str> = b.split_whitespace().collect();

        if words_a.is_empty() && words_b.is_empty() {
            return 1.0;
        }

        let intersection = words_a.intersection(&words_b).count();
        let union = words_a.union(&words_b).count();

        if union == 0 {
            return 0.0;
        }

        intersection as f64 / union as f64
    }

    /// Detects a modal conflict between two obligations.
    ///
    /// A modal conflict occurs when:
    /// - Same obligor (party)
    /// - Same or similar action
    /// - Different obligation types (e.g., Duty vs Permission)
    pub fn detect_modal_conflict(
        &self,
        a: &NormalizedObligation,
        b: &NormalizedObligation,
    ) -> Option<Scored<Conflict>> {
        // Same obligor required
        if a.obligor != b.obligor {
            return None;
        }

        // Different obligation type required
        if a.obligation_type == b.obligation_type {
            return None;
        }

        // Similar action required
        let similarity = self.action_similarity(&a.action, &b.action);
        if similarity < self.similarity_threshold {
            return None;
        }

        let explanation = format!(
            "{} has conflicting obligations: '{}' ({:?}) vs '{}' ({:?})",
            a.obligor, a.action, a.obligation_type, b.action, b.obligation_type
        );

        let conflict = Conflict::new(
            a.original_span,
            b.original_span,
            ConflictType::ModalConflict,
            explanation,
        );

        // Confidence based on action similarity
        Some(Scored::rule_based(conflict, similarity, "modal_conflict"))
    }

    /// Detects a temporal conflict between two obligations.
    ///
    /// A temporal conflict occurs when:
    /// - Same obligor
    /// - Same or similar action
    /// - Incompatible timing (e.g., "within 15 days" vs "within 30 days")
    ///
    /// The temporal_tolerance controls how much difference is considered a conflict.
    /// For example, 0.5 means a 50% difference triggers a conflict.
    pub fn detect_temporal_conflict(
        &self,
        a: &NormalizedObligation,
        b: &NormalizedObligation,
    ) -> Option<Scored<Conflict>> {
        // Same obligor required
        if a.obligor != b.obligor {
            return None;
        }

        // Both must have timing information
        let timing_a = a.timing.as_ref()?;
        let timing_b = b.timing.as_ref()?;

        // Similar action required
        let similarity = self.action_similarity(&a.action, &b.action);
        if similarity < self.similarity_threshold {
            return None;
        }

        // Convert to days for comparison
        let days_a = timing_a.to_approx_days();
        let days_b = timing_b.to_approx_days();

        // Calculate relative difference
        let max_days = days_a.max(days_b);
        if max_days == 0.0 {
            return None;
        }

        let diff = (days_a - days_b).abs() / max_days;

        // Check if difference exceeds tolerance
        if diff <= self.temporal_tolerance {
            return None;
        }

        let explanation = format!(
            "{}'s obligation to '{}' has conflicting timing: {} {:?} vs {} {:?}",
            a.obligor,
            a.action,
            timing_a.value,
            timing_a.unit,
            timing_b.value,
            timing_b.unit
        );

        let conflict = Conflict::new(
            a.original_span,
            b.original_span,
            ConflictType::TemporalConflict,
            explanation,
        );

        // Higher difference = higher confidence in the conflict
        let confidence = (similarity + diff.min(1.0)) / 2.0;
        Some(Scored::rule_based(conflict, confidence, "temporal_conflict"))
    }

    /// Detects a party conflict between two obligations.
    ///
    /// A party conflict occurs when:
    /// - Different obligors (parties)
    /// - Same or similar action
    /// - Same obligation type (both duties or both prohibitions)
    ///
    /// This indicates contradictory assignments of the same responsibility.
    pub fn detect_party_conflict(
        &self,
        a: &NormalizedObligation,
        b: &NormalizedObligation,
    ) -> Option<Scored<Conflict>> {
        // Different obligor required
        if a.obligor == b.obligor {
            return None;
        }

        // Same obligation type (both must do the same thing)
        if a.obligation_type != b.obligation_type {
            return None;
        }

        // Only duties and prohibitions can conflict this way
        if a.obligation_type == ObligationType::Permission {
            return None;
        }

        // Similar action required (high threshold for party conflicts)
        let similarity = self.action_similarity(&a.action, &b.action);
        if similarity < self.similarity_threshold {
            return None;
        }

        let explanation = format!(
            "Same action '{}' assigned to different parties: {} and {}",
            a.action, a.obligor, b.obligor
        );

        let conflict = Conflict::new(
            a.original_span,
            b.original_span,
            ConflictType::ContradictoryParties,
            explanation,
        );

        Some(Scored::rule_based(conflict, similarity, "party_conflict"))
    }

    /// Detects all conflicts among a set of normalized obligations.
    ///
    /// Compares each pair of obligations and returns all detected conflicts.
    /// Obligations are compared in deterministic order by (line_index, token_start).
    pub fn detect_conflicts(
        &self,
        obligations: &[NormalizedObligation],
    ) -> Vec<Scored<Conflict>> {
        let mut conflicts = Vec::new();

        // Sort by position for deterministic ordering
        let mut sorted: Vec<_> = obligations.iter().collect();
        sorted.sort_by_key(|o| (o.line_index, o.original_span.start.token));

        // Compare all pairs
        for i in 0..sorted.len() {
            for j in (i + 1)..sorted.len() {
                let a = sorted[i];
                let b = sorted[j];

                // Try each conflict type
                if let Some(conflict) = self.detect_modal_conflict(a, b) {
                    conflicts.push(conflict);
                }

                if let Some(conflict) = self.detect_temporal_conflict(a, b) {
                    conflicts.push(conflict);
                }

                if let Some(conflict) = self.detect_party_conflict(a, b) {
                    conflicts.push(conflict);
                }
            }
        }

        conflicts
    }

    /// Normalizes an obligation phrase and classifies its topic.
    ///
    /// This is a convenience method that uses the internal normalizer and classifier.
    pub fn normalize_and_classify(
        &self,
        scored: &Scored<crate::obligation::ObligationPhrase>,
        line_index: usize,
        start_token: usize,
        end_token: usize,
    ) -> NormalizedObligation {
        let mut obligation = self.normalizer.normalize(scored, line_index, start_token, end_token);
        obligation.topic = self.classifier.classify(&obligation);
        obligation
    }

    /// Detects conflicts in a contract document.
    ///
    /// This method:
    /// 1. Queries all `Scored<ObligationPhrase>` from document lines
    /// 2. Normalizes each obligation with its position
    /// 3. Runs conflict detection on all normalized obligations
    /// 4. Returns all detected conflicts with confidence scores
    ///
    /// # Requirements
    /// The document must have been processed with at least:
    /// - `ObligationPhraseResolver`
    ///
    /// Use `Pipeline::standard().run_on_text(text)` to ensure proper resolver ordering.
    ///
    /// # Example
    /// ```ignore
    /// use layered_contracts::{ConflictDetector, Pipeline};
    ///
    /// let doc = Pipeline::standard().run_on_text(contract_text)?;
    /// let detector = ConflictDetector::new();
    /// let conflicts = detector.detect_in_document(&doc);
    /// ```
    pub fn detect_in_document(
        &self,
        doc: &crate::document::ContractDocument,
    ) -> Vec<Scored<Conflict>> {
        use crate::obligation::ObligationPhrase;
        use layered_nlp::x;

        let mut obligations = Vec::new();

        for (line_index, line) in doc.lines().iter().enumerate() {
            // Find all ObligationPhrase attributes in this line
            for found in line.find(&x::attr::<Scored<ObligationPhrase>>()) {
                let scored = found.attr();

                // Only process obligations above confidence threshold
                if scored.confidence < self.confidence_threshold {
                    continue;
                }

                // Get the span of the obligation within the line
                let (start_token, end_token) = found.range();

                // Normalize and classify
                let normalized = self.normalize_and_classify(
                    scored,
                    line_index,
                    start_token,
                    end_token,
                );

                obligations.push(normalized);
            }
        }

        self.detect_conflicts(&obligations)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocPosition;

    fn make_span(start_line: usize, start_token: usize, end_line: usize, end_token: usize) -> DocSpan {
        DocSpan::new(
            DocPosition::new(start_line, start_token),
            DocPosition::new(end_line, end_token),
        )
    }

    #[test]
    fn test_conflict_combined_span_same_line() {
        let conflict = Conflict::new(
            make_span(0, 0, 0, 5),
            make_span(0, 10, 0, 15),
            ConflictType::ModalConflict,
            "Test conflict",
        );

        let combined = conflict.combined_span();
        assert_eq!(combined.start.line, 0);
        assert_eq!(combined.start.token, 0);
        assert_eq!(combined.end.line, 0);
        assert_eq!(combined.end.token, 15);
    }

    #[test]
    fn test_conflict_combined_span_different_lines() {
        let conflict = Conflict::new(
            make_span(2, 5, 2, 10),
            make_span(0, 0, 0, 5),
            ConflictType::TemporalConflict,
            "Test conflict",
        );

        let combined = conflict.combined_span();
        assert_eq!(combined.start.line, 0);
        assert_eq!(combined.start.token, 0);
        assert_eq!(combined.end.line, 2);
        assert_eq!(combined.end.token, 10);
    }

    #[test]
    fn test_conflict_combined_span_overlapping() {
        let conflict = Conflict::new(
            make_span(1, 0, 3, 10),
            make_span(2, 5, 4, 15),
            ConflictType::ScopeOverlap,
            "Overlapping obligations",
        );

        let combined = conflict.combined_span();
        assert_eq!(combined.start.line, 1);
        assert_eq!(combined.start.token, 0);
        assert_eq!(combined.end.line, 4);
        assert_eq!(combined.end.token, 15);
    }

    #[test]
    fn test_conflict_type_descriptions() {
        assert_eq!(
            ConflictType::ModalConflict.description(),
            "Modal conflict (shall vs may)"
        );
        assert_eq!(
            ConflictType::TemporalConflict.description(),
            "Temporal conflict (incompatible timing)"
        );
        assert_eq!(
            ConflictType::ContradictoryParties.description(),
            "Contradictory parties (same action, different obligors)"
        );
        assert_eq!(
            ConflictType::ScopeOverlap.description(),
            "Scope overlap (conflicting requirements)"
        );
    }

    #[test]
    fn test_obligation_topic_is_hashable() {
        use std::collections::HashSet;
        let mut topics = HashSet::new();
        topics.insert(ObligationTopic::Payment);
        topics.insert(ObligationTopic::Delivery);
        topics.insert(ObligationTopic::Payment); // duplicate
        assert_eq!(topics.len(), 2);
    }

    #[test]
    fn test_conflict_type_is_hashable() {
        use std::collections::HashSet;
        let mut types = HashSet::new();
        types.insert(ConflictType::ModalConflict);
        types.insert(ConflictType::TemporalConflict);
        types.insert(ConflictType::ModalConflict); // duplicate
        assert_eq!(types.len(), 2);
    }

    #[test]
    fn test_conflict_canonical_ordering() {
        // When span_b comes before span_a, they should be swapped
        let early_span = make_span(0, 0, 0, 5);
        let late_span = make_span(2, 10, 2, 15);

        // Pass late first, early second
        let conflict = Conflict::new(
            late_span,
            early_span,
            ConflictType::ModalConflict,
            "Test",
        );

        // span_a should be the earlier one
        assert_eq!(conflict.span_a.start.line, 0);
        assert_eq!(conflict.span_b.start.line, 2);
    }


    #[test]
    fn test_normalized_obligation_construction() {
        let timing = NormalizedTiming::new(30.0, TimeUnit::Days, false);
        let span = make_span(0, 0, 0, 10);

        let obligation = NormalizedObligation::new(
            "company".to_string(),
            ObligationType::Duty,
            "deliver goods".to_string(),
            Some(timing),
            span,
            0,
            ObligationTopic::Delivery,
        );

        assert_eq!(obligation.obligor, "company");
        assert_eq!(obligation.obligation_type, ObligationType::Duty);
        assert_eq!(obligation.action, "deliver goods");
        assert!(obligation.timing.is_some());
        assert_eq!(obligation.topic, ObligationTopic::Delivery);
    }

    // ========================================================================
    // Gate 1: ObligationNormalizer Tests
    // ========================================================================

    #[test]
    fn test_normalize_party_strips_articles() {
        let normalizer = ObligationNormalizer::new();

        assert_eq!(normalizer.normalize_party("The Company"), "company");
        assert_eq!(normalizer.normalize_party("the Seller"), "seller");
        assert_eq!(normalizer.normalize_party("A Contractor"), "contractor");
        assert_eq!(normalizer.normalize_party("an Agent"), "agent");
        assert_eq!(normalizer.normalize_party("Buyer"), "buyer");
    }

    #[test]
    fn test_normalize_party_handles_whitespace() {
        let normalizer = ObligationNormalizer::new();

        assert_eq!(normalizer.normalize_party("  The Company  "), "company");
        assert_eq!(normalizer.normalize_party("the   Seller"), "seller");
    }

    #[test]
    fn test_normalize_action_lemmatizes_verbs() {
        let normalizer = ObligationNormalizer::new();

        assert_eq!(normalizer.normalize_action("delivers goods"), "deliver goods");
        assert_eq!(normalizer.normalize_action("delivered the products"), "deliver the products");
        assert_eq!(normalizer.normalize_action("pays the invoice"), "pay the invoice");
        assert_eq!(normalizer.normalize_action("paid promptly"), "pay promptly");
    }

    #[test]
    fn test_normalize_action_handles_nominalizations() {
        let normalizer = ObligationNormalizer::new();

        // Nominalizations map to base verb
        assert_eq!(normalizer.normalize_action("delivery of goods"), "deliver of goods");
        assert_eq!(normalizer.normalize_action("payment due"), "pay due");
        assert_eq!(normalizer.normalize_action("termination notice"), "terminate notice");
    }

    #[test]
    fn test_normalize_action_preserves_unknown_words() {
        let normalizer = ObligationNormalizer::new();

        assert_eq!(normalizer.normalize_action("xyz goods abc"), "xyz goods abc");
        assert_eq!(normalizer.normalize_action("delivers xyz"), "deliver xyz");
    }

    #[test]
    fn test_normalize_timing_numeric_days() {
        let normalizer = ObligationNormalizer::new();

        let timing = normalizer.normalize_timing("within 30 days").unwrap();
        assert_eq!(timing.value, 30.0);
        assert_eq!(timing.unit, TimeUnit::Days);
    }

    #[test]
    fn test_normalize_timing_numeric_business_days() {
        let normalizer = ObligationNormalizer::new();

        let timing = normalizer.normalize_timing("within 15 business days").unwrap();
        assert_eq!(timing.value, 15.0);
        assert_eq!(timing.unit, TimeUnit::BusinessDays);

        let timing2 = normalizer.normalize_timing("10 working days").unwrap();
        assert_eq!(timing2.value, 10.0);
        assert_eq!(timing2.unit, TimeUnit::BusinessDays);
    }

    #[test]
    fn test_normalize_timing_numeric_weeks_months_years() {
        let normalizer = ObligationNormalizer::new();

        let weeks = normalizer.normalize_timing("within 2 weeks").unwrap();
        assert_eq!(weeks.value, 2.0);
        assert_eq!(weeks.unit, TimeUnit::Weeks);

        let months = normalizer.normalize_timing("60 months").unwrap();
        assert_eq!(months.value, 60.0);
        assert_eq!(months.unit, TimeUnit::Months);

        let years = normalizer.normalize_timing("5 years").unwrap();
        assert_eq!(years.value, 5.0);
        assert_eq!(years.unit, TimeUnit::Years);
    }

    #[test]
    fn test_normalize_timing_written_numbers() {
        let normalizer = ObligationNormalizer::new();

        let timing = normalizer.normalize_timing("within five days").unwrap();
        assert_eq!(timing.value, 5.0);
        assert_eq!(timing.unit, TimeUnit::Days);

        let timing2 = normalizer.normalize_timing("thirty days").unwrap();
        assert_eq!(timing2.value, 30.0);
        assert_eq!(timing2.unit, TimeUnit::Days);
    }

    #[test]
    fn test_normalize_timing_compound_numbers() {
        let normalizer = ObligationNormalizer::new();

        let timing = normalizer.normalize_timing("twenty-five days").unwrap();
        assert_eq!(timing.value, 25.0);
        assert_eq!(timing.unit, TimeUnit::Days);
    }

    #[test]
    fn test_normalize_timing_prefers_digits_over_written() {
        let normalizer = ObligationNormalizer::new();

        // When both present, prefer the digits (e.g., "thirty (30) days")
        let timing = normalizer.normalize_timing("thirty (30) days").unwrap();
        assert_eq!(timing.value, 30.0);
        assert_eq!(timing.unit, TimeUnit::Days);
    }

    #[test]
    fn test_normalize_timing_no_timing() {
        let normalizer = ObligationNormalizer::new();

        assert!(normalizer.normalize_timing("deliver goods promptly").is_none());
        assert!(normalizer.normalize_timing("pay when due").is_none());
    }

    #[test]
    fn test_normalize_timing_eighteen_vs_eight() {
        let normalizer = ObligationNormalizer::new();

        // "eighteen" should match before "eight"
        let timing = normalizer.normalize_timing("eighteen days").unwrap();
        assert_eq!(timing.value, 18.0);

        let timing2 = normalizer.normalize_timing("eight days").unwrap();
        assert_eq!(timing2.value, 8.0);
    }

    #[test]
    fn test_extract_obligor_name_term_ref() {
        let normalizer = ObligationNormalizer::new();

        let obligor = ObligorReference::TermRef {
            term_name: "The Company".to_string(),
            confidence: 1.0,
        };

        assert_eq!(normalizer.extract_obligor_name(&obligor), "company");
    }

    #[test]
    fn test_extract_obligor_name_pronoun_ref() {
        let normalizer = ObligationNormalizer::new();

        let obligor = ObligorReference::PronounRef {
            pronoun: "It".to_string(),
            resolved_to: "The Seller".to_string(),
            is_defined_term: true,
            confidence: 0.9,
        };

        assert_eq!(normalizer.extract_obligor_name(&obligor), "seller");
    }

    #[test]
    fn test_extract_obligor_name_noun_phrase() {
        let normalizer = ObligationNormalizer::new();

        let obligor = ObligorReference::NounPhrase {
            text: "the contractor".to_string(),
        };

        assert_eq!(normalizer.extract_obligor_name(&obligor), "contractor");
    }

    // ========================================================================
    // Gate 2: TopicClassifier Tests
    // ========================================================================

    fn make_obligation(action: &str) -> NormalizedObligation {
        NormalizedObligation::new(
            "company".to_string(),
            ObligationType::Duty,
            action.to_string(),
            None,
            make_span(0, 0, 0, 10),
            0,
            ObligationTopic::Other, // Will be classified
        )
    }

    #[test]
    fn test_classify_payment_topic() {
        let classifier = TopicClassifier::new();

        assert_eq!(
            classifier.classify(&make_obligation("pay the invoice within 30 days")),
            ObligationTopic::Payment
        );
        assert_eq!(
            classifier.classify(&make_obligation("remit payment promptly")),
            ObligationTopic::Payment
        );
        assert_eq!(
            classifier.classify(&make_obligation("reimburse all costs")),
            ObligationTopic::Payment
        );
    }

    #[test]
    fn test_classify_delivery_topic() {
        let classifier = TopicClassifier::new();

        assert_eq!(
            classifier.classify(&make_obligation("deliver goods to buyer")),
            ObligationTopic::Delivery
        );
        assert_eq!(
            classifier.classify(&make_obligation("ship products within 10 days")),
            ObligationTopic::Delivery
        );
        assert_eq!(
            classifier.classify(&make_obligation("provide materials as specified")),
            ObligationTopic::Delivery
        );
    }

    #[test]
    fn test_classify_confidentiality_topic() {
        let classifier = TopicClassifier::new();

        assert_eq!(
            classifier.classify(&make_obligation("keep information confidential")),
            ObligationTopic::Confidentiality
        );
        assert_eq!(
            classifier.classify(&make_obligation("not disclose trade secrets")),
            ObligationTopic::Confidentiality
        );
        assert_eq!(
            classifier.classify(&make_obligation("maintain secrecy of proprietary data")),
            ObligationTopic::Confidentiality
        );
    }

    #[test]
    fn test_classify_termination_topic() {
        let classifier = TopicClassifier::new();

        assert_eq!(
            classifier.classify(&make_obligation("terminate agreement upon breach")),
            ObligationTopic::Termination
        );
        assert_eq!(
            classifier.classify(&make_obligation("cancel subscription with notice")),
            ObligationTopic::Termination
        );
        assert_eq!(
            classifier.classify(&make_obligation("revoke access immediately")),
            ObligationTopic::Termination
        );
    }

    #[test]
    fn test_classify_indemnification_topic() {
        let classifier = TopicClassifier::new();

        assert_eq!(
            classifier.classify(&make_obligation("indemnify and hold harmless")),
            ObligationTopic::Indemnification
        );
        assert_eq!(
            classifier.classify(&make_obligation("defend against claims")),
            ObligationTopic::Indemnification
        );
        assert_eq!(
            classifier.classify(&make_obligation("pay all damages and losses")),
            ObligationTopic::Indemnification
        );
    }

    #[test]
    fn test_classify_notice_topic() {
        let classifier = TopicClassifier::new();

        assert_eq!(
            classifier.classify(&make_obligation("notify party of changes")),
            ObligationTopic::Notice
        );
        assert_eq!(
            classifier.classify(&make_obligation("provide written notice")),
            ObligationTopic::Notice
        );
        assert_eq!(
            classifier.classify(&make_obligation("inform of any breach")),
            ObligationTopic::Notice
        );
    }

    #[test]
    fn test_classify_other_topic() {
        let classifier = TopicClassifier::new();

        assert_eq!(
            classifier.classify(&make_obligation("comply with applicable laws")),
            ObligationTopic::Other
        );
        assert_eq!(
            classifier.classify(&make_obligation("maintain accurate records")),
            ObligationTopic::Other
        );
    }

    #[test]
    fn test_word_boundary_prevents_false_positives() {
        let classifier = TopicClassifier::new();

        // "repayment" should NOT match "payment" due to word boundary check
        assert_eq!(
            classifier.classify(&make_obligation("schedule repayment plan")),
            ObligationTopic::Other
        );

        // "prepaid" should NOT match "paid"
        assert_eq!(
            classifier.classify(&make_obligation("use prepaid card")),
            ObligationTopic::Other
        );

        // "undelivered" should NOT match "deliver"
        assert_eq!(
            classifier.classify(&make_obligation("handle undelivered parcels")),
            ObligationTopic::Other
        );
    }

    #[test]
    fn test_word_boundary_allows_valid_matches() {
        let classifier = TopicClassifier::new();

        // Keyword at start of action
        assert_eq!(
            classifier.classify(&make_obligation("payment is due")),
            ObligationTopic::Payment
        );

        // Keyword at end of action
        assert_eq!(
            classifier.classify(&make_obligation("make payment")),
            ObligationTopic::Payment
        );

        // Keyword with punctuation
        assert_eq!(
            classifier.classify(&make_obligation("pay, as agreed")),
            ObligationTopic::Payment
        );
    }

    #[test]
    fn test_group_by_topic() {
        let classifier = TopicClassifier::new();
        let obligations = vec![
            make_obligation("pay invoice"),
            make_obligation("deliver goods"),
            make_obligation("pay fees"),
            make_obligation("notify party"),
            make_obligation("comply with rules"),
        ];

        let groups = group_by_topic(obligations, &classifier);

        assert_eq!(groups.get(&ObligationTopic::Payment).map(|v| v.len()), Some(2));
        assert_eq!(groups.get(&ObligationTopic::Delivery).map(|v| v.len()), Some(1));
        assert_eq!(groups.get(&ObligationTopic::Notice).map(|v| v.len()), Some(1));
        assert_eq!(groups.get(&ObligationTopic::Other).map(|v| v.len()), Some(1));
        assert!(groups.get(&ObligationTopic::Confidentiality).is_none());
    }

    #[test]
    fn test_topic_classification_is_case_insensitive() {
        let classifier = TopicClassifier::new();

        assert_eq!(
            classifier.classify(&make_obligation("PAY THE INVOICE")),
            ObligationTopic::Payment
        );
        assert_eq!(
            classifier.classify(&make_obligation("Deliver Goods")),
            ObligationTopic::Delivery
        );
    }

    #[test]
    fn test_indemnification_takes_precedence() {
        let classifier = TopicClassifier::new();

        // "damages" is in both Indemnification and potentially overlaps with Payment (costs)
        // Indemnification should be checked first due to specificity
        assert_eq!(
            classifier.classify(&make_obligation("pay damages and losses")),
            ObligationTopic::Indemnification
        );
    }

    // ========================================================================
    // Gate 3: ConflictDetector Tests
    // ========================================================================

    fn make_obligation_full(
        obligor: &str,
        obligation_type: ObligationType,
        action: &str,
        timing: Option<NormalizedTiming>,
        line_index: usize,
    ) -> NormalizedObligation {
        NormalizedObligation::new(
            obligor.to_string(),
            obligation_type,
            action.to_string(),
            timing,
            make_span(line_index, 0, line_index, 10),
            line_index,
            ObligationTopic::Other,
        )
    }

    #[test]
    fn test_action_similarity_identical() {
        let detector = ConflictDetector::new();
        assert_eq!(detector.action_similarity("deliver goods", "deliver goods"), 1.0);
    }

    #[test]
    fn test_action_similarity_partial_overlap() {
        let detector = ConflictDetector::new();
        // "deliver" in common, 2/3 union = 0.333...
        let sim = detector.action_similarity("deliver goods", "deliver products");
        assert!(sim > 0.3 && sim < 0.4);
    }

    #[test]
    fn test_action_similarity_no_overlap() {
        let detector = ConflictDetector::new();
        assert_eq!(detector.action_similarity("pay invoice", "ship products"), 0.0);
    }

    #[test]
    fn test_action_similarity_empty_strings() {
        let detector = ConflictDetector::new();
        assert_eq!(detector.action_similarity("", ""), 1.0);
    }

    #[test]
    fn test_detect_modal_conflict() {
        let detector = ConflictDetector::new();

        let shall_deliver = make_obligation_full(
            "company",
            ObligationType::Duty,
            "deliver goods",
            None,
            0,
        );
        let may_deliver = make_obligation_full(
            "company",
            ObligationType::Permission,
            "deliver goods",
            None,
            1,
        );

        let conflict = detector.detect_modal_conflict(&shall_deliver, &may_deliver);
        assert!(conflict.is_some());

        let scored = conflict.unwrap();
        assert_eq!(scored.value.conflict_type, ConflictType::ModalConflict);
        assert!(scored.confidence >= 0.7); // High similarity
    }

    #[test]
    fn test_no_modal_conflict_different_obligor() {
        let detector = ConflictDetector::new();

        let company_shall = make_obligation_full(
            "company",
            ObligationType::Duty,
            "deliver goods",
            None,
            0,
        );
        let vendor_may = make_obligation_full(
            "vendor",
            ObligationType::Permission,
            "deliver goods",
            None,
            1,
        );

        let conflict = detector.detect_modal_conflict(&company_shall, &vendor_may);
        assert!(conflict.is_none());
    }

    #[test]
    fn test_no_modal_conflict_same_obligation_type() {
        let detector = ConflictDetector::new();

        let shall_deliver = make_obligation_full(
            "company",
            ObligationType::Duty,
            "deliver goods",
            None,
            0,
        );
        let shall_ship = make_obligation_full(
            "company",
            ObligationType::Duty,
            "deliver products",
            None,
            1,
        );

        let conflict = detector.detect_modal_conflict(&shall_deliver, &shall_ship);
        assert!(conflict.is_none());
    }

    #[test]
    fn test_detect_temporal_conflict() {
        let detector = ConflictDetector::new();

        // 10 vs 30 days = 66.7% difference, clearly exceeds 50% tolerance
        let deliver_10_days = make_obligation_full(
            "company",
            ObligationType::Duty,
            "deliver goods",
            Some(NormalizedTiming::new(10.0, TimeUnit::Days, false)),
            0,
        );
        let deliver_30_days = make_obligation_full(
            "company",
            ObligationType::Duty,
            "deliver goods",
            Some(NormalizedTiming::new(30.0, TimeUnit::Days, false)),
            1,
        );

        let conflict = detector.detect_temporal_conflict(&deliver_10_days, &deliver_30_days);
        assert!(conflict.is_some());

        let scored = conflict.unwrap();
        assert_eq!(scored.value.conflict_type, ConflictType::TemporalConflict);
    }

    #[test]
    fn test_no_temporal_conflict_similar_timing() {
        let detector = ConflictDetector::new();

        let deliver_29_days = make_obligation_full(
            "company",
            ObligationType::Duty,
            "deliver goods",
            Some(NormalizedTiming::new(29.0, TimeUnit::Days, false)),
            0,
        );
        let deliver_30_days = make_obligation_full(
            "company",
            ObligationType::Duty,
            "deliver goods",
            Some(NormalizedTiming::new(30.0, TimeUnit::Days, false)),
            1,
        );

        // 29 vs 30 is only ~3.3% difference, well under 50% tolerance
        let conflict = detector.detect_temporal_conflict(&deliver_29_days, &deliver_30_days);
        assert!(conflict.is_none());
    }

    #[test]
    fn test_detect_party_conflict() {
        let detector = ConflictDetector::new();

        let company_deliver = make_obligation_full(
            "company",
            ObligationType::Duty,
            "deliver goods",
            None,
            0,
        );
        let vendor_deliver = make_obligation_full(
            "vendor",
            ObligationType::Duty,
            "deliver goods",
            None,
            1,
        );

        let conflict = detector.detect_party_conflict(&company_deliver, &vendor_deliver);
        assert!(conflict.is_some());

        let scored = conflict.unwrap();
        assert_eq!(scored.value.conflict_type, ConflictType::ContradictoryParties);
    }

    #[test]
    fn test_no_party_conflict_permission() {
        let detector = ConflictDetector::new();

        // Permissions don't conflict - both parties MAY do something
        let company_may = make_obligation_full(
            "company",
            ObligationType::Permission,
            "deliver goods",
            None,
            0,
        );
        let vendor_may = make_obligation_full(
            "vendor",
            ObligationType::Permission,
            "deliver goods",
            None,
            1,
        );

        let conflict = detector.detect_party_conflict(&company_may, &vendor_may);
        assert!(conflict.is_none());
    }

    #[test]
    fn test_detect_conflicts_multiple() {
        let detector = ConflictDetector::new();

        let obligations = vec![
            // Modal conflict pair
            make_obligation_full("company", ObligationType::Duty, "deliver goods", None, 0),
            make_obligation_full("company", ObligationType::Permission, "deliver goods", None, 1),
            // Party conflict pair
            make_obligation_full("vendor", ObligationType::Duty, "deliver goods", None, 2),
        ];

        let conflicts = detector.detect_conflicts(&obligations);

        // Should detect: 1 modal conflict (company shall vs may) + 2 party conflicts
        // (company duty vs vendor duty, company permission vs vendor duty - but permission doesn't create party conflict)
        assert!(conflicts.len() >= 1);

        // At least one modal conflict
        assert!(conflicts.iter().any(|c| c.value.conflict_type == ConflictType::ModalConflict));
    }

    #[test]
    fn test_detect_conflicts_deterministic_ordering() {
        let detector = ConflictDetector::new();

        // Same obligations in different order should produce same results
        let obligations1 = vec![
            make_obligation_full("company", ObligationType::Duty, "deliver", None, 2),
            make_obligation_full("company", ObligationType::Permission, "deliver", None, 0),
            make_obligation_full("company", ObligationType::Duty, "deliver", None, 1),
        ];

        let obligations2 = vec![
            make_obligation_full("company", ObligationType::Permission, "deliver", None, 0),
            make_obligation_full("company", ObligationType::Duty, "deliver", None, 1),
            make_obligation_full("company", ObligationType::Duty, "deliver", None, 2),
        ];

        let conflicts1 = detector.detect_conflicts(&obligations1);
        let conflicts2 = detector.detect_conflicts(&obligations2);

        // Same number of conflicts
        assert_eq!(conflicts1.len(), conflicts2.len());
    }

    #[test]
    fn test_conflict_detector_with_custom_thresholds() {
        let detector = ConflictDetector::with_thresholds(0.9, 0.6, 0.3);

        assert_eq!(detector.similarity_threshold, 0.9);
        assert_eq!(detector.confidence_threshold, 0.6);
        assert_eq!(detector.temporal_tolerance, 0.3);
    }

    // ========================================================================
    // Gate 4: Document Integration Tests
    // ========================================================================

    /// Helper to run full resolver chain on text (needed for document integration tests).
    /// Pipeline::standard() doesn't include POSTagResolver which is needed for obligor detection.
    fn run_full_pipeline(text: &str) -> crate::document::ContractDocument {
        use layered_part_of_speech::POSTagResolver;
        use crate::{
            ContractKeywordResolver, DefinedTermResolver, ObligationPhraseResolver,
            ProhibitionResolver, PronounChainResolver, PronounResolver,
            SectionHeaderResolver, SectionReferenceResolver, TemporalExpressionResolver,
            TermReferenceResolver,
        };

        crate::document::ContractDocument::from_text(text)
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
    fn test_detect_in_document_modal_conflict() {
        // Contract text with modal conflict: Company shall deliver and Company may deliver
        // Note: Actions must be similar enough (>0.7 Jaccard similarity)
        let text = r#"ABC Corp (the "Company") shall deliver goods.
ABC Corp (the "Company") may deliver goods."#;

        let doc = run_full_pipeline(text);

        let detector = ConflictDetector::new();
        let conflicts = detector.detect_in_document(&doc);

        // Should detect a modal conflict
        assert!(
            conflicts.iter().any(|c| c.value.conflict_type == ConflictType::ModalConflict),
            "Should detect modal conflict between shall and may for same action. Found {} conflicts: {:?}",
            conflicts.len(),
            conflicts.iter().map(|c| format!("{:?}: {}", c.value.conflict_type, c.value.explanation)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_detect_in_document_party_conflict() {
        // Contract text with party conflict: Both Company and Vendor shall deliver
        let text = r#"ABC Corp (the "Company") shall deliver products.
XYZ Inc (the "Vendor") shall deliver products."#;

        let doc = run_full_pipeline(text);
        let detector = ConflictDetector::new();
        let conflicts = detector.detect_in_document(&doc);

        // Should detect a party conflict
        assert!(
            conflicts.iter().any(|c| c.value.conflict_type == ConflictType::ContradictoryParties),
            "Should detect party conflict when different parties must do same action. Found {} conflicts: {:?}",
            conflicts.len(),
            conflicts.iter().map(|c| &c.value.conflict_type).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_detect_in_document_no_conflicts() {
        // Contract text with no conflicts
        let text = r#"ABC Corp (the "Company") shall pay the invoice.
XYZ Inc (the "Vendor") shall deliver products."#;

        let doc = run_full_pipeline(text);
        let detector = ConflictDetector::new();
        let conflicts = detector.detect_in_document(&doc);

        // Different actions by different parties - no conflict
        assert!(
            conflicts.is_empty(),
            "Should not detect conflicts when actions are different. Found: {:?}",
            conflicts.iter().map(|c| &c.value.explanation).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_detect_in_document_respects_confidence_threshold() {
        let text = r#"ABC Corp (the "Company") shall deliver goods.
ABC Corp (the "Company") may deliver goods."#;

        let doc = run_full_pipeline(text);

        // Use a very high confidence threshold that obligations won't meet
        let detector = ConflictDetector::with_thresholds(0.7, 0.99, 0.5);
        let conflicts = detector.detect_in_document(&doc);

        // Should filter out obligations below threshold
        // Note: ObligationPhrase typically has confidence around 0.75-0.90
        // With threshold 0.99, most should be filtered
        assert!(
            conflicts.is_empty(),
            "High confidence threshold should filter out all conflicts. Found: {:?}",
            conflicts.len()
        );
    }
}
