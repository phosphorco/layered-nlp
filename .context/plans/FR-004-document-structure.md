# FR-004: Enhanced Document Structure

## Summary

Extend document structure modeling to handle exhibits as separate documents, amendment patterns, recital classification, multi-level section nesting, footnotes, and precedence clauses.

## Current State

`DocumentStructureBuilder` creates a flat tree of sections based on detected headers. `DocumentAligner` compares sections by ID and content similarity. This misses:
- Exhibits/schedules as logically separate documents
- Amendment documents that modify external agreements
- WHEREAS recitals as distinct from operative sections
- Deep nesting in lettered subsections
- Footnote-to-obligation correlation
- Precedence rules affecting interpretation

## Edge Cases

### 1. Exhibits Referencing Back to Main Document

**Problem:** Exhibits may reference definitions or sections from the main document, creating circular dependencies.

```
MASTER AGREEMENT
Article I. Definitions
  "Services" means the services described in Exhibit A.

EXHIBIT A - STATEMENT OF WORK
1.1 Scope. Contractor shall perform the Services (as defined in the Agreement)
    in accordance with Section 3.2 of the Agreement.
```

**Challenges:**
- Forward reference from main → exhibit ("described in Exhibit A")
- Back reference from exhibit → main ("as defined in the Agreement", "Section 3.2")
- Term resolution must span document boundaries
- Cross-document section references need explicit scoping

### 2. Amendment Patterns: Targeted vs. Wholesale

**Problem:** Amendments vary from surgical clause edits to complete section replacements.

```
AMENDMENT #1 (Targeted):
Section 3.1(a) is amended to replace "30 days" with "45 days".

AMENDMENT #2 (Wholesale):
Article IV is hereby deleted in its entirety and replaced with the following:
"Article IV. Indemnification
4.1 The Company shall indemnify..."
```

**Challenges:**
- Targeted amendments require sub-clause-level position tracking
- Wholesale replacements create new section hierarchies
- "Deleted in its entirety" must track all nested subsections
- Amendment stacking: Amendment #3 may amend Amendment #1

### 3. Nested Exhibits (Exhibit to a Schedule)

**Problem:** Multi-level attachment hierarchies create complex ownership chains.

```
MASTER AGREEMENT
  SCHEDULE 1 - PRICING
    Exhibit A to Schedule 1 - Rate Card
    Exhibit B to Schedule 1 - Volume Discounts
  SCHEDULE 2 - SERVICE LEVELS
    Annex 1 to Schedule 2 - SLA Metrics
```

**Challenges:**
- Three-level hierarchy: Main → Schedule → Exhibit
- Canonical identifiers must encode full path ("SCHEDULE:1/EXHIBIT:A")
- "Exhibit A" alone is ambiguous (could be under Schedule 1 or 2)
- Incorporation by reference may cascade through levels

### 4. Recitals with Operative Obligations (Blurred Boundary)

**Problem:** WHEREAS clauses sometimes contain binding language.

```
WHEREAS, the Parties hereby acknowledge that all prior discussions
shall be deemed confidential and Company shall maintain records
of such discussions for a period of five (5) years.
```

**Challenges:**
- "shall maintain" creates an obligation despite recital context
- Lower legal weight of recitals vs. operative sections
- Must flag obligations in recitals as potential enforceability issues
- Some jurisdictions treat recital obligations differently

### 5. Multi-Paragraph Footnotes

**Problem:** Footnotes can span multiple paragraphs with complex structure.

```
3.1 Payment due within 30 calendar days.[1]

---
[1] Calendar days include weekends and holidays. For purposes of
    this Section 3.1:
    
    (a) If the due date falls on a weekend, payment is due the
        following Monday.
    (b) Federal holidays are determined by the jurisdiction of
        the paying party.
    
    The foregoing shall not apply to payments made by wire transfer.
```

**Challenges:**
- Footnote contains its own nested structure ((a), (b))
- Footnote may contain obligations ("payment is due")
- Must link footnote obligations back to main text context
- Footnote structure must not pollute main document hierarchy

### 6. Conflicting Numbering Schemes

**Problem:** Documents mix incompatible numbering systems.

```
Article I
  Section 1.1
    (a) First point
      (i) Sub-point
      (ii) Sub-point
    (b) Second point
      (A) Alternative scheme
      (B) Alternative scheme
        1. Yet another scheme
        2. Continued
```

**Challenges:**
- (a)/(i)/(A)/1 all represent different depth levels
- Depth detection based on parenthesization vs. just the symbol
- Must handle inconsistent schemes within same document
- Indentation provides visual cues not available in plain text

### 7. Multiple Precedence Clauses

**Problem:** Different sections may define conflicting precedence rules.

```
Section 1.2 Precedence. In the event of conflict between the body
of this Agreement and any Exhibit, the body shall control.

Exhibit A
Section A.5 Precedence. In the event of conflict between this
Statement of Work and any other Exhibit, this Statement of Work
shall control.
```

**Challenges:**
- Conflicting precedence rules create meta-conflicts
- Must track which precedence rule governs which scope
- Precedence rules may reference each other
- "Most specific" vs. "most recent" resolution strategies

### 8. Signature Blocks and Operative Text Boundary

**Problem:** Signature blocks mark the end of operative text but may contain commitments.

```
IN WITNESS WHEREOF, the parties have executed this Agreement as of
the Effective Date.

COMPANY:                          CONTRACTOR:
By: _________________             By: _________________
Name:                             Name:
Title:                            Title:
Date:                             Date:

ACKNOWLEDGED AND AGREED:
[Third Party Guarantor]
By: _________________
```

**Challenges:**
- Text before signatures is operative; text after typically is not
- "ACKNOWLEDGED AND AGREED" sections may add parties/obligations
- Signature dates may differ from Effective Date
- Multi-party signatures create party identification challenges

### 9. Floating Boilerplate Sections

**Problem:** Standard sections like "Definitions" or "Miscellaneous" can appear anywhere.

```
Version A:                    Version B:
Article I. Definitions        Article I. Services
Article II. Services          Article II. Payment
Article III. Payment          Article III. Definitions  ← Moved
Article IV. Miscellaneous     Article IV. Term
                              Article V. Miscellaneous
```

**Challenges:**
- Same logical section appears at different positions
- Section renumbering between versions is common
- Must match by content/title, not by position
- Definition sections need special handling (term-by-term alignment)

## Proposed Improvements

### 1. Exhibits as First-Class Documents

**Problem:** Exhibits are treated as sections, not separate documents.

**Example:**
```
MASTER AGREEMENT
Article I. Definitions
Article II. Services
EXHIBIT A - STATEMENT OF WORK
Exhibit A.1 Scope
Exhibit A.2 Deliverables
EXHIBIT B - PRICING
```

**Ask:**
- Detect document boundaries (EXHIBIT, SCHEDULE, APPENDIX, ATTACHMENT)
- Create two-level structure: main document + appendices
- Track incorporation references ("Exhibit A is incorporated herein")
- Enable independent versioning of exhibits

```rust
pub struct DocumentStructure {
    pub main_document: SectionNode,
    pub appendices: Vec<AppendixDocument>,
}

pub struct AppendixDocument {
    pub kind: AppendixKind,  // Exhibit, Schedule, Appendix
    pub identifier: String,  // "A", "1", etc.
    pub title: Option<String>,
    pub sections: Vec<SectionNode>,
    pub incorporated_by_reference: bool,
}
```

### 2. Amendment Document Parsing

**Problem:** Amendments can't be parsed as change instructions.

**Example:**
```
AMENDMENT TO MASTER SERVICES AGREEMENT

The Agreement dated January 1, 2024 is hereby amended as follows:

1. Section 3.1 is hereby deleted and replaced with:
   "Payment Terms. Invoices due within 30 days."

2. Section 4.2 is amended to add at the end:
   "Force majeure excuses performance."

3. A new Section 7 is hereby added:
   "Section 7. Confidentiality..."
```

**Ask:**
- Detect amendment document patterns
- Parse change instructions (DELETE, MODIFY, ADD, REPLACE)
- Store reference to original agreement
- Enable reconstruction: original + amendments = current version

```rust
pub struct AmendmentDocument {
    pub reference_to_original: String,
    pub effective_date: Option<String>,
    pub amendments: Vec<Amendment>,
}

pub enum Amendment {
    Delete { section_id: String },
    Modify { section_id: String, change: String },
    Add { section_id: String, content: SectionNode },
    Replace { section_id: String, new_content: String },
}
```

### 3. Recital Classification

**Problem:** WHEREAS clauses aren't distinguished from operative sections.

**Example:**
```
WHEREAS, Company desires to engage Contractor for services;
WHEREAS, the parties have negotiated terms;
NOW, THEREFORE, the parties agree:
1. Services...
```

**Ask:**
- Detect WHEREAS/recital patterns
- Classify as preamble (informational, lower legal weight)
- Flag obligations appearing in recitals (unusual, may be unenforceable)
- Distinguish recital changes from operative changes in diff

### 4. Multi-Level Section Nesting

**Problem:** Lettered subsections like (a)(i)(ii) are treated as same depth.

**Example:**
```
Section 3. Payment
(a) Due Date
    (i) Invoice timing
    (ii) Holiday extension
(b) Late Payment
```

**Ask:**
- Detect nesting by indentation or numbering pattern
- Track parent-child relationships for (a) → (i), (ii)
- Preserve hierarchy in document structure
- Enable hierarchical alignment in diff

### 5. Footnote Registry

**Problem:** Footnotes aren't linked to obligations.

**Example:**
```
3.1 Payment due within 30 calendar days.[1]

---
[1] Calendar days include weekends and holidays.
```

**Ask:**
- Detect footnote markers in text ([1], *, †)
- Parse footnote content section
- Link markers to content
- Propagate footnote changes to affected obligations

```rust
pub struct Footnote {
    pub marker: String,
    pub content: String,
    pub referenced_by: Vec<DocSpan>,
}
```

### 6. Precedence Clause Detection

**Problem:** Precedence rules aren't applied to conflict resolution.

**Example:**
```
5.4 Precedence. In event of conflict: (a) schedules, (b) articles, (c) recitals.
```

**Ask:**
- Detect precedence/priority clauses
- Extract ordering rules
- Apply to conflict detection in semantic diff
- Flag precedence changes as high-risk

## Engineering Approach

### Core Type Enhancements

```rust
/// Enhanced document structure with appendix support.
pub struct DocumentStructure {
    /// Root-level sections of the main document body
    pub main_body: DocumentBody,
    /// Attached documents (exhibits, schedules, etc.)
    pub appendices: Vec<AppendixDocument>,
    /// Detected recital/preamble section
    pub preamble: Option<PreambleSection>,
    /// Footnote registry for the entire document
    pub footnotes: FootnoteRegistry,
    /// Precedence rules detected in this document
    pub precedence_rules: Vec<PrecedenceRule>,
    /// Signature block boundary (if detected)
    pub signature_boundary: Option<usize>,
}

/// The main body of a document (before exhibits/schedules).
pub struct DocumentBody {
    pub title: Option<String>,
    pub sections: Vec<SectionNode>,
    /// Line index where main body ends (before first appendix)
    pub end_line: usize,
}

/// A document attached to the main agreement.
#[derive(Debug, Clone)]
pub struct AppendixDocument {
    /// Type of appendix
    pub kind: AppendixKind,
    /// Identifier ("A", "1", "I", etc.)
    pub identifier: String,
    /// Optional title ("Statement of Work")
    pub title: Option<String>,
    /// Full path for nested appendices ("SCHEDULE:1/EXHIBIT:A")
    pub canonical_path: String,
    /// Parent appendix if nested (e.g., "Exhibit A to Schedule 1")
    pub parent: Option<Box<AppendixDocument>>,
    /// Internal section structure
    pub sections: Vec<SectionNode>,
    /// Whether incorporated by reference in main document
    pub incorporation: Option<IncorporationReference>,
    /// Span covering the entire appendix
    pub span: DocSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppendixKind {
    Exhibit,
    Schedule,
    Appendix,
    Annex,
    Attachment,
    Addendum,
}

/// Reference to where an appendix is incorporated.
#[derive(Debug, Clone)]
pub struct IncorporationReference {
    /// Section containing the incorporation
    pub incorporating_section: String,
    /// Line number of incorporation statement
    pub line: usize,
    /// Explicit "incorporated by reference" or implied
    pub explicit: bool,
}
```

### Amendment Types

```rust
/// A parsed amendment document.
#[derive(Debug, Clone)]
pub struct AmendmentDocument {
    /// Reference to the document being amended
    pub target_reference: AmendmentTarget,
    /// Effective date of the amendment
    pub effective_date: Option<TemporalReference>,
    /// Amendment number (if specified)
    pub amendment_number: Option<u32>,
    /// Individual amendment instructions
    pub instructions: Vec<AmendmentInstruction>,
    /// Whether this amends another amendment
    pub amends_amendment: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AmendmentTarget {
    /// Named agreement with optional date
    NamedAgreement {
        name: String,
        dated: Option<String>,
    },
    /// Reference to prior amendment
    PriorAmendment {
        amendment_number: u32,
    },
}

#[derive(Debug, Clone)]
pub struct AmendmentInstruction {
    /// Target section being modified
    pub target: SectionTarget,
    /// Type of modification
    pub operation: AmendmentOperation,
    /// New content (for Add/Replace operations)
    pub new_content: Option<String>,
    /// Parsed section structure of new content
    pub new_sections: Vec<SectionNode>,
    /// Span in the amendment document
    pub source_span: DocSpan,
    /// Confidence in parsing this instruction
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub enum SectionTarget {
    /// Full section reference ("Section 3.1")
    Section { id: String },
    /// Subsection reference ("Section 3.1(a)")
    Subsection { section_id: String, subsection: String },
    /// Clause-level reference ("Section 3.1(a)(ii)")
    Clause { section_id: String, path: Vec<String> },
    /// Text-based reference ("the second paragraph of Section 5")
    TextReference { section_id: String, description: String },
}

#[derive(Debug, Clone)]
pub enum AmendmentOperation {
    /// Delete section entirely
    Delete,
    /// Delete and replace with new content
    Replace,
    /// Insert new content at the end
    AppendToEnd,
    /// Insert new content at the beginning
    PrependToBeginning,
    /// Insert after specific text
    InsertAfter { after_text: String },
    /// Targeted text replacement within section
    TextSubstitution { old_text: String, new_text: String },
    /// Add a new section (not replacing existing)
    AddNew,
    /// Renumber section
    Renumber { new_id: String },
}
```

### Preamble and Recital Types

```rust
/// The preamble/recitals section of a document.
#[derive(Debug, Clone)]
pub struct PreambleSection {
    /// Individual recital clauses
    pub recitals: Vec<Recital>,
    /// Span covering the entire preamble
    pub span: DocSpan,
    /// Transition marker ("NOW, THEREFORE")
    pub transition: Option<TransitionMarker>,
}

#[derive(Debug, Clone)]
pub struct Recital {
    /// Recital identifier ("A", "1", or implicit ordering)
    pub identifier: Option<String>,
    /// Full text of the recital
    pub text: String,
    /// Obligations detected within recital (flagged as unusual)
    pub embedded_obligations: Vec<EmbeddedObligation>,
    /// Span within document
    pub span: DocSpan,
}

#[derive(Debug, Clone)]
pub struct EmbeddedObligation {
    /// The obligation phrase detected
    pub phrase: String,
    /// Warning message about enforceability
    pub warning: String,
    /// Span of the obligation within the recital
    pub span: DocSpan,
}

#[derive(Debug, Clone)]
pub struct TransitionMarker {
    /// Common patterns: "NOW, THEREFORE", "NOW THEREFORE", "IT IS AGREED"
    pub text: String,
    /// Line number
    pub line: usize,
}
```

### Footnote Types

```rust
/// Registry of all footnotes in a document.
#[derive(Debug, Clone, Default)]
pub struct FootnoteRegistry {
    /// All footnotes indexed by marker
    pub footnotes: HashMap<String, Footnote>,
    /// Markers in order of appearance
    pub order: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Footnote {
    /// Marker text ("[1]", "*", "†", etc.)
    pub marker: String,
    /// Footnote content (may span multiple paragraphs)
    pub content: FootnoteContent,
    /// All locations where this footnote is referenced
    pub references: Vec<FootnoteReference>,
}

#[derive(Debug, Clone)]
pub struct FootnoteContent {
    /// Full text content
    pub text: String,
    /// Nested structure within footnote (if any)
    pub nested_sections: Vec<SectionNode>,
    /// Obligations detected within footnote
    pub embedded_obligations: Vec<DocSpan>,
    /// Span covering the footnote definition
    pub definition_span: DocSpan,
}

#[derive(Debug, Clone)]
pub struct FootnoteReference {
    /// Section containing the reference
    pub section_id: Option<String>,
    /// Line number of the reference
    pub line: usize,
    /// Token position of the marker
    pub token_pos: usize,
    /// Surrounding context text
    pub context: String,
}

impl FootnoteRegistry {
    pub fn get(&self, marker: &str) -> Option<&Footnote> {
        self.footnotes.get(marker)
    }

    pub fn references_in_section(&self, section_id: &str) -> Vec<(&str, &FootnoteReference)> {
        self.footnotes
            .iter()
            .flat_map(|(marker, fn_)| {
                fn_.references
                    .iter()
                    .filter(|r| r.section_id.as_deref() == Some(section_id))
                    .map(move |r| (marker.as_str(), r))
            })
            .collect()
    }
}
```

### Precedence Rule Types

```rust
/// A detected precedence/priority rule.
#[derive(Debug, Clone)]
pub struct PrecedenceRule {
    /// Section where this rule is defined
    pub source_section: String,
    /// Line number of the rule
    pub source_line: usize,
    /// The ordering specified (highest priority first)
    pub ordering: Vec<PrecedenceLevel>,
    /// Scope of this rule (entire document or specific sections)
    pub scope: PrecedenceScope,
    /// Raw text of the precedence clause
    pub raw_text: String,
    /// Confidence in parsing
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrecedenceLevel {
    /// Main body of the agreement
    MainBody,
    /// Specific appendix type
    AppendixType(AppendixKind),
    /// Specific named appendix
    NamedAppendix { kind: AppendixKind, identifier: String },
    /// Recitals/preamble
    Recitals,
    /// Section kind (Articles, Sections, etc.)
    SectionKind(SectionKind),
}

#[derive(Debug, Clone)]
pub enum PrecedenceScope {
    /// Applies to entire document
    Document,
    /// Applies only within a specific appendix
    WithinAppendix { appendix_path: String },
    /// Applies between specific items only
    BetweenItems { items: Vec<String> },
}

/// Result of applying precedence to a conflict.
#[derive(Debug, Clone)]
pub struct PrecedenceResolution {
    /// The precedence rule applied
    pub rule: PrecedenceRule,
    /// Which provision "wins"
    pub winner: DocSpan,
    /// Which provision is superseded
    pub loser: DocSpan,
    /// Explanation of resolution
    pub explanation: String,
}
```

### Enhanced SectionNode

```rust
/// Enhanced section node with cross-reference support.
#[derive(Debug, Clone)]
pub struct SectionNode {
    /// The section header information
    pub header: SectionHeader,
    /// Line number where this section starts
    pub start_line: usize,
    /// Line number where this section ends (exclusive)
    pub end_line: Option<usize>,
    /// Document span covering the entire section
    pub content_span: DocSpan,
    /// Child sections (subsections)
    pub children: Vec<SectionNode>,
    /// Numbering scheme used in this section
    pub numbering_scheme: NumberingScheme,
    /// Cross-references detected within this section
    pub internal_references: Vec<InternalReference>,
    /// Footnote markers appearing in this section
    pub footnote_markers: Vec<String>,
    /// Classification for special handling
    pub classification: SectionClassification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberingScheme {
    /// 1, 2, 3
    ArabicNumeric,
    /// I, II, III
    RomanUpper,
    /// i, ii, iii
    RomanLower,
    /// A, B, C
    AlphaUpper,
    /// a, b, c
    AlphaLower,
    /// (a), (b), (c)
    AlphaParenthesized,
    /// (i), (ii), (iii)
    RomanParenthesized,
    /// 1.1, 1.2, 1.3
    DottedNumeric,
    /// No numbering detected
    None,
}

#[derive(Debug, Clone)]
pub struct InternalReference {
    /// Target of the reference ("Section 3.1", "Exhibit A")
    pub target: String,
    /// Whether target is within same document or in appendix
    pub target_location: ReferenceLocation,
    /// Span of the reference text
    pub span: DocSpan,
}

#[derive(Debug, Clone)]
pub enum ReferenceLocation {
    SameDocument,
    Appendix { path: String },
    External { document_name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionClassification {
    /// Normal operative section
    Operative,
    /// Definitions section (term-by-term alignment)
    Definitions,
    /// Boilerplate (miscellaneous, general provisions)
    Boilerplate,
    /// Signature block area
    SignatureBlock,
    /// Recital/preamble content
    Preamble,
}
```

### Resolver Implementations

```rust
/// Resolver for detecting preamble/recital sections.
pub struct RecitalResolver {
    /// Keywords that indicate recital start
    recital_keywords: Vec<&'static str>,
    /// Keywords that indicate transition to operative text
    transition_keywords: Vec<&'static str>,
}

impl Default for RecitalResolver {
    fn default() -> Self {
        Self {
            recital_keywords: vec!["WHEREAS", "RECITALS", "BACKGROUND", "WITNESSETH"],
            transition_keywords: vec![
                "NOW, THEREFORE",
                "NOW THEREFORE",
                "IT IS AGREED",
                "THE PARTIES AGREE",
                "IN CONSIDERATION",
            ],
        }
    }
}

/// Resolver for detecting footnote markers and definitions.
pub struct FootnoteResolver {
    /// Regex patterns for markers: [1], *, †, (1), etc.
    marker_patterns: Vec<regex::Regex>,
}

/// Resolver for detecting appendix boundaries.
pub struct AppendixBoundaryResolver {
    /// Keywords that indicate appendix start
    appendix_keywords: Vec<&'static str>,
}

impl Default for AppendixBoundaryResolver {
    fn default() -> Self {
        Self {
            appendix_keywords: vec![
                "EXHIBIT", "SCHEDULE", "APPENDIX", "ANNEX", "ATTACHMENT", "ADDENDUM",
            ],
        }
    }
}

/// Resolver for detecting precedence clauses.
pub struct PrecedenceClauseResolver {
    /// Keywords that indicate precedence rules
    precedence_keywords: Vec<&'static str>,
}

impl Default for PrecedenceClauseResolver {
    fn default() -> Self {
        Self {
            precedence_keywords: vec![
                "precedence",
                "prevail",
                "control",
                "supersede",
                "in the event of conflict",
                "in case of conflict",
                "inconsistency",
            ],
        }
    }
}

/// Resolver for detecting amendment instructions.
pub struct AmendmentInstructionResolver {
    /// Patterns for amendment operations
    operation_patterns: AmendmentPatterns,
}

#[derive(Default)]
struct AmendmentPatterns {
    delete_patterns: Vec<&'static str>,
    replace_patterns: Vec<&'static str>,
    add_patterns: Vec<&'static str>,
    modify_patterns: Vec<&'static str>,
}

impl Default for AmendmentInstructionResolver {
    fn default() -> Self {
        Self {
            operation_patterns: AmendmentPatterns {
                delete_patterns: vec![
                    "is hereby deleted",
                    "is deleted in its entirety",
                    "shall be deleted",
                ],
                replace_patterns: vec![
                    "is hereby deleted and replaced",
                    "is amended and restated",
                    "shall read as follows",
                ],
                add_patterns: vec![
                    "is hereby added",
                    "is amended to add",
                    "shall be added",
                    "the following is added",
                ],
                modify_patterns: vec![
                    "is amended to",
                    "is hereby amended",
                    "shall be amended",
                ],
            },
        }
    }
}
```

### Document Processor Integration

```rust
/// Enhanced document structure builder.
pub struct EnhancedDocumentStructureBuilder {
    /// Whether to detect appendices
    pub detect_appendices: bool,
    /// Whether to detect recitals
    pub detect_recitals: bool,
    /// Whether to detect footnotes
    pub detect_footnotes: bool,
    /// Whether to detect precedence rules
    pub detect_precedence: bool,
}

impl DocumentProcessor for EnhancedDocumentStructureBuilder {
    type Output = DocumentStructure;

    fn process(&self, doc: &ContractDocument) -> ProcessResult<Self::Output> {
        let mut result = ProcessResult::ok(DocumentStructure::default());
        
        // Phase 1: Detect appendix boundaries
        let appendix_boundaries = if self.detect_appendices {
            self.detect_appendix_boundaries(doc)
        } else {
            Vec::new()
        };
        
        // Phase 2: Build main body structure (up to first appendix)
        let main_end = appendix_boundaries.first().map(|b| b.start_line).unwrap_or(doc.line_count());
        result.value.main_body = self.build_body_structure(doc, 0, main_end);
        
        // Phase 3: Build appendix structures
        for (i, boundary) in appendix_boundaries.iter().enumerate() {
            let end = appendix_boundaries.get(i + 1).map(|b| b.start_line).unwrap_or(doc.line_count());
            let appendix = self.build_appendix(doc, boundary, end);
            result.value.appendices.push(appendix);
        }
        
        // Phase 4: Detect preamble/recitals
        if self.detect_recitals {
            result.value.preamble = self.detect_preamble(doc, main_end);
        }
        
        // Phase 5: Build footnote registry
        if self.detect_footnotes {
            result.value.footnotes = self.build_footnote_registry(doc);
        }
        
        // Phase 6: Detect precedence rules
        if self.detect_precedence {
            result.value.precedence_rules = self.detect_precedence_rules(doc);
        }
        
        result
    }
}
```

## Success Criteria

- [ ] Exhibits parsed as separate documents with incorporation tracking
- [ ] Amendment documents parsed into change instructions
- [ ] Recitals classified and distinguished from operative sections
- [ ] Multi-level nesting preserved in section hierarchy
- [ ] Footnotes linked to referencing obligations
- [ ] Precedence rules detected and available for conflict resolution
- [ ] Edge cases for nested exhibits handled correctly
- [ ] Amendment stacking (amending amendments) supported
- [ ] Obligations in recitals flagged with enforceability warnings
- [ ] Multiple precedence clauses detected with scope tracking

## Related Edge Cases

- Exhibits as documents (Document #2)
- Amendment patterns (Document #4)
- WHEREAS recitals (Document #3)
- Multi-level subsections (Document #1)
- Footnotes (Document #8)
- Precedence clauses (Document #7)
- Reordered sections (Document #6)

## Dependencies

- Requires FR-003 (Cross-Line Spans) for multi-line section content and `SemanticSpan` infrastructure
- Enhances FR-002 (Reference Resolution) with section content access and cross-document references
- Foundation for FR-006 (Semantic Analysis) precedence application in conflict resolution
- `DocumentResolver` trait from FR-003 enables cross-line structure detection

## Integration Points

### FR-003: Cross-Line Spans
- `SemanticSpan` used for multi-paragraph footnotes
- `DocumentResolver` trait enables document-wide structure detection
- Multi-line recital detection uses cross-line infrastructure

### FR-002: Reference Resolution
- `InternalReference` type provides targets for resolution
- Appendix scoping enables cross-document reference resolution
- `ReferenceLocation` distinguishes same-document vs. appendix references

### FR-006: Semantic Analysis
- `PrecedenceRule` feeds into conflict resolution
- `PrecedenceResolution` type enables precedence-aware diff
- Recital classification affects obligation weight in conflict detection
