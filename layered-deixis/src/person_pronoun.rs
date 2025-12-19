//! Person pronoun resolver for 1st and 2nd person pronouns.
//!
//! This resolver detects first and second person pronouns which are
//! typically skipped by contract-focused pronoun resolvers.

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

use crate::{DeicticCategory, DeicticReference, DeicticSource, DeicticSubcategory};

/// First person singular pronouns
const FIRST_SINGULAR: &[&str] = &["i", "me", "my", "mine", "myself"];

/// First person plural pronouns
const FIRST_PLURAL: &[&str] = &["we", "us", "our", "ours", "ourselves"];

/// Second person pronouns (singular and plural forms are identical in English)
const SECOND_PERSON: &[&str] = &["you", "your", "yours", "yourself", "yourselves"];

/// Resolver that detects 1st and 2nd person pronouns.
///
/// This fills the gap left by contract-focused pronoun resolvers that
/// typically only handle 3rd person pronouns.
#[derive(Debug, Clone, Default)]
pub struct PersonPronounResolver;

impl PersonPronounResolver {
    /// Create a new PersonPronounResolver.
    pub fn new() -> Self {
        Self
    }
}

impl Resolver for PersonPronounResolver {
    type Attr = DeicticReference;

    fn go(&self, sel: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        sel.find_by(&x::token_text())
            .into_iter()
            .filter_map(|(sel, text)| {
                let lower = text.to_lowercase();
                let lower_str = lower.as_str();

                // Single lookup: find pattern and determine subcategory
                let (subcategory, pattern) =
                    if let Some(p) = FIRST_SINGULAR.iter().find(|&&p| p == lower_str).copied() {
                        (DeicticSubcategory::PersonFirst, p)
                    } else if let Some(p) = FIRST_PLURAL.iter().find(|&&p| p == lower_str).copied()
                    {
                        (DeicticSubcategory::PersonFirstPlural, p)
                    } else if let Some(p) = SECOND_PERSON.iter().find(|&&p| p == lower_str).copied()
                    {
                        // Note: In English, "you" is ambiguous between singular and plural.
                        // We default to PersonSecond (singular) - "yourselves" is explicitly plural.
                        let subcat = if lower_str == "yourselves" {
                            DeicticSubcategory::PersonSecondPlural
                        } else {
                            DeicticSubcategory::PersonSecond
                        };
                        (subcat, p)
                    } else {
                        return None;
                    };

                Some(sel.finish_with_attr(DeicticReference::new(
                    DeicticCategory::Person,
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
    fn test_first_person_pronouns() {
        let line = create_line_from_string("I will send you my report.");
        let line = line.run(&PersonPronounResolver);

        let mut display = LLLineDisplay::new(&line);
        display.include::<DeicticReference>();

        insta::assert_snapshot!(display, @r###"
        I     will     send     you     my     report  .
        ╰DeicticReference { category: Person, subcategory: PersonFirst, surface_text: "I", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "i" } }
                                ╰─╯DeicticReference { category: Person, subcategory: PersonSecond, surface_text: "you", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "you" } }
                                        ╰╯DeicticReference { category: Person, subcategory: PersonFirst, surface_text: "my", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "my" } }
        "###);
    }

    #[test]
    fn test_plural_pronouns() {
        let line = create_line_from_string("We will help ourselves and yourselves.");
        let line = line.run(&PersonPronounResolver);

        let mut display = LLLineDisplay::new(&line);
        display.include::<DeicticReference>();

        insta::assert_snapshot!(display, @r###"
        We     will     help     ourselves     and     yourselves  .
        ╰╯DeicticReference { category: Person, subcategory: PersonFirstPlural, surface_text: "We", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "we" } }
                                 ╰───────╯DeicticReference { category: Person, subcategory: PersonFirstPlural, surface_text: "ourselves", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "ourselves" } }
                                                       ╰────────╯DeicticReference { category: Person, subcategory: PersonSecondPlural, surface_text: "yourselves", resolved_referent: None, confidence: 1.0, source: WordList { pattern: "yourselves" } }
        "###);
    }
}
