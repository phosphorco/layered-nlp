# Contract Diff System: Technical Specification

This document specifies the missing components needed to build the semantic contract comparison viewer.
For a higher-level, versioned view that includes clarification replay and history, see `docs/versioned-diff-architecture.md`.

---

## 0. Document Model (CRITICAL: Address Before Implementation)

### The Fundamental Architecture Mismatch

**Problem:** `LLLine` represents a single line of tokenized text. The `Resolver` trait's `go()` method receives one `LLSelection` at a time—typically corresponding to a single line. However, contract sections span multiple lines, and section boundaries require seeing adjacent lines.

**Existing types (from `src/ll_line.rs:64-66`):**
```rust
/// (starts at, ends at) token indexes within a single LLLine
type LRange = (usize, usize);
/// (starts at, ends at) character positions within a single LLLine
type PositionRange = (usize, usize);
```

### Proposed Document Abstraction

We need a new abstraction layer above `LLLine`:

```rust
/// A contract document composed of multiple lines with cross-line structure.
pub struct ContractDocument {
    /// All lines of the document
    pub lines: Vec<LLLine>,
    /// Document-level structure (populated by DocumentStructureBuilder)
    pub structure: Option<DocumentStructure>,
    /// Cross-line attribute index for efficient lookup
    line_attr_index: DocumentAttributeIndex,
}

/// Position within a multi-line document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocPosition {
    /// Line index (0-based)
    pub line: usize,
    /// Token index within that line
    pub token: usize,
}

/// A span within a document (can cross line boundaries)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocSpan {
    pub start: DocPosition,
    pub end: DocPosition,
}

impl DocSpan {
    /// Returns true if this span is within a single line
    pub fn is_single_line(&self) -> bool {
        self.start.line == self.end.line
    }

    /// Convert to LRange if single-line (for compatibility)
    pub fn to_lrange(&self) -> Option<LRange> {
        if self.is_single_line() {
            Some((self.start.token, self.end.token))
        } else {
            None
        }
    }
}
```

### Component Taxonomy

The review correctly identifies that we're mixing resolver and non-resolver components. Here's the clear taxonomy:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    COMPONENT TAXONOMY                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  LAYER 0: Per-Line Resolvers (implement Resolver trait)             │
│  ─────────────────────────────────────────────────────              │
│  These work within a single LLLine, using LLSelection.              │
│                                                                      │
│  • SectionHeaderResolver    - Detects "Section 3.1" headers         │
│  • TemporalExpressionResolver - Detects "within 30 days"            │
│  • (existing) DefinedTermResolver                                    │
│  • (existing) ObligationPhraseResolver                               │
│                                                                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  DOCUMENT PROCESSORS (new trait, multi-line)                         │
│  ───────────────────────────────────────────                         │
│  These operate on ContractDocument, building cross-line structures.  │
│                                                                      │
│  • DocumentStructureBuilder  - Builds section tree from headers      │
│  • SectionReferenceLinker    - Resolves "Section 3.1" references     │
│                                                                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  DIFF ENGINE (two-document operations)                               │
│  ─────────────────────────────────────                               │
│  These compare two ContractDocuments.                                │
│                                                                      │
│  • DocumentAligner           - Aligns sections between documents     │
│  • SemanticDiffEngine        - Produces ContractDiff                 │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### DocumentProcessor Trait

```rust
/// Trait for components that process entire documents (not just lines).
pub trait DocumentProcessor {
    /// The output type produced by this processor
    type Output;

    /// Process the document, potentially mutating it and returning output
    fn process(&self, doc: &mut ContractDocument) -> Result<Self::Output, ProcessError>;
}

/// Example: DocumentStructureBuilder
pub struct DocumentStructureBuilder;

impl DocumentProcessor for DocumentStructureBuilder {
    type Output = DocumentStructure;

    fn process(&self, doc: &mut ContractDocument) -> Result<DocumentStructure, ProcessError> {
        // 1. Iterate through all lines
        // 2. Find SectionHeader attributes (added by SectionHeaderResolver)
        // 3. Build hierarchical structure based on header sequence
        // 4. Assign section boundaries (line N ends where line N+1's header begins)
        // 5. Store in doc.structure and return
    }
}
```

### Processing Pipeline

```rust
// Step 1: Create document from text
let text = include_str!("contract.txt");
let doc = ContractDocument::from_text(text);

// Step 2: Run per-line resolvers on each line
let doc = doc
    .run_resolver(&SectionHeaderResolver)      // Detect headers
    .run_resolver(&DefinedTermResolver)        // Detect term definitions
    .run_resolver(&TemporalExpressionResolver) // Detect time expressions
    .run_resolver(&ObligationPhraseResolver);  // Detect obligations

// Step 3: Run document processors
let structure = DocumentStructureBuilder.process(&mut doc)?;
SectionReferenceLinker.process(&mut doc)?;

// Step 4: Compare two documents
let diff = SemanticDiffEngine::diff(&doc_a, &doc_b)?;
```

### Error Types

```rust
#[derive(Debug, Clone)]
pub enum ProcessError {
    /// Section header parsing failed
    MalformedSectionHeader {
        line: usize,
        text: String,
        reason: String,
    },
    /// Section numbering is inconsistent
    InconsistentNumbering {
        expected: SectionIdentifier,
        found: SectionIdentifier,
        line: usize,
    },
    /// Reference to non-existent section
    DanglingReference {
        reference: String,
        location: DocSpan,
    },
    /// Ambiguous section match during alignment
    AmbiguousAlignment {
        section: SectionIdentifier,
        candidates: Vec<(SectionIdentifier, f64)>,
    },
}

/// Graceful degradation: errors don't halt processing, they're collected
pub struct ProcessResult<T> {
    pub value: T,
    pub errors: Vec<ProcessError>,
    pub warnings: Vec<String>,
}
```

---

## 1. Section Header Resolver (Per-Line)

### Purpose
Parse document structure to identify sections, subsections, articles, exhibits, and their hierarchical relationships.

### Input
Raw contract text processed through layered-nlp tokenization.

### Output
```rust
#[derive(Debug, Clone)]
pub struct SectionHeader {
    /// The section identifier (e.g., "3.1", "IV", "A")
    pub identifier: SectionIdentifier,
    /// Optional title (e.g., "Payment Terms")
    pub title: Option<String>,
    /// Nesting depth (0 = top-level article, 1 = section, 2 = subsection, etc.)
    pub depth: u8,
    /// Token range within the line: (start_idx, end_idx)
    pub span: LRange,
}

#[derive(Debug, Clone)]
pub enum SectionIdentifier {
    /// Numeric: "1", "1.1", "1.1.1"
    Numeric(Vec<u32>),
    /// Roman: "I", "II", "III", "IV"
    Roman(u32),
    /// Alphabetic: "A", "B", "(a)", "(b)"
    Alpha { letter: char, parenthesized: bool },
    /// Named: "Article", "Exhibit", "Schedule", "Annex"
    Named { kind: SectionKind, identifier: String },
}

#[derive(Debug, Clone)]
pub enum SectionKind {
    Article,
    Section,
    Subsection,
    Paragraph,
    Clause,
    Exhibit,
    Schedule,
    Annex,
    Appendix,
    Recital,
    Definition,  // The "Definitions" section specifically
}

/// Represents the full document structure
pub struct DocumentStructure {
    pub sections: Vec<SectionNode>,
}

pub struct SectionNode {
    pub header: SectionHeader,
    /// Document span covering the entire section content (can cross lines)
    pub content_span: DocSpan,
    /// Child sections
    pub children: Vec<SectionNode>,
}
```

### Detection Patterns

**Numeric Sections:**
```
1. Introduction
1.1 Purpose
1.1.1 Scope
2. Definitions
```

**Roman Numeral Articles:**
```
ARTICLE I - DEFINITIONS
ARTICLE II - SERVICES
```

**Alphabetic Lists:**
```
(a) first item
(b) second item
    (i) sub-item
    (ii) sub-item
```

**Named Sections:**
```
EXHIBIT A - STATEMENT OF WORK
Schedule 1: Pricing
Annex B - Technical Specifications
```

### Implementation Approach

1. **Line-by-line scan** for section header patterns
2. **Indentation tracking** for hierarchy (if whitespace-based)
3. **Numbering sequence validation** (1.1 should follow 1, not jump to 3.1)
4. **Title extraction** using heuristics:
   - All-caps following identifier
   - Text after dash/colon
   - Text on same line as identifier

### Resolver Interface

```rust
pub struct SectionStructureResolver;

impl Resolver for SectionStructureResolver {
    type Attr = SectionHeader;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        // Find lines that match section header patterns
        // Return assignments for each detected header
    }
}
```

### Edge Cases

1. **Inconsistent numbering**: Document jumps from 3.1 to 3.5 (missing sections)
2. **Multiple numbering schemes**: Numeric sections with alphabetic sub-clauses
3. **Inline section references vs headers**: "See Section 3" vs "Section 3. Payment Terms"
4. **Definitions section structure**: Often uses quoted terms as pseudo-headers

### Clarifying Questions

1. **Q1.1**: Should we detect section *boundaries* (where content ends) or just headers? Detecting boundaries requires understanding when the next section starts, which may need multi-pass processing.

2. **Q1.2**: How do we handle documents without explicit section numbering? Some contracts use bold/underline formatting instead of numbers. Since we're working with plain text (not rich text), do we assume structure is indicated textually?

3. **Q1.3**: Should `SectionNode` contain references to the semantic content within it (obligations, defined terms)? Or keep structure separate from semantics?

4. **Q1.4**: How do we handle "ARTICLE I" vs "Article 1" vs "ARTICLE ONE"? Normalize to a canonical form?

5. **Q1.5**: For the Definitions section specifically, should each defined term be treated as a sub-section?

---

## 2. Section Reference Resolver

### Purpose
Detect references to sections within the document text (e.g., "pursuant to Section 3.1", "as set forth in Article IV").

### Output

```rust
#[derive(Debug, Clone)]
pub struct SectionReference {
    /// The referenced section identifier
    pub target: SectionIdentifier,
    /// The full reference text (e.g., "Section 3.1 above")
    pub reference_text: String,
    /// Type of reference
    pub reference_type: ReferenceType,
    /// Confidence score
    /// Confidence score (matches Scored<T> which uses f64)
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub enum ReferenceType {
    /// Direct: "Section 3.1"
    Direct,
    /// Range: "Sections 3.1 through 3.5"
    Range { start: SectionIdentifier, end: SectionIdentifier },
    /// List: "Sections 3.1, 3.2, and 3.4"
    List(Vec<SectionIdentifier>),
    /// Relative: "this Section", "the foregoing", "above"
    Relative(RelativeReference),
    /// External: "Section 5 of the Master Agreement"
    External { document: String },
}

#[derive(Debug, Clone)]
pub enum RelativeReference {
    This,       // "this Section"
    Foregoing,  // "the foregoing Section"
    Above,      // "Section 3.1 above"
    Below,      // "Section 3.1 below"
    Hereof,     // "Section 3.1 hereof"
    Herein,     // "as defined herein"
}
```

### Detection Patterns

**Direct References:**
```
"Section 3.1"
"Article IV"
"Clause (a)"
"paragraph (ii)"
"Exhibit A"
"Schedule 2"
```

**Range References:**
```
"Sections 3.1 through 3.5"
"Sections 3.1 - 3.5"
"Articles I through V"
"clauses (a) to (d)"
```

**List References:**
```
"Sections 3.1, 3.2, and 3.4"
"Articles I, III, and V"
```

**Relative References:**
```
"this Section"
"the preceding paragraph"
"as set forth above"
"Section 3.1 hereof"
```

**External References:**
```
"Section 5 of the Master Agreement"
"as defined in the Purchase Agreement"
"Exhibit A to the Credit Agreement"
```

### Implementation Approach

```rust
pub struct SectionReferenceResolver;

impl Resolver for SectionReferenceResolver {
    type Attr = SectionReference;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        // Pattern 1: "Section" + identifier
        // Pattern 2: "Article" + roman/number
        // Pattern 3: "Exhibit/Schedule/Annex" + letter/number
        // Pattern 4: Relative keywords
    }
}
```

### Validation (Post-Processing)

After both SectionStructure and SectionReference are resolved, a validation pass can:
1. Check if referenced sections exist in the document
2. Flag dangling references
3. Resolve relative references to concrete sections based on position

```rust
pub struct ResolvedSectionReference {
    pub reference: SectionReference,
    /// The section this reference points to (if found)
    pub resolved_target: Option<SectionId>,
    /// Is this a valid reference?
    pub is_valid: bool,
    /// If invalid, why?
    pub validation_error: Option<ReferenceError>,
}

pub enum ReferenceError {
    TargetNotFound,
    AmbiguousTarget,  // Multiple sections match
    CircularReference,
    ExternalDocument,  // Can't resolve, different doc
}
```

### Clarifying Questions

1. **Q2.1**: Should section reference resolution happen in the same pass as section structure detection, or as a separate resolver that depends on structure being already detected?

2. **Q2.2**: How do we handle "Section 3" when there's both "Section 3" and "Section 3.1", "3.2", etc.? Does "Section 3" refer to just the header or the entire section including subsections?

3. **Q2.3**: For relative references like "this Section", we need to know which section the current text is *in*. This requires correlating token positions with section boundaries. Is this a third-pass operation?

4. **Q2.4**: Should we track the *purpose* of the reference? E.g., "subject to Section 3.1" (condition), "as defined in Section 1" (definition), "notwithstanding Section 4" (override).

5. **Q2.5**: External document references ("Section 5 of the Master Agreement") - should we attempt to identify the external document name and flag it, even if we can't resolve it?

---

## 3. Temporal Expression Resolver

### Purpose
Extract time-based expressions that define deadlines, durations, and temporal conditions.

### Output

```rust
#[derive(Debug, Clone)]
pub struct TemporalExpression {
    /// The type of temporal expression
    pub kind: TemporalKind,
    /// Normalized duration if applicable
    pub duration: Option<Duration>,
    /// The anchor event (what the time is relative to)
    pub anchor: Option<TemporalAnchor>,
    /// Original text
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum TemporalKind {
    /// Fixed deadline: "by December 31, 2024"
    Deadline { date: Option<NaiveDate> },
    /// Duration from anchor: "within 30 days of"
    DurationFrom,
    /// Duration until anchor: "30 days prior to"
    DurationUntil,
    /// Period: "for a period of 2 years"
    Period,
    /// Recurring: "on each anniversary"
    Recurring,
    /// Conditional timing: "promptly", "immediately", "as soon as practicable"
    Qualitative(QualitativeTiming),
}

#[derive(Debug, Clone)]
pub enum QualitativeTiming {
    Immediately,
    Promptly,
    WithoutDelay,
    AsSoonAsPracticable,
    Reasonable,
    Timely,
}

#[derive(Debug, Clone)]
pub struct Duration {
    pub value: u32,
    pub unit: TimeUnit,
    pub business_days: bool,  // "5 business days" vs "5 days"
}

#[derive(Debug, Clone)]
pub enum TimeUnit {
    Days,
    Weeks,
    Months,
    Years,
}

#[derive(Debug, Clone)]
pub struct TemporalAnchor {
    /// What event is the time relative to?
    pub anchor_text: String,
    /// Classified anchor type
    pub anchor_type: AnchorType,
}

#[derive(Debug, Clone)]
pub enum AnchorType {
    /// "receipt of invoice"
    DocumentEvent(String),
    /// "termination", "expiration"
    ContractEvent(ContractEventType),
    /// "the Closing Date"
    DefinedDate(String),
    /// "written notice"
    NoticeEvent,
    /// "breach", "default"
    BreachEvent,
    /// Calendar-based
    CalendarDate,
}

#[derive(Debug, Clone)]
pub enum ContractEventType {
    EffectiveDate,
    Termination,
    Expiration,
    Closing,
    Commencement,
    Renewal,
}
```

### Detection Patterns

**Duration + Anchor:**
```
"within thirty (30) days of receipt of invoice"
"no later than 5 business days after written notice"
"for a period of two (2) years following termination"
"30 days prior to the expiration date"
```

**Fixed Dates:**
```
"by December 31, 2024"
"on or before the Effective Date"
"as of the Closing Date"
```

**Qualitative:**
```
"promptly notify"
"immediately upon discovery"
"as soon as reasonably practicable"
"without undue delay"
```

**Recurring:**
```
"on each anniversary of the Effective Date"
"monthly"
"within 30 days after the end of each calendar quarter"
```

### Implementation Notes

1. **Number parsing**: Handle both "30" and "thirty (30)" patterns (layered-amount may help)
2. **Unit normalization**: "month" vs "months", "year" vs "years"
3. **Business days**: Explicit detection of "business days" vs "calendar days"
4. **Anchor extraction**: Often requires parsing the prepositional phrase after the duration

### Clarifying Questions

1. **Q3.1**: Should we attempt to *calculate* concrete dates when anchors are known? E.g., if Effective Date is January 1, 2024, can we compute "30 days after Effective Date" = January 31, 2024?

2. **Q3.2**: How do we handle ambiguous expressions like "reasonable time"? Store as qualitative, but should we assign a risk indicator since these are legally contestable?

3. **Q3.3**: The pattern "thirty (30) days" is extremely common. Should we use layered-amount's number parsing or build custom logic?

4. **Q3.4**: For anchor detection, how far should we look after the duration? "within 30 days of receipt of invoice from the Contractor" - is the anchor "receipt of invoice" or "receipt of invoice from the Contractor"?

5. **Q3.5**: Should temporal expressions be linked to the obligations they modify? E.g., "Company shall pay within 30 days" - the 30 days modifies the "pay" obligation.

---

## 4. Document Alignment Algorithm

### Purpose
Given two versions of a contract, align corresponding sections/clauses so we can compare them semantically.

### The Problem

```
DOCUMENT A                      DOCUMENT B
-----------                     -----------
Article I - Definitions         Article I - Definitions
Article II - Services           Article II - Term          ← NEW (inserted)
Article III - Payment           Article III - Services     ← Was Article II
Article IV - Term               Article IV - Payment       ← Was Article III
                                Article V - Confidentiality ← NEW
```

We need to determine:
- Article II (A) = Article III (B) [Services]
- Article III (A) = Article IV (B) [Payment]
- Article IV (A) = Article II (B) [Term - but content may differ]
- Article V (B) is new

### Alignment Strategies

#### Strategy 1: Title Matching
Match sections by title similarity:
```rust
fn align_by_title(a: &[Section], b: &[Section]) -> Vec<Alignment> {
    // Exact title match first
    // Then fuzzy title match (Levenshtein, Jaccard)
    // Then semantic similarity (embeddings)
}
```

**Pros**: Simple, fast, works when titles preserved
**Cons**: Fails if titles changed

#### Strategy 2: Content Similarity
Compare section content using text similarity:
```rust
fn align_by_content(a: &[Section], b: &[Section]) -> Vec<Alignment> {
    // Compute TF-IDF or embedding for each section
    // Find best matches by cosine similarity
    // Use Hungarian algorithm for optimal assignment
}
```

**Pros**: Works even if titles change
**Cons**: Expensive for long documents, may misalign similar-looking sections

#### Strategy 3: Anchor-Based Alignment
Use stable semantic elements as anchors:
```rust
fn align_by_anchors(a: &Document, b: &Document) -> Vec<Alignment> {
    // Find defined terms that appear in both
    // Find obligations with same parties/actions
    // Use these as anchor points
    // Interpolate section alignment between anchors
}
```

**Pros**: Semantically meaningful
**Cons**: Requires semantic extraction first, complex

#### Strategy 4: Hybrid Multi-Pass
```rust
fn align_hybrid(a: &Document, b: &Document) -> Vec<Alignment> {
    // Pass 1: Exact title matches (high confidence)
    // Pass 2: Fuzzy title matches for remaining
    // Pass 3: Defined term anchors
    // Pass 4: Content similarity for stragglers
    // Pass 5: Mark truly new/deleted sections
}
```

### Output

```rust
#[derive(Debug)]
pub struct DocumentAlignment {
    pub alignments: Vec<SectionAlignment>,
}

#[derive(Debug)]
pub struct SectionAlignment {
    pub section_a: Option<SectionId>,
    pub section_b: Option<SectionId>,
    pub alignment_type: AlignmentType,
    /// Confidence score (matches Scored<T> which uses f64)
    pub confidence: f64,
    /// How the alignment was determined
    pub method: AlignmentMethod,
}

#[derive(Debug)]
pub enum AlignmentType {
    /// Same section in both documents
    Matched,
    /// Section only in document A (deleted)
    DeletedFromA,
    /// Section only in document B (added)
    AddedInB,
    /// Section moved to different location
    Moved { from_position: usize, to_position: usize },
    /// Section split into multiple sections
    Split { one_to_many: Vec<SectionId> },
    /// Multiple sections merged into one
    Merged { many_to_one: Vec<SectionId> },
}

#[derive(Debug)]
pub enum AlignmentMethod {
    ExactTitleMatch,
    FuzzyTitleMatch { similarity: f32 },
    ContentSimilarity { similarity: f32 },
    AnchorBased { anchors: Vec<String> },
    Manual,  // Human-corrected
}
```

### Subsection Alignment

Once top-level sections are aligned, recursively align subsections:

```
Section 3 (A) ←→ Section 4 (B)    [Matched by title "Payment"]
    │
    ├── 3.1 (A) ←→ 4.1 (B)       [Matched: "Payment Terms"]
    ├── 3.2 (A) ←→ 4.3 (B)       [Matched by content - 4.2 is new]
    ├── [none]  ←→ 4.2 (B)       [Added: new subsection]
    └── 3.3 (A) ←→ [none]        [Deleted]
```

### Clarifying Questions

1. **Q4.1**: Should alignment be computed lazily (on-demand per section) or eagerly (full document upfront)? Lazy is faster for initial load but may have inconsistencies.

2. **Q4.2**: How do we handle sections that are *partially* moved? E.g., paragraph from Section 3 moved to Section 5, but rest of Section 3 stayed.

3. **Q4.3**: What confidence threshold should trigger "needs human review"? If we're 60% confident sections match, should we flag it?

4. **Q4.4**: For the hybrid approach, what order of methods works best? Title first, or content first?

5. **Q4.5**: Should alignment preserve paragraph-level granularity within sections, or just section-level?

6. **Q4.6**: How do we handle definition section alignment? Each defined term is like a mini-section. Should we align term-by-term?

---

## 5. Semantic Diff Engine

### Purpose
Given aligned documents, produce a structured diff that classifies changes by semantic impact.

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    SEMANTIC DIFF ENGINE                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐    │
│  │  Document A   │   │  Document B   │   │  Alignment   │    │
│  │  (Semantic)   │   │  (Semantic)   │   │   Result     │    │
│  └──────┬───────┘   └──────┬───────┘   └──────┬───────┘    │
│         │                  │                  │             │
│         └──────────────────┼──────────────────┘             │
│                            │                                 │
│                            ▼                                 │
│                   ┌─────────────────┐                       │
│                   │  Diff Drivers   │                       │
│                   └────────┬────────┘                       │
│                            │                                 │
│         ┌──────────────────┼──────────────────┐             │
│         │                  │                  │             │
│         ▼                  ▼                  ▼             │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐       │
│  │  Term Diff  │   │ Obligation  │   │  Section    │       │
│  │   Driver    │   │   Diff      │   │   Diff      │       │
│  └──────┬──────┘   └──────┬──────┘   └──────┬──────┘       │
│         │                  │                  │             │
│         └──────────────────┼──────────────────┘             │
│                            │                                 │
│                            ▼                                 │
│                   ┌─────────────────┐                       │
│                   │ Change Merger & │                       │
│                   │ Impact Analyzer │                       │
│                   └────────┬────────┘                       │
│                            │                                 │
│                            ▼                                 │
│                   ┌─────────────────┐                       │
│                   │  ContractDiff   │                       │
│                   │    (Output)     │                       │
│                   └─────────────────┘                       │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Diff Drivers

#### Term Diff Driver

```rust
pub struct TermDiff {
    pub term_name: String,
    pub change_type: TermChangeType,
    pub original: Option<DefinedTerm>,
    pub revised: Option<DefinedTerm>,
    /// All references to this term in both documents
    pub references: TermReferenceImpact,
}

pub enum TermChangeType {
    Added,
    Removed,
    Redefined {
        scope_change: ScopeChange,
        text_diff: TextDiff,
    },
    Renamed {
        old_name: String,
        new_name: String,
    },
    Unchanged,
}

pub enum ScopeChange {
    Narrowed,   // Definition became more restrictive
    Expanded,   // Definition became broader
    Shifted,    // Different but not clearly narrower/broader
    Unchanged,
}

pub struct TermReferenceImpact {
    /// References in A that still exist in B
    pub preserved: Vec<TermReference>,
    /// References in A that were removed
    pub removed: Vec<TermReference>,
    /// New references in B
    pub added: Vec<TermReference>,
    /// Total count of affected usages
    pub total_affected: usize,
}
```

#### Obligation Diff Driver

```rust
pub struct ObligationDiff {
    pub location: AlignedLocation,
    pub change_type: ObligationChangeType,
    pub original: Option<ObligationPhrase>,
    pub revised: Option<ObligationPhrase>,
    pub party_impact: HashMap<PartyId, PartyImpactLevel>,
}

pub enum ObligationChangeType {
    Added,
    Removed,
    TypeChanged {
        from: ObligationType,  // Duty, Permission, Prohibition
        to: ObligationType,
    },
    ObligorChanged {
        from: String,
        to: String,
    },
    ActionModified {
        text_diff: TextDiff,
    },
    ConditionAdded {
        condition: ConditionRef,
    },
    ConditionRemoved {
        condition: ConditionRef,
    },
    ConditionModified {
        original: ConditionRef,
        revised: ConditionRef,
    },
    Unchanged,
}

pub enum PartyImpactLevel {
    Favorable,    // Change benefits this party
    Unfavorable,  // Change hurts this party
    Neutral,      // No clear impact
    Mixed,        // Some aspects better, some worse
}
```

#### Section Diff Driver

```rust
pub struct SectionDiff {
    pub alignment: SectionAlignment,
    pub change_type: SectionChangeType,
    /// Changes within the section (if matched)
    pub content_changes: Vec<SemanticChange>,
    /// Text-level diff for human review
    pub text_diff: TextDiff,
}

pub enum SectionChangeType {
    Added,
    Removed,
    Moved { from: SectionPath, to: SectionPath },
    Renamed { old_title: String, new_title: String },
    ContentModified,
    Unchanged,
}
```

### Impact Analyzer

After individual diff drivers run, analyze cascading effects:

```rust
pub struct ImpactAnalysis {
    /// Changes that affect other changes
    pub cascades: Vec<CascadeChain>,
    /// Broken references from removed sections
    pub broken_references: Vec<BrokenReference>,
    /// Conditions that now reference changed sections
    pub affected_conditions: Vec<AffectedCondition>,
}

pub struct CascadeChain {
    /// The root change that triggers the cascade
    pub root: ChangeId,
    /// Changes affected by the root
    pub affected: Vec<ChangeId>,
    /// Depth of the cascade (1 = direct, 2+ = transitive)
    pub depth: u8,
}

pub struct BrokenReference {
    pub reference: SectionReference,
    pub location: DocSpan,
    /// The section that was removed
    pub missing_target: SectionIdentifier,
}

pub struct AffectedCondition {
    pub condition: ConditionRef,
    pub in_obligation: ObligationPhrase,
    /// The section the condition references
    pub references_section: SectionIdentifier,
    /// How that section changed
    pub section_change: SectionChangeType,
}
```

### Output: ContractDiff

```rust
pub struct ContractDiff {
    /// All semantic changes, categorized
    pub changes: Vec<SemanticChange>,
    /// Changes grouped by section
    pub by_section: HashMap<SectionId, Vec<ChangeId>>,
    /// Changes grouped by party
    pub by_party: HashMap<PartyId, PartyChangesSummary>,
    /// Impact analysis
    pub impact: ImpactAnalysis,
    /// Summary statistics
    pub summary: DiffSummary,
}

pub struct PartyChangesSummary {
    pub party: PartyId,
    pub duties_added: u32,
    pub duties_removed: u32,
    pub duties_modified: u32,
    pub permissions_added: u32,
    pub permissions_removed: u32,
    pub net_impact: PartyImpactLevel,
}

pub struct DiffSummary {
    pub total_changes: u32,
    pub by_risk: HashMap<RiskLevel, u32>,
    pub by_type: HashMap<ChangeCategory, u32>,
    pub terms_changed: u32,
    pub obligations_changed: u32,
    pub sections_added: u32,
    pub sections_removed: u32,
}
```

### Risk Classification

```rust
pub enum RiskLevel {
    High,    // Material change to obligations, rights, or liability
    Medium,  // Significant change but not to core terms
    Low,     // Minor clarification or formatting
    Info,    // No semantic change, just text cleanup
}

impl SemanticChange {
    pub fn classify_risk(&self) -> RiskLevel {
        match self {
            // High risk
            SemanticChange::Obligation(o) if matches!(
                o.change_type,
                ObligationChangeType::TypeChanged { .. } |
                ObligationChangeType::Removed
            ) => RiskLevel::High,

            // Medium risk
            SemanticChange::Term(t) if matches!(
                t.change_type,
                TermChangeType::Redefined { scope_change: ScopeChange::Narrowed | ScopeChange::Expanded, .. }
            ) => RiskLevel::Medium,

            // etc.
        }
    }
}
```

### Clarifying Questions

1. **Q5.1**: Should text-level diffs (word changes) be computed for all aligned content, or only on-demand when user drills into a change?

2. **Q5.2**: How granular should obligation diffing be? Compare entire obligation phrases, or decompose into (obligor, type, action, conditions) and diff each component?

3. **Q5.3**: For party impact classification (Favorable/Unfavorable), should this be rule-based or use an LLM for nuanced interpretation?

4. **Q5.4**: Should the diff engine be incremental? If document B changes slightly to B', can we compute the diff(A, B') from diff(A, B) without recomputing from scratch?

5. **Q5.5**: How do we handle "semantic no-ops"? E.g., "Company shall" → "the Company shall" is textually different but semantically identical.

6. **Q5.6**: Should we compute a "similarity score" between original and revised obligations to help rank which changes are most significant?

---

## Resolved Design Decisions

Based on analysis of the layered-nlp codebase patterns and architectural constraints, here are the resolved answers to all clarifying questions.

---

### Section 1: Section Structure Resolver

| Question | Decision | Rationale |
|----------|----------|-----------|
| **Q1.1** Boundaries vs headers? | **Headers per-line, boundaries in DocumentStructureBuilder** | `SectionHeaderResolver` detects headers on each line. `DocumentStructureBuilder` (multi-line processor) builds the tree and determines boundaries. |
| **Q1.2** Documents without textual structure? | **Require textual indicators** | `LLLine` has no formatting metadata. No textual markers = single section. |
| **Q1.3** SectionNode contains semantics? | **No, keep separate** | Correlate via overlapping `DocSpan`. Follows layered architecture. |
| **Q1.4** Normalize identifiers? | **Normalize for matching, preserve original** | Store both `canonical: Roman(1)` and `raw_text: "ARTICLE I"`. Use canonical for alignment, raw for display. |
| **Q1.5** Defined terms as sub-sections? | **No** | Terms are semantic (Layer 2), not structural. Align term-by-term in diff engine instead. |

**Note on Roman numerals:** Use the `roman` crate for parsing instead of implementing. It handles edge cases like "XLII" vs "IV".

---

### Section 2: Section Reference Resolver

| Question | Decision | Rationale |
|----------|----------|-----------|
| **Q2.1** Same pass as structure? | **Separate resolver** | Follows layered pattern. `SectionReferenceResolver` depends on `SectionStructureResolver`. |
| **Q2.2** "Section 3" ambiguity? | **Refers to entire section + subsections** | Legal convention. `SectionNode.content_span` covers all children. |
| **Q2.3** Relative reference resolution? | **Third-pass operation** | Pass 2: detect. Pass 3: resolve by correlating with section boundaries. |
| **Q2.4** Track reference purpose? | **Yes, high-confidence patterns only** | "subject to" → Condition, "notwithstanding" → Override. Include confidence scores. |
| **Q2.5** Flag external references? | **Yes, extract document names** | `ReferenceType::External { document: "Master Agreement" }`. Can't resolve but flags dependency. |

---

### Section 3: Temporal Expression Resolver

| Question | Decision | Rationale |
|----------|----------|-----------|
| **Q3.1** Calculate concrete dates? | **No, store structured data only** | Resolvers extract; separate utility computes dates from anchor values. |
| **Q3.2** Risk for vague expressions? | **Yes, via `Scored<T>` + `TemporalRisk` enum** | "reasonable time" gets lower confidence + `Vague` classification. |
| **Q3.3** Reuse layered-amount? | **Yes** | Query `x::attr::<Amount>()` for parsed numbers. Add spelled-out number fallback. |
| **Q3.4** Anchor extraction scope? | **Clause-level boundary, not sentence-level** | Contracts have run-on sentences. Use clause delimiters (`,`, `;`, `and`, `or`, `provided that`) rather than sentence-ending punctuation. Store full `anchor_text` + classified `anchor_type`. |
| **Q3.5** Link to obligations? | **Yes, via post-processing pass** | `ObligationPhrase::temporal_constraint: Option<TemporalExpressionRef>`. Correlate by span. |

---

### Section 4: Document Alignment Algorithm

| Question | Decision | Rationale |
|----------|----------|-----------|
| **Q4.1** Lazy vs eager? | **Eager for top-level, lazy for subsections** | Compute top-level alignment eagerly (typically <20 articles). Subsection alignment computed on-demand and cached. Avoids O(n²) for documents with 500+ subsections. |
| **Q4.2** Partial section moves? | **Section-level alignment only** | Paragraph moves detected as content changes in semantic diff engine. |
| **Q4.3** Confidence threshold? | **Configurable, defaults: ≥0.85 auto, 0.70-0.84 review, <0.50 new/deleted** | Thresholds should be exposed in `AlignmentConfig`. Derive better defaults from empirical testing on real contract corpora. |
| **Q4.4** Method priority? | **Title (exact → fuzzy) → Term anchors → Content similarity** | Ordered by precision descending, cost ascending. Title handles 80%+ of cases. |
| **Q4.5** Paragraph-level? | **No** | Section-level only. Paragraph changes handled by semantic diff. |
| **Q4.6** Definition alignment? | **Yes, term-by-term within Definitions section** | Match by normalized term name. Produces `TermAlignment` for diff engine. |

**Configuration:**
```rust
pub struct AlignmentConfig {
    pub auto_accept_threshold: f64,    // default: 0.85
    pub review_threshold: f64,         // default: 0.70
    pub likely_new_threshold: f64,     // default: 0.50
    pub max_eager_sections: usize,     // default: 50, switch to lazy above this
}
```

---

### Section 5: Semantic Diff Engine

| Question | Decision | Rationale |
|----------|----------|-----------|
| **Q5.1** On-demand text diffs? | **Yes, compute lazily** | Show summary first. Compute `TextDiff` when user drills into change. |
| **Q5.2** Obligation diff granularity? | **Component-level (obligor, type, action, conditions)** | Enables precise classification: `TypeChanged`, `ConditionAdded`, etc. |
| **Q5.3** Rule-based vs LLM party impact? | **Rule-based with clear heuristics** | Fast, deterministic, auditable. LLM as optional enhancement for ambiguous cases. |
| **Q5.4** Incremental diff? | **No, full recomputation** | Contracts change infrequently. Optimize later if needed. |
| **Q5.5** Detect semantic no-ops? | **Normalization pass before diffing** | Compare normalized semantic structures. Text-only changes → `RiskLevel::Info`. |
| **Q5.6** Similarity scores? | **Yes** | Rank changes by magnitude. Aggregate to section and document level. |

---

## Architectural Patterns Summary

The design follows these patterns from the existing codebase:

1. **Layered resolver chain**: Each resolver depends on previous layers via `x::attr::<T>()` queries
2. **Scored<T> wrapper**: Confidence scores with source tracking
3. **Normalization + preservation**: Normalize for matching, preserve original in spans
4. **Token span correlation**: Attributes correlated by overlapping spans
5. **Post-processing for cross-cutting concerns**: Validation, resolution, linking after extraction

---

## Critical Implementation Files

| File | Purpose |
|------|---------|
| `src/ll_line.rs` | Core `Resolver` trait; follow this pattern |
| `layered-contracts/src/defined_term.rs` | Reference for structure detection (multi-pattern) |
| `layered-contracts/src/obligation.rs` | Reference for complex resolver with linking |
| `layered-contracts/src/scored.rs` | `Scored<T>` infrastructure to reuse |
| `layered-amount/src/amounts.rs` | Number parsing to reuse for temporal expressions |

---

## Testing Strategy

### 1. Unit Tests (Per Resolver)
Each resolver should have snapshot tests using `insta`:
```rust
#[test]
fn test_section_header_detection() {
    let line = create_line_from_string("Section 3.1 Payment Terms");
    let line = line.run(&SectionHeaderResolver);
    let display = LLLineDisplay::new(&line);
    display.include::<SectionHeader>();
    insta::assert_snapshot!(display.to_string());
}
```

### 2. Integration Tests (Document Processing)
Test the full pipeline on sample contracts:
```rust
#[test]
fn test_document_structure_extraction() {
    let doc = ContractDocument::from_file("tests/fixtures/sample_nda.txt");
    let doc = doc.run_resolver(&SectionHeaderResolver);
    let structure = DocumentStructureBuilder.process(&mut doc).unwrap();
    insta::assert_debug_snapshot!(structure);
}
```

### 3. Alignment Accuracy (Gold Standard Corpus)
Build a corpus of manually-aligned contract pairs:
```
tests/fixtures/alignment/
├── contract_v1.txt
├── contract_v2.txt
└── expected_alignment.json  # Human-verified alignment
```

Measure precision/recall:
```rust
#[test]
fn test_alignment_accuracy() {
    let (doc_a, doc_b, expected) = load_alignment_test_case("nda_amendment");
    let actual = DocumentAligner::align(&doc_a, &doc_b);
    let metrics = compute_alignment_metrics(&actual, &expected);
    assert!(metrics.precision > 0.90);
    assert!(metrics.recall > 0.90);
}
```

### 4. Property-Based Tests (Fuzzing)
Use `proptest` for edge cases:
- Random section numbering schemes
- Malformed headers
- Missing/duplicate sections
- Unicode edge cases

---

## Implementation Order

```
Phase 1: Foundation
├── ContractDocument struct + DocSpan types
├── SectionHeaderResolver (per-line, detects headers)
├── DocumentStructureBuilder (multi-line, builds tree)
└── TemporalExpressionResolver (per-line)

Phase 2: References & Linking
├── SectionReferenceResolver (per-line, detects references)
├── SectionReferenceLinker (multi-line, resolves targets)
└── TemporalExpressionLinker (links to obligations)

Phase 3: Document Alignment
├── DocumentAligner (two-document comparison)
├── TermAligner (for Definitions section)
└── AlignmentConfig (configurable thresholds)

Phase 4: Semantic Diff Engine
├── TermDiffDriver
├── ObligationDiffDriver
├── SectionDiffDriver
├── ImpactAnalyzer (cascade detection)
└── ContractDiff aggregator

Phase 5: Serialization & UI
└── JSON serialization for web UI
```

---

## Implementation Status

*Updated: December 2024*

### Completed Components

| Component | File | Status |
|-----------|------|--------|
| `ContractDocument` | `document.rs` | ✅ Complete - Document abstraction with line processing |
| `DocPosition`, `DocSpan` | `document.rs` | ✅ Complete - Cross-line position types |
| `SectionHeaderResolver` | `section_header.rs` | ✅ Complete - Detects section headers per-line |
| `SectionReferenceResolver` | `section_reference.rs` | ✅ Complete - Detects section references |
| `SectionReferenceLinker` | `section_reference_linker.rs` | ✅ Complete - Resolves references to sections |
| `DocumentStructureBuilder` | `document_structure.rs` | ✅ Complete - Builds section tree |
| `TemporalExpressionResolver` | `temporal.rs` | ✅ Complete - Detects time expressions |
| `DocumentAligner` | `document_aligner.rs` | ✅ Complete - Aligns sections between versions |
| `SemanticDiffEngine` | `semantic_diff.rs` | ✅ Complete - Produces semantic changes |

### Semantic Diff Features

| Feature | Status | Notes |
|---------|--------|-------|
| Section added/removed detection | ✅ | Via alignment type |
| Obligation modal changes (shall→may) | ✅ | Requires ObligationPhraseResolver |
| Term definition changes | ✅ | Document-level extraction |
| Blast radius computation | ✅ | Counts affected references |
| Temporal expression changes | ✅ | Duration comparisons |
| Risk level classification | ✅ | Critical/High/Medium/Low |
| Two-sided party impact | ✅ | Obligor + beneficiary impacts |
| JSON serialization | ✅ | Full round-trip support |
| External review hints | ✅ | DiffHint integration |

### Not Implemented (Scope Limitations)

| Feature | Reason |
|---------|--------|
| Clause-level extraction | Section + Obligation levels only |
| Condition party extraction | Beneficiary field not populated |
| Per-section blast radius correlation | Blast radius computed at document level |
| Incremental diff | Full recomputation each time |
| LLM integration | Pure Rust, no external calls |

### Architecture Notes

The implementation follows the hybrid architecture from the spec:

```
┌─────────────────────────────────────────────────────────────┐
│                     RUST LAYER                              │
│  (Deterministic, Fast, Confidence-Scored)                   │
├─────────────────────────────────────────────────────────────┤
│  DocumentAligner.compute_candidates() → AlignmentCandidates │
│  SemanticDiffEngine.compute_diff() → SemanticDiffResult     │
│  Both export to JSON for external processing                │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼ Export: JSON
┌─────────────────────────────────────────────────────────────┐
│                   EXTERNAL LAYER                            │
│  (LLM, Expert Review, Domain Knowledge)                     │
├─────────────────────────────────────────────────────────────┤
│  Review low-confidence alignments/changes                   │
│  Provide AlignmentHint[] or DiffHint[] feedback             │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼ Import: Hints
┌─────────────────────────────────────────────────────────────┐
│                     RUST LAYER                              │
├─────────────────────────────────────────────────────────────┤
│  DocumentAligner.apply_hints() → refined AlignmentResult    │
│  SemanticDiffEngine.apply_hints() → refined DiffResult      │
└─────────────────────────────────────────────────────────────┘
```

### Test Coverage

- 243 tests passing in `layered-contracts`
- 16 tests specifically for `SemanticDiffEngine`
- Snapshot tests for document alignment
