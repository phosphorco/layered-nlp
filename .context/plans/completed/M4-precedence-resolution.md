# M4: Precedence Resolution

**FR:** FR-006 – Semantic Analysis and Conflict Detection
**Status:** ✅ Complete — 2025-12-31
**Effort:** M (3-5 hours)
**Dependencies:** M0 ✅, M1 ✅
**Tests:** 49 tests

---

## Overview

M4 extends the baseline ConflictDetector (M1) with precedence resolution capabilities. When conflicts are detected between provisions, M4 applies precedence rules to determine which provision prevails. This implements the second major component of FR-006's conflict detection system.

**Key capabilities:**
- Detect precedence declarations ("notwithstanding Section X", "subject to Y", "in case of conflict, Schedules prevail")
- Parse precedence ordering from contract text
- Apply precedence rules to resolve conflicts detected by M1
- Detect circular or contradictory precedence declarations
- Use `ScopeOperator<PrecedenceOp>` from M0 foundation

**Design insight:** Precedence resolution is separate from conflict detection. M1 finds conflicts; M4 resolves them using precedence rules. This separation allows conflicts to be surfaced even when resolution is ambiguous.

---

## Gates

### Gate 0: Core Types and Precedence Operator
**Status:** ✅ Complete

**Deliverables:**

```rust
// Use ScopeOperator<PrecedenceOp> from M0
// layered-contracts/src/precedence.rs

use layered_contracts::{ScopeOperator, ScopeDimension, ScopeDomain, DocSpan};

/// Payload for precedence operators.
#[derive(Debug, Clone, PartialEq)]
pub struct PrecedenceOp {
    /// The precedence connective ("notwithstanding", "subject to", "except as provided")
    pub connective: String,
    /// Whether this operator overrides its domain (true for "notwithstanding")
    pub overrides_domain: bool,
    /// Referenced sections if explicitly mentioned
    pub referenced_sections: Vec<String>,
}

/// Classification of document sections for precedence ordering.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SectionKind {
    Schedule(Option<String>),  // Schedule A, Schedule B
    Exhibit(Option<String>),
    Appendix(Option<String>),
    Article,
    Section,
    Recital,
    Amendment,
    MainBody,
    Custom(String),
}

/// A precedence rule extracted from contract text.
#[derive(Debug, Clone, PartialEq)]
pub struct PrecedenceRule {
    /// Section where this rule is declared
    pub source_section: String,
    /// Ordered list of section kinds (highest priority first)
    pub ordering: Vec<SectionKind>,
    /// The span containing this rule
    pub span: DocSpan,
}

/// Result of applying precedence to a conflict.
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictResolution {
    /// The conflict being resolved
    pub conflict_id: String,
    /// Which provision prevails
    pub winner: DocSpan,
    /// Basis for the resolution
    pub basis: ResolutionBasis,
    /// Confidence in this resolution
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResolutionBasis {
    /// Explicit precedence clause
    PrecedenceClause {
        rule: PrecedenceRule,
        explanation: String,
    },
    /// Later provision overrides earlier (implicit)
    LaterPrevails { explanation: String },
    /// More specific provision overrides general
    SpecificOverGeneral { explanation: String },
    /// Exception clause applies
    ExceptionApplies { explanation: String },
    /// Cannot determine winner
    Undeterminable { reason: String },
}
```

**Verification:**
- [x] Types compile without errors
- [x] `ScopeOperator<PrecedenceOp>` can be constructed
- [x] All types derive `Debug, Clone, PartialEq`
- [x] 3 unit tests for basic type construction

---

### Gate 1: Precedence Declaration Detection
**Status:** ✅ Complete

**Deliverables:**

```rust
/// Detects precedence declarations in contract text.
pub struct PrecedenceDetector {
    /// Patterns for precedence connectives
    connective_patterns: Vec<PrecedencePattern>,
}

struct PrecedencePattern {
    /// Pattern text ("notwithstanding", "subject to")
    pattern: &'static str,
    /// Whether this overrides domain
    overrides: bool,
}

impl PrecedenceDetector {
    pub fn new() -> Self {
        Self {
            connective_patterns: vec![
                PrecedencePattern {
                    pattern: "notwithstanding",
                    overrides: true,
                },
                PrecedencePattern {
                    pattern: "subject to",
                    overrides: false,
                },
                PrecedencePattern {
                    pattern: "except as provided in",
                    overrides: false,
                },
                // Additional patterns...
            ],
        }
    }

    /// Detect precedence operators in document.
    pub fn detect_operators(
        &self,
        doc: &ContractDocument,
    ) -> Vec<ScopeOperator<PrecedenceOp>> {
        // Scan document for precedence connectives
        // Extract domain spans
        // Create ScopeOperator instances
    }

    /// Detect explicit precedence ordering rules.
    ///
    /// Example: "In case of conflict, Schedules prevail over Articles."
    pub fn detect_ordering_rules(
        &self,
        doc: &ContractDocument,
    ) -> Vec<PrecedenceRule> {
        // Pattern match on "in case of conflict", "in event of inconsistency"
        // Extract section kind ordering
        // Build PrecedenceRule instances
    }
}
```

**Patterns to detect:**
- "notwithstanding Section X"
- "subject to the provisions of Y"
- "except as provided in Z"
- "in case of conflict, A prevails over B"
- "this agreement supersedes all prior agreements"
- "to the extent inconsistent with X, Y shall control"

**Verification:**
- [x] Detect "notwithstanding Section 5" → operator with referenced_sections=["5"]
- [x] Detect "subject to Exhibit A" → operator with overrides_domain=false
- [x] Detect "in case of conflict, Schedules prevail" → ordering rule
- [x] 8 unit tests covering all precedence patterns

---

### Gate 2: Precedence Application Logic
**Status:** ✅ Complete

**Deliverables:**

```rust
/// Applies precedence rules to resolve conflicts.
pub struct PrecedenceResolver {
    /// Default precedence ordering if no explicit rule
    default_ordering: Vec<SectionKind>,
    /// Confidence threshold for applying resolution
    confidence_threshold: f64,
}

impl PrecedenceResolver {
    pub fn new() -> Self {
        Self {
            // Default: specificity principle
            default_ordering: vec![
                SectionKind::Amendment,
                SectionKind::Schedule(None),
                SectionKind::Exhibit(None),
                SectionKind::Section,
                SectionKind::Article,
                SectionKind::Recital,
                SectionKind::MainBody,
            ],
            confidence_threshold: 0.6,
        }
    }

    /// Resolve conflicts using precedence rules.
    pub fn resolve_conflicts(
        &self,
        conflicts: &[Scored<Conflict>],
        rules: &[PrecedenceRule],
        operators: &[ScopeOperator<PrecedenceOp>],
    ) -> Vec<Scored<ConflictResolution>> {
        // For each conflict:
        //   1. Check if any PrecedenceOp covers the conflict spans
        //   2. Check if any PrecedenceRule applies
        //   3. Apply default ordering if no explicit rule
        //   4. Build ConflictResolution with basis
    }

    /// Check if a precedence operator covers a conflict.
    fn operator_covers(
        &self,
        op: &ScopeOperator<PrecedenceOp>,
        conflict: &Conflict,
    ) -> bool {
        // Check if conflict spans overlap with operator domain
    }

    /// Determine winner from precedence rule.
    fn apply_rule(
        &self,
        conflict: &Conflict,
        rule: &PrecedenceRule,
    ) -> Option<ConflictResolution> {
        // Extract section kinds from conflict spans
        // Compare using rule ordering
        // Build resolution with PrecedenceClause basis
    }

    /// Detect circular precedence declarations.
    pub fn detect_circular(
        &self,
        rules: &[PrecedenceRule],
    ) -> Vec<CircularPrecedence> {
        // Build precedence graph
        // Detect cycles
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CircularPrecedence {
    pub cycle: Vec<String>, // Section IDs forming the cycle
    pub explanation: String,
}
```

**Resolution strategy:**
1. Check for `ScopeOperator<PrecedenceOp>` covering conflict → use operator
2. Check for explicit `PrecedenceRule` → apply ordering
3. Fall back to default ordering (specificity principle)
4. If still ambiguous → `ResolutionBasis::Undeterminable`

**Verification:**
- [x] Resolve conflict using "Schedules prevail" rule
- [x] Resolve using "notwithstanding" operator
- [x] Detect circular precedence: A > B, B > C, C > A
- [x] Apply default ordering when no explicit rule
- [x] 10 unit tests for resolution scenarios

---

### Gate 3: Section Kind Extraction
**Status:** ✅ Complete

**Deliverables:**

```rust
/// Extract SectionKind from DocSpan or section identifier.
pub struct SectionClassifier {
    /// Patterns for section kind detection
    patterns: HashMap<SectionKind, Vec<&'static str>>,
}

impl SectionClassifier {
    pub fn new() -> Self {
        // Build pattern map:
        //   Schedule → ["Schedule", "Annex", "Attachment"]
        //   Exhibit → ["Exhibit"]
        //   Article → ["Article"]
        //   etc.
    }

    /// Classify a section by its identifier.
    pub fn classify(&self, section_id: &str) -> SectionKind {
        // Match against patterns
        // Extract letter/number if present (e.g., "Schedule A")
        // Return appropriate SectionKind variant
    }

    /// Extract section ID from DocSpan using document structure.
    pub fn section_id_from_span(
        &self,
        span: &DocSpan,
        doc: &ContractDocument,
    ) -> Option<String> {
        // Query document structure
        // Find containing section
        // Return section identifier
    }
}
```

**Verification:**
- [x] Classify "Schedule A" → `SectionKind::Schedule(Some("A"))`
- [x] Classify "Exhibit B" → `SectionKind::Exhibit(Some("B"))`
- [x] Classify "Article 5" → `SectionKind::Article`
- [x] Extract section ID from DocSpan
- [x] 6 unit tests for classification

---

### Gate 4: Document Integration
**Status:** ✅ Complete

**Deliverables:**

```rust
impl PrecedenceResolver {
    /// Resolve conflicts in a document.
    pub fn resolve_in_document(
        &self,
        doc: &ContractDocument,
        conflicts: &[Scored<Conflict>],
    ) -> Vec<Scored<ConflictResolution>> {
        // Step 1: Detect precedence operators
        let detector = PrecedenceDetector::new();
        let operators = detector.detect_operators(doc);
        let rules = detector.detect_ordering_rules(doc);

        // Step 2: Detect circular precedence
        let circular = self.detect_circular(&rules);
        if !circular.is_empty() {
            // Log warning or create undeterminable resolutions
        }

        // Step 3: Resolve each conflict
        self.resolve_conflicts(conflicts, &rules, &operators)
    }
}
```

**Integration with M1:**
```rust
// Example usage combining M1 + M4
let detector = ConflictDetector::new();
let conflicts = detector.detect_in_document(&doc);

let resolver = PrecedenceResolver::new();
let resolutions = resolver.resolve_in_document(&doc, &conflicts);

for resolution in &resolutions {
    match &resolution.value.basis {
        ResolutionBasis::PrecedenceClause { rule, explanation } => {
            println!("Resolved by precedence: {}", explanation);
        }
        ResolutionBasis::Undeterminable { reason } => {
            println!("Cannot resolve: {}", reason);
        }
        _ => {}
    }
}
```

**Verification:**
- [x] End-to-end test: detect conflict, apply precedence, get resolution
- [x] Test with explicit precedence clause
- [x] Test with "notwithstanding" operator
- [x] Test with circular precedence (undeterminable)
- [x] 5 integration tests

---

### Gate 5: Comprehensive Testing and Documentation
**Status:** ✅ Complete

**Deliverables:**
- [x] Module documentation with usage examples
- [x] Export all types from `lib.rs`
- [x] End-to-end tests with realistic contract excerpts
- [x] Edge case tests:
  - [x] Multiple precedence rules applying to same conflict
  - [x] Precedence rule with no matching conflicts
  - [x] "Notwithstanding" with missing section reference
  - [x] Circular precedence between 3+ sections
- [x] Update M4 plan status in FR-005-006-implementation-roadmap.md

**Verification:**
- [x] All M4 tests pass (49 tests)
- [x] All layered-contracts tests pass
- [x] `cargo doc` builds without warnings
- [x] Types accessible from crate root

---

## Design Decisions

### 1. ScopeOperator<PrecedenceOp> vs direct spans

Use M0's `ScopeOperator` foundation to unify precedence with other scope-bearing operators (negation, quantifiers in M7). The trigger span is the precedence connective; the domain is the span being overridden or conditioned.

### 2. Separate detection from resolution

`PrecedenceDetector` finds precedence declarations; `PrecedenceResolver` applies them to conflicts. This separation allows:
- Displaying precedence clauses independently of conflicts
- Testing precedence parsing without conflict detection
- Applying precedence in future contexts beyond conflict resolution

### 3. Default ordering as fallback

When no explicit precedence rule exists, apply the specificity principle (amendments > schedules > sections > recitals). This provides best-effort resolution rather than always returning "undeterminable."

### 4. Circular precedence as warning, not error

Circular precedence declarations are detected and flagged, but don't crash the resolver. Conflicts with circular precedence get `ResolutionBasis::Undeterminable` with explanation.

---

## Success Criteria

After M4:
- [x] Precedence declarations detected from contract text
- [x] `ScopeOperator<PrecedenceOp>` instances created for "notwithstanding", "subject to" patterns
- [x] Explicit precedence ordering rules parsed ("Schedules prevail over Articles")
- [x] Conflicts from M1 resolved using precedence rules
- [x] Circular precedence detected and flagged
- [x] Integration with M1 ConflictDetector seamless
- [x] All tests pass with realistic contract language (49 tests)

---

## Non-Goals for M4

- **No condition-based precedence:** "If X, then A prevails; otherwise B prevails" deferred to M5
- **No multi-document precedence:** Cross-document precedence (main agreement vs incorporated terms) out of scope
- **No temporal precedence:** "Later amendments prevail" handled by default ordering, not explicit temporal logic
- **No precedence confidence scoring:** Use fixed confidence based on basis type (explicit rule: 0.9, default: 0.7, undeterminable: 0.3)

---

## Appendix A: Precedence Patterns Reference

| Pattern | Example | Overrides? | Domain |
|---------|---------|------------|--------|
| notwithstanding | "Notwithstanding Section 5, the Company may..." | Yes | Section 5 |
| subject to | "Subject to Exhibit A, Buyer shall..." | No | Exhibit A conditions |
| except as provided | "Except as provided in 3.2, all obligations..." | No | Section 3.2 exceptions |
| in case of conflict | "In case of conflict, Schedules prevail." | N/A | Ordering rule |
| supersedes | "This Agreement supersedes all prior agreements." | Yes | Prior agreements |
| to the extent inconsistent | "To the extent inconsistent with Article 2, this Section controls." | Yes | Article 2 |

---

## Appendix B: Example Precedence Scenarios

### Scenario 1: Explicit precedence clause

```
Section 15.3: In the event of any conflict between the provisions of this Agreement
and the Schedules, the Schedules shall prevail.

Section 2.1: Payment due within 30 days.
Schedule A: Payment due within 15 days.
```

**M1 output:** Temporal conflict (30 days vs 15 days)
**M4 output:** Resolution → Schedule A prevails (basis: PrecedenceClause from Section 15.3)

### Scenario 2: "Notwithstanding" operator

```
Section 3.1: The Company shall indemnify the Buyer.
Section 3.2: Notwithstanding Section 3.1, the Company shall not indemnify claims
             arising from Buyer's negligence.
```

**M1 output:** Modal conflict (shall indemnify vs shall not indemnify)
**M4 output:** Resolution → Section 3.2 prevails (basis: PrecedenceClause from "notwithstanding" operator)

### Scenario 3: Circular precedence

```
Section 10: In case of conflict, this Section prevails over Section 11.
Section 11: In case of conflict, this Section prevails over Section 12.
Section 12: In case of conflict, this Section prevails over Section 10.
```

**M4 output:** CircularPrecedence detected (cycle: [10, 11, 12])
**Resolution:** Undeterminable (reason: "Circular precedence among Sections 10, 11, 12")

---

## Learnings & Deviations

This section will capture implementation learnings and deviations from the plan. Initially empty.
