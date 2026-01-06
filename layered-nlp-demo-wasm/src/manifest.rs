//! Resolver manifest types for demo metadata.

use layered_nlp::LLLine;
use serde::{Deserialize, Serialize};

use crate::extractors::*;

/// Serializable projection of AssociatedSpan for WASM boundary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawAssociation {
    pub label: String,
    pub glyph: Option<String>,
    pub target_start: u32,
    pub target_end: u32,
}

/// A span extracted from an LLLine, ready for WASM serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSpan {
    pub start: u32,
    pub end: u32,
    pub label: String,
    pub metadata: Option<serde_json::Value>,
    pub associations: Vec<RawAssociation>,
}

/// Tags for classifying resolver stability
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolverTag {
    Stable,
    Experimental,
}

/// Manifest entry for a resolver.
///
/// Note: The `extract` field is not serializable. Gate 3 creates a separate
/// metadata projection for WASM boundary serialization.
pub struct ResolverManifest {
    pub name: &'static str,
    pub description: &'static str,
    pub color: &'static str, // CSS hex color
    pub tags: &'static [ResolverTag],
    pub extract: fn(&LLLine) -> Vec<RawSpan>,
}

/// All resolver manifests for the demo
pub static RESOLVER_MANIFESTS: &[ResolverManifest] = &[
    ResolverManifest {
        name: "ContractKeyword",
        description: "Contract structural keywords (shall, must, if, etc.)",
        color: "#ff6b6b",
        tags: &[ResolverTag::Stable],
        extract: extract_contract_keywords,
    },
    ResolverManifest {
        name: "DefinedTerm",
        description: "Defined terms in the document",
        color: "#4ecdc4",
        tags: &[ResolverTag::Stable],
        extract: extract_defined_terms,
    },
    ResolverManifest {
        name: "TermReference",
        description: "References to defined terms",
        color: "#95e1d3",
        tags: &[ResolverTag::Stable],
        extract: extract_term_references,
    },
    ResolverManifest {
        name: "PronounReference",
        description: "Pronouns with resolution candidates",
        color: "#f38181",
        tags: &[ResolverTag::Stable],
        extract: extract_pronoun_references,
    },
    ResolverManifest {
        name: "ObligationPhrase",
        description: "Obligation phrases with conditions",
        color: "#45b7d1",
        tags: &[ResolverTag::Stable],
        extract: extract_obligation_phrases,
    },
    ResolverManifest {
        name: "PronounChain",
        description: "Coreferential pronoun chains",
        color: "#aa96da",
        tags: &[ResolverTag::Stable],
        extract: extract_pronoun_chains,
    },
    ResolverManifest {
        name: "ContractClause",
        description: "Contract clauses with obligor and duty",
        color: "#fcbad3",
        tags: &[ResolverTag::Stable],
        extract: extract_contract_clauses,
    },
    ResolverManifest {
        name: "ClauseAggregate",
        description: "Grouped clauses by obligor",
        color: "#a8d8ea",
        tags: &[ResolverTag::Stable],
        extract: extract_clause_aggregates,
    },
    ResolverManifest {
        name: "ObligationNode",
        description: "Accountability graph nodes",
        color: "#ffd3b6",
        tags: &[ResolverTag::Stable],
        extract: extract_obligation_nodes,
    },
    ResolverManifest {
        name: "DeicticReference",
        description: "Deictic expressions (pronouns, place, time, discourse)",
        color: "#dcedc1",
        tags: &[ResolverTag::Stable],
        extract: extract_deictic_references,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_tag_serialization() {
        let stable = ResolverTag::Stable;
        let experimental = ResolverTag::Experimental;

        assert_eq!(serde_json::to_string(&stable).unwrap(), "\"stable\"");
        assert_eq!(
            serde_json::to_string(&experimental).unwrap(),
            "\"experimental\""
        );
    }

    #[test]
    fn test_raw_span_creation() {
        let span = RawSpan {
            start: 0,
            end: 10,
            label: "test".to_string(),
            metadata: None,
            associations: vec![],
        };
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 10);
    }

    #[test]
    fn test_raw_association_creation() {
        let assoc = RawAssociation {
            label: "refers_to".to_string(),
            glyph: Some("->".to_string()),
            target_start: 20,
            target_end: 30,
        };
        assert_eq!(assoc.label, "refers_to");
        assert!(assoc.glyph.is_some());
    }

    #[test]
    fn test_manifest_creation_with_all_tag_variants() {
        fn dummy_extractor(_: &LLLine) -> Vec<RawSpan> {
            vec![]
        }

        let manifest = ResolverManifest {
            name: "test",
            description: "Test resolver",
            color: "#ff0000",
            tags: &[ResolverTag::Stable, ResolverTag::Experimental],
            extract: dummy_extractor,
        };

        assert_eq!(manifest.name, "test");
        assert_eq!(manifest.tags.len(), 2);
        assert!(manifest.tags.contains(&ResolverTag::Stable));
        assert!(manifest.tags.contains(&ResolverTag::Experimental));
    }
}
