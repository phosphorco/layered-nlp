use layered_nlp::{
    create_line_from_string, x, LLCursorAssignment, LLLine, LLLineDisplay, LLSelection, Resolver,
};
use layered_part_of_speech::POSTagResolver;

use crate::{
    ContractClause, ContractClauseResolver, ContractKeywordResolver, DefinedTermResolver,
    ObligationPhrase, ObligationPhraseResolver, PronounChain, PronounChainResolver,
    PronounResolver, ProhibitionResolver, Scored, TermReferenceResolver,
};

fn base_clause_pipeline(input: &str) -> LLLine {
    create_line_from_string(input)
        .run(&POSTagResolver::default())
        .run(&ContractKeywordResolver::default())
        .run(&ProhibitionResolver::default())
        .run(&DefinedTermResolver::default())
        .run(&TermReferenceResolver::default())
        .run(&PronounResolver::default())
        .run(&ObligationPhraseResolver::default())
        .run(&PronounChainResolver::default())
}

fn render_clause_line(ll_line: &LLLine, show_chains: bool) -> String {
    let mut display = LLLineDisplay::new(ll_line);
    if show_chains {
        display.include::<Scored<PronounChain>>();
    }
    display.include::<Scored<ObligationPhrase>>();
    display.include::<Scored<ContractClause>>();
    format!("{}", display)
}

fn test_clauses(input: &str) -> String {
    let ll_line = base_clause_pipeline(input).run(&ContractClauseResolver::default());
    render_clause_line(&ll_line, true)
}

fn test_clauses_with_verified_chain(input: &str) -> String {
    let ll_line = base_clause_pipeline(input)
        .run(&MarkChainsVerified)
        .run(&ContractClauseResolver::default());
    // Skip pronoun chain display to avoid duplicate verified overlays.
    render_clause_line(&ll_line, false)
}

struct MarkChainsVerified;

impl Resolver for MarkChainsVerified {
    type Attr = Scored<PronounChain>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        selection
            .find_by(&x::attr::<Scored<PronounChain>>())
            .into_iter()
            .map(|(sel, chain)| {
                let mut patched = chain.value.clone();
                patched.has_verified_mention = true;
                sel.finish_with_attr(Scored::verified(patched))
            })
            .collect()
    }
}

// ============ Basic Clause Formation ============

#[test]
fn clause_from_defined_term_obligation() {
    insta::assert_snapshot!(test_clauses(
        r#"ABC Corp (the "Company") shall deliver finished goods."#
    ));
}

#[test]
fn clause_with_known_condition_entity() {
    insta::assert_snapshot!(test_clauses(
        r#"ABC Corp (the "Company") shall deliver goods if the Company receives payment."#
    ));
}

#[test]
fn clause_condition_unknown_entity_penalty() {
    insta::assert_snapshot!(test_clauses(
        r#"ABC Corp (the "Company") shall deliver goods if Inspector approves."#
    ));
}

#[test]
fn clause_from_pronoun_obligor() {
    insta::assert_snapshot!(test_clauses(
        r#"ABC Corp (the "Company") exists. It shall deliver replacement parts."#
    ));
}

#[test]
fn multiple_clauses_ordered_by_offset() {
    insta::assert_snapshot!(test_clauses(
        r#"ABC Corp (the "Seller") shall deliver goods. XYZ Inc (the "Buyer") may inspect the goods."#
    ));
}

#[test]
fn clause_prohibition_shall_not() {
    insta::assert_snapshot!(test_clauses(
        r#"ABC Corp (the "Company") shall not disclose Confidential Information."#
    ));
}

#[test]
fn clause_empty_action_penalty() {
    insta::assert_snapshot!(test_clauses(
        r#"ABC Corp (the "Company") shall."#
    ));
}

#[test]
fn clause_subject_to_condition_allowedlist() {
    insta::assert_snapshot!(test_clauses(
        r#"ABC Corp (the "Company") shall pay the fee subject to Section 5."#
    ));
}

#[test]
fn clause_verified_chain_bonus() {
    insta::assert_snapshot!(test_clauses_with_verified_chain(
        r#"ABC Corp (the "Company") exists. It shall deliver replacement parts."#
    ));
}
