//! Semantic role labeling for contract obligations.
//!
//! Extracts semantic roles (Agent, Patient, Theme, Recipient, Beneficiary)
//! from obligation phrases to enable equivalence detection across
//! syntactic variations (active/passive, nominalization).
//!
//! # Architecture
//!
//! - **M8 Gate 1**: Semantic role types (Agent, Patient, Theme, etc.)
//! - **M8 Gate 2**: SemanticRoleLabeler for extracting roles from ObligationPhrase
//! - **M8 Gate 3**: ObligationNormalizer for equivalence detection
//!
//! # Example
//!
//! ```ignore
//! use layered_contracts::{ObligationPhrase, SemanticRoleLabeler};
//!
//! let obligation = ObligationPhrase { /* ... */ };
//! let labeler = SemanticRoleLabeler::new();
//! let frame = labeler.extract_frame(&obligation);
//!
//! // Access semantic roles
//! if let Some(agent) = frame.value.agent() {
//!     println!("Agent: {}", agent.filler);
//! }
//! ```

use crate::{DocPosition, DocSpan, ObligationPhrase, ObligationType, ObligorReference, Scored};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// M8 GATE 1: SEMANTIC ROLE TYPES
// ============================================================================

/// Semantic roles in obligation frames (frame semantics).
///
/// Based on frame semantics, these roles capture "who does what to whom"
/// independent of surface syntax. Note: This is distinct from
/// `layered_nlp_document::SemanticRole` which is used for span links.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArgumentRole {
    /// Entity performing the action (Seller in "Seller delivers")
    Agent,
    /// Entity undergoing the action (Buyer in "Seller pays Buyer")
    Patient,
    /// Theme/topic of the action (goods in "deliver goods")
    Theme,
    /// Recipient of a transfer (Buyer in "deliver to Buyer")
    Recipient,
    /// Entity benefiting from the action
    Beneficiary,
    /// Instrument used (written notice in "notify in writing")
    Instrument,
    /// Location (premises in "deliver to premises")
    Location,
    /// Time (30 days in "within 30 days")
    Time,
    /// Manner (writing in "in writing")
    Manner,
    /// Source (origin of motion/transfer)
    Source,
    /// Goal (destination of motion/transfer)
    Goal,
}

/// A semantic argument with its role assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameArgument {
    /// The role this argument plays
    pub role: ArgumentRole,
    /// The text filling this role
    pub filler: String,
    /// The span of the filler text
    pub span: DocSpan,
    /// Confidence in this role assignment
    pub confidence: f64,
}

/// A semantic frame extracted from an obligation.
///
/// Represents the predicate-argument structure of an obligation,
/// capturing "who does what to whom" independent of syntax.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObligationFrame {
    /// The predicate (action verb, lemmatized)
    pub predicate: String,
    /// Arguments with role labels
    pub arguments: Vec<FrameArgument>,
    /// The span of the entire frame
    pub span: DocSpan,
    /// Whether this is passive voice
    pub is_passive: bool,
}

impl ObligationFrame {
    /// Get the Agent argument, if present.
    pub fn agent(&self) -> Option<&FrameArgument> {
        self.arguments
            .iter()
            .find(|arg| arg.role == ArgumentRole::Agent)
    }

    /// Get the Patient argument, if present.
    pub fn patient(&self) -> Option<&FrameArgument> {
        self.arguments
            .iter()
            .find(|arg| arg.role == ArgumentRole::Patient)
    }

    /// Get the Theme argument, if present.
    pub fn theme(&self) -> Option<&FrameArgument> {
        self.arguments
            .iter()
            .find(|arg| arg.role == ArgumentRole::Theme)
    }

    /// Get all arguments with a specific role.
    pub fn get_role(&self, role: ArgumentRole) -> Vec<&FrameArgument> {
        self.arguments
            .iter()
            .filter(|arg| arg.role == role)
            .collect()
    }
}

// ============================================================================
// M8 GATE 2: SEMANTIC ROLE LABELER
// ============================================================================

/// Labels semantic roles in obligation phrases.
///
/// Uses pattern-based heuristics rather than ML for reliability and
/// explainability. Handles active/passive voice and by-phrases.
pub struct SemanticRoleLabeler {
    /// Prepositions that signal specific roles
    role_prepositions: HashMap<&'static str, ArgumentRole>,
    /// Verbs that take specific role patterns
    verb_patterns: HashMap<&'static str, VerbFrame>,
}

/// Expected argument structure for a verb.
#[derive(Debug, Clone)]
struct VerbFrame {
    /// Core roles for this verb (Agent, Patient, Theme)
    #[allow(dead_code)]
    core_roles: Vec<ArgumentRole>,
    /// Common prepositional arguments
    #[allow(dead_code)]
    prep_roles: HashMap<&'static str, ArgumentRole>,
}

impl SemanticRoleLabeler {
    pub fn new() -> Self {
        let mut role_prepositions = HashMap::new();

        // By-phrase marks Agent in passive
        role_prepositions.insert("by", ArgumentRole::Agent);

        // To-phrases mark Recipient or Goal
        role_prepositions.insert("to", ArgumentRole::Recipient);

        // For-phrases mark Beneficiary
        role_prepositions.insert("for", ArgumentRole::Beneficiary);

        // In-phrases mark Location or Manner
        role_prepositions.insert("in", ArgumentRole::Manner);

        // With-phrases mark Instrument
        role_prepositions.insert("with", ArgumentRole::Instrument);

        // From-phrases mark Source
        role_prepositions.insert("from", ArgumentRole::Source);

        // Temporal prepositions
        role_prepositions.insert("within", ArgumentRole::Time);
        role_prepositions.insert("before", ArgumentRole::Time);
        role_prepositions.insert("after", ArgumentRole::Time);

        let mut verb_patterns = HashMap::new();

        // "deliver" takes Agent, Theme, Recipient
        verb_patterns.insert(
            "deliver",
            VerbFrame {
                core_roles: vec![ArgumentRole::Agent, ArgumentRole::Theme],
                prep_roles: HashMap::from([("to", ArgumentRole::Recipient)]),
            },
        );

        // "indemnify" takes Agent, Patient
        verb_patterns.insert(
            "indemnify",
            VerbFrame {
                core_roles: vec![ArgumentRole::Agent, ArgumentRole::Patient],
                prep_roles: HashMap::new(),
            },
        );

        // "provide" takes Agent, Theme, Recipient
        verb_patterns.insert(
            "provide",
            VerbFrame {
                core_roles: vec![ArgumentRole::Agent, ArgumentRole::Theme],
                prep_roles: HashMap::from([("to", ArgumentRole::Recipient)]),
            },
        );

        // "notify" takes Agent, Patient
        verb_patterns.insert(
            "notify",
            VerbFrame {
                core_roles: vec![ArgumentRole::Agent, ArgumentRole::Patient],
                prep_roles: HashMap::from([("in", ArgumentRole::Manner)]),
            },
        );

        Self {
            role_prepositions,
            verb_patterns,
        }
    }

    /// Extract semantic frame from an obligation phrase.
    ///
    /// Strategy:
    /// 1. Identify predicate (action verb)
    /// 2. Label obligor as Agent (or Patient if passive)
    /// 3. Extract arguments from action text
    /// 4. Assign roles based on prepositions and verb patterns
    pub fn extract_frame(&self, obligation: &ObligationPhrase) -> Scored<ObligationFrame> {
        let predicate = self.lemmatize_verb(&obligation.action);
        let is_passive = self.is_passive(&obligation.action);

        let mut arguments = Vec::new();

        // Extract Agent from obligor
        if let Some(agent_arg) = self.extract_agent(obligation, is_passive) {
            arguments.push(agent_arg);
        }

        // Extract other arguments from action text
        arguments.extend(self.extract_arguments(&obligation.action, &predicate, is_passive));

        // Placeholder span - in real implementation would use actual span
        let span = DocSpan::new(
            DocPosition { line: 0, token: 0 },
            DocPosition { line: 0, token: 1 },
        );

        let frame = ObligationFrame {
            predicate,
            arguments,
            span,
            is_passive,
        };

        let confidence = self.compute_confidence(&frame);

        Scored::rule_based(frame, confidence, "semantic_role_labeler")
    }

    /// Lemmatize a verb to its base form.
    ///
    /// Simplified implementation - maps common legal verb forms.
    pub fn lemmatize_verb(&self, action: &str) -> String {
        let lower = action.to_lowercase();

        // Common legal verb forms
        let lemmas = [
            ("delivers", "deliver"),
            ("delivered", "deliver"),
            ("delivering", "deliver"),
            ("provides", "provide"),
            ("provided", "provide"),
            ("providing", "provide"),
            ("indemnifies", "indemnify"),
            ("indemnified", "indemnify"),
            ("indemnifying", "indemnify"),
            ("notifies", "notify"),
            ("notified", "notify"),
            ("notifying", "notify"),
            ("notification", "notify"),
            ("notice", "notify"),
            ("pays", "pay"),
            ("paid", "pay"),
            ("paying", "pay"),
            ("payment", "pay"),
        ];

        for (inflected, base) in &lemmas {
            if lower.contains(inflected) {
                return base.to_string();
            }
        }

        // Return first verb-like token
        lower
            .split_whitespace()
            .next()
            .unwrap_or(&lower)
            .to_string()
    }

    /// Check if action text is passive voice.
    ///
    /// Heuristics:
    /// - Contains "be" verb + past participle
    /// - Contains "is/are/was/were delivered/provided/etc"
    pub fn is_passive(&self, action: &str) -> bool {
        let lower = action.to_lowercase();

        let be_verbs = ["is", "are", "was", "were", "be", "been", "being"];
        let past_participles = [
            "delivered", "provided", "indemnified", "notified", "paid", "given", "made",
            "performed",
        ];

        let has_be = be_verbs.iter().any(|v| lower.contains(v));
        let has_participle = past_participles.iter().any(|p| lower.contains(p));

        has_be && has_participle
    }

    /// Extract Agent argument from obligor.
    fn extract_agent(
        &self,
        obligation: &ObligationPhrase,
        is_passive: bool,
    ) -> Option<FrameArgument> {
        // In active voice, obligor is Agent
        // In passive voice, obligor is Patient (Agent comes from by-phrase)

        let (role, filler) = if is_passive {
            // Passive: obligor is Patient
            (
                ArgumentRole::Patient,
                self.obligor_text(&obligation.obligor),
            )
        } else {
            // Active: obligor is Agent
            (ArgumentRole::Agent, self.obligor_text(&obligation.obligor))
        };

        // Placeholder span
        let span = DocSpan::new(
            DocPosition { line: 0, token: 0 },
            DocPosition { line: 0, token: 1 },
        );

        Some(FrameArgument {
            role,
            filler,
            span,
            confidence: 0.9,
        })
    }

    /// Extract obligor text from ObligorReference.
    fn obligor_text(&self, obligor: &ObligorReference) -> String {
        match obligor {
            ObligorReference::TermRef { term_name, .. } => term_name.clone(),
            ObligorReference::PronounRef { resolved_to, .. } => resolved_to.clone(),
            ObligorReference::NounPhrase { text } => text.clone(),
        }
    }

    /// Extract arguments from action text.
    ///
    /// Strategy:
    /// - Split on prepositions
    /// - Assign roles based on preposition type
    /// - Extract direct object (first NP after verb)
    fn extract_arguments(
        &self,
        action: &str,
        predicate: &str,
        is_passive: bool,
    ) -> Vec<FrameArgument> {
        let mut arguments = Vec::new();

        // Simple heuristic: look for prepositional phrases
        let tokens: Vec<&str> = action.split_whitespace().collect();

        let mut i = 0;
        while i < tokens.len() {
            let token = tokens[i].to_lowercase();

            // Check if this is a role preposition
            if let Some(&role) = self.role_prepositions.get(token.as_str()) {
                // Extract the phrase following this preposition
                let phrase_start = i + 1;
                let phrase_end = self.find_phrase_end(&tokens, phrase_start);

                if phrase_end > phrase_start {
                    let filler = tokens[phrase_start..phrase_end].join(" ");

                    // Placeholder span
                    let span = DocSpan::new(
                        DocPosition { line: 0, token: 0 },
                        DocPosition { line: 0, token: 1 },
                    );

                    arguments.push(FrameArgument {
                        role,
                        filler,
                        span,
                        confidence: 0.8,
                    });
                }

                i = phrase_end;
            } else {
                i += 1;
            }
        }

        // Extract Theme (direct object) if not passive
        if !is_passive {
            if let Some(theme) = self.extract_direct_object(&tokens, predicate) {
                arguments.push(theme);
            }
        }

        arguments
    }

    /// Find the end of a prepositional phrase.
    ///
    /// Heuristic: extends to next preposition or clause boundary.
    fn find_phrase_end(&self, tokens: &[&str], start: usize) -> usize {
        let boundaries = [",", ";", ".", "and", "or"];

        for i in start..tokens.len() {
            let token = tokens[i].to_lowercase();

            // Stop at clause boundaries
            if boundaries.contains(&token.as_str()) {
                return i;
            }

            // Stop at next preposition
            if self.role_prepositions.contains_key(token.as_str()) {
                return i;
            }
        }

        tokens.len()
    }

    /// Extract direct object (Theme) from tokens.
    ///
    /// Simplified: assumes first noun phrase after verb is Theme.
    fn extract_direct_object(&self, tokens: &[&str], predicate: &str) -> Option<FrameArgument> {
        // Find predicate position
        let pred_idx = tokens
            .iter()
            .position(|t| self.lemmatize_verb(t) == predicate)?;

        // Extract NP following predicate
        let start = pred_idx + 1;
        let end = self.find_phrase_end(tokens, start);

        if end > start {
            let filler = tokens[start..end].join(" ");

            // Placeholder span
            let span = DocSpan::new(
                DocPosition { line: 0, token: 0 },
                DocPosition { line: 0, token: 1 },
            );

            Some(FrameArgument {
                role: ArgumentRole::Theme,
                filler,
                span,
                confidence: 0.75,
            })
        } else {
            None
        }
    }

    /// Compute confidence for a semantic frame.
    fn compute_confidence(&self, frame: &ObligationFrame) -> f64 {
        let mut confidence: f64 = 0.8;

        // Boost if we found Agent
        if frame.agent().is_some() {
            confidence += 0.1;
        }

        // Boost if we found Theme or Patient
        if frame.theme().is_some() || frame.patient().is_some() {
            confidence += 0.05;
        }

        // Penalize if passive with no by-phrase (missing Agent)
        if frame.is_passive && frame.agent().is_none() {
            confidence -= 0.15;
        }

        confidence.min(0.95)
    }
}

impl Default for SemanticRoleLabeler {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// M8 GATE 3: OBLIGATION EQUIVALENCE
// ============================================================================

/// Normalized obligation for equivalence comparison (enhanced version).
///
/// Abstracts over syntax to enable matching obligations that differ in:
/// - Voice (active/passive)
/// - Modality (shall/must/will)
/// - Wording (deliver/provide goods)
///
/// Note: This is an enhanced version with semantic role arguments.
/// The simpler `conflict_detector::NormalizedObligation` is used for conflict detection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnhancedNormalizedObligation {
    /// Canonical obligor (Agent role filler)
    pub obligor: String,
    /// Canonical modal
    pub modal: CanonicalModal,
    /// Predicate (lemmatized verb)
    pub predicate: String,
    /// Semantic arguments (sorted by role)
    pub arguments: Vec<FrameArgument>,
    /// Original text for debugging
    pub original_text: String,
}

/// Canonical modal forms for equivalence comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanonicalModal {
    /// Duty: shall, must, will
    Shall,
    /// Soft duty: should, ought to
    Should,
    /// Permission: may, can
    May,
    /// Prohibition: shall not, must not
    ShallNot,
}

impl CanonicalModal {
    /// Convert from ObligationType.
    pub fn from_obligation_type(ot: ObligationType) -> Self {
        match ot {
            ObligationType::Duty => Self::Shall,
            ObligationType::Permission => Self::May,
            ObligationType::Prohibition => Self::ShallNot,
        }
    }
}

/// Result of comparing two normalized obligations.
#[derive(Debug, Clone)]
pub enum EquivalenceResult {
    /// Obligations are semantically equivalent
    Equivalent { confidence: f64 },
    /// Different modal (shall → may)
    ModalDifference {
        from: CanonicalModal,
        to: CanonicalModal,
    },
    /// Different action (deliver → provide)
    ActionDifference { similarity: f64 },
    /// Different timing
    TimingDifference { explanation: String },
    /// Different in multiple dimensions
    Different,
}

/// Normalizes obligations for equivalence comparison (enhanced version).
///
/// This is an enhanced normalizer that uses semantic role labeling.
/// For simpler normalization, see `conflict_detector::ObligationNormalizer`.
pub struct EnhancedObligationNormalizer {
    /// Role labeler for extracting semantic structure
    role_labeler: SemanticRoleLabeler,
    /// Synonym sets for action verbs
    verb_synonyms: HashMap<&'static str, Vec<&'static str>>,
}

impl EnhancedObligationNormalizer {
    pub fn new() -> Self {
        let mut verb_synonyms = HashMap::new();

        // Synonym sets for common legal verbs
        verb_synonyms.insert("deliver", vec!["provide", "furnish", "supply"]);
        verb_synonyms.insert("notify", vec!["inform", "advise", "give notice"]);
        verb_synonyms.insert("indemnify", vec!["hold harmless", "reimburse"]);
        verb_synonyms.insert("pay", vec!["remit", "compensate"]);

        Self {
            role_labeler: SemanticRoleLabeler::new(),
            verb_synonyms,
        }
    }

    /// Normalize an obligation for comparison.
    pub fn normalize(&self, obligation: &ObligationPhrase) -> EnhancedNormalizedObligation {
        let frame_scored = self.role_labeler.extract_frame(obligation);
        let frame = frame_scored.value;

        // Extract Agent as canonical obligor
        let obligor = frame
            .agent()
            .map(|arg| arg.filler.clone())
            .unwrap_or_else(|| "UNKNOWN".to_string());

        let modal = CanonicalModal::from_obligation_type(obligation.obligation_type);
        let predicate = frame.predicate.clone();

        // Sort arguments by role for consistent comparison
        let mut arguments = frame.arguments;
        arguments.sort_by_key(|arg| arg.role as u8);

        EnhancedNormalizedObligation {
            obligor,
            modal,
            predicate,
            arguments,
            original_text: obligation.action.clone(),
        }
    }

    /// Compare two normalized obligations for equivalence.
    pub fn equivalent(
        &self,
        a: &EnhancedNormalizedObligation,
        b: &EnhancedNormalizedObligation,
    ) -> EquivalenceResult {
        // 1. Check obligor
        let obligor_match = self.same_obligor(a, b);

        // 2. Check modal
        let modal_match = a.modal == b.modal;

        // 3. Check action (predicate + synonyms)
        let action_similarity = self.action_similarity(&a.predicate, &b.predicate);
        let action_match = action_similarity > 0.8;

        // 4. Check arguments
        let args_match = self.arguments_match(&a.arguments, &b.arguments);

        // Determine result
        if obligor_match && modal_match && action_match && args_match {
            EquivalenceResult::Equivalent {
                confidence: 0.95 * action_similarity,
            }
        } else if obligor_match && action_match && args_match {
            EquivalenceResult::ModalDifference {
                from: a.modal,
                to: b.modal,
            }
        } else if obligor_match && modal_match && args_match {
            EquivalenceResult::ActionDifference {
                similarity: action_similarity,
            }
        } else {
            EquivalenceResult::Different
        }
    }

    /// Check if obligors refer to the same entity.
    fn same_obligor(&self, a: &EnhancedNormalizedObligation, b: &EnhancedNormalizedObligation) -> bool {
        // Normalize capitalization and whitespace
        let a_norm = a.obligor.to_lowercase().trim().to_string();
        let b_norm = b.obligor.to_lowercase().trim().to_string();

        a_norm == b_norm
    }

    /// Compute similarity between action predicates.
    fn action_similarity(&self, pred_a: &str, pred_b: &str) -> f64 {
        // Exact match
        if pred_a == pred_b {
            return 1.0;
        }

        // Check synonyms
        if let Some(synonyms) = self.verb_synonyms.get(pred_a) {
            if synonyms.contains(&pred_b) {
                return 0.9;
            }
        }

        if let Some(synonyms) = self.verb_synonyms.get(pred_b) {
            if synonyms.contains(&pred_a) {
                return 0.9;
            }
        }

        // No match
        0.0
    }

    /// Check if argument lists match.
    fn arguments_match(&self, args_a: &[FrameArgument], args_b: &[FrameArgument]) -> bool {
        // Must have same number of arguments
        if args_a.len() != args_b.len() {
            return false;
        }

        // Check each argument (already sorted by role)
        for (arg_a, arg_b) in args_a.iter().zip(args_b.iter()) {
            if arg_a.role != arg_b.role {
                return false;
            }

            // Normalize filler text for comparison
            let filler_a = arg_a.filler.to_lowercase().trim().to_string();
            let filler_b = arg_b.filler.to_lowercase().trim().to_string();

            if filler_a != filler_b {
                return false;
            }
        }

        true
    }
}

impl Default for EnhancedObligationNormalizer {
    fn default() -> Self {
        Self::new()
    }
}
