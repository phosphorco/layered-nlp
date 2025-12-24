# FR-002: Advanced Reference Resolution

## Summary

Extend reference resolution beyond simple pronoun-antecedent matching to handle bridging references, split antecedents, cross-references to document sections, and transitive participant chains.

## Current State

The `PronounResolver` matches pronouns to prior defined terms using string similarity and token distance. The `SectionReferenceResolver` detects section references but doesn't link them to actual content. This misses:
- Implicit semantic relationships (bridging)
- Conjunctive antecedents ("Buyer and Seller... They")
- Resolution of cross-referenced content
- Chains of participants introduced transitively

## Edge Cases

### 1. Ambiguous Pronouns with Multiple Candidate Antecedents

**Problem:** When multiple viable antecedents exist, the current distance-based heuristic may select incorrectly.

**Examples:**
```
The Company shall notify the Contractor. It shall respond within 10 days.
```
(Does "It" refer to Company or Contractor? Syntactically ambiguous; requires semantic role analysis.)

```
The Buyer shall pay the Seller. The Seller shall deliver goods. They shall cooperate.
```
("They" has two possible antecedent resolutions: {Buyer, Seller} jointly, or just Seller as the most recent.)

**Signals for disambiguation:**
- Semantic role: Subject vs Object position
- Verb semantics: "respond" implies recipient, "pay" implies payer
- Plural agreement: "They" must match plurality of candidates
- Section context: Obligations typically bind the same party within a clause

### 2. Split Antecedents (Conjunctive NPs)

**Problem:** Plural pronouns referring to coordinated noun phrases.

**Examples:**
```
The Buyer and Seller (the "Parties") agree. They shall negotiate in good faith.
```
```
Buyer and Seller shall each indemnify the other for losses caused by such party.
```
("each...the other...such party" creates reciprocal references within a single sentence.)

**Patterns to detect:**
- `X and Y (the "Z")` → defines Z as plural entity
- `X and Y shall each` → distributive predicate
- `the other party` → reflexive reference to coordination partner
- `such party` / `such parties` → anaphoric to nearest party mention

### 3. Cataphora (Forward References)

**Problem:** Pronouns appear before their antecedents.

**Examples:**
```
Before it terminates, this Agreement shall remain in force.
```
("it" refers forward to "Agreement")

```
Unless otherwise specified herein, the defined terms shall have the meanings given.
```
("herein" refers to the document not yet fully introduced)

**Detection strategy:**
- Trigger cataphora scan when backward resolution yields no candidates
- Lower confidence for cataphoric resolutions (0.6x multiplier)
- Common patterns: "Before X...", "Until X...", "Unless X...", "If X..."

### 4. Nested Cross-References

**Problem:** References contain sub-references requiring recursive resolution.

**Examples:**
```
as defined in Section 4.2(a)(iii) of Exhibit B hereto
```
(Three levels: Section → subsection → attached exhibit)

```
subject to the limitations set forth in Section 3 above and Article V below
```
(Multiple targets with relative position markers)

```
the Fee as set forth on Schedule 1 attached hereto and incorporated herein
```
(Schedule reference with incorporation clause)

**Structure needed:**
- Hierarchical reference tree, not flat list
- Track reference depth for risk assessment
- Detect "incorporated herein/hereto" patterns

### 5. Circular or Mutually-Dependent Definitions

**Problem:** Terms defined in terms of each other create resolution cycles.

**Examples:**
```
"Affiliate" means any entity that Controls, is Controlled by, or is under
common Control with the Company.

"Control" means ownership of Voting Securities of any Affiliate.
```
(Circular: Affiliate → Control → Affiliate)

**Detection:**
- Build definition dependency graph
- Detect strongly-connected components (cycles)
- Flag for human review with appropriate warning

### 6. Legal Deictics ("herein/hereof/hereby" family)

**Problem:** These archaic forms have document-scoped semantics.

**Examples:**
```
the terms and conditions contained herein
hereby agrees and acknowledges
the rights granted hereunder
the parties hereto
as defined hereinabove / hereinbelow
```

**Semantics mapping:**
| Term | Scope | Direction |
|------|-------|-----------|
| herein | this document | any |
| hereof | this document | any |
| hereby | this clause/sentence | immediate |
| hereunder | this document/section | following |
| hereto | this document | attachment |
| hereinabove | this document | preceding |
| hereinbelow | this document | following |
| thereof | that (referenced) document | any |
| therein | that (referenced) document | any |

### 7. Cross-Document References

**Problem:** References to external documents (exhibits, amendments, master agreements).

**Examples:**
```
as set forth in the Master Agreement dated January 1, 2024
pursuant to Exhibit C attached hereto
as amended by Amendment No. 1
incorporated by reference as if fully set forth herein
```

**Required tracking:**
- External document registry (exhibits, schedules, annexes)
- Document hierarchy (amendments modify base)
- "Incorporated by reference" creates implicit content

### 8. Bridging Inferences

**Problem:** References to semantically-related but not explicitly mentioned concepts.

**Examples:**
```
The Contractor shall deliver Products. The shipment must arrive on time.
```
("shipment" bridges to "delivery of Products" via semantic frame)

```
The Company shall make a Payment. The funds must be in USD.
```
("funds" bridges to "Payment" via meronymy/part-of relation)

**Bridging relation types:**
- Part-whole: "Products" → "shipment" (contained by)
- Event-participant: "Payment" → "funds" (instrument)
- Result: "termination" → "the terminated Agreement"
- Attribute: "the price" → "the Products" (price of)

## Engineering Approach

### Core Type Definitions

```rust
use layered_nlp::{Association, AssociatedSpan, SpanRef, LLSelection, Resolver};
use crate::scored::Scored;
use crate::document::{DocPosition, DocSpan};

/// A resolved reference with its target and resolution metadata.
#[derive(Debug, Clone)]
pub struct ResolvedReference {
    /// The referring expression (pronoun, term reference, section reference)
    pub referent_type: ReferentType,
    /// Resolution candidates ranked by confidence
    pub candidates: Vec<ResolutionCandidate>,
    /// Resolution status
    pub status: ResolutionStatus,
}

/// Classification of referring expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferentType {
    /// Personal/possessive pronoun: it, they, its, their
    Pronoun(PronounType),
    /// Demonstrative: this, that, these, those
    Demonstrative,
    /// Relative: which, who, that (as relative pronoun)
    Relative,
    /// Definite NP: "the Company", "such party"
    DefiniteNP,
    /// Legal deictic: herein, hereof, hereby, etc.
    LegalDeictic(DeicticScope),
    /// Section/document reference: Section 3.1, Exhibit A
    SectionReference,
    /// Bridging: implicit semantic relation
    Bridging(BridgingRelation),
}

/// Scope of legal deictic expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeicticScope {
    /// herein, hereof - entire current document
    Document,
    /// hereby - immediate clause/sentence
    Clause,
    /// hereunder - current section and below
    Section,
    /// hereto - attached documents
    Attachment,
    /// therein, thereof - referenced external document
    External,
}

/// Bridging relation types for implicit references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgingRelation {
    /// Part contained by whole (shipment ← Products)
    PartOf,
    /// Whole contains parts (Products → shipment)
    Contains,
    /// Event participant (Payment → funds as instrument)
    EventParticipant { role: String },
    /// Result of event (termination → terminated Agreement)
    ResultOf,
    /// Attribute of entity (the price → the Products)
    AttributeOf,
}

/// A candidate antecedent/target with confidence scoring.
#[derive(Debug, Clone)]
pub struct ResolutionCandidate {
    /// Location of the candidate antecedent
    pub target: DocSpan,
    /// Text of the candidate
    pub text: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Factors contributing to the score
    pub score_factors: Vec<ScoreFactor>,
    /// Whether this is a split/coordinated antecedent
    pub is_split_antecedent: bool,
    /// Component spans if split antecedent
    pub components: Vec<DocSpan>,
}

/// Factors that contribute to resolution confidence.
#[derive(Debug, Clone)]
pub enum ScoreFactor {
    /// Distance in tokens (lower is better)
    TokenDistance { distance: usize, weight: f64 },
    /// Is a defined term
    DefinedTerm { weight: f64 },
    /// Number/gender agreement
    Agreement { matches: bool, weight: f64 },
    /// Same sentence bonus
    SameSentence { weight: f64 },
    /// Syntactic role match (subject → subject)
    SyntacticRole { role: String, weight: f64 },
    /// Semantic frame match
    SemanticFrame { frame: String, weight: f64 },
    /// Recency (most recent mention)
    Recency { weight: f64 },
    /// Cataphoric (forward reference penalty)
    Cataphoric { weight: f64 },
}

/// Resolution status for tracking completeness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionStatus {
    /// Single high-confidence resolution
    Resolved,
    /// Multiple candidates, ambiguous
    Ambiguous { candidate_count: usize },
    /// No candidates found
    Unresolved,
    /// Circular definition detected
    Circular { cycle: Vec<String> },
    /// Forward reference pending resolution
    PendingCataphora,
    /// External document reference
    External { document: String },
}
```

### Association Types for Provenance

```rust
/// Association: reference points to its antecedent.
#[derive(Debug, Clone)]
pub struct AntecedentLink;

impl Association for AntecedentLink {
    fn label(&self) -> &'static str { "antecedent" }
    fn glyph(&self) -> Option<&'static str> { Some("→") }
}

/// Association: split antecedent component.
#[derive(Debug, Clone)]
pub struct SplitAntecedentComponent {
    pub component_index: usize,
}

impl Association for SplitAntecedentComponent {
    fn label(&self) -> &'static str { "split_component" }
    fn glyph(&self) -> Option<&'static str> { Some("∧") }
}

/// Association: bridging inference source.
#[derive(Debug, Clone)]
pub struct BridgingSource {
    pub relation: BridgingRelation,
}

impl Association for BridgingSource {
    fn label(&self) -> &'static str { "bridging" }
    fn glyph(&self) -> Option<&'static str> { Some("~") }
}

/// Association: section reference target.
#[derive(Debug, Clone)]
pub struct SectionTarget;

impl Association for SectionTarget {
    fn label(&self) -> &'static str { "section_target" }
    fn glyph(&self) -> Option<&'static str> { Some("§") }
}

/// Association: cataphoric (forward) reference.
#[derive(Debug, Clone)]
pub struct CataphoricLink;

impl Association for CataphoricLink {
    fn label(&self) -> &'static str { "cataphoric" }
    fn glyph(&self) -> Option<&'static str> { Some("←") }
}
```

### Multi-Pass Resolution Architecture

```rust
/// Document-level reference resolution engine.
/// 
/// Uses a multi-pass approach:
/// 1. Candidate Collection: Gather all potential antecedents per line
/// 2. Cross-Line Search: Extend search to preceding lines (FR-003)
/// 3. Scoring: Apply weighted factors to each candidate
/// 4. Resolution: Select best candidate(s) or flag ambiguity
/// 5. Cataphora Pass: Resolve forward references after main pass
pub struct ReferenceResolutionEngine {
    /// Configuration for scoring weights
    config: ResolutionConfig,
    /// Definition dependency graph for cycle detection
    definition_graph: DefinitionGraph,
    /// Document structure for section resolution (FR-004)
    document_structure: Option<DocumentStructure>,
}

#[derive(Debug, Clone)]
pub struct ResolutionConfig {
    /// Base confidence for nearest candidate
    pub base_confidence: f64,
    /// Bonus for defined terms
    pub defined_term_bonus: f64,
    /// Bonus for same sentence
    pub same_sentence_bonus: f64,
    /// Bonus for number/gender agreement
    pub agreement_bonus: f64,
    /// Penalty for multiple candidates
    pub ambiguity_penalty: f64,
    /// Multiplier for cataphoric resolutions
    pub cataphoric_multiplier: f64,
    /// Maximum lines to search backward
    pub max_backward_lines: usize,
    /// Minimum confidence threshold
    pub min_confidence_threshold: f64,
}

impl Default for ResolutionConfig {
    fn default() -> Self {
        Self {
            base_confidence: 0.50,
            defined_term_bonus: 0.30,
            same_sentence_bonus: 0.10,
            agreement_bonus: 0.15,
            ambiguity_penalty: 0.20,
            cataphoric_multiplier: 0.60,
            max_backward_lines: 10,
            min_confidence_threshold: 0.40,
        }
    }
}

/// Graph for tracking definition dependencies.
pub struct DefinitionGraph {
    /// term_name -> terms it depends on
    edges: HashMap<String, Vec<String>>,
}

impl DefinitionGraph {
    /// Detect circular definitions using Tarjan's SCC algorithm.
    pub fn find_cycles(&self) -> Vec<Vec<String>> {
        // Returns strongly-connected components with size > 1
        todo!()
    }
    
    /// Add a definition dependency edge.
    pub fn add_dependency(&mut self, term: &str, depends_on: &str) {
        self.edges
            .entry(term.to_string())
            .or_default()
            .push(depends_on.to_string());
    }
}
```

### Per-Line Resolver: ReferenceCollector

```rust
/// Per-line resolver that collects reference expressions and local candidates.
/// 
/// This is the first pass - it identifies referring expressions and
/// marks potential antecedents within the same line. Cross-line resolution
/// happens at the document level.
#[derive(Default)]
pub struct ReferenceCollectorResolver;

impl Resolver for ReferenceCollectorResolver {
    type Attr = Scored<ReferenceExpression>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();
        
        // 1. Collect pronouns (existing PronounResolver logic)
        results.extend(self.collect_pronouns(&selection));
        
        // 2. Collect legal deictics
        results.extend(self.collect_legal_deictics(&selection));
        
        // 3. Collect coordination patterns for split antecedents
        results.extend(self.collect_coordinations(&selection));
        
        // 4. Collect definite NPs ("the Company", "such party")
        results.extend(self.collect_definite_nps(&selection));
        
        results
    }
}

/// Intermediate representation before full resolution.
#[derive(Debug, Clone)]
pub struct ReferenceExpression {
    /// Type of referring expression
    pub expr_type: ReferentType,
    /// Surface text
    pub text: String,
    /// Local candidates (same line)
    pub local_candidates: Vec<LocalCandidate>,
    /// Whether this needs cross-line search
    pub needs_cross_line: bool,
}

#[derive(Debug, Clone)]
pub struct LocalCandidate {
    /// Token range within the line
    pub span: (usize, usize),
    /// Candidate text
    pub text: String,
    /// Preliminary confidence
    pub confidence: f64,
}
```

### Split Antecedent Detection (integrates with FR-005)

```rust
/// Resolver for detecting coordination patterns that form split antecedents.
#[derive(Default)]
pub struct CoordinationResolver;

impl Resolver for CoordinationResolver {
    type Attr = Coordination;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        // Pattern: "X and Y", "X, Y, and Z"
        // Pattern: "X and Y (the 'Z')" - explicit group naming
        // Pattern: "each of X and Y"
        todo!()
    }
}

/// A coordinated phrase that can serve as a plural antecedent.
#[derive(Debug, Clone)]
pub struct Coordination {
    /// Individual conjuncts
    pub conjuncts: Vec<Conjunct>,
    /// Group label if explicitly named: (the "Parties")
    pub group_label: Option<String>,
    /// Coordination type
    pub coord_type: CoordinationType,
}

#[derive(Debug, Clone)]
pub struct Conjunct {
    /// Token range
    pub span: (usize, usize),
    /// Text of this conjunct
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinationType {
    /// "X and Y" - joint reference
    And,
    /// "X or Y" - alternative
    Or,
    /// "X and/or Y" - either or both
    AndOr,
}
```

### Document-Level Resolution Pass

```rust
impl ReferenceResolutionEngine {
    /// Resolve all references in a document.
    /// 
    /// This is the main entry point for document-level resolution.
    pub fn resolve_document(&mut self, doc: &ContractDocument) -> ResolutionResult {
        let mut resolutions = Vec::new();
        let mut errors = Vec::new();

        // Pass 1: Collect all reference expressions and local candidates per line
        let expressions = self.collect_expressions(doc);
        
        // Pass 2: Build antecedent index from preceding context
        let antecedent_index = self.build_antecedent_index(doc);
        
        // Pass 3: Score and resolve each reference
        for (line_idx, expr_list) in expressions.iter().enumerate() {
            for expr in expr_list {
                let resolution = self.resolve_expression(
                    expr,
                    line_idx,
                    &antecedent_index,
                    doc,
                );
                resolutions.push(resolution);
            }
        }
        
        // Pass 4: Cataphora resolution (forward references)
        self.resolve_cataphora(&mut resolutions, doc);
        
        // Pass 5: Cycle detection in definitions
        let cycles = self.definition_graph.find_cycles();
        for cycle in cycles {
            errors.push(ResolutionError::CircularDefinition { terms: cycle });
        }
        
        ResolutionResult { resolutions, errors }
    }
    
    /// Resolve a single expression against the antecedent index.
    fn resolve_expression(
        &self,
        expr: &ReferenceExpression,
        line_idx: usize,
        antecedent_index: &AntecedentIndex,
        doc: &ContractDocument,
    ) -> ResolvedReference {
        let candidates = match &expr.expr_type {
            ReferentType::Pronoun(ptype) => {
                self.find_pronoun_candidates(expr, line_idx, *ptype, antecedent_index)
            }
            ReferentType::LegalDeictic(scope) => {
                self.resolve_deictic(*scope, line_idx, doc)
            }
            ReferentType::SectionReference => {
                self.resolve_section_reference(expr, doc)
            }
            ReferentType::Bridging(relation) => {
                self.find_bridging_candidates(expr, line_idx, relation, antecedent_index)
            }
            _ => Vec::new(),
        };
        
        let status = self.determine_status(&candidates);
        
        ResolvedReference {
            referent_type: expr.expr_type.clone(),
            candidates,
            status,
        }
    }
}

/// Index of potential antecedents for efficient lookup.
pub struct AntecedentIndex {
    /// Defined terms by name (lowercase)
    defined_terms: HashMap<String, Vec<IndexedAntecedent>>,
    /// All noun phrases by line
    noun_phrases: Vec<Vec<IndexedAntecedent>>,
    /// Coordinations that form plural antecedents
    coordinations: Vec<IndexedCoordination>,
}

#[derive(Debug, Clone)]
pub struct IndexedAntecedent {
    pub doc_span: DocSpan,
    pub text: String,
    pub is_defined_term: bool,
    pub is_plural: bool,
}

#[derive(Debug, Clone)]
pub struct IndexedCoordination {
    pub doc_span: DocSpan,
    pub coordination: Coordination,
}
```

## Proposed Improvements

### 1. Bridging Reference Detection

**Problem:** References to implicitly related concepts aren't resolved.

**Example:**
```
The Contractor shall deliver products. The shipment must arrive on time.
```
("shipment" relates to "products" through semantic association, not explicit mention)

**Ask:** Detect bridging references using:
- Head-noun relationships (products → shipment as container)
- Event-based bridging (delivery event → shipment)
- Co-hyponym detection (both relate to "goods")

### 2. Split Antecedent Resolution

**Problem:** Plural pronouns referring to conjoined NPs aren't resolved correctly.

**Example:**
```
The Buyer and Seller agree. They shall execute the agreement.
```

**Ask:**
- Detect "X and Y" coordination patterns
- Mark coordinated NPs as plural entities
- Resolve "they/their/them" to coordinated groups
- Distinguish joint vs several predicates ("They shall each pay" vs "They shall pay together")

### 3. Cross-Reference Content Resolution

**Problem:** Section references are detected but not linked to actual content.

**Example:**
```
The Company shall pay the Fees as defined in Schedule A, pursuant to
the payment terms set forth in Section 4.2, except as provided in Exhibit C.
```

**Ask:**
- Link "as defined in X" to definition content
- Link "pursuant to X" to authority source
- Link "except as provided in X" to exception content
- Build resolved obligation with integrated reference content
- Track reference purpose (definition, authority, exception, limit)

### 4. Transitive Participant Chains

**Problem:** Participants introduced through transactions aren't tracked.

**Example:**
```
The Seller delivers products to the Buyer. The Buyer shall sell them
to end customers. They must ensure quality.
```

**Ask:**
- Track transaction chains (Seller → Buyer → end customers)
- Build participant graph with confidence-weighted edges
- Use semantic role labeling (Agent, Patient, Recipient)
- Weight antecedent candidates by role relevance, not just distance

### 5. Cataphora (Forward References)

**Problem:** Forward references aren't detected.

**Example:**
```
Before it terminates, this Agreement shall remain in full force.
```

**Ask:**
- Detect cataphoric patterns ("Before X, Y..." where pronoun precedes antecedent)
- Scan forward when backward resolution fails
- Lower confidence for cataphoric resolutions

## Success Criteria

- [ ] Bridging references detected with semantic similarity scoring
- [ ] Coordinated NPs resolved as plural antecedents
- [ ] Cross-references linked to section content with purpose classification
- [ ] Participant chains tracked across transaction descriptions
- [ ] Cataphoric references resolved with appropriate confidence
- [ ] Circular definitions detected and flagged
- [ ] Legal deictics (herein/hereof/etc.) resolved to appropriate scope
- [ ] Ambiguous pronouns flagged with multiple candidates for review
- [ ] Cross-document references tracked with external document registry

## Integration Points

### FR-003: Cross-Line Spans
- `AntecedentIndex` requires cross-line span storage from FR-003
- `DocSpan` type shared between FR-002 and FR-003
- Backward search for antecedents uses FR-003's cross-line iteration

### FR-004: Document Structure
- Section reference resolution requires `DocumentStructure` from FR-004
- Exhibit/Schedule resolution requires attachment registry from FR-004
- "herein/hereof" scope resolution depends on section hierarchy

### FR-005: Syntactic Enhancement
- Split antecedent detection uses coordination patterns from FR-005
- Syntactic role matching for disambiguation uses parse data from FR-005
- "each...the other" patterns require syntactic analysis

## Dependencies

- Requires FR-003 (Cross-Line Spans) for cross-line antecedent search
- Requires FR-004 (Document Structure) for section content resolution
- May benefit from FR-005 (Syntactic Enhancement) for coordination parsing

## Related Edge Cases

- Bridging references (Linguistic #1)
- Split antecedents (Linguistic #2)
- Cross-reference complexity (Contract #4)
- Transitive participants (Linguistic #7)
- Cataphora (mentioned in Linguistic analysis)
- Circular definitions (new)
- Legal deictics (new)
- Cross-document references (new)
