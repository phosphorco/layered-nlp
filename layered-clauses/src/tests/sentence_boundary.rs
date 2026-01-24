use crate::{SentenceBoundary, SentenceBoundaryResolver};
use layered_nlp::{create_line_from_input_tokens, InputToken, LLLine, LLLineDisplay};

fn test_setup(sentence: &'static str) -> LLLine {
    create_line_from_input_tokens(
        vec![InputToken::text(sentence.to_string(), Vec::new())],
        |text| text.encode_utf16().count(),
    )
}

#[test]
fn test_basic_sentences() {
    let ll_line = test_setup("Hello world. Goodbye.")
        .run(&SentenceBoundaryResolver::new());

    let mut ll_line_display = LLLineDisplay::new(&ll_line);
    ll_line_display.include::<SentenceBoundary>();

    insta::assert_snapshot!(ll_line_display, @r###"
    Hello     world  .     Goodbye  .
                     ╰SentenceBoundary(High)
                                    ╰SentenceBoundary(Medium)
    "###);
}

#[test]
fn test_abbreviations() {
    let ll_line = test_setup("Dr. Smith arrived.")
        .run(&SentenceBoundaryResolver::new());

    let mut ll_line_display = LLLineDisplay::new(&ll_line);
    ll_line_display.include::<SentenceBoundary>();

    // The period after "Dr" should NOT be marked as a boundary
    // Only the final period should be marked
    insta::assert_snapshot!(ll_line_display, @r###"
    Dr  .     Smith     arrived  .
                                 ╰SentenceBoundary(Medium)
    "###);
}

#[test]
fn test_questions() {
    let ll_line = test_setup("Why? Because.")
        .run(&SentenceBoundaryResolver::new());

    let mut ll_line_display = LLLineDisplay::new(&ll_line);
    ll_line_display.include::<SentenceBoundary>();

    insta::assert_snapshot!(ll_line_display, @r###"
    Why  ?     Because  .
         ╰SentenceBoundary(High)
                        ╰SentenceBoundary(Medium)
    "###);
}

#[test]
fn test_multiple_boundaries() {
    let ll_line = test_setup("First. Second. Third.")
        .run(&SentenceBoundaryResolver::new());

    let mut ll_line_display = LLLineDisplay::new(&ll_line);
    ll_line_display.include::<SentenceBoundary>();

    insta::assert_snapshot!(ll_line_display, @r###"
    First  .     Second  .     Third  .
           ╰SentenceBoundary(High)
                         ╰SentenceBoundary(High)
                                      ╰SentenceBoundary(Medium)
    "###);
}

#[test]
fn test_no_boundary() {
    let ll_line = test_setup("Inc. is a company")
        .run(&SentenceBoundaryResolver::new());

    let mut ll_line_display = LLLineDisplay::new(&ll_line);
    ll_line_display.include::<SentenceBoundary>();

    // No sentence boundaries should be detected (period is part of "Inc.")
    insta::assert_snapshot!(ll_line_display, @"Inc  .     is     a     company");
}

#[test]
fn test_exclamation() {
    let ll_line = test_setup("Stop! Go now.")
        .run(&SentenceBoundaryResolver::new());

    let mut ll_line_display = LLLineDisplay::new(&ll_line);
    ll_line_display.include::<SentenceBoundary>();

    insta::assert_snapshot!(ll_line_display, @r###"
    Stop  !     Go     now  .
          ╰SentenceBoundary(High)
                            ╰SentenceBoundary(Medium)
    "###);
}

#[test]
fn test_mixed_punctuation() {
    let ll_line = test_setup("What happened? Nothing. Everything is fine!")
        .run(&SentenceBoundaryResolver::new());

    let mut ll_line_display = LLLineDisplay::new(&ll_line);
    ll_line_display.include::<SentenceBoundary>();

    insta::assert_snapshot!(ll_line_display, @r###"
    What     happened  ?     Nothing  .     Everything     is     fine  !
                       ╰SentenceBoundary(High)
                                      ╰SentenceBoundary(High)
                                                                        ╰SentenceBoundary(Medium)
    "###);
}

#[test]
fn test_lowercase_after_period() {
    let ll_line = test_setup("End of sentence. then lowercase.")
        .run(&SentenceBoundaryResolver::new());

    let mut ll_line_display = LLLineDisplay::new(&ll_line);
    ll_line_display.include::<SentenceBoundary>();

    // First period followed by lowercase "then" should be Low confidence
    insta::assert_snapshot!(ll_line_display, @r###"
    End     of     sentence  .     then     lowercase  .
                             ╰SentenceBoundary(Low)
                                                       ╰SentenceBoundary(Medium)
    "###);
}

#[test]
fn test_custom_abbreviations() {
    let ll_line = test_setup("The Acme Corp. announced today.")
        .run(&SentenceBoundaryResolver::new().with_custom_abbreviations(&["corp"]));

    let mut ll_line_display = LLLineDisplay::new(&ll_line);
    ll_line_display.include::<SentenceBoundary>();

    // "Corp." should not be treated as boundary since we added it to abbreviations
    insta::assert_snapshot!(ll_line_display, @r###"
    The     Acme     Corp  .     announced     today  .
                                                      ╰SentenceBoundary(Medium)
    "###);
}

#[test]
fn test_multiple_abbreviations() {
    let ll_line = test_setup("Dr. Johnson and Mr. Smith met at 3 p.m. today.")
        .run(&SentenceBoundaryResolver::new());

    let mut ll_line_display = LLLineDisplay::new(&ll_line);
    ll_line_display.include::<SentenceBoundary>();

    // Only the final period should be detected as a sentence boundary
    insta::assert_snapshot!(ll_line_display, @r###"
    Dr  .     Johnson     and     Mr  .     Smith     met     at     3     p.m  .     today  .
                                                                                             ╰SentenceBoundary(Medium)
    "###);
}
