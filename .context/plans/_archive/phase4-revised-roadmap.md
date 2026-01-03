# Phase 4 Revised Roadmap: Foundation-First Architecture

**Last Updated:** 2025-12-31
**Status:** Critical Path Complete (305 passing tests)

## Executive Summary

Following architectural review, Phase 4 is restructured around two unifying abstractions:

1. **SpanLink<R>** — Generic typed relations between spans (attachments, roles, conflicts, hierarchy)
2. **ScopeOperator<O>** — Operators with trigger + domain (negation, quantifiers, precedence, deictic frames)

This foundation-first approach:
- Reduces one-off logic across milestones
- Enables N-best ambiguity preservation naturally
- Leverages the existing span-stacking architecture
- Makes the document a navigable graph per the Associative Spans vision

---

## Revised Milestone Structure

### Critical Path (Contract Intelligence)

```
M0 (Foundation) → M2 (Clause) → M4 (Precedence) → M7 (Scope) → M8 (Roles/Equiv)
        ↓              ↓
      M5 (Deictic)   M6 (Attachment)  ← Quality upgrades (parallelizable)
```

| Milestone | Title | Effort | Status | Tests | Critical Path |
|-----------|-------|--------|--------|-------|---------------|
| **M0** | SpanLink + ScopeOperator Foundation | M | ✅ Done | 36 | ✓ |
| M1 | ConflictDetector | S/M | ✅ Done | 54 | — |
| **M2** | ClauseBoundary + Coordination | M | ✅ Done | 61 | ✓ |
| M3 | TermsOfArt | S | ✅ Done | 36 | — |
| **M4** | Precedence Resolution | M | ✅ Done | 49 | ✓ |
| M5 | Metalinguistic + Deictic | M | 📋 Optional | — | — |
| M6 | PP/Relative Attachment | M/L | 📋 Optional | — | — |
| **M7** | Negation + Quantifier Scope | M | ✅ Done | 17 | ✓ |
| **M8** | Semantic Roles + Equivalence | L | ✅ Done | 52 | ✓ |

---

## M0: SpanLink & ScopeOperator Foundation (NEW)

**Goal:** Provide unified abstractions for relations and scope-bearing operators.

### Core Types

#### SpanLink<R, S>

```rust
/// Generic binary relation between anchor span and target span.
/// Anchor is where the attribute is stored; target is what it points to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanLink<R, S = SpanRef> {
    /// Semantic role of target with respect to anchor
    pub role: R,
    /// The target span participating in this relation
    pub target: S,
}

// Type aliases
pub type LineSpanLink<R> = SpanLink<R, SpanRef>;
pub type DocSpanLink<R> = SpanLink<R, DocSpan>;
```

**Usage patterns:**
- Clause hierarchy: `child.assign(Scored<SpanLink<ClauseRole::Parent>>)`
- PP attachment: `pp.assign(Scored<SpanLink<PPRole::Head>>)` — emit multiple for N-best
- Semantic roles: `predicate.assign(Scored<SpanLink<SemanticRole::Agent>>)`
- Conflicts: `conflict_span.assign(SpanLink<ConflictRole::Left/Right>)`

#### ScopeOperator<O>

```rust
/// A scope-bearing operator with trigger span and domain span.
#[derive(Debug, Clone)]
pub struct ScopeOperator<O> {
    /// Dimension: Negation, Quantifier, Precedence, Deictic, Other
    pub dimension: ScopeDimension,
    /// The trigger span ("not", "each", "notwithstanding")
    pub trigger: DocSpan,
    /// Domain this operator scopes over (with N-best alternatives)
    pub domain: ScopeDomain,
    /// Operator-specific payload
    pub payload: O,
}

/// Domain with ambiguity support
#[derive(Debug, Clone)]
pub struct ScopeDomain {
    /// Candidates sorted by descending score (best first)
    pub candidates: Vec<Scored<DocSpan>>,
}
```

**Usage patterns:**
- Negation: `ScopeOperator<NegationOp>` with domain covering VP/clause
- Quantifier: `ScopeOperator<QuantifierOp>` with domain over NP
- Precedence: `ScopeOperator<PrecedenceOp>` governing sections
- Deictic: `ScopeOperator<DeicticFrame>` for quoted/reported speech

### Deliverables

- [x] `span_link.rs` — SpanLink<R, S> core type + role enums
- [x] `scope_operator.rs` — ScopeOperator<O> + ScopeDomain + ScopeDimension
- [x] `scope_index.rs` — ScopeIndex for "what scopes cover this span?" queries
- [x] Integration with existing AssociatedSpan for provenance
- [x] Pipeline hooks for emitting SpanLinks/ScopeOperators
- [x] 36 tests covering type mechanics and queries

**Effort:** M (2-3 days)
**Dependencies:** None (foundation layer)
**Unlocks:** All remaining milestones
**Status:** ✅ Complete

---

## N-Best Ambiguity Strategy

### Core Design

Resolvers emit multiple `Scored<T>` for ambiguous cases. A new `Ambiguous<T>` wrapper aggregates them:

```rust
#[derive(Debug, Clone)]
pub enum AmbiguityFlag {
    None,
    LowConfidence,         // best.score < 0.6
    CompetingAlternatives, // alt.score >= best.score - 0.1
}

#[derive(Debug, Clone)]
pub struct Ambiguous<T> {
    pub best: Scored<T>,
    pub alternatives: Vec<Scored<T>>,
    pub flag: AmbiguityFlag,
}

#[derive(Debug, Clone)]
pub struct AmbiguityConfig {
    pub n_best: usize,        // default: 4
    pub min_score: f32,       // default: 0.25
    pub low_confidence: f32,  // default: 0.6
    pub ambiguity_margin: f32, // default: 0.1
}
```

### API Strategy

- **Existing APIs unchanged:** `query_all()` returns best-only (backward compatible)
- **New N-best APIs:** `query_all_ambiguous(&cfg)` returns `Vec<Ambiguous<T>>`
- **Human review hook:** Check `flag != AmbiguityFlag::None` to surface uncertain cases

---

## Revised Milestone Details

### M2: ClauseBoundary + Coordination

**Reframed as:** Emit clause structure as `SpanLink<ClauseRole>` relations.

```rust
pub enum ClauseRole {
    Parent,    // anchor clause's parent
    Child,     // anchor clause's child
    Conjunct,  // coordination sibling
    Exception, // exception/carve-out
}
```

**Deliverables:**
- [x] `clause_boundary_resolver.rs` emitting SpanLink<ClauseRole>
- [x] Coordination structure as SpanLinks between conjuncts
- [x] API: "Given this span, what clause(s) contain it?"
- [x] 61 tests: single/multi-sentence, complex coordination

**Depends on:** M0
**Effort:** M
**Status:** ✅ Complete

---

### M4: Precedence Resolution

**Reframed as:** Precedence clauses become `ScopeOperator<PrecedenceOp>`.

```rust
pub struct PrecedenceOp {
    pub connective: String,       // "subject to", "notwithstanding"
    pub overrides_domain: bool,   // true if this clause overrides
    pub referenced_sections: Vec<String>,
}
```

**Deliverables:**
- [x] `precedence_resolver.rs` emitting ScopeOperator<PrecedenceOp>
- [x] Integration with M1 ConflictDetector for resolution
- [x] ConflictResolution struct citing precedence source
- [x] 49 tests: A vs B with "subject to", chains, cycles

**Depends on:** M0, M1, M2 (for clause scope)
**Effort:** M
**Status:** ✅ Complete

---

### M7: Negation + Quantifier Scope

**Reframed as:** Scope operators over clause structures.

```rust
pub struct NegationOp {
    pub marker: String,          // "not", "never", "no"
    pub kind: NegationKind,      // Sentential, Nominal, Other
}

pub struct QuantifierOp {
    pub marker: String,          // "each", "every", "any"
    pub kind: QuantifierKind,    // Universal, Existential, Negative
    pub cardinality: Option<QuantifierCardinality>,
}
```

**Deliverables:**
- [x] `negation_scope_resolver.rs` emitting ScopeOperator<NegationOp>
- [x] `quantifier_scope_resolver.rs` emitting ScopeOperator<QuantifierOp>
- [x] Scope computation using M2 clause boundaries
- [x] Interaction logic: negation vs quantifier ordering
- [x] 17 tests: simple negation, double negation, scope ambiguities

**Depends on:** M0, M2
**Effort:** M
**Status:** ✅ Complete

---

### M5: Metalinguistic + Deictic (Optional - Off Critical Path)

**Reframed as:** SpanLinks for references + ScopeOperator for deictic frames.

```rust
pub struct DeicticFrame {
    pub speaker: Option<String>,
    pub time_anchor: Option<String>,
    pub quoted_span: Option<DocSpan>,
}
```

**Deliverables:**
- [ ] `metalinguistic_resolver.rs` — links for "foregoing", "above", "below"
- [ ] `deictic_resolver.rs` — ScopeOperator<DeicticFrame> for quoted speech
- [ ] Tests: intra-section, cross-section references

**Depends on:** M0, FR-002/004
**Effort:** M
**Status:** 📋 Optional (not on critical path)

---

### M6: PP/Relative Attachment (Optional - Off Critical Path)

**Reframed as:** SpanLink<AttachmentRole> with N-best alternatives.

```rust
pub enum AttachmentRole {
    Head,      // PP/RC attaches to this head
}
```

**Deliverables:**
- [ ] `pp_attachment_resolver.rs` emitting multiple Scored<SpanLink<AttachmentRole>>
- [ ] `relative_clause_resolver.rs` for RC attachment
- [ ] N-best preservation (beam-like behavior)
- [ ] Tests: classic attachment ambiguities

**Depends on:** M0, M2
**Effort:** M/L
**Status:** 📋 Optional (not on critical path)

---

### M8: Semantic Roles + Equivalence (2-Gate)

**Gate 1: Semantic Roles** — SpanLink<SemanticRole> from predicates to arguments.

```rust
pub enum SemanticRole {
    Agent,
    Theme,
    Beneficiary,
    Condition,
    Exception,
    Location,
    Time,
}
```

**Gate 2: Obligation Equivalence** — Compare normalized obligations.

```rust
pub struct ObligationEquivalence {
    pub obligation_a: DocSpan,
    pub obligation_b: DocSpan,
    pub similarity: f64,
    pub differences: Vec<EquivalenceDifference>,
}
```

**Deliverables:**
- [x] Gate 1: `semantic_role_resolver.rs` mapping obligations to roles
- [x] Gate 2: equivalence detection using roles + scope + precedence
- [x] 52 tests: equivalent pairs, near-equivalents with scope differences

**Depends on:** M0, M2, M4, M7
**Effort:** L (split across 2 gates)
**Status:** ✅ Complete

---

## Implementation Order

### Phase 1: Foundation (Week 1) ✅
1. **M0** — SpanLink + ScopeOperator core types ✅

### Phase 2: Structure (Week 2) ✅
2. **M2** — ClauseBoundary + Coordination (critical path) ✅
3. **M5** — Metalinguistic + Deictic (optional, deferred)

### Phase 3: Operators (Week 3) ✅
4. **M4** — Precedence Resolution (critical path) ✅
5. **M7** — Negation + Quantifier Scope (critical path) ✅
6. **M6** — PP/Relative Attachment (optional, deferred)

### Phase 4: Semantics (Week 4) ✅
7. **M8 Gate 1** — Semantic Roles ✅
8. **M8 Gate 2** — Obligation Equivalence ✅

---

## Completion Summary

**Critical Path Status:** All milestones complete

| Completed Milestones | Tests |
|---------------------|-------|
| M0: SpanLink + ScopeOperator Foundation | 36 |
| M1: ConflictDetector | 54 |
| M2: ClauseBoundary + Coordination | 61 |
| M3: TermsOfArt | 36 |
| M4: Precedence Resolution | 49 |
| M7: Negation + Quantifier Scope | 17 |
| M8: Semantic Roles + Equivalence | 52 |
| **Total** | **305** |

**Remaining Optional Milestones:**
- M5: Metalinguistic + Deictic (quality upgrade, not blocking)
- M6: PP/Relative Attachment (quality upgrade, not blocking)

---

## Success Metrics

After Phase 4 completion:

1. **Conflict Resolution:** Conflicts show precedence-based resolution with citations
2. **Scope Awareness:** "Shall not" correctly scoped; quantifiers handled
3. **Semantic Equivalence:** Duplicate/overlapping obligations detected
4. **N-Best Preserved:** Ambiguous cases flagged for review with alternatives
5. **Graph Navigable:** All relations traversable via SpanLink/AssociatedSpan

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| M0 over-engineering | Start minimal; add features as milestones need them |
| N-best explosion | Enforce n_best=4, min_score=0.25 thresholds |
| Scope interaction complexity | Keep ScopeOperator structural; logic in downstream |
| Cross-line span confusion | Use DocSpan consistently; SpanRef only for line-local |

---

## Open Questions (Resolved)

1. ~~Should M0 be committed before or after updating this roadmap?~~ **Resolved:** M0 committed and complete.
2. ~~Do we retrofit M1/M3 to use SpanLink, or leave as-is?~~ **Resolved:** Implementation complete.
3. ~~Should AmbiguityConfig be per-resolver or global?~~ **Resolved:** Implementation complete.

---

*This roadmap supersedes FR-005-006-implementation-roadmap.md. Critical path implementation complete as of 2025-12-31.*
