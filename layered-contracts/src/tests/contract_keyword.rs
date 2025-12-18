use layered_nlp::{create_line_from_string, LLLineDisplay};

use crate::{ContractKeyword, ContractKeywordResolver, ProhibitionResolver};

fn test_keywords(input: &str) -> String {
    let ll_line =
        create_line_from_string(input).run(&ContractKeywordResolver::default());

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<ContractKeyword>();

    format!("{}", display)
}

fn test_with_prohibition(input: &str) -> String {
    let ll_line = create_line_from_string(input)
        .run(&ContractKeywordResolver::default())
        .run(&ProhibitionResolver::default());

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<ContractKeyword>();

    format!("{}", display)
}

#[test]
fn basic_shall() {
    insta::assert_snapshot!(test_keywords("The Contractor shall deliver the goods"), @r###"
    The     Contractor     shall     deliver     the     goods
                           ╰───╯Shall
    "###);
}

#[test]
fn basic_may() {
    insta::assert_snapshot!(test_keywords("The Company may terminate this Agreement"), @r###"
    The     Company     may     terminate     this     Agreement
                        ╰─╯May
    "###);
}

#[test]
fn basic_must() {
    insta::assert_snapshot!(test_keywords("Contractor must provide notice"), @r###"
    Contractor     must     provide     notice
                   ╰──╯Shall
    "###);
}

#[test]
fn definition_means() {
    insta::assert_snapshot!(test_keywords("Company means ABC Corporation"), @r###"
    Company     means     ABC     Corporation
                ╰───╯Means
    "###);
}

#[test]
fn definition_includes() {
    insta::assert_snapshot!(test_keywords("Services includes consulting and support"), @r###"
    Services     includes     consulting     and     support
                 ╰──────╯Includes
    "###);
}

#[test]
fn definition_hereinafter() {
    insta::assert_snapshot!(test_keywords("ABC Corp hereinafter the Company"), @r###"
    ABC     Corp     hereinafter     the     Company
                     ╰─────────╯Hereinafter
    "###);
}

#[test]
fn conditional_if() {
    insta::assert_snapshot!(test_keywords("If Contractor fails to deliver"), @r###"
    If     Contractor     fails     to     deliver
    ╰╯If
    "###);
}

#[test]
fn conditional_when() {
    insta::assert_snapshot!(test_keywords("When the Agreement terminates"), @r###"
    When     the     Agreement     terminates
    ╰──╯If
    "###);
}

#[test]
fn conditional_unless() {
    insta::assert_snapshot!(test_keywords("unless otherwise agreed in writing"), @r###"
    unless     otherwise     agreed     in     writing
    ╰────╯Unless
    "###);
}

#[test]
fn conditional_provided() {
    insta::assert_snapshot!(test_keywords("provided that notice is given"), @r###"
    provided     that     notice     is     given
    ╰──────╯Provided
    "###);
}

#[test]
fn subject_to() {
    insta::assert_snapshot!(test_keywords("subject to the terms hereof"), @r###"
    subject     to     the     terms     hereof
    ╰────────────╯SubjectTo
    "###);
}

#[test]
fn party_reference() {
    insta::assert_snapshot!(test_keywords("Each party shall notify the other party"), @r###"
    Each     party     shall     notify     the     other     party
             ╰───╯Party
                       ╰───╯Shall
                                                              ╰───╯Party
    "###);
}

#[test]
fn multiple_keywords_in_sentence() {
    insta::assert_snapshot!(test_keywords("If the Contractor shall fail to deliver, the Company may terminate"), @r###"
    If     the     Contractor     shall     fail     to     deliver  ,     the     Company     may     terminate
    ╰╯If
                                  ╰───╯Shall
                                                                                               ╰─╯May
    "###);
}

#[test]
fn case_insensitive() {
    insta::assert_snapshot!(test_keywords("SHALL deliver and MAY terminate"), @r###"
    SHALL     deliver     and     MAY     terminate
    ╰───╯Shall
                                  ╰─╯May
    "###);
}

#[test]
fn no_keywords() {
    insta::assert_snapshot!(test_keywords("The quick brown fox jumps over the lazy dog"), @"The     quick     brown     fox     jumps     over     the     lazy     dog");
}

#[test]
fn subject_alone_not_matched() {
    // "subject" without "to" should not match
    insta::assert_snapshot!(test_keywords("The subject matter of this Agreement"), @"The     subject     matter     of     this     Agreement");
}

#[test]
fn keyword_at_start() {
    insta::assert_snapshot!(test_keywords("Shall deliver goods within 30 days"), @r###"
    Shall     deliver     goods     within     30     days
    ╰───╯Shall
    "###);
}

#[test]
fn keyword_at_end() {
    insta::assert_snapshot!(test_keywords("The obligations include"), @r###"
    The     obligations     include
                            ╰─────╯Includes
    "###);
}

#[test]
fn upon_conditional() {
    insta::assert_snapshot!(test_keywords("Upon termination of this Agreement"), @r###"
    Upon     termination     of     this     Agreement
    ╰──╯If
    "###);
}

// ============ ProhibitionResolver Tests ============

#[test]
fn shall_not_prohibition() {
    insta::assert_snapshot!(test_with_prohibition("Contractor shall not disclose any information"), @r###"
    Contractor     shall     not     disclose     any     information
                   ╰───╯Shall
                   ╰───────────╯ShallNot
    "###);
}

#[test]
fn must_not_prohibition() {
    insta::assert_snapshot!(test_with_prohibition("Company must not assign this Agreement"), @r###"
    Company     must     not     assign     this     Agreement
                ╰──╯Shall
                ╰──────────╯ShallNot
    "###);
}

#[test]
fn shall_without_not() {
    // "shall" alone should remain Shall, not get upgraded
    insta::assert_snapshot!(test_with_prohibition("Contractor shall deliver goods"), @r###"
    Contractor     shall     deliver     goods
                   ╰───╯Shall
    "###);
}

// ============ Complex Contract Sentence Tests ============

#[test]
fn complex_obligation_with_condition() {
    insta::assert_snapshot!(test_keywords(
        "If the Contractor fails to perform, the Company may terminate this Agreement provided that written notice is given"
    ), @r###"
    If     the     Contractor     fails     to     perform  ,     the     Company     may     terminate     this     Agreement     provided     that     written     notice     is     given
    ╰╯If
                                                                                      ╰─╯May
                                                                                                                                   ╰──────╯Provided
    "###);
}

#[test]
fn definition_clause() {
    insta::assert_snapshot!(test_keywords(
        "Services means all consulting and professional services including training"
    ), @r###"
    Services     means     all     consulting     and     professional     services     including     training
                 ╰───╯Means
                                                                                        ╰───────╯Includes
    "###);
}

#[test]
fn party_obligations() {
    insta::assert_snapshot!(test_keywords(
        "Each party shall indemnify the other party subject to the limitations herein"
    ), @r###"
    Each     party     shall     indemnify     the     other     party     subject     to     the     limitations     herein
             ╰───╯Party
                       ╰───╯Shall
                                                                 ╰───╯Party
                                                                           ╰────────────╯SubjectTo
    "###);
}
