//! Deictic mapping resolver for contract language.
//!
//! This resolver maps contract-specific deictic attributes (PronounReference,
//! TemporalExpression, SectionReference) to the unified `DeicticReference` type
//! from the `layered_deixis` crate.
//!
//! This respects Rust's orphan rule: the domain crate does the mapping since
//! it owns neither the trait nor the type from layered_deixis.

use layered_deixis::{
    DeicticCategory, DeicticReference, DeicticSource, DeicticSubcategory, ResolvedReferent,
};
use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

use crate::pronoun::{PronounReference, PronounType};
use crate::Scored;
use crate::section_reference::{ReferenceType, RelativeReference, SectionReference};
use crate::temporal::{TemporalExpression, TemporalType};

/// Resolver that maps contract-specific deictic attributes to unified `DeicticReference`.
///
/// This resolver should run AFTER:
/// - `PronounResolver`
/// - `TemporalExpressionResolver`
/// - `SectionReferenceResolver`
///
/// It reads attributes from those resolvers and produces corresponding
/// `DeicticReference` attributes for unified deixis analysis.
#[derive(Debug, Clone, Default)]
pub struct DeicticResolver;

impl DeicticResolver {
    /// Create a new DeicticResolver.
    pub fn new() -> Self {
        Self
    }

    /// Map PronounType to DeicticSubcategory.
    fn map_pronoun_type(pronoun_type: PronounType) -> DeicticSubcategory {
        match pronoun_type {
            PronounType::ThirdSingularNeuter
            | PronounType::ThirdSingularMasculine
            | PronounType::ThirdSingularFeminine => DeicticSubcategory::PersonThirdSingular,
            PronounType::ThirdPlural => DeicticSubcategory::PersonThirdPlural,
            PronounType::Relative => DeicticSubcategory::PersonRelative,
            PronounType::Other => DeicticSubcategory::Other,
        }
    }

    /// Map TemporalType to DeicticSubcategory.
    fn map_temporal_type(temporal_type: &TemporalType) -> DeicticSubcategory {
        match temporal_type {
            // Absolute dates are not truly deictic (they don't shift with context)
            TemporalType::Date { .. } => DeicticSubcategory::Other,
            TemporalType::Duration { .. } => DeicticSubcategory::TimeDuration,
            TemporalType::Deadline { .. } => DeicticSubcategory::TimeDeadline,
            TemporalType::DefinedDate { .. } => DeicticSubcategory::TimeDefinedTerm,
            TemporalType::RelativeTime { .. } => DeicticSubcategory::TimeRelative,
        }
    }

    /// Map RelativeReference to DeicticSubcategory.
    fn map_relative_reference(rel_ref: RelativeReference) -> DeicticSubcategory {
        match rel_ref {
            RelativeReference::This => DeicticSubcategory::DiscourseSectionRef,
            RelativeReference::Foregoing | RelativeReference::Above => {
                DeicticSubcategory::DiscourseAnaphoric
            }
            RelativeReference::Below => DeicticSubcategory::DiscourseCataphoric,
            RelativeReference::Hereof | RelativeReference::Herein => {
                DeicticSubcategory::DiscourseThisDocument
            }
        }
    }
}

impl Resolver for DeicticResolver {
    type Attr = DeicticReference;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        // Map PronounReference → Person deixis
        for (sel, scored_pronoun) in selection.find_by(&x::attr::<Scored<PronounReference>>()) {
            let pronoun_ref = &scored_pronoun.value;
            let subcategory = Self::map_pronoun_type(pronoun_ref.pronoun_type);

            // Build resolved referent from best candidate.
            // Note: There are two confidence values:
            // - candidate.confidence: how confident we are about this specific antecedent resolution
            // - scored_pronoun.confidence: how confident we are that this is a pronoun at all
            let resolved = pronoun_ref.candidates.first().map(|candidate| {
                ResolvedReferent::new(&candidate.text, candidate.confidence)
            });

            let mut deictic = DeicticReference::new(
                DeicticCategory::Person,
                subcategory,
                &pronoun_ref.pronoun,
                DeicticSource::PronounResolver,
            )
            .with_confidence(scored_pronoun.confidence);

            if let Some(referent) = resolved {
                deictic = deictic.with_referent(referent);
            }

            results.push(sel.finish_with_attr(deictic));
        }

        // Map TemporalExpression → Time deixis
        for (sel, temporal) in selection.find_by(&x::attr::<TemporalExpression>()) {
            let subcategory = Self::map_temporal_type(&temporal.temporal_type);

            // Skip absolute dates - they're not truly deictic
            if matches!(temporal.temporal_type, TemporalType::Date { .. }) {
                continue;
            }

            results.push(sel.finish_with_attr(
                DeicticReference::new(
                    DeicticCategory::Time,
                    subcategory,
                    &temporal.text,
                    DeicticSource::TemporalResolver,
                )
                .with_confidence(temporal.confidence),
            ));
        }

        // Map SectionReference with RelativeReference → Discourse deixis
        for (sel, section_ref) in selection.find_by(&x::attr::<SectionReference>()) {
            // Only map relative references - direct references aren't deictic
            if let ReferenceType::Relative(rel_ref) = section_ref.reference_type {
                let subcategory = Self::map_relative_reference(rel_ref);

                results.push(sel.finish_with_attr(
                    DeicticReference::new(
                        DeicticCategory::Discourse,
                        subcategory,
                        &section_ref.reference_text,
                        DeicticSource::SectionReferenceResolver,
                    )
                    .with_confidence(section_ref.confidence),
                ));
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pronoun::AntecedentCandidate;
    use layered_nlp::{create_line_from_string, LLLineDisplay};

    // Helper to add a pronoun reference to a line for testing
    fn add_pronoun_ref(
        selection: LLSelection,
        pronoun: &str,
        pronoun_type: PronounType,
        antecedent: Option<(&str, f64)>,
    ) -> Vec<LLCursorAssignment<Scored<PronounReference>>> {
        selection
            .find_by(&x::token_text())
            .into_iter()
            .filter_map(|(sel, text)| {
                if text.to_lowercase() == pronoun.to_lowercase() {
                    let candidates = antecedent
                        .map(|(text, conf)| {
                            vec![AntecedentCandidate {
                                text: text.to_string(),
                                is_defined_term: true,
                                token_distance: 5,
                                confidence: conf,
                            }]
                        })
                        .unwrap_or_default();

                    Some(sel.finish_with_attr(Scored::rule_based(
                        PronounReference {
                            pronoun: text.to_string(),
                            pronoun_type,
                            candidates,
                        },
                        0.85,
                        "test",
                    )))
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn test_pronoun_mapping() {
        let line = create_line_from_string("The Company shall deliver. It must comply.");

        // Manually add a pronoun reference for "It"
        struct TestPronounResolver;
        impl Resolver for TestPronounResolver {
            type Attr = Scored<PronounReference>;
            fn go(&self, sel: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
                add_pronoun_ref(
                    sel,
                    "it",
                    PronounType::ThirdSingularNeuter,
                    Some(("Company", 0.9)),
                )
            }
        }

        let line = line.run(&TestPronounResolver).run(&DeicticResolver);

        let mut display = LLLineDisplay::new(&line);
        display.include::<DeicticReference>();

        insta::assert_snapshot!(display.to_string(), @r###"
        The     Company     shall     deliver  .     It     must     comply  .
                                                     ╰╯DeicticReference { category: Person, subcategory: PersonThirdSingular, surface_text: "It", resolved_referent: Some(ResolvedReferent { text: "Company", span: None, resolution_confidence: 0.9 }), confidence: 0.85, source: PronounResolver }
        "###);
    }

    #[test]
    fn test_temporal_mapping() {
        let line = create_line_from_string("Payment due within.");

        // Add a temporal expression for "within" (simplified - real resolver would span more)
        struct TestTemporalResolver;
        impl Resolver for TestTemporalResolver {
            type Attr = TemporalExpression;
            fn go(&self, sel: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
                sel.find_by(&x::token_text())
                    .into_iter()
                    .filter_map(|(sel, text)| {
                        if text.to_lowercase() == "within" {
                            Some(sel.finish_with_attr(TemporalExpression {
                                temporal_type: TemporalType::Deadline {
                                    deadline_type: crate::temporal::DeadlineType::Within,
                                    reference: Box::new(TemporalType::Duration {
                                        value: 30,
                                        unit: crate::temporal::DurationUnit::Days,
                                        written_form: None,
                                    }),
                                },
                                text: "within 30 days".to_string(),
                                confidence: 0.85,
                            }))
                        } else {
                            None
                        }
                    })
                    .collect()
            }
        }

        let line = line.run(&TestTemporalResolver).run(&DeicticResolver);

        let mut display = LLLineDisplay::new(&line);
        display.include::<DeicticReference>();

        insta::assert_snapshot!(display.to_string(), @r###"
        Payment     due     within  .
                            ╰────╯DeicticReference { category: Time, subcategory: TimeDeadline, surface_text: "within 30 days", resolved_referent: None, confidence: 0.85, source: TemporalResolver }
        "###);
    }

    #[test]
    fn test_section_reference_mapping() {
        let line = create_line_from_string("As defined herein.");

        // Add a section reference for "herein"
        struct TestSectionRefResolver;
        impl Resolver for TestSectionRefResolver {
            type Attr = SectionReference;
            fn go(&self, sel: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
                sel.find_by(&x::token_text())
                    .into_iter()
                    .filter_map(|(sel, text)| {
                        if text.to_lowercase() == "herein" {
                            Some(sel.finish_with_attr(SectionReference {
                                target: None,
                                reference_text: "herein".to_string(),
                                reference_type: ReferenceType::Relative(RelativeReference::Herein),
                                purpose: None,
                                confidence: 0.7,
                            }))
                        } else {
                            None
                        }
                    })
                    .collect()
            }
        }

        let line = line.run(&TestSectionRefResolver).run(&DeicticResolver);

        let mut display = LLLineDisplay::new(&line);
        display.include::<DeicticReference>();

        insta::assert_snapshot!(display.to_string(), @r###"
        As     defined     herein  .
                           ╰────╯DeicticReference { category: Discourse, subcategory: DiscourseThisDocument, surface_text: "herein", resolved_referent: None, confidence: 0.7, source: SectionReferenceResolver }
        "###);
    }
}
