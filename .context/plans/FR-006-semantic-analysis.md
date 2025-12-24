# FR-006: Semantic Analysis and Conflict Detection

## Summary

Add semantic analysis capabilities for detecting conflicting provisions, applying precedence rules, tracking deictic context shifts, and resolving metalinguistic references.

## Current State

The `SemanticDiffEngine` compares obligations and terms in isolation. No cross-provision analysis is performed. This misses:
- Conflicting obligations between sections
- Precedence rules that affect interpretation
- Perspective shifts in reference resolution
- Distinction between term use and term mention

## Proposed Improvements

### 1. Conflict Detection

**Problem:** Contradictory obligations aren't flagged.

**Example:**
```
Section 2.3: Payment due within 15 days.
Schedule 1: Payment due within 30 days.
```

**Ask:**
- Detect obligations on same topic (payment, delivery, confidentiality)
- Compare obligation content for conflicts
- Flag contradictions with both locations
- Suggest resolution based on precedence (if available)

```rust
pub struct Conflict {
    pub obligation_a: DocSpan,
    pub obligation_b: DocSpan,
    pub conflict_type: ConflictType,
    pub resolution: Option<Resolution>,
}

pub enum ConflictType {
    ContradictoryTerms,      // 15 days vs 30 days
    ContradictoryParties,    // Buyer shall vs Seller shall
    ScopeOverlap,            // Both cover same situation differently
    TemporalConflict,        // Different effective dates
}
```

### 2. Precedence Rule Application

**Problem:** Precedence clauses are detected but not applied.

**Example:**
```
5.4 In event of conflict: (a) schedules take precedence over (b) articles.
```

**Ask:**
- Parse precedence ordering
- Apply to detected conflicts
- Report resolution with precedence citation
- Flag changes to precedence rules as high-risk

```rust
pub struct PrecedenceRule {
    pub source_section: String,
    pub ordering: Vec<SectionKind>,  // [Schedule, Article, Exhibit, Recital]
}

pub struct ConflictResolution {
    pub conflict: Conflict,
    pub winner: DocSpan,
    pub precedence_applied: PrecedenceRule,
}
```

### 3. Deictic Context Tracking

**Problem:** Perspective shifts affect reference resolution.

**Example:**
```
Company A agrees that Company B shall indemnify it.
As of the execution date, it shall have no liability.
```
(First "it" = Company A; second "it" could shift based on context)

**Ask:**
- Track current deictic center (whose perspective?)
- Detect perspective shift markers ("As of...", "From X's perspective")
- Adjust reference resolution based on context
- Build deictic context stack that updates with markers

### 4. Metalinguistic Reference Resolution

**Problem:** Use vs mention isn't distinguished.

**Example:**
```
The term "Agreement" shall mean this document.
References to the Agreement shall be capitalized.
```
(First is mention/definition; second is use)

**Ask:**
- Detect mention contexts ("the term X", "as defined", "references to")
- Distinguish definition scope (global vs section-specific)
- Prevent circular definition detection
- Track redefinitions ("For purposes of Section 5, 'Party' means...")

```rust
pub enum TermUsage {
    Use,           // Actually referring to the thing
    Mention,       // Talking about the term itself
    Definition,    // Defining what the term means
    Redefinition,  // Scoped redefinition for a section
}

pub struct TermOccurrence {
    pub term: String,
    pub usage: TermUsage,
    pub scope: Option<String>,  // Section scope for redefinitions
    pub location: DocSpan,
}
```

### 5. Semantic Role Labeling

**Problem:** No tracking of who does what to whom.

**Example:**
```
The Seller delivers products to the Buyer.
```
(Seller = Agent, products = Theme, Buyer = Recipient)

**Ask:**
- Extract semantic roles from obligation patterns
- Track Agent, Patient, Theme, Recipient, Beneficiary
- Use roles for reference resolution weighting
- Enable obligation comparison by role alignment

### 6. Obligation Equivalence Detection

**Problem:** Semantically equivalent obligations with different wording aren't matched.

**Example:**
```
Document A: "shall provide written notice within 30 days"
Document B: "must notify in writing no later than 30 days"
```

**Ask:**
- Normalize obligation components (modal, action, timing, manner)
- Compare normalized forms for equivalence
- Score semantic similarity beyond text similarity
- Reduce false-positive "changes" in diff

## Success Criteria

- [ ] Conflicting provisions detected and reported
- [ ] Precedence rules applied to resolve conflicts
- [ ] Deictic context tracked through document
- [ ] Use/mention distinction preserved
- [ ] Semantic roles extracted from obligations
- [ ] Equivalent obligations matched despite rewording

## Related Edge Cases

- Conflicting provisions (Document #7)
- Precedence clauses (Document #7)
- Deictic center shifts (Linguistic #9)
- Metalinguistic references (Linguistic #5)
- Transitive participants (Linguistic #7) - uses semantic roles

## Dependencies

- Requires FR-004 (Document Structure) for precedence clause detection
- Requires FR-001 (Obligation Structure) for normalized comparison
- Enhances FR-002 (Reference Resolution) with deictic context
- Benefits from FR-005 (Syntactic Enhancement) for role extraction

---

## Edge Cases

This section catalogs complex semantic reasoning scenarios that the conflict detection and semantic analysis systems must handle.

### 1. Exception vs Main Obligation Conflicts

**Scenario:** An exception clause conflicts with or entirely negates a main obligation.

```
Section 3.1: The Company shall indemnify the Buyer against all claims.
Section 3.1(a): Notwithstanding the foregoing, the Company shall not be
               liable for any claims arising from Buyer's negligence.
Section 3.1(b): Except as provided in Section 3.1(a), the Company shall
               indemnify claims caused by third parties, including claims
               arising from Buyer's contractors' negligence.
```

**Challenges:**
- 3.1(b) creates an exception to the exception (contractor negligence ≠ Buyer's negligence?)
- "Notwithstanding" signals 3.1(a) overrides 3.1 but unclear if it overrides 3.1(b)
- Contractor actions may or may not be imputed to Buyer

**Required Analysis:**
- Build exception hierarchy: 3.1 → 3.1(a) → 3.1(b)
- Detect "exception to exception" patterns
- Flag potential scope overlap between 3.1(a) and 3.1(b)

### 2. Temporal Conflicts

**Scenario:** Different timing requirements create irreconcilable obligations.

```
Section 4.1: Buyer shall provide notice within 30 calendar days.
Section 4.2: Buyer shall provide notice within a reasonable time.
Section 4.3: Buyer shall provide notice promptly, but in no event later
             than 15 business days after discovery.
```

**Challenges:**
- "30 calendar days" vs "15 business days" - which is shorter depends on calendar
- "Reasonable time" is vague; may be shorter or longer than 30 days
- "Promptly" + "15 business days" creates two-tier timing (aspirational + hard limit)

**Required Analysis:**
- Normalize temporal expressions to comparable units where possible
- Flag vague temporals ("reasonable", "promptly") as requiring interpretation
- Detect hard vs soft deadlines
- Identify when the same triggering event has multiple deadlines

### 3. Implicit Conflicts from Obligation Chains

**Scenario:** Obligations that individually are consistent but create conflicts when chained.

```
Section 2.1: Seller shall deliver Products to Buyer by June 1.
Section 2.2: Buyer shall inspect Products within 5 days of delivery.
Section 2.3: Buyer shall provide defect notice within 3 days of inspection.
Section 2.4: Seller shall cure defects within 10 days of notice.
Section 5.1: All obligations under this Agreement terminate on June 15.
```

**Challenges:**
- If delivery on June 1, inspection by June 6, notice by June 9, cure by June 19 - after termination
- No individual obligation is wrong, but the chain creates an impossibility
- Requires forward projection of obligation completion dates

**Required Analysis:**
- Build obligation dependency graph
- Project deadlines through chains
- Detect chains that exceed external constraints (termination dates, sunset clauses)

### 4. Precedence Ambiguity

**Scenario:** Unclear which document or section takes priority.

```
Master Agreement Section 15.3: In case of conflict, Schedules prevail
                               over this Agreement.
Schedule A, Section 2: In case of conflict, the Master Agreement prevails.

---OR---

Section 1: This Agreement incorporates the Terms of Service by reference.
Section 5: In case of conflict, this Agreement prevails.
Terms of Service: These Terms supersede all other agreements.
```

**Challenges:**
- Circular precedence declarations
- Precedence for incorporated documents vs defined within them
- "Prevails" vs "supersedes" may have different legal meanings

**Required Analysis:**
- Detect precedence clauses in all document layers
- Build precedence graph; detect cycles
- Flag circular precedence as Critical risk
- Consider meta-precedence (which precedence clause governs precedence?)

### 5. Deictic Shifts in Quoted Speech and Hypotheticals

**Scenario:** Perspective changes within a single sentence or clause.

```
The Agreement states that "the Company shall indemnify you against
all claims." For purposes of this Exhibit, "you" refers to the
Subcontractor.

---

In the event that a court determines that this clause is unenforceable,
then the Company shall have the right it would have had under Section 3.
```

**Challenges:**
- "you" in quotes requires context to resolve (original addressee vs current reader)
- "it would have had" is hypothetical/counterfactual deictic reference
- Section-scoped redefinitions create local deictic contexts

**Required Analysis:**
- Track quotation boundaries; flag deictics inside quotes for special handling
- Build deictic context stack: push on entering quotes/hypotheticals, pop on exit
- Detect scoped redefinitions ("For purposes of X, Y means Z")

### 6. Use/Mention Distinction Failures

**Scenario:** Ambiguity between talking about a term vs using it.

```
The term "Confidential Information" shall include the term "Trade Secrets".
References to "the Company" shall include its Affiliates.
"Force Majeure" as used in Section 5 has the meaning given in Exhibit B.
```

**Challenges:**
- First sentence: is "Trade Secrets" being defined or referenced?
- Second sentence: expanding definition vs substitution instruction
- Third sentence: scoped definition reference

**Required Analysis:**
- Detect mention-frame markers: "the term X", "references to X", "X as used in"
- Distinguish: Definition, Redefinition (scoped), Reference-expansion, Substitution
- Track definition scope: global vs section-specific

### 7. Semantic Equivalence with Syntactic Variation

**Scenario:** Same obligation expressed differently across documents.

```
Document A: "The Seller shall deliver the Products within 30 days."
Document B: "Products must be delivered by Seller no later than 30 days."
Document C: "Delivery of Products by Seller: within thirty (30) days."

---

Document A: "shall provide written notice"
Document B: "must notify in writing"
Document C: "written notification shall be given"
```

**Challenges:**
- Active vs passive voice
- Modal synonyms (shall/must/will)
- Noun vs verb forms (notice/notify, delivery/deliver)
- Number formats (30/thirty)

**Required Analysis:**
- Normalize to canonical form: [OBLIGOR] [MODAL] [ACTION] [TIMING] [MANNER]
- Build synonym sets: {shall, must, will}, {notify, give notice, provide notice}
- Parse passive to recover logical obligor
- Standardize number representations

### 8. Role Assignment with Passive Voice and Nominalization

**Scenario:** Semantic roles obscured by grammatical transformation.

```
Active:   "The Company shall indemnify the Buyer."
Passive:  "The Buyer shall be indemnified by the Company."
Nominal:  "Indemnification of the Buyer by the Company is required."
Agentless: "The Buyer shall be indemnified."
```

**Challenges:**
- All four express same semantic roles but with different surface syntax
- Agentless passive hides the Agent (who indemnifies?)
- Nominalization ("indemnification") requires decomposition

**Required Analysis:**
- Extract semantic frame: Indemnify(Agent=Company, Patient=Buyer)
- Handle by-phrases in passives as Agent markers
- Decompose nominalizations to underlying verb + arguments
- Flag agentless passives as ambiguous (unknown obligor)

### 9. Context-Dependent Contradictions

**Scenario:** Obligations that conflict only under certain conditions.

```
Section 3: If Products are defective, Seller shall provide a full refund.
Section 4: If Products are defective due to Buyer misuse, Seller shall
           repair at Buyer's cost.
Section 5: If Products are defective due to manufacturing error, Seller
           shall replace at no cost.
```

**Challenges:**
- Sections 3, 4, 5 all cover "defective Products" but with different triggers
- Section 3 (full refund) may conflict with Section 5 (replacement)
- Need to determine if conditions are mutually exclusive

**Required Analysis:**
- Build condition lattice: defective ⊃ {defective+misuse, defective+manufacturing}
- Check mutual exclusivity of sub-conditions
- Detect overlapping condition scopes with different consequences
- Flag as conflict only if conditions can co-occur

### 10. Quantifier Scope Conflicts

**Scenario:** Different readings of quantified obligations.

```
Each Party shall not disclose any Confidential Information.
No Party shall disclose all Confidential Information.
```

**Challenges:**
- "Each...not...any" = ∀party.∀info.¬disclose(party, info) — total prohibition
- "No...all" = ¬∃party.∀info.disclose(party, info) — partial disclosure OK
- Surface similarity masks semantic difference

**Required Analysis:**
- Parse quantifier + negation scope
- Generate formal representation: ∀x.∀y.¬P(x,y) vs ¬∃x.∀y.P(x,y)
- Detect when scope ambiguity creates potential conflicts
- Flag for human review if scope is genuinely ambiguous

---

## Engineering Approach

This section defines the resolver architecture for FR-006. These resolvers sit at the top of the semantic stack, consuming outputs from FR-001 through FR-005.

### Architecture Overview

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                        FR-006 Semantic Layer                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐         │
│  │ ConflictDetector│  │ PrecedenceApp   │  │ DeicticContext  │         │
│  │                 │  │                 │  │ Tracker         │         │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘         │
│           │                    │                    │                   │
│  ┌────────┴────────┐  ┌────────┴────────┐  ┌────────┴────────┐         │
│  │ ObligationNorm  │  │ TermUsageClass  │  │ SemanticRole    │         │
│  │                 │  │                 │  │ Labeler         │         │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘         │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
                              ▲    ▲    ▲
                              │    │    │
        ┌─────────────────────┘    │    └─────────────────────┐
        │                          │                          │
┌───────┴───────┐  ┌───────────────┴───────────────┐  ┌───────┴───────┐
│   FR-001      │  │          FR-004               │  │   FR-005      │
│  Obligation   │  │   Document Structure          │  │   Syntactic   │
│  Structure    │  │   (precedence, sections)      │  │   Structure   │
└───────────────┘  └───────────────────────────────┘  └───────────────┘
        ▲                          ▲                          ▲
        │                          │                          │
┌───────┴───────┐  ┌───────────────┴───────────────┐  ┌───────┴───────┐
│   FR-002      │  │          FR-003               │  │ layered-deixis│
│  Reference    │  │   Cross-Line Spans            │  │               │
│  Resolution   │  │                               │  │               │
└───────────────┘  └───────────────────────────────┘  └───────────────┘
```

### Core Types

```rust
// ============================================================================
// CONFLICT DETECTION TYPES
// ============================================================================

/// A conflict between two or more provisions in a document or across documents.
#[derive(Debug, Clone)]
pub struct Conflict {
    /// Unique identifier for this conflict
    pub conflict_id: String,
    /// The provisions involved in this conflict
    pub provisions: Vec<ConflictingProvision>,
    /// Type of conflict detected
    pub conflict_type: ConflictType,
    /// If a resolution is determinable, the winning provision
    pub resolution: Option<ConflictResolution>,
    /// Confidence in conflict detection (0.0-1.0)
    pub confidence: f64,
    /// Explanation of why this is a conflict
    pub explanation: String,
}

/// A provision involved in a conflict.
#[derive(Debug, Clone)]
pub struct ConflictingProvision {
    /// Document source (main, exhibit, schedule)
    pub document_source: DocumentSource,
    /// Section identifier
    pub section_id: String,
    /// Line number within section
    pub line: usize,
    /// The obligation or term at this provision
    pub content: ProvisionContent,
    /// Full text excerpt
    pub text: String,
}

/// Content type of a conflicting provision.
#[derive(Debug, Clone)]
pub enum ProvisionContent {
    /// An obligation with normalized form
    Obligation(NormalizedObligation),
    /// A term definition
    TermDefinition {
        term: String,
        definition: String,
    },
    /// A temporal requirement
    Temporal {
        value: Option<u32>,
        unit: Option<String>,
        vague_marker: Option<String>, // "reasonable", "promptly"
    },
    /// A precedence declaration
    Precedence(Vec<SectionKind>),
}

/// Classification of conflict types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictType {
    /// Direct contradiction: 15 days vs 30 days, shall vs shall not
    Direct {
        dimension: ConflictDimension,
    },
    /// Temporal conflict: incompatible deadlines or durations
    Temporal {
        earlier: TemporalBound,
        later: TemporalBound,
    },
    /// Scope overlap: both cover same situation with different outcomes
    ScopeOverlap {
        shared_scope: String,
        outcomes: Vec<String>,
    },
    /// Party-based: different obligations for same party on same matter
    PartyBased {
        party: String,
        conflicting_obligations: Vec<String>,
    },
    /// Exception conflict: exception and main obligation interact problematically
    Exception {
        main_obligation_id: String,
        exception_id: String,
        interaction: ExceptionInteraction,
    },
    /// Circular: circular definitions or precedence
    Circular {
        cycle: Vec<String>, // IDs forming the cycle
    },
    /// Conditional: conflict only under certain conditions
    Conditional {
        condition: String,
        underlying_conflict: Box<ConflictType>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictDimension {
    Modal,           // shall vs may vs shall not
    Quantity,        // 15 vs 30
    Party,           // Buyer vs Seller
    Action,          // deliver vs withhold
    Manner,          // written vs oral
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExceptionInteraction {
    /// Exception entirely negates the main obligation
    FullNegation,
    /// Exception to exception reinstates main obligation
    DoubleException,
    /// Exceptions overlap in scope
    OverlappingExceptions,
    /// Exception scope is ambiguous
    AmbiguousScope,
}

/// Resolution of a conflict.
#[derive(Debug, Clone)]
pub struct ConflictResolution {
    /// Which provision prevails
    pub winner: String, // provision ID
    /// Source of resolution
    pub basis: ResolutionBasis,
    /// Confidence in resolution
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub enum ResolutionBasis {
    /// Precedence clause in document
    PrecedenceClause {
        clause_section: String,
        ordering: Vec<SectionKind>,
    },
    /// Later section overrides earlier (implicit)
    LaterPrevails,
    /// More specific provision overrides general
    SpecificOverGeneral,
    /// Exception applies
    ExceptionApplies,
    /// Cannot determine
    Undeterminable { reason: String },
}

// ============================================================================
// PRECEDENCE TYPES
// ============================================================================

/// A parsed precedence rule from the document.
#[derive(Debug, Clone)]
pub struct PrecedenceRule {
    /// Section where this rule is defined
    pub source_section: String,
    /// The ordering of document parts, highest priority first
    pub ordering: Vec<SectionKind>,
    /// Scope of this rule (entire document, specific sections, etc.)
    pub scope: PrecedenceScope,
    /// Confidence in parsing
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SectionKind {
    Schedule(Option<String>),  // Schedule, Schedule A
    Exhibit(Option<String>),   // Exhibit, Exhibit B
    Appendix(Option<String>),
    Article,
    Section,
    Recital,
    Amendment,
    MainBody,
    Custom(String),
}

#[derive(Debug, Clone)]
pub enum PrecedenceScope {
    /// Applies to entire document
    Document,
    /// Applies only to specific sections
    Sections(Vec<String>),
    /// Applies only for specific subject matter
    SubjectMatter(String),
}

/// Result of applying precedence rules to a conflict.
#[derive(Debug, Clone)]
pub struct PrecedenceApplication {
    pub conflict_id: String,
    pub applicable_rules: Vec<PrecedenceRule>,
    pub resolution: Option<ConflictResolution>,
    pub warnings: Vec<String>,
}

// ============================================================================
// DEICTIC CONTEXT TYPES
// ============================================================================

/// Tracks deictic context through document traversal.
#[derive(Debug, Clone)]
pub struct DeicticContext {
    /// Stack of contexts (push on entering quotes/hypotheticals, pop on exit)
    pub context_stack: Vec<DeicticFrame>,
    /// Current deictic center (whose perspective?)
    pub current_center: DeicticCenter,
    /// Active term redefinitions (scoped overrides)
    pub scoped_definitions: Vec<ScopedDefinition>,
}

/// A frame in the deictic context stack.
#[derive(Debug, Clone)]
pub struct DeicticFrame {
    /// Type of frame
    pub frame_type: DeicticFrameType,
    /// Deictic center for this frame
    pub center: DeicticCenter,
    /// Token span of this frame
    pub span: (usize, usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeicticFrameType {
    /// Normal document text
    Document,
    /// Inside quoted speech
    Quote { attribution: Option<String> },
    /// Inside a hypothetical/counterfactual
    Hypothetical,
    /// Inside a definition
    Definition { term: String },
    /// Section-scoped context
    SectionScope { section_id: String },
}

/// The deictic center - whose perspective is active.
#[derive(Debug, Clone)]
pub struct DeicticCenter {
    /// Primary perspective holder ("Company", "Buyer", "drafting party")
    pub primary: Option<String>,
    /// Secondary addressee (for "you")
    pub addressee: Option<String>,
    /// Reference time for temporal deictics
    pub reference_time: ReferenceTime,
    /// Reference place for spatial deictics
    pub reference_place: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ReferenceTime {
    /// The execution date of the agreement
    ExecutionDate,
    /// The effective date
    EffectiveDate,
    /// A specific defined date
    DefinedDate(String),
    /// Relative to an event
    EventRelative { event: String, offset: Option<String> },
}

/// A scoped term redefinition.
#[derive(Debug, Clone)]
pub struct ScopedDefinition {
    /// The term being redefined
    pub term: String,
    /// The scoped definition
    pub definition: String,
    /// Scope of this redefinition
    pub scope: String, // section ID
    /// Original definition if known
    pub original: Option<String>,
}

// ============================================================================
// TERM USAGE CLASSIFICATION
// ============================================================================

/// Classification of how a term is used at a given location.
#[derive(Debug, Clone)]
pub struct TermOccurrence {
    /// The term text
    pub term: String,
    /// How it's being used
    pub usage: TermUsage,
    /// Scope for redefinitions
    pub scope: Option<String>,
    /// Token span
    pub span: (usize, usize),
    /// Confidence
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermUsage {
    /// Actually referring to the thing the term denotes
    Use,
    /// Talking about the term itself (metalinguistic)
    Mention,
    /// Initial definition of the term
    Definition,
    /// Scoped redefinition for a section
    Redefinition { scope: String },
    /// Expanding references to include additional entities
    ReferenceExpansion { includes: Vec<String> },
    /// Substitution instruction
    Substitution { replacement: String },
}

// ============================================================================
// OBLIGATION NORMALIZATION
// ============================================================================

/// A normalized obligation suitable for equivalence comparison.
#[derive(Debug, Clone)]
pub struct NormalizedObligation {
    /// Canonical obligor (normalized party name)
    pub obligor: String,
    /// Obligation modal in canonical form
    pub modal: CanonicalModal,
    /// Canonical action verb (lemmatized)
    pub action: String,
    /// Direct object / patient
    pub patient: Option<String>,
    /// Beneficiary / recipient
    pub beneficiary: Option<String>,
    /// Timing in normalized form
    pub timing: Option<NormalizedTiming>,
    /// Manner specifications
    pub manner: Vec<String>,
    /// Conditions (normalized)
    pub conditions: Vec<NormalizedCondition>,
    /// Original text for debugging
    pub original_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalModal {
    Shall,       // shall, must, will (duty)
    Should,      // should, ought to (soft duty)
    May,         // may, can (permission)
    ShallNot,    // shall not, must not (prohibition)
}

#[derive(Debug, Clone)]
pub struct NormalizedTiming {
    /// Numeric value (None if vague)
    pub value: Option<u32>,
    /// Unit (days, months, years)
    pub unit: Option<String>,
    /// Vague modifier if present
    pub vague: Option<VagueModifier>,
    /// Reference point
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VagueModifier {
    Reasonable,
    Prompt,
    Timely,
    Immediate,
    Without undue delay,
}

#[derive(Debug, Clone)]
pub struct NormalizedCondition {
    /// Condition type (if, unless, provided that)
    pub condition_type: ConditionType,
    /// Normalized condition content
    pub content: String,
    /// Nested sub-conditions
    pub subconditions: Vec<NormalizedCondition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionType {
    Trigger,      // if, when, upon
    Exception,    // unless, except
    Provided,     // provided that
    Consequence,  // in which case
}

// ============================================================================
// SEMANTIC ROLE LABELING
// ============================================================================

/// Semantic roles extracted from an obligation.
#[derive(Debug, Clone)]
pub struct SemanticFrame {
    /// The predicate (verb/action)
    pub predicate: String,
    /// Frame arguments
    pub arguments: Vec<SemanticArgument>,
    /// Token span of the frame
    pub span: (usize, usize),
}

#[derive(Debug, Clone)]
pub struct SemanticArgument {
    /// Role type
    pub role: SemanticRole,
    /// Filler text
    pub filler: String,
    /// Token span
    pub span: (usize, usize),
    /// Confidence
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticRole {
    /// Entity performing the action
    Agent,
    /// Entity undergoing the action
    Patient,
    /// Theme/topic of the action
    Theme,
    /// Recipient of a transfer
    Recipient,
    /// Entity benefiting from the action
    Beneficiary,
    /// Instrument used
    Instrument,
    /// Location
    Location,
    /// Time
    Time,
    /// Manner
    Manner,
    /// Source (origin of motion/transfer)
    Source,
    /// Goal (destination of motion/transfer)
    Goal,
}
```

### Resolver Designs

#### 1. ConflictDetector

```rust
/// Detects conflicts between provisions across a document.
///
/// Consumes:
/// - FR-001: ObligationPhrase, ConditionRef, ExceptionHierarchy
/// - FR-002: ResolvedReference (for party normalization)
/// - FR-004: DocumentStructure, SectionNode
/// - FR-005: ClauseBoundary, QuantifierScope
pub struct ConflictDetector {
    /// Similarity threshold for matching obligations on same topic
    topic_similarity_threshold: f64,
    /// Whether to detect conditional conflicts
    detect_conditional: bool,
}

impl ConflictDetector {
    pub fn new() -> Self {
        Self {
            topic_similarity_threshold: 0.6,
            detect_conditional: true,
        }
    }

    /// Analyze document for conflicts.
    pub fn detect_conflicts(&self, doc: &ContractDocument) -> Vec<Conflict> {
        let mut conflicts = Vec::new();

        // Step 1: Extract and normalize all obligations
        let obligations = self.extract_obligations(doc);

        // Step 2: Group by topic (payment, delivery, confidentiality, etc.)
        let by_topic = self.group_by_topic(&obligations);

        // Step 3: Within each topic, compare pairs for conflicts
        for (topic, obs) in &by_topic {
            conflicts.extend(self.detect_topic_conflicts(topic, obs));
        }

        // Step 4: Detect exception interactions
        conflicts.extend(self.detect_exception_conflicts(doc));

        // Step 5: Detect temporal chains that exceed constraints
        conflicts.extend(self.detect_chain_conflicts(doc));

        // Step 6: Detect circular definitions/precedence
        conflicts.extend(self.detect_circular_conflicts(doc));

        conflicts
    }

    fn detect_topic_conflicts(
        &self,
        topic: &str,
        obligations: &[NormalizedObligation],
    ) -> Vec<Conflict> {
        let mut conflicts = Vec::new();

        for (i, ob_a) in obligations.iter().enumerate() {
            for ob_b in obligations.iter().skip(i + 1) {
                // Check for direct conflicts
                if let Some(conflict) = self.check_direct_conflict(ob_a, ob_b) {
                    conflicts.push(conflict);
                    continue;
                }

                // Check for conditional conflicts
                if self.detect_conditional {
                    if let Some(conflict) = self.check_conditional_conflict(ob_a, ob_b) {
                        conflicts.push(conflict);
                    }
                }
            }
        }

        conflicts
    }

    fn check_direct_conflict(
        &self,
        a: &NormalizedObligation,
        b: &NormalizedObligation,
    ) -> Option<Conflict> {
        // Same obligor, same action, different modal
        if self.same_obligor(a, b) && self.same_action(a, b) {
            if a.modal != b.modal {
                return Some(self.build_modal_conflict(a, b));
            }
        }

        // Same obligation, different timing
        if self.same_obligor(a, b) && self.same_action(a, b) {
            if let Some(conflict) = self.check_timing_conflict(&a.timing, &b.timing) {
                return Some(conflict);
            }
        }

        None
    }
}
```

#### 2. PrecedenceApplicator

```rust
/// Applies precedence rules to resolve detected conflicts.
///
/// Consumes:
/// - FR-004: PrecedenceRule from DocumentStructure
/// - ConflictDetector output
pub struct PrecedenceApplicator {
    /// Default precedence if no explicit rule
    default_precedence: Vec<SectionKind>,
}

impl PrecedenceApplicator {
    pub fn new() -> Self {
        Self {
            // Default: specificity principle
            default_precedence: vec![
                SectionKind::Amendment,
                SectionKind::Schedule(None),
                SectionKind::Exhibit(None),
                SectionKind::Section,
                SectionKind::Article,
                SectionKind::Recital,
            ],
        }
    }

    /// Apply precedence rules to conflicts.
    pub fn apply(
        &self,
        conflicts: &[Conflict],
        precedence_rules: &[PrecedenceRule],
    ) -> Vec<PrecedenceApplication> {
        conflicts
            .iter()
            .map(|conflict| self.apply_to_conflict(conflict, precedence_rules))
            .collect()
    }

    fn apply_to_conflict(
        &self,
        conflict: &Conflict,
        rules: &[PrecedenceRule],
    ) -> PrecedenceApplication {
        // Find applicable rules for this conflict
        let applicable: Vec<_> = rules
            .iter()
            .filter(|r| self.rule_applies(r, conflict))
            .cloned()
            .collect();

        // Check for conflicting precedence rules
        if self.rules_conflict(&applicable) {
            return PrecedenceApplication {
                conflict_id: conflict.conflict_id.clone(),
                applicable_rules: applicable,
                resolution: Some(ConflictResolution {
                    winner: String::new(),
                    basis: ResolutionBasis::Undeterminable {
                        reason: "Conflicting precedence rules".into(),
                    },
                    confidence: 0.0,
                }),
                warnings: vec!["Circular or conflicting precedence detected".into()],
            };
        }

        // Apply highest-priority rule
        let resolution = applicable
            .first()
            .and_then(|rule| self.resolve_with_rule(rule, conflict));

        PrecedenceApplication {
            conflict_id: conflict.conflict_id.clone(),
            applicable_rules: applicable,
            resolution,
            warnings: vec![],
        }
    }
}
```

#### 3. DeicticContextTracker

```rust
/// Tracks deictic context through document traversal.
///
/// Extends layered-deixis with:
/// - Quote/hypothetical frame tracking
/// - Scoped redefinition handling
/// - Perspective shift detection
pub struct DeicticContextTracker {
    /// Current context
    context: DeicticContext,
    /// Markers that signal perspective shift
    shift_markers: Vec<&'static str>,
}

impl DeicticContextTracker {
    pub fn new() -> Self {
        Self {
            context: DeicticContext {
                context_stack: vec![DeicticFrame {
                    frame_type: DeicticFrameType::Document,
                    center: DeicticCenter::default(),
                    span: (0, 0),
                }],
                current_center: DeicticCenter::default(),
                scoped_definitions: vec![],
            },
            shift_markers: vec![
                "as of",
                "from the perspective of",
                "for purposes of",
                "in the event that",
                "assuming that",
            ],
        }
    }

    /// Process a token and update context.
    pub fn process_token(&mut self, token: &str, span: (usize, usize)) {
        // Check for quote entry
        if token == "\"" || token == """ {
            self.push_quote_frame(span);
        }

        // Check for quote exit
        if self.in_quote() && (token == "\"" || token == """) {
            self.pop_frame();
        }

        // Check for shift markers
        for marker in &self.shift_markers {
            if token.to_lowercase().starts_with(marker) {
                self.handle_shift_marker(marker, span);
            }
        }

        // Check for scoped redefinition patterns
        if self.is_scoped_redefinition_start(token) {
            self.handle_scoped_redefinition(span);
        }
    }

    /// Resolve a deictic reference in current context.
    pub fn resolve_deictic(&self, deictic: &DeicticReference) -> ResolvedDeictic {
        // Check if we're in a special frame
        let frame = self.context.context_stack.last().unwrap();

        match frame.frame_type {
            DeicticFrameType::Quote { ref attribution } => {
                // Inside quotes, deictics refer to original utterance context
                ResolvedDeictic {
                    resolved_to: attribution.clone(),
                    context_note: Some("Inside quoted speech".into()),
                    confidence: 0.7, // Lower confidence in quotes
                }
            }
            DeicticFrameType::Hypothetical => {
                ResolvedDeictic {
                    resolved_to: None,
                    context_note: Some("Hypothetical/counterfactual context".into()),
                    confidence: 0.5,
                }
            }
            _ => {
                // Normal resolution using current center
                self.resolve_with_center(deictic, &self.context.current_center)
            }
        }
    }
}
```

#### 4. ObligationNormalizer

```rust
/// Normalizes obligations for equivalence comparison.
///
/// Consumes:
/// - FR-001: ObligationPhrase
/// - FR-005: SemanticFrame (for role extraction)
pub struct ObligationNormalizer {
    /// Synonym sets for modal normalization
    modal_synonyms: HashMap<&'static str, CanonicalModal>,
    /// Verb lemmatizer (simplified)
    verb_lemmas: HashMap<&'static str, &'static str>,
    /// Party name normalizer
    party_aliases: HashMap<String, String>,
}

impl ObligationNormalizer {
    pub fn new() -> Self {
        let mut modal_synonyms = HashMap::new();
        modal_synonyms.insert("shall", CanonicalModal::Shall);
        modal_synonyms.insert("must", CanonicalModal::Shall);
        modal_synonyms.insert("will", CanonicalModal::Shall);
        modal_synonyms.insert("should", CanonicalModal::Should);
        modal_synonyms.insert("ought", CanonicalModal::Should);
        modal_synonyms.insert("may", CanonicalModal::May);
        modal_synonyms.insert("can", CanonicalModal::May);

        let mut verb_lemmas = HashMap::new();
        verb_lemmas.insert("delivers", "deliver");
        verb_lemmas.insert("delivered", "deliver");
        verb_lemmas.insert("providing", "provide");
        verb_lemmas.insert("provides", "provide");
        verb_lemmas.insert("notifies", "notify");
        verb_lemmas.insert("notifying", "notify");
        verb_lemmas.insert("notification", "notify");
        verb_lemmas.insert("notice", "notify");

        Self {
            modal_synonyms,
            verb_lemmas,
            party_aliases: HashMap::new(),
        }
    }

    /// Normalize an obligation for comparison.
    pub fn normalize(&self, obligation: &ObligationPhrase) -> NormalizedObligation {
        NormalizedObligation {
            obligor: self.normalize_party(&obligation.obligor),
            modal: self.normalize_modal(&obligation.modal),
            action: self.normalize_action(&obligation.action),
            patient: obligation.patient.as_ref().map(|p| self.normalize_party(p)),
            beneficiary: obligation.beneficiary.as_ref().map(|b| self.normalize_party(b)),
            timing: obligation.timing.as_ref().map(|t| self.normalize_timing(t)),
            manner: obligation.manner.iter().map(|m| m.to_lowercase()).collect(),
            conditions: obligation.conditions.iter().map(|c| self.normalize_condition(c)).collect(),
            original_text: obligation.original_text.clone(),
        }
    }

    /// Compare two normalized obligations for equivalence.
    pub fn equivalent(&self, a: &NormalizedObligation, b: &NormalizedObligation) -> EquivalenceResult {
        let obligor_match = a.obligor == b.obligor;
        let modal_match = a.modal == b.modal;
        let action_match = a.action == b.action;
        let timing_match = self.timing_equivalent(&a.timing, &b.timing);

        if obligor_match && modal_match && action_match && timing_match {
            EquivalenceResult::Equivalent { confidence: 0.95 }
        } else if obligor_match && action_match && timing_match {
            EquivalenceResult::ModalDifference {
                from: a.modal,
                to: b.modal,
            }
        } else if obligor_match && modal_match && timing_match {
            EquivalenceResult::ActionDifference {
                similarity: self.action_similarity(&a.action, &b.action),
            }
        } else {
            EquivalenceResult::Different
        }
    }
}

#[derive(Debug)]
pub enum EquivalenceResult {
    Equivalent { confidence: f64 },
    ModalDifference { from: CanonicalModal, to: CanonicalModal },
    ActionDifference { similarity: f64 },
    TimingDifference { explanation: String },
    Different,
}
```

#### 5. SemanticRoleLabeler

```rust
/// Labels semantic roles in obligation phrases.
///
/// Uses pattern-based heuristics rather than ML for reliability.
pub struct SemanticRoleLabeler {
    /// Patterns for role extraction
    patterns: Vec<RolePattern>,
}

#[derive(Debug)]
struct RolePattern {
    /// Regex or keyword pattern
    pattern: PatternType,
    /// Role to assign
    role: SemanticRole,
    /// Position relative to verb
    position: Position,
}

#[derive(Debug)]
enum PatternType {
    Keyword(Vec<&'static str>),
    ByPhrase,           // "by X" → Agent (in passive)
    ToPhrase,           // "to X" → Recipient/Goal
    ForPhrase,          // "for X" → Beneficiary
    WithPhrase,         // "with X" → Instrument
    PreVerbalNP,        // NP before verb → Agent (active)
    PostVerbalNP,       // NP after verb → Patient
}

#[derive(Debug)]
enum Position {
    PreVerb,
    PostVerb,
    InPP,
}

impl SemanticRoleLabeler {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                RolePattern {
                    pattern: PatternType::ByPhrase,
                    role: SemanticRole::Agent,
                    position: Position::InPP,
                },
                RolePattern {
                    pattern: PatternType::ToPhrase,
                    role: SemanticRole::Recipient,
                    position: Position::InPP,
                },
                RolePattern {
                    pattern: PatternType::ForPhrase,
                    role: SemanticRole::Beneficiary,
                    position: Position::InPP,
                },
                RolePattern {
                    pattern: PatternType::Keyword(vec!["within", "before", "after", "by"]),
                    role: SemanticRole::Time,
                    position: Position::InPP,
                },
            ],
        }
    }

    /// Extract semantic frame from an obligation.
    pub fn extract_frame(&self, obligation: &ObligationPhrase) -> SemanticFrame {
        let mut arguments = Vec::new();

        // Agent is the obligor (or by-phrase in passive)
        if let Some(agent) = self.extract_agent(obligation) {
            arguments.push(agent);
        }

        // Patient is typically the direct object
        if let Some(patient) = self.extract_patient(obligation) {
            arguments.push(patient);
        }

        // Extract PP-based arguments
        arguments.extend(self.extract_pp_arguments(obligation));

        SemanticFrame {
            predicate: obligation.action.clone(),
            arguments,
            span: obligation.span,
        }
    }

    fn extract_agent(&self, obligation: &ObligationPhrase) -> Option<SemanticArgument> {
        // Check for agentless passive
        if obligation.is_passive && obligation.by_phrase.is_none() {
            return None; // Ambiguous agent
        }

        let (filler, span) = if obligation.is_passive {
            // Agent from by-phrase
            obligation.by_phrase.as_ref().map(|bp| (bp.text.clone(), bp.span))?
        } else {
            // Agent is obligor
            (obligation.obligor.clone(), obligation.obligor_span)
        };

        Some(SemanticArgument {
            role: SemanticRole::Agent,
            filler,
            span,
            confidence: if obligation.is_passive { 0.9 } else { 0.95 },
        })
    }
}
```

#### 6. TermUsageClassifier

```rust
/// Classifies term usage as use, mention, definition, or redefinition.
///
/// Detects metalinguistic contexts.
pub struct TermUsageClassifier {
    /// Patterns indicating mention context
    mention_patterns: Vec<&'static str>,
    /// Patterns indicating definition
    definition_patterns: Vec<&'static str>,
    /// Patterns indicating scoped redefinition
    redefinition_patterns: Vec<&'static str>,
}

impl TermUsageClassifier {
    pub fn new() -> Self {
        Self {
            mention_patterns: vec![
                "the term",
                "the word",
                "references to",
                "as used in",
                "the expression",
            ],
            definition_patterns: vec![
                "means",
                "shall mean",
                "is defined as",
                "refers to",
                "includes",
            ],
            redefinition_patterns: vec![
                "for purposes of this section",
                "for purposes of section",
                "in this article",
                "for the purposes of",
            ],
        }
    }

    /// Classify a term occurrence.
    pub fn classify(&self, term: &str, context: &str) -> TermOccurrence {
        let lower_context = context.to_lowercase();

        // Check for scoped redefinition first (most specific)
        for pattern in &self.redefinition_patterns {
            if lower_context.contains(pattern) {
                let scope = self.extract_scope(&lower_context, pattern);
                return TermOccurrence {
                    term: term.to_string(),
                    usage: TermUsage::Redefinition { scope: scope.clone() },
                    scope: Some(scope),
                    span: (0, 0), // Set by caller
                    confidence: 0.9,
                };
            }
        }

        // Check for definition patterns
        for pattern in &self.definition_patterns {
            if lower_context.contains(pattern) {
                // Also check for mention markers
                let is_mention = self.mention_patterns
                    .iter()
                    .any(|mp| lower_context.contains(mp));

                return TermOccurrence {
                    term: term.to_string(),
                    usage: if is_mention {
                        TermUsage::Definition
                    } else {
                        TermUsage::Use
                    },
                    scope: None,
                    span: (0, 0),
                    confidence: 0.85,
                };
            }
        }

        // Check for mention patterns
        for pattern in &self.mention_patterns {
            if lower_context.contains(pattern) {
                return TermOccurrence {
                    term: term.to_string(),
                    usage: TermUsage::Mention,
                    scope: None,
                    span: (0, 0),
                    confidence: 0.9,
                };
            }
        }

        // Default: regular use
        TermOccurrence {
            term: term.to_string(),
            usage: TermUsage::Use,
            scope: None,
            span: (0, 0),
            confidence: 0.7,
        }
    }
}
```

### Integration with SemanticDiffEngine

The existing `SemanticDiffEngine` should be extended to use these resolvers:

```rust
impl SemanticDiffEngine {
    /// Enhanced diff computation using FR-006 resolvers.
    pub fn compute_diff_v2(
        &self,
        original: &ContractDocument,
        revised: &ContractDocument,
    ) -> SemanticDiffResult {
        let normalizer = ObligationNormalizer::new();
        let conflict_detector = ConflictDetector::new();
        let precedence_applicator = PrecedenceApplicator::new();

        // Step 1: Normalize obligations in both documents
        let orig_normalized = self.normalize_document(original, &normalizer);
        let rev_normalized = self.normalize_document(revised, &normalizer);

        // Step 2: Match equivalent obligations (reduces false positives)
        let matched = self.match_obligations(&orig_normalized, &rev_normalized, &normalizer);

        // Step 3: Detect intra-document conflicts in revised version
        let conflicts = conflict_detector.detect_conflicts(revised);

        // Step 4: Apply precedence rules to conflicts
        let precedence_rules = self.extract_precedence_rules(revised);
        let resolutions = precedence_applicator.apply(&conflicts, &precedence_rules);

        // Step 5: Build final diff including conflict information
        self.build_enhanced_diff(matched, conflicts, resolutions)
    }
}
```

### Algorithm Sketches

#### Obligation Chain Analysis

```rust
/// Detect obligation chains that exceed external constraints.
fn detect_chain_conflicts(&self, doc: &ContractDocument) -> Vec<Conflict> {
    let mut graph = ObligationGraph::new();

    // Build dependency graph
    for obligation in doc.obligations() {
        let node = graph.add_obligation(obligation);

        // Find trigger: "upon X", "after X", "following X"
        if let Some(trigger) = obligation.trigger_event() {
            if let Some(source_node) = graph.find_by_event(&trigger) {
                graph.add_edge(source_node, node);
            }
        }
    }

    // Find terminal constraints (termination dates, sunset clauses)
    let terminals = self.extract_terminal_constraints(doc);

    // Project deadlines through chains
    let mut conflicts = Vec::new();
    for terminal in &terminals {
        let chains = graph.chains_to(terminal);
        for chain in chains {
            if let Some(conflict) = self.check_chain_exceeds(chain, terminal) {
                conflicts.push(conflict);
            }
        }
    }

    conflicts
}
```

#### Exception Hierarchy Analysis

```rust
/// Detect conflicts between exceptions and main obligations.
fn detect_exception_conflicts(&self, doc: &ContractDocument) -> Vec<Conflict> {
    let mut conflicts = Vec::new();

    for section in doc.sections() {
        // Build exception tree for this section
        let exception_tree = self.build_exception_tree(section);

        // Check for double exceptions (exception to exception)
        for exception in &exception_tree.exceptions {
            if !exception.sub_exceptions.is_empty() {
                conflicts.push(self.build_double_exception_conflict(
                    &exception_tree.main_obligation,
                    exception,
                ));
            }
        }

        // Check for overlapping exception scopes
        for (i, exc_a) in exception_tree.exceptions.iter().enumerate() {
            for exc_b in exception_tree.exceptions.iter().skip(i + 1) {
                if self.scopes_overlap(exc_a, exc_b) {
                    conflicts.push(self.build_overlapping_exception_conflict(exc_a, exc_b));
                }
            }
        }
    }

    conflicts
}
```

### Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_modal_conflict() {
        let detector = ConflictDetector::new();
        let doc = ContractDocument::from_text(
            "Section 1: Buyer shall pay within 30 days.\n\
             Section 2: Buyer may pay within 30 days."
        );

        let conflicts = detector.detect_conflicts(&doc);
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(
            conflicts[0].conflict_type,
            ConflictType::Direct { dimension: ConflictDimension::Modal }
        ));
    }

    #[test]
    fn test_temporal_conflict() {
        let detector = ConflictDetector::new();
        let doc = ContractDocument::from_text(
            "Section 1: Notice within 30 calendar days.\n\
             Section 2: Notice within 15 business days."
        );

        let conflicts = detector.detect_conflicts(&doc);
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(conflicts[0].conflict_type, ConflictType::Temporal { .. }));
    }

    #[test]
    fn test_equivalence_detection() {
        let normalizer = ObligationNormalizer::new();

        let ob_a = ObligationPhrase::parse("Seller shall deliver Products within 30 days");
        let ob_b = ObligationPhrase::parse("Products must be delivered by Seller within 30 days");

        let norm_a = normalizer.normalize(&ob_a);
        let norm_b = normalizer.normalize(&ob_b);

        assert!(matches!(
            normalizer.equivalent(&norm_a, &norm_b),
            EquivalenceResult::Equivalent { .. }
        ));
    }

    #[test]
    fn test_deictic_context_in_quotes() {
        let mut tracker = DeicticContextTracker::new();

        tracker.process_token("\"", (0, 1));
        tracker.process_token("you", (1, 4));

        let deictic = DeicticReference::new(
            DeicticCategory::Person,
            DeicticSubcategory::PersonSecond,
            "you",
            DeicticSource::WordList { pattern: "you" },
        );

        let resolved = tracker.resolve_deictic(&deictic);
        assert!(resolved.context_note.is_some());
        assert!(resolved.confidence < 1.0);
    }
}
```
