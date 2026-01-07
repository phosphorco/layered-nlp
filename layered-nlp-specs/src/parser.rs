//! Parser for `.nlp` fixture files.

use crate::errors::{SpecError, SpecResult};
use crate::fixture::{
    Assertion, AssertionBody, CompareOp, EntityDef, FieldCheck, MarkerId, NlpFixture, Paragraph,
    RefTarget, SpanMarker,
};

/// Parse span markers from text, returning spans, entities, and normalized text.
///
/// Input text may contain «ID:text» markers. This function:
/// - Extracts all span markers with their IDs (numeric or named) and text
/// - Computes character ranges in the normalized (marker-free) text
/// - Returns the normalized text with markers removed
/// - Separates numeric markers (spans) from named markers (entities)
pub fn parse_spans(
    input: &str,
    paragraph_idx: usize,
) -> SpecResult<(String, Vec<SpanMarker>, Vec<EntityDef>)> {
    let mut spans = Vec::new();
    let mut entities = Vec::new();
    let mut normalized = String::new();
    let mut chars = input.char_indices().peekable();

    while let Some((pos, ch)) = chars.next() {
        if ch == '«' {
            // Start of span marker
            let start_normalized = normalized.len();

            // Collect until we find the colon (ID part)
            // ID can be numeric (1, 2) or named (T, Landlord)
            let mut id_str = String::new();
            loop {
                match chars.next() {
                    Some((_, c)) if c == ':' => break,
                    Some((_, c)) if c.is_alphanumeric() || c == '_' => id_str.push(c),
                    Some((_, c)) => {
                        return Err(SpecError::Parse {
                            line: count_lines(input, pos),
                            message: format!(
                                "Invalid span marker: expected alphanumeric or ':', found '{}'",
                                c
                            ),
                        });
                    }
                    None => {
                        return Err(SpecError::Parse {
                            line: count_lines(input, pos),
                            message: "Unclosed span marker: expected ':'".to_string(),
                        });
                    }
                }
            }

            // Parse the ID - numeric or named
            let marker_id = if id_str.chars().all(|c| c.is_ascii_digit()) {
                let id: usize = id_str.parse().map_err(|_| SpecError::Parse {
                    line: count_lines(input, pos),
                    message: format!("Invalid span ID: '{}'", id_str),
                })?;
                MarkerId::Numeric(id)
            } else {
                MarkerId::Named(id_str.clone())
            };

            // Collect the text until »
            let mut text = String::new();
            loop {
                match chars.next() {
                    Some((_, '»')) => break,
                    Some((_, c)) => {
                        text.push(c);
                        normalized.push(c);
                    }
                    None => {
                        return Err(SpecError::Parse {
                            line: count_lines(input, pos),
                            message: "Unclosed span marker: expected '»'".to_string(),
                        });
                    }
                }
            }

            let end_normalized = normalized.len();
            let char_range = start_normalized..end_normalized;

            // Create span marker
            spans.push(SpanMarker {
                id: marker_id.clone(),
                text: text.clone(),
                char_range: char_range.clone(),
            });

            // Named markers also create entity definitions
            if let MarkerId::Named(ref name) = marker_id {
                entities.push(EntityDef {
                    id: name.clone(),
                    text,
                    paragraph_idx,
                    char_range,
                });
            }
        } else {
            normalized.push(ch);
        }
    }

    Ok((normalized, spans, entities))
}

/// Count lines up to a byte position (for error messages).
fn count_lines(input: &str, byte_pos: usize) -> usize {
    input[..byte_pos.min(input.len())]
        .chars()
        .filter(|&c| c == '\n')
        .count()
        + 1
}

/// Parse a full `.nlp` fixture file.
pub fn parse_fixture(input: &str) -> SpecResult<NlpFixture> {
    let mut title = None;
    let mut paragraph_lines: Vec<Vec<&str>> = vec![Vec::new()];
    let mut assertion_lines = Vec::new();

    for (line_num, line) in input.lines().enumerate() {
        let trimmed = line.trim();

        // Parse title from # header
        if trimmed.starts_with("# ") && title.is_none() {
            title = Some(trimmed[2..].trim().to_string());
        }
        // Paragraph separator
        else if trimmed == "---" {
            paragraph_lines.push(Vec::new());
        }
        // Parse assertions from > blockquotes
        else if trimmed.starts_with("> ") {
            assertion_lines.push((line_num + 1, trimmed[2..].trim()));
        }
        // Skip empty lines and comments
        else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            // Accumulate non-assertion content
            if let Some(last) = paragraph_lines.last_mut() {
                last.push(line);
            }
        }
    }

    // Parse each paragraph
    let mut paragraphs = Vec::new();
    let mut all_entities = Vec::new();

    for (idx, lines) in paragraph_lines.into_iter().enumerate() {
        if lines.is_empty() {
            continue;
        }

        let content = lines.join("\n");
        let (text, spans, entities) = parse_spans(&content, idx)?;

        paragraphs.push(Paragraph {
            index: idx,
            text,
            spans,
        });

        all_entities.extend(entities);
    }

    // Parse assertions
    let mut assertions = Vec::new();
    for (line_num, assertion_text) in assertion_lines {
        assertions.push(parse_assertion(assertion_text, line_num)?);
    }

    Ok(NlpFixture {
        title,
        paragraphs,
        entities: all_entities,
        assertions,
    })
}

/// Parse a single assertion line: [n]: Type(body) or §Entity: Type(body)
fn parse_assertion(input: &str, source_line: usize) -> SpecResult<Assertion> {
    let input = input.trim();

    // Check for entity reference §ID
    let target = if input.starts_with('§') {
        // Parse §EntityId: ...
        // Note: § is multi-byte UTF-8, so we need to skip its full byte length
        let after_section = &input['§'.len_utf8()..];
        let colon_pos = after_section.find(':').ok_or_else(|| SpecError::Parse {
            line: source_line,
            message: format!("Expected ':' after entity reference: {}", input),
        })?;
        let entity_id = after_section[..colon_pos].trim().to_string();
        (RefTarget::Entity(entity_id), &after_section[colon_pos + 1..])
    } else if input.starts_with('[') {
        // Parse [N]: ... or ["text"]: ... or ["text"@n]: ...
        let bracket_end = input.find(']').ok_or_else(|| SpecError::Parse {
            line: source_line,
            message: format!("Unclosed bracket in assertion: {}", input),
        })?;

        let ref_str = &input[1..bracket_end];
        let target = parse_ref_target(ref_str, source_line)?;

        // Expect colon after bracket
        let rest = &input[bracket_end + 1..].trim_start();
        if !rest.starts_with(':') {
            return Err(SpecError::Parse {
                line: source_line,
                message: format!("Expected ':' after reference: {}", input),
            });
        }
        (target, &rest[1..])
    } else {
        return Err(SpecError::Parse {
            line: source_line,
            message: format!("Assertion must start with '[' or '§': {}", input),
        });
    };

    let type_and_body = target.1.trim();

    // Parse Type(body)
    let paren_start = type_and_body.find('(').ok_or_else(|| SpecError::Parse {
        line: source_line,
        message: format!("Expected Type(body) format: {}", type_and_body),
    })?;

    let span_type = type_and_body[..paren_start].trim().to_string();

    if !type_and_body.ends_with(')') {
        return Err(SpecError::Parse {
            line: source_line,
            message: format!("Unclosed parenthesis: {}", type_and_body),
        });
    }

    let body_str = &type_and_body[paren_start + 1..type_and_body.len() - 1];
    let body = parse_assertion_body(body_str, source_line)?;

    Ok(Assertion {
        target: target.0,
        span_type,
        body,
        source_line,
    })
}

/// Parse reference target: numeric ID, text reference, or entity.
fn parse_ref_target(input: &str, source_line: usize) -> SpecResult<RefTarget> {
    let input = input.trim();

    // Check for text reference: "text" or "text"@n
    if input.starts_with('"') {
        // Find closing quote
        let end_quote = input[1..].find('"').ok_or_else(|| SpecError::Parse {
            line: source_line,
            message: format!("Unclosed quote in text reference: {}", input),
        })? + 1;

        let text = input[1..end_quote].to_string();
        let rest = &input[end_quote + 1..];

        // Check for occurrence index @n
        let occurrence = if rest.starts_with('@') {
            rest[1..].parse().map_err(|_| SpecError::Parse {
                line: source_line,
                message: format!("Invalid occurrence index in '{}': expected number", input),
            })?
        } else {
            0
        };

        Ok(RefTarget::TextRef { text, occurrence })
    } else {
        // Numeric span reference
        let id: usize = input.parse().map_err(|_| SpecError::Parse {
            line: source_line,
            message: format!("Invalid span reference '{}': expected number", input),
        })?;
        Ok(RefTarget::Span(id))
    }
}

/// Parse assertion body: comma-separated field checks.
fn parse_assertion_body(input: &str, source_line: usize) -> SpecResult<AssertionBody> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(AssertionBody::default());
    }

    let mut field_checks = Vec::new();

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        field_checks.push(parse_field_check(part, source_line)?);
    }

    Ok(AssertionBody { field_checks })
}

/// Parse a single field check: field=value, field>=value, etc.
fn parse_field_check(input: &str, source_line: usize) -> SpecResult<FieldCheck> {
    // Try different operators in order of specificity
    let (field, operator, expected) = if let Some(pos) = input.find(">=") {
        (&input[..pos], CompareOp::Gte, &input[pos + 2..])
    } else if let Some(pos) = input.find("<=") {
        (&input[..pos], CompareOp::Lte, &input[pos + 2..])
    } else if let Some(pos) = input.find("~=") {
        (&input[..pos], CompareOp::Contains, &input[pos + 2..])
    } else if let Some(pos) = input.find('=') {
        (&input[..pos], CompareOp::Equals, &input[pos + 1..])
    } else {
        return Err(SpecError::Parse {
            line: source_line,
            message: format!("Invalid field check '{}': expected 'field=value'", input),
        });
    };

    Ok(FieldCheck {
        field: field.trim().to_string(),
        expected: expected.trim().to_string(),
        operator,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_span() {
        let (normalized, spans, entities) = parse_spans("Hello «1:world»!", 0).unwrap();
        assert_eq!(normalized, "Hello world!");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].id, MarkerId::Numeric(1));
        assert_eq!(spans[0].text, "world");
        assert_eq!(spans[0].char_range, 6..11);
        assert!(entities.is_empty()); // Numeric ID does not create entity
    }

    #[test]
    fn test_parse_named_span() {
        let (normalized, spans, entities) = parse_spans("«T:The Tenant» shall pay.", 0).unwrap();
        assert_eq!(normalized, "The Tenant shall pay.");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].id, MarkerId::Named("T".to_string()));
        assert_eq!(spans[0].text, "The Tenant");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].id, "T");
        assert_eq!(entities[0].text, "The Tenant");
    }

    #[test]
    fn test_parse_multiple_spans() {
        let (normalized, spans, _) = parse_spans("«1:A» and «2:B»", 0).unwrap();
        assert_eq!(normalized, "A and B");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].id, MarkerId::Numeric(1));
        assert_eq!(spans[0].text, "A");
        assert_eq!(spans[1].id, MarkerId::Numeric(2));
        assert_eq!(spans[1].text, "B");
    }

    #[test]
    fn test_parse_assertion() {
        let assertion = parse_assertion("[1]: Obligation(modal=shall)", 1).unwrap();
        assert_eq!(assertion.target, RefTarget::Span(1));
        assert_eq!(assertion.span_type, "Obligation");
        assert_eq!(assertion.body.field_checks.len(), 1);
        assert_eq!(assertion.body.field_checks[0].field, "modal");
        assert_eq!(assertion.body.field_checks[0].expected, "shall");
    }

    #[test]
    fn test_parse_entity_assertion() {
        let assertion = parse_assertion("§T: Party(role=tenant)", 1).unwrap();
        assert_eq!(assertion.target, RefTarget::Entity("T".to_string()));
        assert_eq!(assertion.span_type, "Party");
    }

    #[test]
    fn test_parse_text_ref_assertion() {
        let assertion = parse_assertion("[\"shall pay\"]: Obligation()", 1).unwrap();
        assert_eq!(
            assertion.target,
            RefTarget::TextRef {
                text: "shall pay".to_string(),
                occurrence: 0
            }
        );
    }

    #[test]
    fn test_parse_text_ref_with_occurrence() {
        let assertion = parse_assertion("[\"rent\"@2]: Amount()", 1).unwrap();
        assert_eq!(
            assertion.target,
            RefTarget::TextRef {
                text: "rent".to_string(),
                occurrence: 2
            }
        );
    }

    #[test]
    fn test_parse_fixture() {
        let input = r#"# Test: Simple

The «1:shall pay» test.

> [1]: Obligation(modal=shall)
"#;
        let fixture = parse_fixture(input).unwrap();
        assert_eq!(fixture.title, Some("Test: Simple".to_string()));
        assert_eq!(fixture.normalized_text(), "The shall pay test.");
        assert_eq!(fixture.spans().len(), 1);
        assert_eq!(fixture.assertions.len(), 1);
    }

    #[test]
    fn test_parse_multi_paragraph() {
        let input = r#"# Multi-paragraph

First paragraph with «1:span one».

---

Second paragraph with «2:span two».

> [1]: TypeA()
> [2]: TypeB()
"#;
        let fixture = parse_fixture(input).unwrap();
        assert_eq!(fixture.paragraphs.len(), 2);
        assert_eq!(fixture.paragraphs[0].text, "First paragraph with span one.");
        assert_eq!(
            fixture.paragraphs[1].text,
            "Second paragraph with span two."
        );
        assert_eq!(fixture.spans().len(), 2);
    }

    #[test]
    fn test_parse_with_entities() {
        let input = r#"«T:The Tenant» «1:shall pay» rent to «L:the Landlord».

> §T: Party(role=tenant)
> §L: Party(role=landlord)
> [1]: Obligation(modal=shall)
"#;
        let fixture = parse_fixture(input).unwrap();
        assert_eq!(fixture.entities.len(), 2);
        assert_eq!(fixture.entity_by_id("T").unwrap().text, "The Tenant");
        assert_eq!(fixture.entity_by_id("L").unwrap().text, "the Landlord");
        assert_eq!(fixture.assertions.len(), 3);
    }

    #[test]
    fn test_parse_tenant_example() {
        // Example from the task specification
        let input = "The Tenant «1:shall pay» rent of «2:$2,000» monthly.";
        let (normalized, spans, _) = parse_spans(input, 0).unwrap();

        assert_eq!(normalized, "The Tenant shall pay rent of $2,000 monthly.");
        assert_eq!(spans.len(), 2);

        // Span 1: "shall pay" at char_range 11..20
        assert_eq!(spans[0].id, MarkerId::Numeric(1));
        assert_eq!(spans[0].text, "shall pay");
        assert_eq!(spans[0].char_range, 11..20);

        // Span 2: "$2,000" at char_range 29..35
        assert_eq!(spans[1].id, MarkerId::Numeric(2));
        assert_eq!(spans[1].text, "$2,000");
        assert_eq!(spans[1].char_range, 29..35);
    }

    #[test]
    fn test_parse_error_unclosed_marker() {
        let result = parse_spans("Hello «1:world", 0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, SpecError::Parse { .. }));
    }

    #[test]
    fn test_parse_assertion_with_multiple_fields() {
        let assertion =
            parse_assertion("[1]: Obligation(modal=shall, confidence>=0.8)", 1).unwrap();
        assert_eq!(assertion.body.field_checks.len(), 2);
        assert_eq!(assertion.body.field_checks[0].field, "modal");
        assert_eq!(assertion.body.field_checks[0].operator, CompareOp::Equals);
        assert_eq!(assertion.body.field_checks[1].field, "confidence");
        assert_eq!(assertion.body.field_checks[1].operator, CompareOp::Gte);
    }

    #[test]
    fn test_backwards_compat_normalized_text() {
        let input = r#"First para.

---

Second para.
"#;
        let fixture = parse_fixture(input).unwrap();
        // normalized_text() joins paragraphs with double newlines
        assert_eq!(fixture.normalized_text(), "First para.\n\nSecond para.");
    }

    #[test]
    fn test_span_by_numeric_id() {
        let input = "«1:A» «T:B» «2:C»";
        let fixture = parse_fixture(input).unwrap();

        let (para, span) = fixture.span_by_numeric_id(1).unwrap();
        assert_eq!(span.text, "A");
        assert_eq!(para.index, 0);

        let (para, span) = fixture.span_by_numeric_id(2).unwrap();
        assert_eq!(span.text, "C");
        assert_eq!(para.index, 0);

        // Named span should not be found by numeric ID
        assert!(fixture.span_by_numeric_id(3).is_none());
    }
}
