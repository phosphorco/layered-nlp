use layered_nlp::{create_line_from_string, LLLineDisplay};
use layered_part_of_speech::POSTagResolver;

use crate::{
    ContractKeywordResolver, DefinedTerm, DefinedTermResolver, PronounChain, PronounChainResolver,
    PronounReference, PronounResolver, Scored, TermReference, TermReferenceResolver,
};

fn test_chains(input: &str) -> String {
    let ll_line = create_line_from_string(input)
        .run(&POSTagResolver::default())
        .run(&ContractKeywordResolver::default())
        .run(&DefinedTermResolver::default())
        .run(&TermReferenceResolver::default())
        .run(&PronounResolver::default())
        .run(&PronounChainResolver::default());

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<Scored<DefinedTerm>>();
    display.include::<Scored<TermReference>>();
    display.include::<Scored<PronounReference>>();
    display.include::<Scored<PronounChain>>();

    format!("{}", display)
}

// ============ Basic Chain Formation ============

#[test]
fn chain_definition_and_reference() {
    // Basic chain: defined term + term reference
    insta::assert_snapshot!(test_chains(
        r#"ABC Corp (the "Company") exists. The Company shall deliver."#
    ));
}

#[test]
fn chain_definition_and_pronoun() {
    // Chain: defined term + pronoun
    insta::assert_snapshot!(test_chains(
        r#"ABC Corp (the "Company") exists. It shall deliver."#
    ));
}

#[test]
fn chain_definition_reference_and_pronoun() {
    // Full chain: definition + reference + pronoun
    insta::assert_snapshot!(test_chains(
        r#"ABC Corp (the "Company") exists. The Company agrees. It shall deliver."#
    ));
}

// ============ Multiple Chains ============

#[test]
fn multiple_chains_separate_entities() {
    // Two separate chains for different entities
    insta::assert_snapshot!(test_chains(
        r#"ABC Corp (the "Seller") and XYZ Inc (the "Buyer") agree. The Seller delivers. The Buyer pays."#
    ));
}

#[test]
fn multiple_chains_with_pronouns() {
    // Multiple chains, each with pronoun references
    insta::assert_snapshot!(test_chains(
        r#"ABC Corp (the "Company") exists. XYZ Inc (the "Vendor") exists. The Company delivers. It agrees."#
    ));
}

// ============ Pronoun Resolution ============

#[test]
fn pronoun_attaches_to_nearest_antecedent() {
    // Pronoun should attach to most recent/relevant antecedent
    insta::assert_snapshot!(test_chains(
        r#"ABC Corp (the "Company") exists. It shall deliver goods."#
    ));
}

#[test]
fn pronoun_with_competing_antecedents() {
    // Multiple possible antecedents - pronoun should pick best candidate
    insta::assert_snapshot!(test_chains(
        r#"ABC Corp (the "Seller") sold to XYZ Inc (the "Buyer"). It shall deliver."#
    ));
}

#[test]
fn plural_pronoun_resolution() {
    // "they" should resolve to plural entities
    insta::assert_snapshot!(test_chains(
        r#"ABC Corp (the "Company") and its affiliates exist. They shall comply."#
    ));
}

// ============ Chain Without Pronouns ============

#[test]
fn chain_only_references_no_pronouns() {
    // Chain formed only from definition + references (no pronouns)
    insta::assert_snapshot!(test_chains(
        r#"ABC Corp (the "Company") exists. The Company shall deliver. The Company shall pay."#
    ));
}

// ============ Edge Cases ============

#[test]
fn single_mention_no_chain() {
    // Single mention should not form a chain (needs at least 2)
    insta::assert_snapshot!(test_chains(
        r#"ABC Corp (the "Company") exists."#
    ));
}

#[test]
fn pronoun_without_matching_chain() {
    // Pronoun with no matching defined term chain
    insta::assert_snapshot!(test_chains(
        r#"The vendor exists. It shall deliver."#
    ));
}

#[test]
fn reference_without_definition() {
    // Term reference without a formal definition still forms chain
    insta::assert_snapshot!(test_chains(
        r#"The Company shall deliver. The Company shall pay."#
    ));
}

// ============ Real Contract Examples ============

#[test]
fn contract_delivery_clause_chain() {
    insta::assert_snapshot!(test_chains(
        r#"ABC Corporation (the "Seller") agrees to sell. The Seller shall deliver Products. It warrants quality."#
    ));
}

#[test]
fn contract_confidentiality_chain() {
    insta::assert_snapshot!(test_chains(
        r#"XYZ Inc (the "Receiving Party") acknowledges receipt. The Receiving Party shall protect information. It shall not disclose."#
    ));
}

#[test]
fn contract_multiple_parties_chain() {
    insta::assert_snapshot!(test_chains(
        r#"ABC Corp (the "Licensor") and XYZ Inc (the "Licensee") enter this Agreement. The Licensor grants rights. The Licensee shall pay royalties. It shall report usage."#
    ));
}
