# M0: SpanLink & ScopeOperator Foundation

**FR:** Phase 4 Foundation (cross-cutting)
**Status:** ✅ Complete (2025-12-31) — 36 tests
**Effort:** M (2-3 days)
**Priority:** Critical — blocks all remaining Phase 4 milestones

## Summary

Introduce two unifying abstractions that all Phase 4 milestones will use:

1. **SpanLink<R, S>** — Generic typed relation between spans
2. **ScopeOperator<O>** — Operator with trigger span and domain span

This aligns Phase 4 with the layered-nlp architecture philosophy:
- Everything is a span
- Relationships are first-class
- Multiple interpretations stack
- Queries are type-driven

---

## Gates

### Gate 1: SpanLink Core Types
**Status:** ✅ Complete

**Deliverables:**

```rust
// layered-nlp/src/span_link.rs (or layered-contracts/src/span_link.rs)

use crate::SpanRef;

/// Generic binary relation from anchor span to target span.
///
/// The anchor is the span where this attribute is stored.
/// The target is what the relation points to.
///
/// # Type Parameters
/// - `R`: Role enum specific to the relation family
/// - `S`: Span type (SpanRef for line-local, DocSpan for document-level)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpanLink<R, S = SpanRef> {
    /// Semantic role of target with respect to anchor
    pub role: R,
    /// The target span
    pub target: S,
}

impl<R, S> SpanLink<R, S> {
    pub fn new(role: R, target: S) -> Self {
        Self { role, target }
    }
}

/// Line-local link (default)
pub type LineSpanLink<R> = SpanLink<R, SpanRef>;

// When DocSpan is in scope:
// pub type DocSpanLink<R> = SpanLink<R, DocSpan>;
```

**Role enum examples (to be defined per milestone):**

```rust
// M2: Clause hierarchy
pub enum ClauseRole {
    Parent,
    Child,
    Conjunct,
    Exception,
}

// M6: PP/RC attachment
pub enum AttachmentRole {
    Head,
}

// M8: Semantic roles
pub enum SemanticRole {
    Agent,
    Theme,
    Beneficiary,
    Condition,
    Exception,
    Location,
    Time,
}

// M4: Conflicts (if we refactor M1)
pub enum ConflictRole {
    Left,
    Right,
}
```

**Verification:**
- [ ] SpanLink<R> compiles with various R types
- [ ] Works with Scored<SpanLink<R>> wrapper
- [ ] TypeId distinguishes SpanLink<ClauseRole> from SpanLink<SemanticRole>
- [ ] 5 unit tests for core mechanics

---

### Gate 2: ScopeOperator Core Types
**Status:** ✅ Complete

**Deliverables:**

```rust
// layered-contracts/src/scope_operator.rs

use crate::{DocSpan, Scored};

/// Dimension of a scope-bearing operator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeDimension {
    Negation,
    Quantifier,
    Precedence,
    Deictic,
    Other(&'static str),
}

/// Domain with N-best ambiguity support
#[derive(Debug, Clone)]
pub struct ScopeDomain {
    /// Candidates sorted by descending score (best first)
    pub candidates: Vec<Scored<DocSpan>>,
}

impl ScopeDomain {
    /// Unambiguous domain from single span
    pub fn from_single(span: DocSpan) -> Self {
        Self {
            candidates: vec![Scored::certain(span)],
        }
    }

    /// Best-scoring domain span (panics if empty)
    pub fn primary(&self) -> &DocSpan {
        &self.candidates[0].value
    }

    /// Is this domain ambiguous?
    pub fn is_ambiguous(&self) -> bool {
        self.candidates.len() > 1
    }
}

/// A scope-bearing operator with trigger and domain.
#[derive(Debug, Clone)]
pub struct ScopeOperator<O> {
    /// High-level dimension (Negation, Quantifier, etc.)
    pub dimension: ScopeDimension,
    /// The trigger span ("not", "each", "notwithstanding")
    pub trigger: DocSpan,
    /// Domain this operator scopes over
    pub domain: ScopeDomain,
    /// Operator-specific payload
    pub payload: O,
}

impl<O> ScopeOperator<O> {
    pub fn new(
        dimension: ScopeDimension,
        trigger: DocSpan,
        domain: ScopeDomain,
        payload: O,
    ) -> Self {
        Self { dimension, trigger, domain, payload }
    }
}
```

**Payload types (sketched, to be refined per milestone):**

```rust
// M7: Negation
pub struct NegationOp {
    pub marker: String,
    pub kind: NegationKind,
}

// M7: Quantifier
pub struct QuantifierOp {
    pub marker: String,
    pub kind: QuantifierKind,
}

// M4: Precedence
pub struct PrecedenceOp {
    pub connective: String,
    pub overrides_domain: bool,
    pub referenced_sections: Vec<String>,
}

// M5: Deictic
pub struct DeicticFrame {
    pub speaker: Option<String>,
    pub time_anchor: Option<String>,
}
```

**Verification:**
- [ ] ScopeOperator<O> compiles with various O types
- [ ] ScopeDomain handles single and N-best cases
- [ ] ScopeDimension distinguishes operator families
- [ ] 5 unit tests for core mechanics

---

### Gate 3: ScopeIndex Query Helper
**Status:** ✅ Complete

**Deliverables:**

```rust
// layered-contracts/src/scope_index.rs

use crate::{DocSpan, DocPosition, ScopeOperator, ScopeDimension};

/// Index for efficient "what scopes cover this position/span?" queries.
#[derive(Debug)]
pub struct ScopeIndex<'a, O> {
    scopes: &'a [ScopeOperator<O>],
}

impl<'a, O> ScopeIndex<'a, O> {
    pub fn new(scopes: &'a [ScopeOperator<O>]) -> Self {
        Self { scopes }
    }

    /// All operators whose primary domain covers the given position.
    pub fn covering_position(&self, pos: &DocPosition) -> impl Iterator<Item = &'a ScopeOperator<O>> {
        self.scopes.iter().filter(move |op| op.domain.primary().contains(pos))
    }

    /// All operators whose primary domain overlaps the given span.
    pub fn covering_span(&self, span: &DocSpan) -> impl Iterator<Item = &'a ScopeOperator<O>> {
        self.scopes.iter().filter(move |op| op.domain.primary().overlaps(span))
    }

    /// Filtered by dimension.
    pub fn of_dimension_covering_span(
        &self,
        dim: ScopeDimension,
        span: &DocSpan,
    ) -> impl Iterator<Item = &'a ScopeOperator<O>> {
        self.covering_span(span).filter(move |op| op.dimension == dim)
    }
}
```

**Required DocSpan methods:**
- [ ] `DocSpan::contains(&DocPosition) -> bool`
- [ ] `DocSpan::overlaps(&DocSpan) -> bool`

**Verification:**
- [ ] ScopeIndex queries work correctly
- [ ] Dimension filtering works
- [ ] 5 unit tests for query scenarios

---

### Gate 4: Ambiguity Types
**Status:** ✅ Complete

**Deliverables:**

```rust
// layered-contracts/src/ambiguity.rs

use crate::Scored;

/// Flag indicating ambiguity severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbiguityFlag {
    /// No ambiguity concerns
    None,
    /// Best score is below low_confidence threshold
    LowConfidence,
    /// Multiple alternatives with similar scores
    CompetingAlternatives,
}

/// Configuration for N-best aggregation
#[derive(Debug, Clone)]
pub struct AmbiguityConfig {
    /// Max candidates to keep (including best)
    pub n_best: usize,
    /// Absolute score floor
    pub min_score: f64,
    /// Best < this triggers LowConfidence flag
    pub low_confidence: f64,
    /// Alt >= best - margin triggers CompetingAlternatives
    pub ambiguity_margin: f64,
}

impl Default for AmbiguityConfig {
    fn default() -> Self {
        Self {
            n_best: 4,
            min_score: 0.25,
            low_confidence: 0.6,
            ambiguity_margin: 0.1,
        }
    }
}

/// Wrapper for N-best results with ambiguity metadata
#[derive(Debug, Clone)]
pub struct Ambiguous<T> {
    /// Top-scoring candidate
    pub best: Scored<T>,
    /// Alternatives (sorted descending, excludes best)
    pub alternatives: Vec<Scored<T>>,
    /// Computed ambiguity flag
    pub flag: AmbiguityFlag,
}

impl<T> Ambiguous<T> {
    /// Create from candidates using config
    pub fn from_candidates(
        mut candidates: Vec<Scored<T>>,
        cfg: &AmbiguityConfig,
    ) -> Option<Self> {
        // Prune below min_score
        candidates.retain(|c| c.confidence >= cfg.min_score);
        if candidates.is_empty() {
            return None;
        }

        // Sort by score descending
        candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        // Truncate to n_best
        candidates.truncate(cfg.n_best);

        // Split best from alternatives
        let mut iter = candidates.into_iter();
        let best = iter.next().unwrap();
        let alternatives: Vec<_> = iter.collect();

        // Compute flag
        let flag = Self::compute_flag(&best, &alternatives, cfg);

        Some(Self { best, alternatives, flag })
    }

    fn compute_flag(best: &Scored<T>, alts: &[Scored<T>], cfg: &AmbiguityConfig) -> AmbiguityFlag {
        if best.confidence < cfg.low_confidence {
            return AmbiguityFlag::LowConfidence;
        }

        let has_competing = alts.iter().any(|alt| {
            alt.confidence >= best.confidence - cfg.ambiguity_margin
        });

        if has_competing {
            AmbiguityFlag::CompetingAlternatives
        } else {
            AmbiguityFlag::None
        }
    }
}
```

**Verification:**
- [ ] Ambiguous::from_candidates works correctly
- [ ] AmbiguityFlag computed correctly for edge cases
- [ ] 8 unit tests for aggregation logic

---

### Gate 5: Integration & Exports
**Status:** ✅ Complete

**Deliverables:**
- [ ] Export SpanLink, ScopeOperator, ScopeIndex, Ambiguous from lib.rs
- [ ] Add DocSpan::contains and DocSpan::overlaps methods
- [ ] Documentation with examples
- [ ] Update PHASE4-IMPLEMENTATION-AUDIT.md

**Verification:**
- [ ] All types accessible from crate root
- [ ] `cargo doc` builds without warnings
- [ ] All 25+ tests pass

---

## Design Decisions

### 1. SpanLink is binary (anchor + target)

Higher-arity relations (e.g., conflict with 2 participants) are modeled as multiple SpanLinks from a shared anchor:

```rust
// Conflict between obligation A and obligation B
// Anchor = conflict marker span
conflict_span.assign(SpanLink::new(ConflictRole::Left, obligation_a));
conflict_span.assign(SpanLink::new(ConflictRole::Right, obligation_b));
```

### 2. ScopeOperator is structural only

The operator stores trigger + domain + payload. Semantic interpretation (e.g., flip polarity for negation) happens in downstream logic that queries "what scopes cover this obligation?"

### 3. Ambiguity via stacking + aggregation

Resolvers emit multiple `Scored<T>`. Stacking stores them. `Ambiguous::from_candidates` aggregates per-slot with pruning and flagging.

### 4. SpanRef for line-local, DocSpan for cross-line

SpanLink<R, S> is generic over span type. Most line-level resolvers use SpanRef; document-level analysis uses DocSpan.

---

## Non-Goals for M0

- **No retrofitting M1/M3:** ConflictDetector and TermsOfArt remain as-is
- **No resolver changes yet:** Just foundation types
- **No pipeline integration:** That comes in M2+

---

## Success Criteria

After M0:
- [ ] Other milestones can `use layered_contracts::{SpanLink, ScopeOperator, Ambiguous}`
- [ ] No one-off relation types needed in M2-M8
- [ ] N-best preservation is natural via Ambiguous<T>
- [ ] ScopeIndex answers "what covers this span?" efficiently

---

## Appendix: Type Summary

| Type | Purpose | Used By |
|------|---------|---------|
| `SpanLink<R, S>` | Binary span relation | M2, M4, M5, M6, M8 |
| `ScopeOperator<O>` | Trigger + domain | M4, M5, M7 |
| `ScopeDomain` | N-best domain spans | M4, M5, M7 |
| `ScopeIndex` | Query helper | M4, M7, M8 |
| `Ambiguous<T>` | N-best aggregation | M6, M7 |
| `AmbiguityConfig` | Pruning thresholds | All |
| `AmbiguityFlag` | Review surfacing | All |

---

## Learnings & Deviations

These implementation learnings capture where the final design diverged from the plan, with rationale:

### 1. ScopeDimension::Other uses String instead of &'static str

**Plan:** `Other(&'static str)` for custom scope dimensions
**Implemented:** `Other(String)`

**Rationale:** Static lifetime constraints prevent runtime flexibility. Contract analysis encounters operator types unknown at compile time (e.g., "notwithstanding" clauses discovered in custom contracts). Using `String` allows resolvers to classify operators dynamically without requiring source code changes.

**Trade-off:** Small heap allocation cost vs. extensibility for domain-specific operators.

### 2. ScopeDomain::primary() returns Option<&DocSpan> instead of panicking

**Plan:** `pub fn primary(&self) -> &DocSpan` that panics on empty domains
**Implemented:** `pub fn primary(&self) -> Option<&DocSpan>`

**Rationale:** Defensive API design that allows callers to handle edge cases gracefully. While empty domains represent programming errors, returning `Option` enables explicit error handling via `?` operator in Effect code paths. Documents that empty domains are contract violations but doesn't enforce at runtime.

**Pattern:** Follows layered-nlp's philosophy of making invariant violations queryable rather than fatal.

### 3. Serde derives are unconditional, not feature-gated

**Plan:** `#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]`
**Implemented:** `#[derive(serde::Serialize, serde::Deserialize)]`

**Rationale:** Serde is a required dependency for layered-contracts (document snapshots, WASM bindings). Feature flags add complexity with no benefit when the feature is always enabled. Simpler code paths, no conditional compilation.

**Benefit:** Removes cognitive overhead from reading code—every type is serializable by default.

### 4. AmbiguityConfig validation uses debug_assert

**Plan:** Runtime panic if `n_best == 0`
**Implemented:** `debug_assert!(n_best > 0)` in construction path

**Rationale:** Catches misuse during development/testing while avoiding runtime overhead in release builds. Invalid configs are logic bugs, not user input errors. Debug builds catch the issue during integration testing; release builds optimize away the check.

**Pattern:** Aligns with Rust's debug_assert philosophy for internal invariants.
