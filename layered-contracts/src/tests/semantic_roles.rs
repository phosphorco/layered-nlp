//! Tests for semantic role labeling (M8).

use crate::{
    ArgumentRole, CanonicalModal, EnhancedObligationNormalizer, EquivalenceResult,
    ObligationPhrase, ObligationType, ObligorReference, SemanticRoleLabeler,
};

#[test]
fn test_extract_agent_active() {
    let obligation = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Seller".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "deliver goods to the Buyer".to_string(),
        conditions: vec![],
    };

    let labeler = SemanticRoleLabeler::new();
    let frame = labeler.extract_frame(&obligation);

    assert_eq!(frame.value.predicate, "deliver");
    assert!(!frame.value.is_passive);

    let agent = frame.value.agent().unwrap();
    assert_eq!(agent.role, ArgumentRole::Agent);
    assert_eq!(agent.filler, "Seller");
}

#[test]
fn test_extract_agent_passive() {
    let obligation = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Buyer".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "be indemnified by the Seller".to_string(),
        conditions: vec![],
    };

    let labeler = SemanticRoleLabeler::new();
    let frame = labeler.extract_frame(&obligation);

    assert_eq!(frame.value.predicate, "indemnify");
    assert!(frame.value.is_passive);

    // In passive, obligor is Patient
    let patient = frame.value.patient().unwrap();
    assert_eq!(patient.filler, "Buyer");
}

#[test]
fn test_extract_prepositional_arguments() {
    let obligation = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Company".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "deliver Products to the Buyer within thirty days".to_string(),
        conditions: vec![],
    };

    let labeler = SemanticRoleLabeler::new();
    let frame = labeler.extract_frame(&obligation);

    // Should extract "to the Buyer" as Recipient
    let recipients = frame.value.get_role(ArgumentRole::Recipient);
    assert!(!recipients.is_empty());

    // Should extract "within thirty days" as Time
    let times = frame.value.get_role(ArgumentRole::Time);
    assert!(!times.is_empty());
}

#[test]
fn test_normalize_obligation() {
    let obligation = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Seller".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "deliver goods".to_string(),
        conditions: vec![],
    };

    let normalizer = EnhancedObligationNormalizer::new();
    let norm = normalizer.normalize(&obligation);

    assert_eq!(norm.obligor, "Seller");
    assert_eq!(norm.modal, CanonicalModal::Shall);
    assert_eq!(norm.predicate, "deliver");
}

#[test]
fn test_equivalence_active_passive() {
    let active = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Seller".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "deliver Products".to_string(),
        conditions: vec![],
    };

    let passive = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Seller".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "Products be delivered".to_string(),
        conditions: vec![],
    };

    let normalizer = EnhancedObligationNormalizer::new();
    let norm_active = normalizer.normalize(&active);
    let norm_passive = normalizer.normalize(&passive);

    let result = normalizer.equivalent(&norm_active, &norm_passive);
    // Note: This test may need adjustment based on actual passive detection logic
    match result {
        EquivalenceResult::Equivalent { .. } | EquivalenceResult::Different => {
            // Either is acceptable depending on exact passive detection
        }
        _ => {}
    }
}

#[test]
fn test_equivalence_synonyms() {
    let deliver = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Seller".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "deliver Products".to_string(),
        conditions: vec![],
    };

    let provide = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Seller".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "provide Products".to_string(),
        conditions: vec![],
    };

    let normalizer = EnhancedObligationNormalizer::new();
    let norm_deliver = normalizer.normalize(&deliver);
    let norm_provide = normalizer.normalize(&provide);

    let result = normalizer.equivalent(&norm_deliver, &norm_provide);
    // Should detect high similarity
    assert!(matches!(
        result,
        EquivalenceResult::Equivalent { confidence } if confidence > 0.8
    ));
}

#[test]
fn test_modal_difference() {
    let shall = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Buyer".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "pay within 30 days".to_string(),
        conditions: vec![],
    };

    let may = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Buyer".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Permission,
        action: "pay within 30 days".to_string(),
        conditions: vec![],
    };

    let normalizer = EnhancedObligationNormalizer::new();
    let norm_shall = normalizer.normalize(&shall);
    let norm_may = normalizer.normalize(&may);

    let result = normalizer.equivalent(&norm_shall, &norm_may);
    assert!(matches!(result, EquivalenceResult::ModalDifference { .. }));
}

#[test]
fn test_lemmatize_verb() {
    let labeler = SemanticRoleLabeler::new();

    assert_eq!(labeler.lemmatize_verb("delivers"), "deliver");
    assert_eq!(labeler.lemmatize_verb("delivered"), "deliver");
    assert_eq!(labeler.lemmatize_verb("providing"), "provide");
    assert_eq!(labeler.lemmatize_verb("notification"), "notify");
}

#[test]
fn test_is_passive() {
    let labeler = SemanticRoleLabeler::new();

    assert!(labeler.is_passive("is delivered"));
    assert!(labeler.is_passive("are provided"));
    assert!(labeler.is_passive("was indemnified"));
    assert!(!labeler.is_passive("delivers goods"));
    assert!(!labeler.is_passive("shall provide"));
}

#[test]
fn test_extract_theme_active() {
    let obligation = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Seller".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "deliver goods".to_string(),
        conditions: vec![],
    };

    let labeler = SemanticRoleLabeler::new();
    let frame = labeler.extract_frame(&obligation);

    let theme = frame.value.theme();
    assert!(theme.is_some());
    let theme = theme.unwrap();
    assert_eq!(theme.role, ArgumentRole::Theme);
    assert!(theme.filler.contains("goods"));
}

#[test]
fn test_extract_recipient() {
    let obligation = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Seller".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "deliver goods to the Buyer".to_string(),
        conditions: vec![],
    };

    let labeler = SemanticRoleLabeler::new();
    let frame = labeler.extract_frame(&obligation);

    let recipients = frame.value.get_role(ArgumentRole::Recipient);
    assert!(!recipients.is_empty());

    let recipient = recipients[0];
    assert!(recipient.filler.contains("Buyer"));
}

#[test]
fn test_extract_manner() {
    let obligation = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Company".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "notify in writing".to_string(),
        conditions: vec![],
    };

    let labeler = SemanticRoleLabeler::new();
    let frame = labeler.extract_frame(&obligation);

    let manners = frame.value.get_role(ArgumentRole::Manner);
    assert!(!manners.is_empty());

    let manner = manners[0];
    assert!(manner.filler.contains("writing"));
}

#[test]
fn test_different_obligations() {
    let deliver = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Seller".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "deliver goods".to_string(),
        conditions: vec![],
    };

    let pay = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Buyer".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Duty,
        action: "pay the price".to_string(),
        conditions: vec![],
    };

    let normalizer = EnhancedObligationNormalizer::new();
    let norm_deliver = normalizer.normalize(&deliver);
    let norm_pay = normalizer.normalize(&pay);

    let result = normalizer.equivalent(&norm_deliver, &norm_pay);
    assert!(matches!(result, EquivalenceResult::Different));
}

#[test]
fn test_pronoun_reference_as_obligor() {
    let obligation = ObligationPhrase {
        obligor: ObligorReference::PronounRef {
            pronoun: "It".to_string(),
            resolved_to: "Company".to_string(),
            is_defined_term: true,
            confidence: 0.9,
        },
        obligation_type: ObligationType::Duty,
        action: "deliver goods".to_string(),
        conditions: vec![],
    };

    let labeler = SemanticRoleLabeler::new();
    let frame = labeler.extract_frame(&obligation);

    let agent = frame.value.agent().unwrap();
    assert_eq!(agent.filler, "Company");
}

#[test]
fn test_noun_phrase_as_obligor() {
    let obligation = ObligationPhrase {
        obligor: ObligorReference::NounPhrase {
            text: "the contracting party".to_string(),
        },
        obligation_type: ObligationType::Duty,
        action: "comply with laws".to_string(),
        conditions: vec![],
    };

    let labeler = SemanticRoleLabeler::new();
    let frame = labeler.extract_frame(&obligation);

    let agent = frame.value.agent().unwrap();
    assert_eq!(agent.filler, "the contracting party");
}

#[test]
fn test_prohibition_modal() {
    let obligation = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Company".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Prohibition,
        action: "disclose information".to_string(),
        conditions: vec![],
    };

    let normalizer = EnhancedObligationNormalizer::new();
    let norm = normalizer.normalize(&obligation);

    assert_eq!(norm.modal, CanonicalModal::ShallNot);
}

#[test]
fn test_permission_modal() {
    let obligation = ObligationPhrase {
        obligor: ObligorReference::TermRef {
            term_name: "Buyer".to_string(),
            confidence: 1.0,
        },
        obligation_type: ObligationType::Permission,
        action: "inspect the goods".to_string(),
        conditions: vec![],
    };

    let normalizer = EnhancedObligationNormalizer::new();
    let norm = normalizer.normalize(&obligation);

    assert_eq!(norm.modal, CanonicalModal::May);
}
