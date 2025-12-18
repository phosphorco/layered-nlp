//! Pronoun chain resolution for contract language analysis.
//!
//! This resolver builds coreference chains that link all mentions of an entity
//! (defined terms, term references, and pronouns) into cohesive clusters.
//!
//! Example:
//! ```text
//! ABC Corp (the "Company") shall deliver. It must comply. The Company agrees.
//!              ╰─────────╯                 ╰╯              ╰───────────╯
//!              Chain #1 seed               Chain #1        Chain #1
//!              "Company"                   pronoun         reference
//! ```
//!
//! Chains enable downstream systems to answer questions like "who is 'it' referring to?"
//! and to aggregate all obligations/permissions for a given party.

use std::collections::HashMap;

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

use crate::defined_term::DefinedTerm;
use crate::pronoun::PronounReference;
use crate::scored::Scored;
use crate::term_reference::TermReference;

/// A single mention within a pronoun chain.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainMention {
    /// The surface text of this mention
    pub text: String,
    /// What type of mention this is
    pub mention_type: MentionType,
    /// Individual confidence for this mention's chain membership
    pub confidence: f64,
    /// Token offset from start of line (for ordering)
    pub token_offset: usize,
}

/// The type of mention in a chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MentionType {
    /// The original definition (e.g., `"Company" means ABC Corp`)
    Definition,
    /// A reference to a defined term (e.g., `the Company`)
    TermReference,
    /// A pronoun resolved to this chain (e.g., `It`)
    Pronoun,
}

/// A coreference chain linking all mentions of a single entity.
#[derive(Debug, Clone, PartialEq)]
pub struct PronounChain {
    /// Unique identifier for this chain
    pub chain_id: u32,
    /// The canonical name for this entity (usually the defined term name)
    pub canonical_name: String,
    /// Whether this chain is rooted in a formally defined term
    pub is_defined_term: bool,
    /// All mentions in this chain, ordered by position in text
    pub mentions: Vec<ChainMention>,
    /// Whether any mention in this chain has been verified (confidence = 1.0)
    pub has_verified_mention: bool,
}

impl PronounChain {
    /// Get the number of mentions in this chain.
    pub fn mention_count(&self) -> usize {
        self.mentions.len()
    }

    /// Get the highest confidence among all mentions.
    pub fn best_mention_confidence(&self) -> f64 {
        self.mentions
            .iter()
            .map(|m| m.confidence)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0)
    }
}

/// Internal structure for building chains during resolution.
#[derive(Debug)]
struct ChainBuilder {
    chain_id: u32,
    canonical_name: String,
    is_defined_term: bool,
    mentions: Vec<ChainMention>,
}

impl ChainBuilder {
    fn new(chain_id: u32, canonical_name: String, is_defined_term: bool) -> Self {
        Self {
            chain_id,
            canonical_name,
            is_defined_term,
            mentions: Vec::new(),
        }
    }

    fn add_mention(&mut self, mention: ChainMention) {
        self.mentions.push(mention);
    }

    fn build(mut self) -> PronounChain {
        // Sort mentions by token offset
        self.mentions.sort_by_key(|m| m.token_offset);

        let has_verified = self.mentions.iter().any(|m| (m.confidence - 1.0).abs() < 0.001);

        PronounChain {
            chain_id: self.chain_id,
            canonical_name: self.canonical_name,
            is_defined_term: self.is_defined_term,
            mentions: self.mentions,
            has_verified_mention: has_verified,
        }
    }
}

/// Resolver for building pronoun coreference chains.
///
/// Requires that the following resolvers have already been run:
/// - `DefinedTermResolver`
/// - `TermReferenceResolver`
/// - `PronounResolver`
pub struct PronounChainResolver {
    /// Minimum confidence for a pronoun to be attached to a chain
    min_attachment_confidence: f64,
    /// Confidence decay per low-confidence link in the chain
    chain_confidence_decay: f64,
}

impl Default for PronounChainResolver {
    fn default() -> Self {
        Self {
            min_attachment_confidence: 0.40,
            chain_confidence_decay: 0.05,
        }
    }
}

impl PronounChainResolver {
    /// Create a new resolver with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a resolver with custom settings.
    pub fn with_settings(min_attachment_confidence: f64, chain_confidence_decay: f64) -> Self {
        Self {
            min_attachment_confidence,
            chain_confidence_decay,
        }
    }

    /// Estimate token offset for a selection (for ordering mentions).
    fn estimate_offset(&self, selection: &LLSelection) -> usize {
        // Count tokens from start to this selection
        // This is an approximation since we can't access internal indices
        let mut count = 0;
        let mut current = selection.clone();

        while current.match_first_backwards(&x::token_text()).is_some() {
            if let Some((prev_sel, _)) = current.match_first_backwards(&x::token_text()) {
                count += 1;
                current = prev_sel;
            } else {
                break;
            }
        }

        count
    }

    /// Calculate overall chain confidence based on mention confidences.
    ///
    /// Strategy: Start with the best mention confidence, then apply a small
    /// decay for each mention below a threshold. This penalizes chains that
    /// are stitched together from many uncertain links.
    fn calculate_chain_confidence(&self, chain: &PronounChain) -> f64 {
        let best = chain.best_mention_confidence();

        // Count low-confidence mentions (below 0.7)
        let low_conf_count = chain
            .mentions
            .iter()
            .filter(|m| m.confidence < 0.7)
            .count();

        // Apply decay for low-confidence links
        let decay = (low_conf_count as f64) * self.chain_confidence_decay;

        (best - decay).clamp(0.0, 1.0)
    }
}

impl Resolver for PronounChainResolver {
    type Attr = Scored<PronounChain>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        // Step 1: Collect all defined terms as chain seeds
        let defined_terms: Vec<_> = selection
            .find_by(&x::attr::<Scored<DefinedTerm>>())
            .into_iter()
            .collect();

        // Step 2: Collect all term references
        let term_refs: Vec<_> = selection
            .find_by(&x::attr::<Scored<TermReference>>())
            .into_iter()
            .collect();

        // Step 3: Collect all pronoun references
        let pronoun_refs: Vec<_> = selection
            .find_by(&x::attr::<Scored<PronounReference>>())
            .into_iter()
            .collect();

        // Step 4: Build chains keyed by canonical name (lowercased for matching)
        let mut chains: HashMap<String, ChainBuilder> = HashMap::new();
        let mut next_chain_id: u32 = 1;

        // Seed chains from defined terms
        for (sel, scored_term) in &defined_terms {
            let key = scored_term.value.term_name.to_lowercase();
            let offset = self.estimate_offset(sel);

            let chain = chains.entry(key.clone()).or_insert_with(|| {
                let id = next_chain_id;
                next_chain_id += 1;
                ChainBuilder::new(id, scored_term.value.term_name.clone(), true)
            });

            chain.add_mention(ChainMention {
                text: scored_term.value.term_name.clone(),
                mention_type: MentionType::Definition,
                confidence: scored_term.confidence,
                token_offset: offset,
            });
        }

        // Add term references to their chains
        for (sel, scored_ref) in &term_refs {
            let key = scored_ref.value.term_name.to_lowercase();
            let offset = self.estimate_offset(sel);

            // Get the surface text for this reference
            let surface_text = sel
                .find_first_by(&x::token_text())
                .map(|(_, t)| t.to_string())
                .unwrap_or_else(|| scored_ref.value.term_name.clone());

            let chain = chains.entry(key.clone()).or_insert_with(|| {
                let id = next_chain_id;
                next_chain_id += 1;
                // If we see a reference without a definition, it's not a defined term
                ChainBuilder::new(id, scored_ref.value.term_name.clone(), false)
            });

            chain.add_mention(ChainMention {
                text: surface_text,
                mention_type: MentionType::TermReference,
                confidence: scored_ref.confidence,
                token_offset: offset,
            });
        }

        // Step 5: Attach pronouns to chains based on their candidate scores
        for (sel, scored_pron) in &pronoun_refs {
            let offset = self.estimate_offset(sel);

            // Find the best candidate that has a chain
            for candidate in &scored_pron.value.candidates {
                if candidate.confidence < self.min_attachment_confidence {
                    continue;
                }

                let key = candidate.text.to_lowercase();

                if let Some(chain) = chains.get_mut(&key) {
                    chain.add_mention(ChainMention {
                        text: scored_pron.value.pronoun.clone(),
                        mention_type: MentionType::Pronoun,
                        confidence: candidate.confidence,
                        token_offset: offset,
                    });
                    break; // Only attach to the best matching chain
                }
            }
        }

        // Step 6: Convert builders to chains and create assignments
        let mut results = Vec::new();

        // Sort by canonical name for deterministic output
        let mut sorted_chains: Vec<_> = chains.into_iter().collect();
        sorted_chains.sort_by(|(a, _), (b, _)| a.cmp(b));

        for (_, builder) in sorted_chains {
            // Skip chains with only one mention (no coreference)
            if builder.mentions.len() < 2 {
                continue;
            }

            let chain = builder.build();
            let confidence = self.calculate_chain_confidence(&chain);

            // Assign the chain to the first mention's position (the seed)
            // Find the selection for the first mention
            let _first_mention_offset = chain.mentions.first().map(|m| m.token_offset).unwrap_or(0);

            // Find a selection at approximately this offset
            // We'll use the defined term or first term reference as the anchor
            let anchor_sel = defined_terms
                .iter()
                .find(|(_, t)| t.value.term_name.to_lowercase() == chain.canonical_name.to_lowercase())
                .map(|(s, _)| s)
                .or_else(|| {
                    term_refs
                        .iter()
                        .find(|(_, t)| {
                            t.value.term_name.to_lowercase() == chain.canonical_name.to_lowercase()
                        })
                        .map(|(s, _)| s)
                });

            if let Some(sel) = anchor_sel {
                results.push(sel.clone().finish_with_attr(Scored::rule_based(
                    chain,
                    confidence,
                    "pronoun_chain",
                )));
            }
        }

        results
    }
}
