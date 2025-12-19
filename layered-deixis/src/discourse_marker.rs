//! Discourse marker resolver.
//!
//! This resolver detects discourse connectives like "however", "therefore",
//! "moreover", etc. These are deictic in the sense that they reference
//! relationships between parts of the discourse.

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

use crate::{DeicticCategory, DeicticReference, DeicticSource, DeicticSubcategory};

/// Discourse markers - all map to DiscourseMarker subcategory.
/// The semantic function (contrast, consequence, addition) could be
/// tracked in metadata if needed.
const DISCOURSE_MARKERS: &[&str] = &[
    // Contrast/concession
    "however",
    "nevertheless",
    "nonetheless",
    "although",
    "though",
    "yet",
    "but",
    "still",
    "conversely",
    // Consequence/result
    "therefore",
    "thus",
    "hence",
    "consequently",
    "accordingly",
    "so",
    // Addition/continuation
    "moreover",
    "furthermore",
    "additionally",
    "besides",
    "also",
    "indeed",
    "likewise",
    "similarly",
    // Clarification/exemplification
    "namely",
    "specifically",
    // Summary
    "overall",
    "finally",
    "ultimately",
];

/// Resolver that detects discourse markers/connectives.
///
/// Discourse markers create cohesion by signaling relationships between
/// parts of the text (contrast, consequence, addition, etc.).
#[derive(Debug, Clone, Default)]
pub struct DiscourseMarkerResolver;

impl DiscourseMarkerResolver {
    /// Create a new DiscourseMarkerResolver.
    pub fn new() -> Self {
        Self
    }
}

impl Resolver for DiscourseMarkerResolver {
    type Attr = DeicticReference;

    fn go(&self, sel: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        sel.find_by(&x::token_text())
            .into_iter()
            .filter_map(|(sel, text)| {
                let lower = text.to_lowercase();
                let lower_str = lower.as_str();

                let pattern = DISCOURSE_MARKERS
                    .iter()
                    .find(|&&p| p == lower_str)
                    .copied()?;

                Some(sel.finish_with_attr(DeicticReference::new(
                    DeicticCategory::Discourse,
                    DeicticSubcategory::DiscourseMarker,
                    text.to_string(),
                    DeicticSource::WordList { pattern },
                )))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layered_nlp::{create_line_from_string, LLLineDisplay};

    #[test]
    fn test_discourse_markers() {
        let line =
            create_line_from_string("However, the contract is valid. Therefore, we proceed.");
        let line = line.run(&DiscourseMarkerResolver);

        let mut display = LLLineDisplay::new(&line);
        display.include::<DeicticReference>();

        insta::assert_snapshot!(display, @r###"
        However  ,     the     contract     is     valid  .     Therefore  ,     we     proceed  .
        ╰─────╯DeicticReference { category: Discourse, subcategory: DiscourseMarker, surface_text: "However", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "however" } }
                                                                ╰───────╯DeicticReference { category: Discourse, subcategory: DiscourseMarker, surface_text: "Therefore", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "therefore" } }
        "###);
    }
}
