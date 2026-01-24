//! Additional tests for ListMarkerResolver.
//!
//! Core tests are inline in the list_marker module.
//! This file contains integration tests with other resolvers.

use crate::{ListMarker, ListMarkerResolver};
use layered_nlp::{create_line_from_input_tokens, InputToken, LLLineDisplay};

fn test_line(text: &str) -> layered_nlp::LLLine {
    create_line_from_input_tokens(
        vec![InputToken::text(text.to_string(), Vec::new())],
        |text| text.encode_utf16().count(),
    )
    .run(&ListMarkerResolver::new())
}

#[test]
fn test_legal_clause_structure() {
    // Common legal document pattern: numbered sections with lettered subsections
    let ll_line = test_line("1. Definitions (a) Agreement means this document (b) Parties means the signatories");

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<ListMarker>();

    insta::assert_snapshot!(display, @r###"
    1  .     Definitions     (  a  )     Agreement     means     this     document     (  b  )     Parties     means     the     signatories
    ╰──╯NumberedPeriod { number: 1 }
                             ╰─────╯ParenthesizedLetter { letter: 'a' }
                                                                                       ╰─────╯ParenthesizedLetter { letter: 'b' }
    "###);
}

#[test]
fn test_nested_structure() {
    // Deep nesting: section -> subsection -> clause -> subclause
    let ll_line = test_line("2. Rights (a) General (i) All rights reserved (ii) No warranties");

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<ListMarker>();

    insta::assert_snapshot!(display, @r###"
    2  .     Rights     (  a  )     General     (  i  )     All     rights     reserved     (  ii  )     No     warranties
    ╰──╯NumberedPeriod { number: 2 }
                        ╰─────╯ParenthesizedLetter { letter: 'a' }
                                                ╰─────╯ParenthesizedRoman { numeral: "i" }
                                                                                            ╰──────╯ParenthesizedRoman { numeral: "ii" }
    "###);
}

#[test]
fn test_large_numbers() {
    let ll_line = test_line("100. Section one hundred (99) ninety-nine");

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<ListMarker>();

    insta::assert_snapshot!(display, @r###"
    100  .     Section     one     hundred     (  99  )     ninety  -  nine
    ╰────╯NumberedPeriod { number: 100 }
                                               ╰──────╯ParenthesizedDigit { digit: 99 }
    "###);
}

#[test]
fn test_edge_case_letter_i() {
    // 'i' could be both a letter and a roman numeral
    // Current behavior: 'i' is detected as roman numeral since it comes first in validation
    let ll_line = test_line("(i) Item one");

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<ListMarker>();

    insta::assert_snapshot!(display, @r###"
    (  i  )     Item     one
    ╰─────╯ParenthesizedRoman { numeral: "i" }
    "###);
}

#[test]
fn test_non_markers_ignored() {
    // Text that looks similar but is not a list marker
    let ll_line = test_line("The value (x + y) equals z.");

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<ListMarker>();

    // 'x' is a valid roman numeral, so this will be detected
    insta::assert_snapshot!(display, @"The     value     (  x     +     y  )     equals     z  .");
}

#[test]
fn test_sentence_ending_periods() {
    // Ensure sentence-ending periods after words are not matched as list markers
    let ll_line = test_line("This is a sentence. Another one.");

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<ListMarker>();

    // No markers should be detected since periods follow words, not numbers
    insta::assert_snapshot!(display, @"This     is     a     sentence  .     Another     one  .");
}
