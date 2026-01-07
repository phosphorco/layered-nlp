//! Document context for resolving entities and spans during evaluation.

use crate::fixture::{EntityDef, MarkerId, NlpFixture, RefTarget, SpanMarker};
use std::ops::Range;

/// Document context tracks entities, paragraphs, and cross-refs during evaluation.
#[derive(Debug)]
pub struct DocumentContext<'a> {
    fixture: &'a NlpFixture,
}

impl<'a> DocumentContext<'a> {
    /// Create a new context for a fixture.
    pub fn new(fixture: &'a NlpFixture) -> Self {
        Self { fixture }
    }

    /// Resolve an entity by ID.
    pub fn resolve_entity(&self, id: &str) -> Option<&EntityDef> {
        self.fixture.entity_by_id(id)
    }

    /// Resolve a span by paragraph index and numeric ID.
    pub fn resolve_span(&self, paragraph_idx: usize, span_id: usize) -> Option<&SpanMarker> {
        self.fixture
            .paragraphs
            .get(paragraph_idx)
            .and_then(|para| {
                para.spans
                    .iter()
                    .find(|s| s.id == MarkerId::Numeric(span_id))
            })
    }

    /// Resolve a span by numeric ID (any paragraph).
    pub fn resolve_span_any(&self, span_id: usize) -> Option<(usize, &SpanMarker)> {
        for (idx, para) in self.fixture.paragraphs.iter().enumerate() {
            if let Some(span) = para
                .spans
                .iter()
                .find(|s| s.id == MarkerId::Numeric(span_id))
            {
                return Some((idx, span));
            }
        }
        None
    }

    /// Resolve a text reference to a character range.
    pub fn resolve_text_ref(&self, text: &str, occurrence: usize) -> Option<Range<usize>> {
        let normalized = self.fixture.normalized_text();
        let mut found = 0;
        let mut start = 0;

        while let Some(pos) = normalized[start..].find(text) {
            if found == occurrence {
                let abs_pos = start + pos;
                return Some(abs_pos..abs_pos + text.len());
            }
            found += 1;
            start += pos + 1;
        }

        None
    }

    /// Resolve a RefTarget to an expected text.
    pub fn resolve_ref_target(&self, target: &RefTarget) -> Option<String> {
        match target {
            RefTarget::Span(id) => self.resolve_span_any(*id).map(|(_, span)| span.text.clone()),
            RefTarget::Entity(id) => self.resolve_entity(id).map(|e| e.text.clone()),
            RefTarget::TextRef { text, .. } => Some(text.clone()),
        }
    }

    /// Get the fixture.
    pub fn fixture(&self) -> &NlpFixture {
        self.fixture
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_fixture;

    #[test]
    fn test_resolve_entity() {
        let fixture = parse_fixture(
            r#"
# Test
«T:The Tenant» agrees.
"#,
        )
        .unwrap();
        let ctx = DocumentContext::new(&fixture);

        let entity = ctx.resolve_entity("T").unwrap();
        assert_eq!(entity.text, "The Tenant");
    }

    #[test]
    fn test_resolve_span() {
        let fixture = parse_fixture(
            r#"
# Test
«1:shall pay» rent.
"#,
        )
        .unwrap();
        let ctx = DocumentContext::new(&fixture);

        let span = ctx.resolve_span(0, 1).unwrap();
        assert_eq!(span.text, "shall pay");
    }

    #[test]
    fn test_resolve_text_ref() {
        let fixture = parse_fixture(
            r#"
# Test
The Tenant shall pay rent.
"#,
        )
        .unwrap();
        let ctx = DocumentContext::new(&fixture);

        let range = ctx.resolve_text_ref("pay", 0).unwrap();
        let text = &ctx.fixture().normalized_text()[range];
        assert_eq!(text, "pay");
    }
}
