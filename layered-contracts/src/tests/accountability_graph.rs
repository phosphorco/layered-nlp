use layered_nlp::{create_line_from_string, x, LLCursorAssignment, LLLine, LLLineDisplay, LLSelection, Resolver};
use layered_part_of_speech::POSTagResolver;

use crate::{
    AccountabilityGraphResolver, ClauseAggregate, ClauseAggregationResolver, ContractClause,
    ContractClauseResolver, ContractKeywordResolver, DefinedTermResolver, ObligationNode,
    ObligationPhraseResolver, PronounChain, PronounChainResolver, PronounResolver,
    ProhibitionResolver, Scored, TermReferenceResolver,
};

fn base_graph_line(input: &str) -> LLLine {
    create_line_from_string(input)
        .run(&POSTagResolver::default())
        .run(&ContractKeywordResolver::default())
        .run(&ProhibitionResolver::default())
        .run(&DefinedTermResolver::default())
        .run(&TermReferenceResolver::default())
        .run(&PronounResolver::default())
        .run(&ObligationPhraseResolver::default())
        .run(&PronounChainResolver::default())
        .run(&ContractClauseResolver::default())
        .run(&ClauseAggregationResolver::default())
}

fn display_graph(ll_line: &LLLine, include_chains: bool) -> String {
    let mut display = LLLineDisplay::new(ll_line);
    if include_chains {
        display.include::<Scored<PronounChain>>();
    }
    display.include::<Scored<ContractClause>>();
    display.include::<Scored<ClauseAggregate>>();
    display.include::<Scored<ObligationNode>>();
    format!("{}", display)
}

fn test_graph(input: &str) -> String {
    let ll_line = base_graph_line(input).run(&AccountabilityGraphResolver::default());
    display_graph(&ll_line, true)
}

fn test_graph_with_verified(input: &str) -> String {
    let ll_line = base_graph_line(input)
        .run(&MarkChainsVerified)
        .run(&AccountabilityGraphResolver::default());
    display_graph(&ll_line, false)
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

// ============ Beneficiary Detection ============

#[test]
fn graph_beneficiary_link() {
    insta::assert_snapshot!(test_graph(
        r#"XYZ Inc (the "Buyer") exists. ABC Corp (the "Seller") shall deliver goods to the Buyer."#
    ));
}

#[test]
fn graph_beneficiary_needs_verification() {
    insta::assert_snapshot!(test_graph(
        r#"The Vendor shall deliver goods to Regional Authority."#
    ));
}

// ============ Condition Edges ============

#[test]
fn graph_condition_links() {
    insta::assert_snapshot!(test_graph(
        r#"ABC Corp (the "Company") shall deliver goods if the Buyer provides written notice."#
    ));
}

// ============ Confidence Adjustments ============

#[test]
fn graph_verified_beneficiary_bonus() {
    insta::assert_snapshot!(test_graph_with_verified(
        r#"XYZ Inc (the "Buyer") exists. ABC Corp (the "Seller") shall deliver goods to the Buyer."#
    ));
}
