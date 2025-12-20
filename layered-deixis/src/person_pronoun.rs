//! Person pronoun resolver for all person pronouns.
//!
//! This resolver detects pronouns of all persons (1st, 2nd, 3rd).
//! It provides basic deictic detection for pronouns that may not be
//! resolved by domain-specific resolvers.

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

use crate::{DeicticCategory, DeicticReference, DeicticSource, DeicticSubcategory};

/// First person singular pronouns
const FIRST_SINGULAR: &[&str] = &["i", "me", "my", "mine", "myself"];

/// First person plural pronouns
const FIRST_PLURAL: &[&str] = &["we", "us", "our", "ours", "ourselves"];

/// Second person pronouns (singular and plural forms are identical in English)
const SECOND_PERSON: &[&str] = &["you", "your", "yours", "yourself", "yourselves"];

/// Third person singular pronouns (masculine, feminine, neuter)
const THIRD_SINGULAR: &[&str] = &[
    "he", "him", "his", "himself", // masculine
    "she", "her", "hers", "herself", // feminine
    "it", "its", "itself", // neuter
];

/// Third person plural pronouns
const THIRD_PLURAL: &[&str] = &["they", "them", "their", "theirs", "themselves"];

/// Resolver that detects person pronouns (1st, 2nd, and 3rd person).
///
/// This provides basic deictic detection for all pronouns. Domain-specific
/// resolvers (like contract pronoun resolvers) may provide additional
/// resolution to specific antecedents.
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
                    } else if let Some(p) = THIRD_SINGULAR.iter().find(|&&p| p == lower_str).copied()
                    {
                        (DeicticSubcategory::PersonThirdSingular, p)
                    } else if let Some(p) = THIRD_PLURAL.iter().find(|&&p| p == lower_str).copied()
                    {
                        (DeicticSubcategory::PersonThirdPlural, p)
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

    #[test]
    fn test_third_person_pronouns() {
        let line = create_line_from_string("Amy went to the store and she bought a loaf of bread.");
        let line = line.run(&PersonPronounResolver);

        let mut display = LLLineDisplay::new(&line);
        display.include::<DeicticReference>();

        // "she" should be detected as 3rd person singular
        let deictic_refs: Vec<_> = line
            .find(&layered_nlp::x::attr::<DeicticReference>())
            .into_iter()
            .collect();

        assert_eq!(deictic_refs.len(), 1, "Expected 1 deixis span for 'she'");

        let deictic = deictic_refs[0].attr();
        assert_eq!(deictic.category, DeicticCategory::Person);
        assert_eq!(deictic.subcategory, DeicticSubcategory::PersonThirdSingular);
        assert_eq!(deictic.surface_text, "she");
    }

    #[test]
    fn test_third_person_it() {
        let line = create_line_from_string("The company filed its report. It was approved.");
        let line = line.run(&PersonPronounResolver);

        let deictic_refs: Vec<_> = line
            .find(&layered_nlp::x::attr::<DeicticReference>())
            .into_iter()
            .collect();

        // "its" and "It" should both be detected
        assert_eq!(deictic_refs.len(), 2, "Expected 2 deixis spans for 'its' and 'It'");

        for find in &deictic_refs {
            let deictic = find.attr();
            assert_eq!(deictic.category, DeicticCategory::Person);
            assert_eq!(deictic.subcategory, DeicticSubcategory::PersonThirdSingular);
        }
    }
}
