//! Place/spatial deictic resolver.
//!
//! This resolver detects spatial deictic words like "here", "there",
//! "elsewhere", etc.

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

use crate::{DeicticCategory, DeicticReference, DeicticSource, DeicticSubcategory};

/// Proximal spatial deictics (near speaker)
const PROXIMAL: &[&str] = &["here"];

/// Distal spatial deictics (away from speaker)
const DISTAL: &[&str] = &["there"];

/// Other/indefinite spatial deictics
const OTHER: &[&str] = &[
    "elsewhere",
    "somewhere",
    "anywhere",
    "nowhere",
    "everywhere",
    "nearby",
    "yonder",
];

/// Resolver that detects spatial/place deictic expressions.
#[derive(Debug, Clone, Default)]
pub struct PlaceDeicticResolver;

impl PlaceDeicticResolver {
    /// Create a new PlaceDeicticResolver.
    pub fn new() -> Self {
        Self
    }
}

impl Resolver for PlaceDeicticResolver {
    type Attr = DeicticReference;

    fn go(&self, sel: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        sel.find_by(&x::token_text())
            .into_iter()
            .filter_map(|(sel, text)| {
                let lower = text.to_lowercase();
                let lower_str = lower.as_str();

                // Single lookup: find pattern and determine subcategory
                let (subcategory, pattern) =
                    if let Some(p) = PROXIMAL.iter().find(|&&p| p == lower_str).copied() {
                        (DeicticSubcategory::PlaceProximal, p)
                    } else if let Some(p) = DISTAL.iter().find(|&&p| p == lower_str).copied() {
                        (DeicticSubcategory::PlaceDistal, p)
                    } else if let Some(p) = OTHER.iter().find(|&&p| p == lower_str).copied() {
                        (DeicticSubcategory::PlaceOther, p)
                    } else {
                        return None;
                    };

                Some(sel.finish_with_attr(DeicticReference::new(
                    DeicticCategory::Place,
                    subcategory,
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
    fn test_place_deictics() {
        let line = create_line_from_string("The goods are here, not there or elsewhere.");
        let line = line.run(&PlaceDeicticResolver);

        let mut display = LLLineDisplay::new(&line);
        display.include::<DeicticReference>();

        insta::assert_snapshot!(display, @r###"
        The     goods     are     here  ,     not     there     or     elsewhere  .
                                  ╰──╯DeicticReference { category: Place, subcategory: PlaceProximal, surface_text: "here", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "here" } }
                                                      ╰───╯DeicticReference { category: Place, subcategory: PlaceDistal, surface_text: "there", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "there" } }
                                                                       ╰───────╯DeicticReference { category: Place, subcategory: PlaceOther, surface_text: "elsewhere", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "elsewhere" } }
        "###);
    }
}
