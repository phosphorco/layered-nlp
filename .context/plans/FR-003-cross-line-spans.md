# FR-003: Cross-Line Semantic Spans

## Summary

Extend the resolver architecture to support semantic spans that cross line boundaries, enabling detection of multi-line definitions, ellipsis recovery, and obligations spanning multiple sentences.

## Current State

Resolvers operate on single `LLLine` objects. The `ContractDocument` processes text line-by-line:
```rust
for (source_idx, line_text) in text.lines().enumerate() {
    lines.push(layered_nlp::create_line_from_string(line_text));
}
```

This misses:
- Definitions spanning multiple lines with exclusions
- VP ellipsis in coordinated structures
- Obligations split across sentence boundaries

## Proposed Improvements

### 1. Multi-Line Definition Capture

**Problem:** Only the first line of a definition is captured.

**Example:**
```
1.1 "Confidential Information" means any information disclosed by one
    party to the other, including trade secrets and customer lists,
    provided that Confidential Information shall not include information
    that (a) is publicly available, (b) was rightfully received from a
    third party, or (c) is independently developed.
```

**Ask:**
- Detect definition start pattern on line 1
- Scan forward to find definition end (next section, blank line, or new term)
- Create `SemanticSpan` covering full definition range
- Parse exclusions ("shall not include") as structured data

### 2. VP Ellipsis Recovery

**Problem:** Elided verbs in coordinated structures aren't recovered.

**Example:**
```
The Seller shall deliver goods and the Buyer [shall deliver] payment.
```

**Ask:**
- Detect coordination patterns with missing predicates
- Use first conjunct as template for missing structure
- Generate implicit obligations with confidence scores
- Track provenance ("inferred from coordination with...")

### 3. Multi-Sentence Obligation Spans

**Problem:** Obligations can span multiple sentences but are detected per-line.

**Example:**
```
The Company shall indemnify the Client. This obligation extends to all
claims arising from the Company's negligence. Such indemnification shall
include reasonable attorney's fees.
```

**Ask:**
- Detect obligation continuation patterns ("This obligation", "Such X")
- Link continuation sentences to primary obligation
- Build complete obligation span across sentences

### 4. Semantic Span Infrastructure

**Problem:** No infrastructure for cross-line spans.

**Ask:** Introduce core types:
```rust
pub struct DocPosition {
    pub line_idx: usize,
    pub token_start: usize,
    pub token_end: usize,
}

pub struct SemanticSpan {
    pub start: DocPosition,
    pub end: DocPosition,
    pub span_type: SemanticSpanType,
}

pub enum SemanticSpanType {
    DefinedTerm { term_name: String, exclusions: Vec<String> },
    Obligation { obligor: String, action: String },
    Condition { condition_type: ConditionType },
    CrossReference { target: String },
}
```

### 5. Cross-Line Resolver Trait

**Problem:** `Resolver` trait operates on single lines.

**Ask:** Add optional cross-line resolver interface:
```rust
pub trait DocumentResolver {
    type Attr;
    fn resolve_document(&self, doc: &ContractDocument) -> Vec<SemanticSpan>;
}
```

## Success Criteria

- [ ] Multi-line definitions captured with full exclusion text
- [ ] VP ellipsis generates implicit obligations
- [ ] Multi-sentence obligations linked correctly
- [ ] SemanticSpan infrastructure supports cross-line ranges
- [ ] Backward compatible with existing single-line resolvers

## Related Edge Cases

- Multi-line definitions (Document #5)
- VP ellipsis/gapping (Linguistic #4)
- Long-distance dependencies (Linguistic #8)

## Dependencies

- Foundation for FR-001 (nested conditions spanning lines)
- Foundation for FR-002 (cross-references to multi-line content)
- Required by FR-004 (document structure with semantic spans)

---

## Edge Cases

This section catalogs tricky multi-line scenarios that the cross-line span infrastructure must handle correctly.

### EC-1: Definitions Split Across Pages with Intervening Content

**Scenario:** A definition begins on one page, continues after a page header/footer, and ends on another page.

```
"Confidential Information" means any information disclosed by one party
                                                                    [Page 3]
to the other, including but not limited to:
    (a) trade secrets;
    (b) customer lists; and
                                                                    [Page 4]
    (c) technical documentation,
provided that Confidential Information shall not include information that
is publicly available.
```

**Challenges:**
- Page headers/footers must be filtered from semantic content
- The definition must be reconstructed as a single span despite physical interruptions
- Line numbers must map back to source locations (already supported via `line_to_source`)

### EC-2: VP Ellipsis with Complex Antecedent Recovery

**Scenario:** The elided predicate has modifiers that may or may not transfer.

```
The Seller shall deliver goods within 30 days of each order, and the 
Buyer [shall deliver?] payment.
```

**Challenges:**
- Should "within 30 days of each order" transfer to the inferred obligation?
- Coordination of different subjects with shared predicates
- Confidence scoring for partial vs. full recovery

**More Complex:**
```
The Contractor shall complete Phase 1 by March 1, Phase 2 by June 1, and
the Subcontractor Phase 3.
```
Here "shall complete" carries forward, and timing patterns must be inferred.

### EC-3: Obligations Interrupted by Parentheticals or Footnotes

**Scenario:** An obligation sentence contains inline parentheticals that break token flow.

```
The Company shall indemnify (see Section 12 for limitations) the Client
against all claims arising from the Company's negligence.
```

**Challenges:**
- The parenthetical interrupts the obligation phrase
- "shall indemnify ... the Client" is the core obligation, with parenthetical as annotation
- Should produce: (1) obligation span for full sentence, (2) cross-reference for Section 12

**With Footnotes:**
```
The Contractor shall provide insurance coverage¹ for all work performed.

¹ As defined in Exhibit A.
```

Footnote reference interrupts the obligation and creates a cross-document reference.

### EC-4: Nested Multi-Line Structures

**Scenario:** A definition contains a sub-definition.

```
"Intellectual Property" means:
    (a) "Patents" means all patents, patent applications, and patent rights;
    (b) "Copyrights" means all works of authorship and copyright registrations;
    (c) "Trade Secrets" means all confidential information as defined in
        Section 3.1 above.
```

**Challenges:**
- Outer definition spans multiple lines
- Each sub-definition is also a `SemanticSpan` with its own bounds
- Sub-definitions may themselves span multiple lines (as in (c))
- Need parent-child relationship between spans

### EC-5: Line Breaks Within Tokens (Hyphenation)

**Scenario:** Words are hyphenated at line end due to justification.

```
The Company shall provide comprehen-
sive coverage for all employees.
```

**Challenges:**
- "comprehen-" and "sive" are currently separate tokens on separate lines
- Semantic analysis needs to treat "comprehensive" as a single word
- Hyphen at line-end is different from compound words like "long-term"

**Detection Heuristics:**
- Token ending in `-` at line end + next line starts with lowercase letter
- Dictionary validation that concatenation forms a valid word

### EC-6: Headers and Footers Interrupting Logical Flow

**Scenario:** Running headers/footers appear between logical paragraphs.

```
The Seller warrants that all products delivered shall conform to the
specifications set forth in Exhibit A.

                    PURCHASE AGREEMENT - Page 5

The warranty period shall be twelve (12) months from the date of delivery.
```

**Challenges:**
- "PURCHASE AGREEMENT - Page 5" is not part of the contract content
- Must detect and exclude page headers/footers from semantic analysis
- Patterns: centered text, page numbers, document titles in all-caps

### EC-7: Tables and Structured Content

**Scenario:** Tabular data spans multiple lines with column alignment.

```
Fee Schedule:
    Service                     Rate            Minimum
    Initial Consultation        $500/hour       $1,000
    Ongoing Support             $250/hour       n/a
    Emergency Services          $750/hour       $2,000
```

**Challenges:**
- Each row is a logical unit but columns are separated by whitespace
- Table headers define semantics for subsequent rows
- Table cells may contain multi-word content
- Need `TableSpan` with row/column structure, not just linear spans

### EC-8: Enumerated Lists with Mixed Content

**Scenario:** A list where items have different structures.

```
The Contractor shall:
(a) provide all necessary equipment;
(b) maintain insurance coverage of at least $1,000,000, which coverage
    shall remain in effect for the duration of the Agreement and for
    a period of two (2) years thereafter;
(c) comply with all applicable laws; and
(d) submit monthly progress reports.
```

**Challenges:**
- (a), (c), (d) are single-line items
- (b) spans 3 lines with complex temporal conditions
- All items inherit "shall" from the lead-in
- Need to track list structure AND individual item spans

### EC-9: Cross-Reference Chains Across Lines

**Scenario:** A reference points to another reference.

```
Subject to the terms of Section 5.2 and the conditions set forth in
Section 7.1 (which incorporates by reference the terms of Exhibit B),
the parties shall...
```

**Challenges:**
- "Section 5.2" is a cross-reference
- "Section 7.1" is a cross-reference that itself references "Exhibit B"
- Resolution requires multi-step traversal
- Spans for references may cross line boundaries (e.g., "Exhibit B" could wrap)

### EC-10: Conditional Obligations Spanning Multiple Sentences

**Scenario:** A condition in one sentence affects obligations in subsequent sentences.

```
If the Company fails to make payment within thirty (30) days, the following
shall apply. The Contractor may suspend performance. The Company shall pay
all accrued fees. All deadlines shall be extended by the period of delay.
```

**Challenges:**
- The "If" clause on line 1 scopes over three subsequent obligations
- Each obligation sentence is a separate span
- Need to link all obligations back to the conditional
- The scope of the condition ends at the paragraph break

---

## Engineering Approach

### Current Architecture Analysis

The existing `layered-nlp` architecture is fundamentally **line-oriented**:

1. **LLLine** (`src/ll_line.rs:108-112`): A single line of tokenized text with attached attributes
   - Contains `ll_tokens: Vec<LLToken>` for the tokens
   - Contains `LLLineAttrs` for type-bucketed attributes keyed by `(start_idx, end_idx)` ranges

2. **LLSelection** (`src/ll_line/ll_selection.rs:29-35`): A span within a single `LLLine`
   - Holds `Rc<LLLine>` reference
   - `start_idx` and `end_idx` are token indices within that line
   - Cannot reference multiple lines

3. **Resolver** trait (`src/ll_line.rs:363-371`): Operates on a single `LLSelection`
   ```rust
   pub trait Resolver {
       type Attr: std::fmt::Debug + 'static + Send + Sync;
       fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>>;
   }
   ```

4. **ContractDocument** (`layered-contracts/src/document.rs:76-83`): Wrapper around `Vec<LLLine>`
   - Already has `DocPosition` and `DocSpan` types (lines 11-73)
   - `run_resolver` applies a single-line resolver to each line independently

### Design Goals

1. **Backward Compatibility**: Existing `Resolver` implementations continue to work unchanged
2. **Incremental Adoption**: New cross-line resolvers are opt-in
3. **Efficient Queries**: O(log n) or O(1) span lookup, not O(n²)
4. **Type Safety**: Cross-line spans carry the same type guarantees as line-local attributes
5. **Composability**: Cross-line and single-line resolvers can be chained

### Core Types

#### Enhanced DocPosition and DocSpan (already exist, need extensions)

```rust
// Already in document.rs - keep as-is for position within document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocPosition {
    pub line: usize,
    pub token: usize,
}

impl DocPosition {
    /// Create a canonical ordering for positions.
    pub fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line.cmp(&other.line)
            .then_with(|| self.token.cmp(&other.token))
    }
}

// Already in document.rs - extend with ordering and containment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocSpan {
    pub start: DocPosition,
    pub end: DocPosition,
}

impl DocSpan {
    /// Returns true if this span fully contains `other`.
    pub fn contains(&self, other: &DocSpan) -> bool {
        self.start <= other.start && self.end >= other.end
    }

    /// Returns true if this span overlaps with `other`.
    pub fn overlaps(&self, other: &DocSpan) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    /// Merge two spans into the smallest span containing both.
    pub fn merge(&self, other: &DocSpan) -> DocSpan {
        DocSpan {
            start: if self.start < other.start { self.start } else { other.start },
            end: if self.end > other.end { self.end } else { other.end },
        }
    }
}
```

#### SemanticSpan: Typed Cross-Line Span

```rust
use std::any::{Any, TypeId};
use std::sync::Arc;

/// A cross-line span with an attached typed attribute.
///
/// Unlike line-local attributes (stored in `LLLineAttrs`), `SemanticSpan` can
/// reference content across multiple lines and is stored at the document level.
#[derive(Debug, Clone)]
pub struct SemanticSpan {
    /// Document-level position range (inclusive)
    pub span: DocSpan,
    /// Type-erased attribute value
    attr: Arc<dyn Any + Send + Sync>,
    /// TypeId for downcasting
    type_id: TypeId,
    /// Associations to other spans (provenance, references)
    associations: Vec<DocAssociatedSpan>,
}

impl SemanticSpan {
    /// Create a new semantic span with a typed attribute.
    pub fn new<T: 'static + Send + Sync + std::fmt::Debug>(
        span: DocSpan,
        attr: T,
    ) -> Self {
        Self {
            span,
            attr: Arc::new(attr),
            type_id: TypeId::of::<T>(),
            associations: Vec::new(),
        }
    }

    /// Create a semantic span with associations.
    pub fn with_associations<T: 'static + Send + Sync + std::fmt::Debug>(
        span: DocSpan,
        attr: T,
        associations: Vec<DocAssociatedSpan>,
    ) -> Self {
        Self {
            span,
            attr: Arc::new(attr),
            type_id: TypeId::of::<T>(),
            associations,
        }
    }

    /// Attempt to downcast to a concrete type.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.attr.downcast_ref::<T>()
    }

    /// Check if this span contains an attribute of type T.
    pub fn is<T: 'static>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    /// Get associations for provenance traversal.
    pub fn associations(&self) -> &[DocAssociatedSpan] {
        &self.associations
    }
}

/// Document-level association (cross-line equivalent of AssociatedSpan).
#[derive(Debug, Clone)]
pub struct DocAssociatedSpan {
    pub target: DocSpan,
    pub label: &'static str,
    pub glyph: Option<&'static str>,
}
```

### Key Design Decision: Store Indices, Not Text

**Decision**: `SemanticSpan` stores only `DocSpan` (indices), not text content.

**Rationale**:
- Text can always be reconstructed from `ContractDocument` and span indices
- Storing text duplicates data and risks inconsistency
- Indices enable efficient overlap detection and containment queries
- Text extraction can be lazy (computed on demand)

```rust
impl ContractDocument {
    /// Extract text for a document-level span.
    pub fn span_text(&self, span: &DocSpan) -> String {
        let mut result = String::new();
        for line_idx in span.start.line..=span.end.line {
            if let Some(line) = self.get_line(line_idx) {
                let tokens = line.ll_tokens();
                let start_tok = if line_idx == span.start.line {
                    span.start.token
                } else {
                    0
                };
                let end_tok = if line_idx == span.end.line {
                    span.end.token
                } else {
                    tokens.len().saturating_sub(1)
                };
                for tok in &tokens[start_tok..=end_tok.min(tokens.len() - 1)] {
                    if let LToken::Text(text, _) = tok.get_token() {
                        result.push_str(text);
                    }
                }
                if line_idx < span.end.line {
                    result.push(' '); // Line break becomes space
                }
            }
        }
        result
    }
}
```

### Span Storage: Interval Tree for Efficient Queries

To avoid O(n²) overlap detection, use an interval tree indexed by line range:

```rust
use std::collections::BTreeMap;

/// Efficient storage for document-level spans with overlap queries.
pub struct SpanIndex {
    /// Spans indexed by starting line for range queries
    by_start_line: BTreeMap<usize, Vec<usize>>,
    /// Spans indexed by type for type-filtered queries
    by_type: HashMap<TypeId, Vec<usize>>,
    /// All spans (index is stable ID)
    spans: Vec<SemanticSpan>,
}

impl SpanIndex {
    pub fn new() -> Self {
        Self {
            by_start_line: BTreeMap::new(),
            by_type: HashMap::new(),
            spans: Vec::new(),
        }
    }

    /// Add a span and return its stable ID.
    pub fn insert(&mut self, span: SemanticSpan) -> usize {
        let id = self.spans.len();
        let start_line = span.span.start.line;
        let type_id = span.type_id;

        self.by_start_line.entry(start_line).or_default().push(id);
        self.by_type.entry(type_id).or_default().push(id);
        self.spans.push(span);
        id
    }

    /// Query all spans of type T.
    pub fn query_by_type<T: 'static>(&self) -> Vec<&SemanticSpan> {
        self.by_type
            .get(&TypeId::of::<T>())
            .map(|ids| ids.iter().filter_map(|&id| self.spans.get(id)).collect())
            .unwrap_or_default()
    }

    /// Query all spans overlapping a given range (by line).
    pub fn query_overlapping(&self, line_start: usize, line_end: usize) -> Vec<&SemanticSpan> {
        self.spans
            .iter()
            .filter(|s| s.span.start.line <= line_end && s.span.end.line >= line_start)
            .collect()
    }

    /// Query spans containing a specific position.
    pub fn query_at(&self, pos: DocPosition) -> Vec<&SemanticSpan> {
        self.spans
            .iter()
            .filter(|s| {
                (s.span.start.line < pos.line || 
                 (s.span.start.line == pos.line && s.span.start.token <= pos.token)) &&
                (s.span.end.line > pos.line ||
                 (s.span.end.line == pos.line && s.span.end.token >= pos.token))
            })
            .collect()
    }
}
```

### DocumentResolver Trait

```rust
/// Trait for resolvers that operate across line boundaries.
///
/// Unlike the single-line `Resolver` trait, `DocumentResolver` receives
/// the entire document and can produce spans that cross lines.
///
/// # Relationship to Resolver
///
/// - Existing `Resolver` implementations continue to work via `run_resolver()`
/// - `DocumentResolver` is for new cross-line analysis
/// - Both can be applied to the same document in sequence
pub trait DocumentResolver {
    /// The attribute type this resolver produces.
    type Attr: std::fmt::Debug + Send + Sync + 'static;

    /// Analyze the document and produce cross-line spans.
    ///
    /// Implementations have read access to all lines and existing attributes.
    /// They return spans that may cross line boundaries.
    fn resolve_document(&self, doc: &ContractDocument) -> Vec<SemanticSpan>;

    /// Optional: declare dependencies on line-level attributes.
    ///
    /// The framework ensures dependent resolvers run first.
    fn requires(&self) -> Vec<TypeId> {
        Vec::new()
    }
}
```

### Enhanced ContractDocument

```rust
pub struct ContractDocument {
    lines: Vec<LLLine>,
    line_to_source: Vec<usize>,
    original_text: String,
    /// Cross-line spans (new field)
    doc_spans: SpanIndex,
}

impl ContractDocument {
    /// Run a document-level resolver and store cross-line spans.
    pub fn run_document_resolver<R: DocumentResolver>(mut self, resolver: &R) -> Self {
        let spans = resolver.resolve_document(&self);
        for span in spans {
            self.doc_spans.insert(span);
        }
        self
    }

    /// Query cross-line spans by type.
    pub fn query_doc_spans<T: 'static>(&self) -> Vec<&SemanticSpan> {
        self.doc_spans.query_by_type::<T>()
    }

    /// Query all spans (line-local and document-level) at a position.
    pub fn query_at(&self, pos: DocPosition) -> QueryResult {
        let doc_spans = self.doc_spans.query_at(pos);
        let line_attrs = self.get_line(pos.line).map(|l| {
            // Return line-level attributes at this token
            // (would need to expose this from LLLine)
        });
        QueryResult { doc_spans, line_attrs }
    }
}
```

### Document Selection for Cross-Line Matching

```rust
/// A selection that can span multiple lines in a document.
///
/// This is the document-level equivalent of `LLSelection`.
pub struct DocSelection<'d> {
    doc: &'d ContractDocument,
    span: DocSpan,
}

impl<'d> DocSelection<'d> {
    /// Create a selection covering the entire document.
    pub fn full(doc: &'d ContractDocument) -> Option<Self> {
        if doc.line_count() == 0 {
            return None;
        }
        let last_line = doc.line_count() - 1;
        let last_token = doc.get_line(last_line)?.ll_tokens().len().saturating_sub(1);
        Some(Self {
            doc,
            span: DocSpan::new(
                DocPosition::new(0, 0),
                DocPosition::new(last_line, last_token),
            ),
        })
    }

    /// Get a single-line selection for a specific line.
    pub fn line(&self, line_idx: usize) -> Option<LLSelection> {
        let line = self.doc.get_line(line_idx)?;
        // Create LLSelection from line...
        // Note: This requires changes to LLSelection to allow construction
        None // placeholder
    }

    /// Get the text content of this selection.
    pub fn text(&self) -> String {
        self.doc.span_text(&self.span)
    }

    /// Find pattern matches across line boundaries.
    pub fn find_pattern<P: DocPattern>(&self, pattern: &P) -> Vec<(DocSelection<'d>, P::Out)> {
        pattern.find_in(self.doc, &self.span)
    }
}

/// Pattern matching trait for cross-line patterns.
pub trait DocPattern {
    type Out;
    fn find_in(&self, doc: &ContractDocument, within: &DocSpan) -> Vec<(DocSelection, Self::Out)>;
}
```

### Example: Multi-Line Definition Resolver

```rust
/// Resolver for definitions that span multiple lines.
pub struct MultiLineDefinitionResolver;

impl DocumentResolver for MultiLineDefinitionResolver {
    type Attr = DefinedTerm;

    fn requires(&self) -> Vec<TypeId> {
        // Depends on ContractKeyword being detected first
        vec![TypeId::of::<ContractKeyword>()]
    }

    fn resolve_document(&self, doc: &ContractDocument) -> Vec<SemanticSpan> {
        let mut results = Vec::new();

        for (line_idx, line) in doc.lines_enumerated() {
            // Look for definition start: "Term" means
            for (range, text, attrs) in line.query::<ContractKeyword>() {
                if !matches!(attrs.first(), Some(ContractKeyword::Means)) {
                    continue;
                }

                // Found "means" - look backwards for quoted term
                let term_name = self.find_preceding_quoted_term(line, range.0);
                if term_name.is_none() {
                    continue;
                }

                // Scan forward to find definition end
                let def_end = self.find_definition_end(doc, line_idx, range.1);

                results.push(SemanticSpan::new(
                    DocSpan::new(
                        DocPosition::new(line_idx, 0), // Or actual term start
                        def_end,
                    ),
                    DefinedTerm {
                        term_name: term_name.unwrap(),
                        definition_type: DefinitionType::QuotedMeans,
                    },
                ));
            }
        }

        results
    }
}

impl MultiLineDefinitionResolver {
    fn find_definition_end(&self, doc: &ContractDocument, start_line: usize, start_token: usize) -> DocPosition {
        // Scan forward through lines until:
        // 1. Blank line (paragraph break)
        // 2. New section header
        // 3. New definition (another "X" means pattern)
        // 4. End of document

        for line_idx in start_line..doc.line_count() {
            let line = doc.get_line(line_idx).unwrap();

            // Check for section headers
            for (range, _, attrs) in line.query::<SectionHeader>() {
                if line_idx > start_line || range.0 > start_token {
                    // Definition ends before this header
                    let prev_line = if range.0 == 0 && line_idx > 0 {
                        line_idx - 1
                    } else {
                        line_idx
                    };
                    let prev_token = if range.0 == 0 {
                        doc.get_line(prev_line)
                            .map(|l| l.ll_tokens().len().saturating_sub(1))
                            .unwrap_or(0)
                    } else {
                        range.0 - 1
                    };
                    return DocPosition::new(prev_line, prev_token);
                }
            }

            // Check for new definition pattern
            // ...similar logic...
        }

        // End of document
        let last_line = doc.line_count() - 1;
        let last_token = doc.get_line(last_line)
            .map(|l| l.ll_tokens().len().saturating_sub(1))
            .unwrap_or(0);
        DocPosition::new(last_line, last_token)
    }

    fn find_preceding_quoted_term(&self, line: &LLLine, means_token: usize) -> Option<String> {
        // Look backwards from "means" for a quoted term
        // This is line-local analysis
        None // placeholder
    }
}
```

### Handling Overlapping Spans

**Decision**: Allow overlapping spans with explicit containment relationships.

Spans can overlap in several ways:
1. **Nesting**: Definition contains sub-definitions
2. **Partial overlap**: Usually indicates an error or ambiguity
3. **Identical bounds**: Multiple attributes on same range

```rust
/// Relationship between two spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanRelation {
    /// First span fully contains second
    Contains,
    /// First span is fully contained in second
    ContainedBy,
    /// Spans overlap but neither contains the other
    Overlaps,
    /// Spans are identical
    Equal,
    /// No overlap
    Disjoint,
}

impl DocSpan {
    pub fn relation_to(&self, other: &DocSpan) -> SpanRelation {
        if self == other {
            SpanRelation::Equal
        } else if self.contains(other) {
            SpanRelation::Contains
        } else if other.contains(self) {
            SpanRelation::ContainedBy
        } else if self.overlaps(other) {
            SpanRelation::Overlaps
        } else {
            SpanRelation::Disjoint
        }
    }
}

impl SpanIndex {
    /// Find spans that contain the given span.
    pub fn find_parents(&self, child: &DocSpan) -> Vec<&SemanticSpan> {
        self.spans
            .iter()
            .filter(|s| s.span.contains(child) && &s.span != child)
            .collect()
    }

    /// Find spans contained within the given span.
    pub fn find_children(&self, parent: &DocSpan) -> Vec<&SemanticSpan> {
        self.spans
            .iter()
            .filter(|s| parent.contains(&s.span) && &s.span != parent)
            .collect()
    }
}
```

### Canonical Coordinate System

**Decision**: Use `(line_idx, token_idx)` pairs where both are 0-indexed internal indices.

- `line_idx`: Index into `ContractDocument.lines` (excludes blank lines)
- `token_idx`: Index into `LLLine.ll_tokens`
- Source line numbers (for display) are obtained via `source_line_number()`
- Character positions are obtained via `LLToken.pos_starts_at` / `pos_ends_at`

```rust
/// Convert document position to character offset in original text.
pub fn doc_position_to_char_offset(
    doc: &ContractDocument,
    pos: DocPosition,
) -> Option<usize> {
    let line = doc.get_line(pos.line)?;
    let token = line.ll_tokens().get(pos.token)?;
    let line_start_offset = /* compute from original_text */;
    Some(line_start_offset + token.pos_starts_at)
}
```

### Line-Local vs. Document-Global Queries

**Design**: Keep both query interfaces, with bridging.

```rust
impl ContractDocument {
    /// Query line-local attributes (existing behavior).
    pub fn query_line<T: 'static>(&self, line_idx: usize) -> Vec<((usize, usize), String, Vec<&T>)> {
        self.get_line(line_idx)
            .map(|l| l.query::<T>())
            .unwrap_or_default()
    }

    /// Query document-level spans.
    pub fn query_doc<T: 'static>(&self) -> Vec<DocSpanQuery<T>> {
        self.doc_spans
            .query_by_type::<T>()
            .into_iter()
            .filter_map(|s| {
                s.downcast_ref::<T>().map(|attr| DocSpanQuery {
                    span: s.span,
                    text: self.span_text(&s.span),
                    attr,
                })
            })
            .collect()
    }

    /// Unified query: both line-local and document-level.
    pub fn query_all<T: 'static>(&self) -> Vec<UnifiedSpan<T>> {
        let mut results = Vec::new();

        // Line-local attributes
        for (line_idx, line) in self.lines_enumerated() {
            for (range, text, attrs) in line.query::<T>() {
                for attr in attrs {
                    results.push(UnifiedSpan {
                        span: DocSpan::single_line(line_idx, range.0, range.1),
                        text,
                        attr,
                        source: SpanSource::Line,
                    });
                }
            }
        }

        // Document-level spans
        for s in self.doc_spans.query_by_type::<T>() {
            if let Some(attr) = s.downcast_ref::<T>() {
                results.push(UnifiedSpan {
                    span: s.span,
                    text: self.span_text(&s.span),
                    attr,
                    source: SpanSource::Document,
                });
            }
        }

        results
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanSource {
    Line,     // From LLLine attributes
    Document, // From SemanticSpan
}
```

### Migration Path and Backward Compatibility

1. **Existing resolvers unchanged**: `Resolver` trait and `run_resolver()` work as before
2. **New resolvers use `DocumentResolver`**: Added via `run_document_resolver()`
3. **Gradual migration**: Resolvers can be rewritten one at a time
4. **Unified queries**: `query_all<T>()` returns both line-local and document-level results

```rust
// Existing code continues to work
let doc = ContractDocument::from_text(text)
    .run_resolver(&SectionHeaderResolver::new())    // Line-local
    .run_resolver(&ContractKeywordResolver::new()); // Line-local

// New cross-line resolvers can be added
let doc = doc
    .run_document_resolver(&MultiLineDefinitionResolver) // Document-level
    .run_document_resolver(&VPEllipsisResolver);         // Document-level

// Query both
for span in doc.query_all::<DefinedTerm>() {
    println!("{}: {} ({})", span.text, span.attr.term_name, 
             if span.source == SpanSource::Document { "multi-line" } else { "single-line" });
}
```

### Implementation Phases

**Phase 1: Core Infrastructure**
- [ ] Add `SpanIndex` to `ContractDocument`
- [ ] Implement `SemanticSpan` with type-erased storage
- [ ] Add `run_document_resolver()` method
- [ ] Add `query_doc<T>()` method

**Phase 2: Document Selection**
- [ ] Implement `DocSelection` for cross-line pattern matching
- [ ] Add basic cross-line patterns (text search, regex)
- [ ] Bridge to line-local `LLSelection` for mixed queries

**Phase 3: Example Resolvers**
- [ ] `MultiLineDefinitionResolver`
- [ ] `MultiSentenceObligationResolver`
- [ ] `VPEllipsisResolver` (basic coordination patterns)

**Phase 4: Advanced Features**
- [ ] Hyphenation detection and token merging
- [ ] Page header/footer filtering
- [ ] Table structure detection
- [ ] Nested span parent-child relationships
