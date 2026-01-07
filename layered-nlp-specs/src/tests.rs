use crate::{parse_fixture, NlpFixture};
use std::fs;
use std::path::Path;

/// Load and parse a fixture file from the fixtures directory.
fn load_fixture(name: &str) -> NlpFixture {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    let content = fs::read_to_string(&path)
        .expect(&format!("Failed to read fixture: {}", name));
    parse_fixture(&content).expect(&format!("Failed to parse fixture: {}", name))
}

#[test]
fn test_simple_obligation_fixture() {
    let fixture = load_fixture("simple-obligation.nlp");

    assert_eq!(fixture.title.as_deref(), Some("Test: Simple Obligation Detection"));
    assert_eq!(fixture.spans().len(), 1);
    assert_eq!(fixture.spans()[0].text, "shall pay");
    assert_eq!(fixture.assertions.len(), 1);
    assert_eq!(fixture.assertions[0].span_type, "Obligation");
}

#[test]
fn test_defined_term_fixture() {
    let fixture = load_fixture("defined-term-single.nlp");

    assert_eq!(fixture.title.as_deref(), Some("Test: Defined Term Detection"));
    assert_eq!(fixture.spans().len(), 1);
    // Note: the text includes the quotes
    assert!(fixture.spans()[0].text.contains("Tenant"));
    assert_eq!(fixture.assertions[0].span_type, "DefinedTerm");
}

#[test]
fn test_permission_fixture() {
    let fixture = load_fixture("permission-simple.nlp");

    assert_eq!(fixture.spans()[0].text, "may enter");
    assert_eq!(fixture.assertions[0].body.field_checks[0].field, "modal");
    assert_eq!(fixture.assertions[0].body.field_checks[0].expected, "may");
}

// ============================================================================
// Gate 1b: Multi-Paragraph Fixtures
// ============================================================================

#[test]
fn test_multi_paragraph_obligation() {
    let fixture = load_fixture("multi-paragraph-obligation.nlp");

    // Should have 2 paragraphs separated by ---
    assert_eq!(fixture.paragraphs.len(), 2);

    // First paragraph defines T (The Tenant)
    assert_eq!(fixture.paragraphs[0].text, "The Tenant agrees to the following terms.");
    assert_eq!(fixture.paragraphs[0].spans.len(), 1);
    assert_eq!(fixture.paragraphs[0].spans[0].text, "The Tenant");

    // Second paragraph has the obligation and defines L (the Landlord)
    assert!(fixture.paragraphs[1].text.contains("shall pay"));
    assert!(fixture.paragraphs[1].text.contains("the Landlord"));

    // Entities: T and L
    assert_eq!(fixture.entities.len(), 2);
    assert_eq!(fixture.entity_by_id("T").unwrap().text, "The Tenant");
    assert_eq!(fixture.entity_by_id("L").unwrap().text, "the Landlord");

    // Assertions
    assert_eq!(fixture.assertions.len(), 1);
    assert_eq!(fixture.assertions[0].span_type, "Obligation");

    // Check bearer=§T field
    let bearer_check = fixture.assertions[0]
        .body
        .field_checks
        .iter()
        .find(|fc| fc.field == "bearer")
        .expect("should have bearer field");
    assert_eq!(bearer_check.expected, "§T");
}

#[test]
fn test_pronoun_reference() {
    let fixture = load_fixture("pronoun-reference.nlp");

    // 2 paragraphs
    assert_eq!(fixture.paragraphs.len(), 2);

    // First paragraph defines entity T
    assert_eq!(fixture.entity_by_id("T").unwrap().text, "The Tenant");

    // Second paragraph has pronoun "It" and obligation "shall provide"
    assert_eq!(fixture.paragraphs[1].spans.len(), 2);
    assert_eq!(fixture.paragraphs[1].spans[0].text, "It");
    assert_eq!(fixture.paragraphs[1].spans[1].text, "shall provide");

    // Two assertions
    assert_eq!(fixture.assertions.len(), 2);

    // First assertion: PronounReference
    assert_eq!(fixture.assertions[0].span_type, "PronounReference");
    let pronoun_type_check = fixture.assertions[0]
        .body
        .field_checks
        .iter()
        .find(|fc| fc.field == "pronoun_type")
        .expect("should have pronoun_type");
    assert_eq!(pronoun_type_check.expected, "ThirdSingularNeuter");

    // Second assertion: Obligation
    assert_eq!(fixture.assertions[1].span_type, "Obligation");
}

#[test]
fn test_defined_term_complex() {
    use crate::fixture::RefTarget;

    let fixture = load_fixture("defined-term-complex.nlp");

    // 2 paragraphs
    assert_eq!(fixture.paragraphs.len(), 2);

    // First paragraph defines entity P (Property)
    assert_eq!(fixture.entity_by_id("P").unwrap().text, "Property");
    assert!(fixture.paragraphs[0].text.contains("means the residential unit"));

    // Second paragraph references Property with numeric span
    let (para, span) = fixture.span_by_numeric_id(1).unwrap();
    assert_eq!(para.index, 1);
    assert_eq!(span.text, "Property");

    // Two assertions
    assert_eq!(fixture.assertions.len(), 2);

    // First assertion: text reference to "Property"
    assert!(matches!(
        &fixture.assertions[0].target,
        RefTarget::TextRef { text, occurrence } if text == "Property" && *occurrence == 0
    ));
    assert_eq!(fixture.assertions[0].span_type, "DefinedTerm");

    // Second assertion: numeric span reference
    assert_eq!(fixture.assertions[1].target, RefTarget::Span(1));
}

#[test]
fn test_full_contract_section() {
    let fixture = load_fixture("full-contract-section.nlp");

    // 3 paragraphs
    assert_eq!(fixture.paragraphs.len(), 3);

    // First paragraph: T and L entities
    assert_eq!(fixture.entity_by_id("T").unwrap().text, "The Tenant");
    assert_eq!(fixture.entity_by_id("L").unwrap().text, "The Landlord");

    // Second paragraph: obligation spans + rent entity + L reference
    // Spans: 1 (shall pay), rent ($2,000), 2 (shall be made), 3 (the Landlord)
    assert_eq!(fixture.paragraphs[1].spans.len(), 4);

    // Entity "rent" should be defined
    assert_eq!(fixture.entity_by_id("rent").unwrap().text, "$2,000");

    // Third paragraph: pronoun and termination
    assert_eq!(fixture.paragraphs[2].spans.len(), 4);

    // All 6 assertions
    assert_eq!(fixture.assertions.len(), 6);

    // Verify span types
    let span_types: Vec<&str> = fixture.assertions.iter().map(|a| a.span_type.as_str()).collect();
    assert_eq!(
        span_types,
        vec![
            "Obligation",
            "Obligation",
            "PronounReference",
            "PronounReference",
            "Obligation",
            "Obligation"
        ]
    );

    // Check modal values
    let modals: Vec<&str> = fixture
        .assertions
        .iter()
        .filter_map(|a| {
            a.body
                .field_checks
                .iter()
                .find(|fc| fc.field == "modal")
                .map(|fc| fc.expected.as_str())
        })
        .collect();
    assert_eq!(modals, vec!["shall", "shall", "fails", "may"]);
}
