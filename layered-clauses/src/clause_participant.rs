//! Clause participant tracking for semantic role labeling.
//!
//! Links resolved pronouns and entities to their roles within clause structure.
//! Supports tracking of Subject (obligee), Object (deliverable), and Obligor
//! (imposer of obligation) roles.

use layered_nlp_document::DocSpan;
use serde::{Deserialize, Serialize};

/// Role of a participant within a clause
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParticipantRole {
    /// The subject/obligee of the clause - entity with duty or permission
    /// Example: "The Tenant shall pay" - Tenant is Subject
    Subject,
    /// The object/deliverable of the clause - entity being acted upon
    /// Example: "shall pay rent" - rent is Object
    Object,
    /// The party imposing the obligation (when distinct from subject)
    /// Example: "as required by Landlord" - Landlord is Obligor
    Obligor,
    /// Indirect object or beneficiary of the action
    /// Example: "provide notice to Tenant" - Tenant is IndirectObject
    IndirectObject,
}

impl ParticipantRole {
    /// Returns true if this role represents an entity performing an action
    pub fn is_actor(&self) -> bool {
        matches!(self, Self::Subject | Self::Obligor)
    }

    /// Returns true if this role represents an entity receiving an action
    pub fn is_recipient(&self) -> bool {
        matches!(self, Self::Object | Self::IndirectObject)
    }

    /// Human-readable description of the role
    pub fn description(&self) -> &'static str {
        match self {
            Self::Subject => "subject (obligee)",
            Self::Object => "object (deliverable)",
            Self::Obligor => "obligor (imposer)",
            Self::IndirectObject => "indirect object (recipient)",
        }
    }
}

/// A participant mention within a clause
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClauseParticipant {
    /// The span of this participant mention in the document
    pub span: DocSpan,
    /// The text of this participant
    pub text: String,
    /// Role of this participant in the clause
    pub role: ParticipantRole,
    /// If this is a pronoun, the span it resolves to (antecedent)
    pub resolved_to: Option<DocSpan>,
    /// Text of what this resolves to
    pub resolved_text: Option<String>,
    /// Whether this is a pronoun that was resolved
    pub is_pronoun: bool,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Whether this needs human review
    pub needs_review: bool,
    /// Reason for review if applicable
    pub review_reason: Option<String>,
}

impl ClauseParticipant {
    /// Create a new participant with explicit entity (not a pronoun)
    pub fn entity(span: DocSpan, text: impl Into<String>, role: ParticipantRole) -> Self {
        Self {
            span,
            text: text.into(),
            role,
            resolved_to: None,
            resolved_text: None,
            is_pronoun: false,
            confidence: 0.9,
            needs_review: false,
            review_reason: None,
        }
    }

    /// Create a participant from a resolved pronoun
    pub fn from_pronoun(
        pronoun_span: DocSpan,
        pronoun_text: impl Into<String>,
        role: ParticipantRole,
        resolved_to: DocSpan,
        resolved_text: impl Into<String>,
        resolution_confidence: f64,
    ) -> Self {
        let confidence = resolution_confidence * 0.9; // Slight penalty for indirection
        Self {
            span: pronoun_span,
            text: pronoun_text.into(),
            role,
            resolved_to: Some(resolved_to),
            resolved_text: Some(resolved_text.into()),
            is_pronoun: true,
            confidence,
            needs_review: confidence < 0.6,
            review_reason: if confidence < 0.6 {
                Some("Low confidence pronoun resolution".to_string())
            } else {
                None
            },
        }
    }

    /// Create an implicit participant (inferred from context)
    pub fn implicit(role: ParticipantRole, inferred_text: impl Into<String>) -> Self {
        Self {
            span: DocSpan::single_line(0, 0, 0), // Zero span for implicit participants
            text: inferred_text.into(),
            role,
            resolved_to: None,
            resolved_text: None,
            is_pronoun: false,
            confidence: 0.5,
            needs_review: true,
            review_reason: Some("Implicit participant - verify inference".to_string()),
        }
    }

    /// Check if this participant is resolved (either explicit entity or resolved pronoun)
    pub fn is_resolved(&self) -> bool {
        !self.is_pronoun || self.resolved_to.is_some()
    }

    /// Get the final resolved text (either direct text or resolved referent)
    pub fn resolved_entity_text(&self) -> &str {
        self.resolved_text.as_deref().unwrap_or(&self.text)
    }

    /// Set the review flag with reason
    pub fn flag_for_review(mut self, reason: impl Into<String>) -> Self {
        self.needs_review = true;
        self.review_reason = Some(reason.into());
        self
    }
}

/// Collection of participants for a clause
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClauseParticipants {
    /// All participants in this clause
    pub participants: Vec<ClauseParticipant>,
    /// The clause span this applies to
    pub clause_span: DocSpan,
}

impl Default for ClauseParticipants {
    fn default() -> Self {
        Self {
            participants: Vec::new(),
            clause_span: DocSpan::single_line(0, 0, 0),
        }
    }
}

impl ClauseParticipants {
    /// Create empty participants for a clause
    pub fn new(clause_span: DocSpan) -> Self {
        Self {
            participants: Vec::new(),
            clause_span,
        }
    }

    /// Add a participant
    pub fn add(&mut self, participant: ClauseParticipant) {
        self.participants.push(participant);
    }

    /// Get all subjects
    pub fn subjects(&self) -> Vec<&ClauseParticipant> {
        self.participants
            .iter()
            .filter(|p| p.role == ParticipantRole::Subject)
            .collect()
    }

    /// Get all objects
    pub fn objects(&self) -> Vec<&ClauseParticipant> {
        self.participants
            .iter()
            .filter(|p| p.role == ParticipantRole::Object)
            .collect()
    }

    /// Get the obligor (if any)
    pub fn obligor(&self) -> Option<&ClauseParticipant> {
        self.participants
            .iter()
            .find(|p| p.role == ParticipantRole::Obligor)
    }

    /// Get all participants by role
    pub fn by_role(&self, role: ParticipantRole) -> Vec<&ClauseParticipant> {
        self.participants
            .iter()
            .filter(|p| p.role == role)
            .collect()
    }

    /// Get the primary subject (highest confidence subject)
    pub fn primary_subject(&self) -> Option<&ClauseParticipant> {
        self.subjects()
            .into_iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Get the primary object (highest confidence object)
    pub fn primary_object(&self) -> Option<&ClauseParticipant> {
        self.objects()
            .into_iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Check if any participant needs review
    pub fn needs_review(&self) -> bool {
        self.participants.iter().any(|p| p.needs_review)
    }

    /// Get all participants that need review
    pub fn participants_needing_review(&self) -> Vec<&ClauseParticipant> {
        self.participants
            .iter()
            .filter(|p| p.needs_review)
            .collect()
    }

    /// Check if this clause has an explicit subject
    pub fn has_subject(&self) -> bool {
        !self.subjects().is_empty()
    }

    /// Count of all participants
    pub fn count(&self) -> usize {
        self.participants.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.participants.is_empty()
    }
}

/// Detects participant patterns in clause text
pub struct ParticipantDetector {
    /// Common subject indicators (often before the modal verb)
    pub subject_patterns: Vec<&'static str>,
    /// Common obligor indicators
    pub obligor_patterns: Vec<&'static str>,
}

impl ParticipantDetector {
    pub fn new() -> Self {
        Self {
            subject_patterns: vec![
                "tenant", "landlord", "lessor", "lessee", "buyer", "seller",
                "party", "parties", "company", "corporation", "employer",
                "employee", "contractor", "client", "customer", "vendor",
            ],
            obligor_patterns: vec![
                "required by", "as directed by", "upon request of",
                "at the direction of", "pursuant to",
            ],
        }
    }

    /// Check if a token looks like a subject (defined term or party name)
    pub fn is_potential_subject(&self, token: &str) -> bool {
        let lower = token.to_lowercase();
        self.subject_patterns.iter().any(|p| lower.contains(p))
    }

    /// Check if text contains an obligor pattern
    pub fn detect_obligor_pattern<'a>(&self, text: &'a str) -> Option<&'static str> {
        let lower = text.to_lowercase();
        self.obligor_patterns
            .iter()
            .find(|p| lower.contains(*p))
            .copied()
    }

    /// Detect the likely role based on position relative to obligation verb
    pub fn detect_role_by_position(
        &self,
        token_position: usize,
        verb_position: Option<usize>,
        is_defined_term: bool,
    ) -> ParticipantRole {
        match verb_position {
            Some(verb_pos) if token_position < verb_pos => {
                // Before the verb = likely subject
                ParticipantRole::Subject
            }
            Some(verb_pos) if token_position > verb_pos => {
                // After the verb = likely object
                ParticipantRole::Object
            }
            _ => {
                // No clear verb or same position - use heuristics
                if is_defined_term {
                    ParticipantRole::Subject // Defined terms are often subjects
                } else {
                    ParticipantRole::Object
                }
            }
        }
    }
}

impl Default for ParticipantDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layered_nlp_document::DocPosition;

    fn make_span(start_line: usize, start_token: usize, end_line: usize, end_token: usize) -> DocSpan {
        DocSpan {
            start: DocPosition { line: start_line, token: start_token },
            end: DocPosition { line: end_line, token: end_token },
        }
    }

    #[test]
    fn test_participant_role_properties() {
        assert!(ParticipantRole::Subject.is_actor());
        assert!(ParticipantRole::Obligor.is_actor());
        assert!(!ParticipantRole::Object.is_actor());

        assert!(ParticipantRole::Object.is_recipient());
        assert!(ParticipantRole::IndirectObject.is_recipient());
        assert!(!ParticipantRole::Subject.is_recipient());
    }

    #[test]
    fn test_clause_participant_entity() {
        let span = make_span(0, 0, 0, 2);
        let participant = ClauseParticipant::entity(span, "Tenant", ParticipantRole::Subject);

        assert_eq!(participant.text, "Tenant");
        assert_eq!(participant.role, ParticipantRole::Subject);
        assert!(!participant.is_pronoun);
        assert!(participant.is_resolved());
        assert!(participant.confidence > 0.8);
    }

    #[test]
    fn test_clause_participant_pronoun_resolution() {
        let pronoun_span = make_span(0, 5, 0, 6);
        let resolved_span = make_span(0, 0, 0, 2);

        let participant = ClauseParticipant::from_pronoun(
            pronoun_span,
            "it",
            ParticipantRole::Subject,
            resolved_span,
            "Tenant",
            0.8,
        );

        assert_eq!(participant.text, "it");
        assert!(participant.is_pronoun);
        assert!(participant.is_resolved());
        assert_eq!(participant.resolved_text, Some("Tenant".to_string()));
        assert_eq!(participant.resolved_entity_text(), "Tenant");
    }

    #[test]
    fn test_clause_participant_implicit() {
        let participant = ClauseParticipant::implicit(
            ParticipantRole::Subject,
            "Tenant (inferred)",
        );

        assert!(participant.needs_review);
        assert_eq!(participant.confidence, 0.5);
    }

    #[test]
    fn test_clause_participants_collection() {
        let clause_span = make_span(0, 0, 0, 10);
        let mut participants = ClauseParticipants::new(clause_span);

        participants.add(ClauseParticipant::entity(
            make_span(0, 0, 0, 2),
            "Tenant",
            ParticipantRole::Subject,
        ));
        participants.add(ClauseParticipant::entity(
            make_span(0, 4, 0, 5),
            "rent",
            ParticipantRole::Object,
        ));

        assert_eq!(participants.count(), 2);
        assert!(participants.has_subject());
        assert_eq!(participants.subjects().len(), 1);
        assert_eq!(participants.objects().len(), 1);
    }

    #[test]
    fn test_clause_participants_primary() {
        let clause_span = make_span(0, 0, 0, 10);
        let mut participants = ClauseParticipants::new(clause_span);

        // Add two subjects with different confidence
        let mut low_conf = ClauseParticipant::entity(
            make_span(0, 0, 0, 2),
            "Party",
            ParticipantRole::Subject,
        );
        low_conf.confidence = 0.6;
        participants.add(low_conf);

        let mut high_conf = ClauseParticipant::entity(
            make_span(0, 3, 0, 4),
            "Tenant",
            ParticipantRole::Subject,
        );
        high_conf.confidence = 0.9;
        participants.add(high_conf);

        let primary = participants.primary_subject().unwrap();
        assert_eq!(primary.text, "Tenant");
    }

    #[test]
    fn test_participant_detector_subject_patterns() {
        let detector = ParticipantDetector::new();

        assert!(detector.is_potential_subject("Tenant"));
        assert!(detector.is_potential_subject("the landlord"));
        assert!(detector.is_potential_subject("COMPANY"));
        assert!(!detector.is_potential_subject("rent"));
        assert!(!detector.is_potential_subject("payment"));
    }

    #[test]
    fn test_participant_detector_obligor_patterns() {
        let detector = ParticipantDetector::new();

        assert!(detector.detect_obligor_pattern("as required by the Landlord").is_some());
        assert!(detector.detect_obligor_pattern("upon request of the Company").is_some());
        assert!(detector.detect_obligor_pattern("Tenant shall pay").is_none());
    }

    #[test]
    fn test_participant_detector_role_by_position() {
        let detector = ParticipantDetector::new();

        // Token before verb = Subject
        let role = detector.detect_role_by_position(0, Some(2), true);
        assert_eq!(role, ParticipantRole::Subject);

        // Token after verb = Object
        let role = detector.detect_role_by_position(4, Some(2), false);
        assert_eq!(role, ParticipantRole::Object);
    }

    #[test]
    fn test_needs_review_flag() {
        let span = make_span(0, 0, 0, 2);
        let participant = ClauseParticipant::entity(span, "Entity", ParticipantRole::Subject)
            .flag_for_review("Ambiguous entity reference");

        assert!(participant.needs_review);
        assert_eq!(participant.review_reason, Some("Ambiguous entity reference".to_string()));
    }

    #[test]
    fn test_participants_needing_review() {
        let clause_span = make_span(0, 0, 0, 10);
        let mut participants = ClauseParticipants::new(clause_span);

        participants.add(ClauseParticipant::entity(
            make_span(0, 0, 0, 2),
            "Tenant",
            ParticipantRole::Subject,
        ));
        participants.add(ClauseParticipant::implicit(
            ParticipantRole::Object,
            "payment",
        ));

        assert!(participants.needs_review());
        assert_eq!(participants.participants_needing_review().len(), 1);
    }
}
