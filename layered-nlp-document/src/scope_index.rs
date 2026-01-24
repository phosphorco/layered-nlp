//! Query helper for finding scope operators that cover positions/spans.

use crate::{DocPosition, DocSpan, ScopeDimension, ScopeOperator};

/// Index for efficient "what scopes cover this position/span?" queries.
///
/// This is a lightweight wrapper that provides iterator-based queries
/// over a collection of scope operators.
#[derive(Debug)]
pub struct ScopeIndex<'a, O> {
    scopes: &'a [ScopeOperator<O>],
}

impl<'a, O> ScopeIndex<'a, O> {
    /// Create a new scope index from a slice of operators.
    pub fn new(scopes: &'a [ScopeOperator<O>]) -> Self {
        Self { scopes }
    }

    /// All operators whose primary domain contains the given position.
    pub fn covering_position<'b>(&'b self, pos: &'b DocPosition) -> impl Iterator<Item = &'a ScopeOperator<O>> + 'b {
        self.scopes.iter().filter(move |op| {
            op.domain.primary().map_or(false, |span| span.contains(pos))
        })
    }

    /// All operators whose primary domain overlaps the given span.
    pub fn covering_span<'b>(&'b self, span: &'b DocSpan) -> impl Iterator<Item = &'a ScopeOperator<O>> + 'b {
        self.scopes.iter().filter(move |op| {
            op.domain.primary().map_or(false, |s| s.overlaps(span))
        })
    }

    /// Operators of a specific dimension whose primary domain overlaps the given span.
    pub fn of_dimension_covering_span<'b>(
        &'b self,
        dim: ScopeDimension,
        span: &'b DocSpan,
    ) -> impl Iterator<Item = &'a ScopeOperator<O>> + 'b {
        self.covering_span(span).filter(move |op| op.dimension == dim)
    }

    /// Total number of scope operators in this index.
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    /// Check if this index is empty.
    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NegationKind, NegationOp, QuantifierKind, QuantifierOp, ScopeDomain};

    fn make_negation_op(trigger_start: usize, trigger_end: usize, domain_start: usize, domain_end: usize) -> ScopeOperator<NegationOp> {
        ScopeOperator::new(
            ScopeDimension::Negation,
            DocSpan::single_line(0, trigger_start, trigger_end),
            ScopeDomain::from_single(DocSpan::single_line(0, domain_start, domain_end)),
            NegationOp { marker: "not".to_string(), kind: NegationKind::Simple },
        )
    }

    #[allow(dead_code)]
    fn make_quantifier_op(trigger_start: usize, trigger_end: usize, domain_start: usize, domain_end: usize) -> ScopeOperator<QuantifierOp> {
        ScopeOperator::new(
            ScopeDimension::Quantifier,
            DocSpan::single_line(0, trigger_start, trigger_end),
            ScopeDomain::from_single(DocSpan::single_line(0, domain_start, domain_end)),
            QuantifierOp { marker: "each".to_string(), kind: QuantifierKind::Universal },
        )
    }

    #[test]
    fn test_empty_index() {
        let scopes: Vec<ScopeOperator<NegationOp>> = vec![];
        let index = ScopeIndex::new(&scopes);

        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert_eq!(index.covering_position(&DocPosition { line: 0, token: 5 }).count(), 0);
    }

    #[test]
    fn test_covering_position() {
        let scopes = vec![
            make_negation_op(0, 1, 2, 10), // domain: tokens 2-10
        ];
        let index = ScopeIndex::new(&scopes);

        // Inside domain
        assert_eq!(index.covering_position(&DocPosition { line: 0, token: 5 }).count(), 1);
        assert_eq!(index.covering_position(&DocPosition { line: 0, token: 2 }).count(), 1); // boundary
        assert_eq!(index.covering_position(&DocPosition { line: 0, token: 10 }).count(), 1); // boundary

        // Outside domain
        assert_eq!(index.covering_position(&DocPosition { line: 0, token: 1 }).count(), 0);
        assert_eq!(index.covering_position(&DocPosition { line: 0, token: 11 }).count(), 0);
    }

    #[test]
    fn test_covering_span() {
        let scopes = vec![
            make_negation_op(0, 1, 2, 10), // domain: tokens 2-10
        ];
        let index = ScopeIndex::new(&scopes);

        // Overlapping
        let overlapping = DocSpan::single_line(0, 5, 15);
        assert_eq!(index.covering_span(&overlapping).count(), 1);

        // Non-overlapping
        let non_overlapping = DocSpan::single_line(0, 11, 20);
        assert_eq!(index.covering_span(&non_overlapping).count(), 0);
    }

    #[test]
    fn test_multiple_operators() {
        let scopes = vec![
            make_negation_op(0, 1, 2, 10),  // domain: tokens 2-10
            make_negation_op(15, 16, 17, 25), // domain: tokens 17-25
        ];
        let index = ScopeIndex::new(&scopes);

        // Position in first domain only
        assert_eq!(index.covering_position(&DocPosition { line: 0, token: 5 }).count(), 1);

        // Position in second domain only
        assert_eq!(index.covering_position(&DocPosition { line: 0, token: 20 }).count(), 1);

        // Position in neither
        assert_eq!(index.covering_position(&DocPosition { line: 0, token: 12 }).count(), 0);
    }

    #[test]
    fn test_dimension_filtering() {
        // Need to use a common payload type for mixed dimensions
        // For this test, we'll test dimension filtering concept differently
        let neg_scopes = vec![
            make_negation_op(0, 1, 2, 10),
        ];
        let index = ScopeIndex::new(&neg_scopes);

        let test_span = DocSpan::single_line(0, 5, 6);

        // Should find negation operators
        assert_eq!(index.of_dimension_covering_span(ScopeDimension::Negation, &test_span).count(), 1);

        // Should not find quantifier operators
        assert_eq!(index.of_dimension_covering_span(ScopeDimension::Quantifier, &test_span).count(), 0);
    }
}
