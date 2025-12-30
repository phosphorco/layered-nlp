use layered_nlp::{create_line_from_string, x};
use layered_part_of_speech::POSTagResolver;
use layered_contracts::{
    ContractKeywordResolver, DefinedTermResolver, PronounChainResolver,
    PronounResolver, Scored, TermReferenceResolver, DefinedTerm, TermReference,
    PronounChain,
};

fn main() {
    let ll_line = create_line_from_string(r#"ABC Corp (the "Company") exists. The Company shall deliver."#)
        .run(&POSTagResolver::default())
        .run(&ContractKeywordResolver::default())
        .run(&DefinedTermResolver::default())
        .run(&TermReferenceResolver::default())
        .run(&PronounResolver::default())
        .run(&PronounChainResolver::default());

    // Check what defined terms we have
    let defined_terms = ll_line.find(&x::attr::<Scored<DefinedTerm>>());
    println!("Defined terms found: {}", defined_terms.len());
    for dt in &defined_terms {
        println!("  - {:?}", dt);
    }

    // Check what term references we have
    let term_refs = ll_line.find(&x::attr::<Scored<TermReference>>());
    println!("\nTerm references found: {}", term_refs.len());
    for tr in &term_refs {
        println!("  - {:?}", tr);
    }

    // Check what pronoun chains we have
    let chains = ll_line.find(&x::attr::<Scored<PronounChain>>());
    println!("\nPronoun chains found: {}", chains.len());
    for chain in &chains {
        println!("  - {:?}", chain);
    }
}
