# Associative Spans: A Vision for Relationship-Rich Text Analysis

> Every extracted fact should prove its origin. Every connection should be navigable. Every claim should have a trail.

## What Are Associative Spans?

Associative Spans transform text analysis from pattern extraction into **graph construction**. When the system identifies an obligation like "the Company shall deliver goods," it doesn't just extract the fact—it creates typed pointers back to the exact words that generated each field:

```
shall deliver goods
╰───╯ ObligationPhrase { obligor: "Company", action: "deliver goods" }
  └─@obligor_source─>[Company definition at tokens 5-8]
  └─#action_span─>[tokens 12-16]
```

These associations are first-class citizens in the data model: stored alongside values, queryable via API, renderable in displays, and traversable as a graph.

---

## Core Capabilities

### 1. Provenance Tracking
Every extracted attribute carries proof of where it came from. No more "trust me" extractions—every field links to its source text.

### 2. Relationship Graphs
Associations knit spans across layers into navigable networks. An obligation connects to its obligor definition, which connects to its term references, which connect to their usage sites.

### 3. Confidence Propagation
When a source span has low confidence, that uncertainty flows through association chains. A pronoun with ambiguous resolution makes every obligation using it less certain.

### 4. Bidirectional Navigation
Follow associations forward ("what does this term affect?") or backward ("where did this obligation come from?"). The graph works both ways.

### 5. Typed Semantics
Associations carry meaning: `@obligor_source` vs `#action_span` vs `→defined_in`. Types enable filtering, visualization, and domain-specific reasoning.

---

## Use Case Taxonomy

### A. Legal & Contract Analysis

#### Interactive "Explain This Clause" Mode
Junior lawyers hover over an obligation and instantly see the provenance chain: which definition established the obligor, where the action verb came from, what conditions qualify it. Every extracted field proves its origin.

#### Negotiation Tracking with Audit Trails
Across 15 redline versions, track how specific obligations evolved. Associations reveal when an `ObligorSource` shifted from a defined term to a pronoun (introducing ambiguity) or when an action verb weakened from "shall" to "may."

#### Risk Heatmaps with Drill-Down
Flag high-risk clauses (unlimited liability, broad indemnification) and let reviewers click through to see exactly why: follow the association chain from risk score → obligation → uncapped language → missing condition.

#### Template Variance Detection
Compare executed contracts against templates at the semantic level. "87% alignment, but this contract uses pronouns where the template had defined terms—potential ambiguity introduced."

#### Cross-Document Portfolio Analysis
Aggregate obligations across 300 leases: "Show me all maintenance duties where obligor is 'Landlord' and deadline contains 'annually'." Associations enable federated queries across document boundaries.

#### Regulatory Compliance Verification
Define compliance rules as required association patterns: "Indemnification must have ObligorSource → 'Sponsor' AND ConditionLink → 'regulatory violations'." Generate audit reports with exact token positions.

---

### B. Diagnostics & Quality Assurance

#### Orphaned Definition Detection
A defined term with zero incoming `TermReference` associations indicates dead definitions—clutter or copy-paste errors. Surface them with squiggly underlines like an IDE.

```
Section 1.1: Warning - Unused Definition
    "Commencement Date" means the first day of service.
    └─⚠ No references found. Did you mean "Effective Date" (used 3 times)?
```

#### Ambiguous Pronoun Warnings
When `PronounReference` has multiple `AntecedentCandidate` associations with similar confidence, flag the ambiguity:

```
"It shall maintain insurance."
 ╰─╯ Ambiguous pronoun
    └─@antecedent─>[A] "Company" (0.72)
    └─@antecedent─>[B] "Client" (0.68)
```

#### Circular Reference Detection
Graph traversal finds cycles: obligation → condition → section reference → back to the obligation. Temporal logic validation catches impossible constraints: "Delivery due 30 days after Effective Date" where Effective Date is "30 days after delivery."

#### Broken Section References
When `Section 7.5` doesn't exist, show smart suggestions based on content similarity and structural analysis:

```
❌ Section 7.5 does not exist
💡 Did you mean Section 7.4 "Service Commencement"?
   Similarity: High - contains keywords "services", "commence"
```

#### Confidence Heatmaps
Document-wide quality reports showing which sections have uncertain interpretations. Click through to see why: "Flagged because pronoun chain traverses 3 references with decaying confidence."

---

### C. Collaboration & Annotation

#### Evidence-Based Commentary
Reviewers add comments that link to specific evidence via associations. "This clause is problematic → see conflicting definition [A] and contradictory Section [B]." Machine validates that referenced spans exist.

#### Multi-Party Approval Workflows
Legal approves based on [A], Finance requests changes because of [B], Business disputes interpretation citing [C]. Associations surface conflicts: "Two reviewers point to the same obligation with different verdicts."

#### Question-Answer Knowledge Bases
"What happens if we miss a deadline?" → Answer links to relevant obligation, condition, and penalty spans via associations. Build searchable Q&A from accumulated question-evidence pairs.

#### Training Data Curation
Annotators mark extractions as TruePositive/FalsePositive/FalseNegative with `KeyEvidence` and `ConfusingPattern` associations explaining why. Generate training features: "When 'shall' appears in hypothetical context, reduce confidence."

---

### D. Visualization & Navigation

#### Provenance Spotlight
Hover over any extracted field to see a visual beam highlighting its source span. Intensity indicates confidence. Every structured extraction proves its origin.

#### Obligation Flow Graphs
Network diagrams where nodes are parties and edges are obligations. Click an edge to highlight the source text. Filter by party to see "what does this contract require of me?"

#### Time-Travel Timeline
Horizontal timeline showing temporal expressions as milestones. Walk through contract execution: "What happens when?" Export schedules directly from association structure.

#### Definition Ripple Visualization
When a defined term changes, concentric ripples show every usage site. Color intensity indicates criticality (high-risk obligations vs. informational clauses).

#### Semantic Diff Heat Maps
Side-by-side comparison colored by impact: blue for text changes, red for semantic changes (obligation type shifted), purple for cascading effects (broken references).

#### Party Perspective Lens
"View as [Party Name]" - your obligations in red, your rights in green, neutral text grayed. Export your checklist of duties with deadlines.

---

### E. Cross-Domain Applications

#### Scientific Papers
Link claims to evidence, methods to results, citations to source text. Trace the provenance of every assertion for reproducibility.

**Association Types:**
- `ClaimEvidence` - assertion → supporting data/figure
- `MethodApplication` - result → methodology that produced it
- `CitationSource` - paraphrase → original cited text

#### Technical Documentation
Bidirectional links between prose and code. Detect documentation drift when docs describe features that don't exist.

**Association Types:**
- `ImplementedBy` - feature description → function/class
- `ExampleUsage` - API description → code sample
- `DeprecationReplacement` - old API → new API

#### Medical Records
Clinical reasoning chains from symptoms → differential diagnoses → tests → findings → diagnosis → treatment.

**Association Types:**
- `SymptomToHypothesis` - symptom → diagnostic consideration
- `FindingSupports` - test result → diagnosis it confirms
- `TreatmentForDiagnosis` - prescription → condition being treated

#### Investigative Journalism
Claim-source verification networks. Every fact links to primary sources, corroboration, or contradiction.

**Association Types:**
- `PrimarySource` - claim → supporting document/interview
- `Corroboration` - source → another source making same claim
- `Attribution` - quote → person who said it

#### Educational Content
Prerequisite graphs showing which concepts must be understood before others.

**Association Types:**
- `RequiresUnderstanding` - concept → prerequisite concepts
- `ExampleOf` - worked example → concept it demonstrates
- `ExerciseAssesses` - problem → skills it tests

#### Support Knowledge Bases
Issue-solution graphs for troubleshooting navigation.

**Association Types:**
- `CausedBy` - symptom → root cause
- `SolvedBy` - problem → solution
- `EscalationPath` - frontline fix → expert intervention

---

## Architectural Principles

### Associations Are First-Class
Not post-hoc metadata—stored in `TypeBucket` alongside values, indexed for lookup, included in all query APIs.

### Type-Safe Semantics
The `Association` trait requires `label() -> &'static str` and optional `glyph()`. Types are compile-time checked; labels enable runtime filtering.

### Parallel Storage Alignment
Values and associations stored in parallel vectors, maintaining index alignment. `insert_with_associations()` ensures the invariant.

### Graph Traversal Ready
`query_with_associations<T>()` returns `Vec<(range, text, Vec<(&T, &[AssociatedSpan])>)>` enabling multi-hop reasoning.

### Confidence-Aware
`Scored<T>` wrapper carries confidence that can propagate through association chains.

---

## Implementation Status

### Complete (Line-Local)
- Core types: `SpanRef`, `Association` trait, `AssociatedSpan`
- Storage: Parallel association vectors in `TypeBucket`
- Builder API: `selection.assign(value).with_association(type, span).build()`
- Display: Arrow rendering with `include_with_associations<T>()`
- Contract domain: `ObligorSource`, `ActionSpan` associations
- Public API: `query_with_associations<T>()` for downstream consumers
- WASM demo: Provenance data in JSON output

### Complete (Cross-Line Extension - FR-003, December 2024)
- `DocPosition` and `DocSpan` for cross-line positions
- `DocAssociatedSpan` - reuses same `Association` trait with `DocSpan` instead of `SpanRef`
- `SemanticSpan` with type-erased storage and cross-line association support
- `SpanIndex` for O(log n) lookup of document-level spans
- `DocumentResolver` trait for document-level pattern detection
- `ContractDocument.query_doc<T>()` and `query_all<T>()` for unified queries
- **Key Design:** Same `Association` trait works for both line-local and document-level associations

### Future Directions
- Cross-document associations (linking spans across files)
- Association validation rules (every obligation must link to a definition)
- Bidirectional query API ("what references this term?")
- Confidence propagation through chains
- Visual graph explorer in web UI
- More association types: `ConditionSpan`, `TemporalAnchor`, `SectionReference`

---

## The Big Picture

Associative Spans transform layered-nlp from a **text analysis tool** into a **knowledge graph engine**. Every extraction becomes a node, every relationship becomes an edge, and the document becomes a navigable network where:

- **Every claim has proof** - provenance chains back to source text
- **Every connection has meaning** - typed associations carry semantics
- **Every uncertainty is visible** - confidence flows through the graph
- **Every perspective is available** - filter by party, time, type, or risk

The document is no longer a wall of text to be read—it's a structured world to be explored.

---

## Getting Started

```rust
use layered_nlp::{Association, AssociatedSpan, SpanRef};

// Define a custom association type
#[derive(Debug, Clone)]
struct EvidenceLink;
impl Association for EvidenceLink {
    fn label(&self) -> &'static str { "evidence" }
    fn glyph(&self) -> Option<&'static str> { Some("📊") }
}

// In your resolver, emit associations
let assignment = selection
    .assign(MyClaim { text: "..." })
    .with_association(EvidenceLink, evidence_selection.span_ref())
    .build();

// Query with associations
for (range, text, attrs) in ll_line.query_with_associations::<MyClaim>() {
    for (claim, associations) in attrs {
        for assoc in associations {
            println!("{} → {} at {:?}",
                claim.text, assoc.label(), assoc.span);
        }
    }
}
```

---

*This document captures the vision for Associative Spans as of December 2024. The feature is actively evolving—contributions and use case suggestions welcome.*
