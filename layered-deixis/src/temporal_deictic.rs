//! Simple temporal deictic resolver.
//!
//! This resolver detects simple temporal deictic words like "now", "then",
//! "today", "yesterday", "tomorrow", etc.
//!
//! Note: More complex temporal expressions (like "within 30 days" or
//! "prior to closing") are handled by domain-specific resolvers.

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

use crate::{DeicticCategory, DeicticReference, DeicticSource, DeicticSubcategory};

/// Present temporal deictics
const PRESENT: &[&str] = &["now", "currently", "presently", "today", "tonight"];

/// Past temporal deictics
const PAST: &[&str] = &[
    "then",
    "yesterday",
    "previously",
    "formerly",
    "earlier",
    "before",
    "ago",
];

/// Future temporal deictics
const FUTURE: &[&str] = &[
    "tomorrow",
    "later",
    "subsequently",
    "hereafter",
    "soon",
    "eventually",
];

/// Resolver that detects simple temporal deictic expressions.
///
/// This resolver handles basic temporal deixis. More complex temporal
/// expressions (durations, deadlines, event-relative times) should be
/// handled by domain-specific resolvers.
#[derive(Debug, Clone, Default)]
pub struct SimpleTemporalResolver;

impl SimpleTemporalResolver {
    /// Create a new SimpleTemporalResolver.
    pub fn new() -> Self {
        Self
    }
}

impl Resolver for SimpleTemporalResolver {
    type Attr = DeicticReference;

    fn go(&self, sel: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        sel.find_by(&x::token_text())
            .into_iter()
            .filter_map(|(sel, text)| {
                let lower = text.to_lowercase();
                let lower_str = lower.as_str();

                // Single lookup: find pattern and determine subcategory
                let (subcategory, pattern) =
                    if let Some(p) = PRESENT.iter().find(|&&p| p == lower_str).copied() {
                        (DeicticSubcategory::TimePresent, p)
                    } else if let Some(p) = PAST.iter().find(|&&p| p == lower_str).copied() {
                        (DeicticSubcategory::TimePast, p)
                    } else if let Some(p) = FUTURE.iter().find(|&&p| p == lower_str).copied() {
                        (DeicticSubcategory::TimeFuture, p)
                    } else {
                        return None;
                    };

                Some(sel.finish_with_attr(DeicticReference::new(
                    DeicticCategory::Time,
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
    fn test_temporal_deictics() {
        let line = create_line_from_string("I will call you now, not yesterday or tomorrow.");
        let line = line.run(&SimpleTemporalResolver);

        let mut display = LLLineDisplay::new(&line);
        display.include::<DeicticReference>();

        insta::assert_snapshot!(display, @r###"
        I     will     call     you     now  ,     not     yesterday     or     tomorrow  .
                                        ╰─╯DeicticReference { category: Time, subcategory: TimePresent, surface_text: "now", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "now" } }
                                                           ╰───────╯DeicticReference { category: Time, subcategory: TimePast, surface_text: "yesterday", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "yesterday" } }
                                                                                ╰──────╯DeicticReference { category: Time, subcategory: TimeFuture, surface_text: "tomorrow", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "tomorrow" } }
        "###);
    }
}
