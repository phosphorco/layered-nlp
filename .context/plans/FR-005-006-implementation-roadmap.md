# FR-005/FR-006 Phase 4 Implementation Roadmap

This document provides a unified view of the Phase 4 implementation progress for:
- **FR-005**: Syntactic Structure Enhancement
- **FR-006**: Semantic Analysis & Conflicts

See also:
- [FR-005-syntactic-enhancement.md](FR-005-syntactic-enhancement.md) - Detailed syntactic milestones
- [FR-006-semantic-analysis.md](FR-006-semantic-analysis.md) - Detailed semantic milestones
- [PHASE4-IMPLEMENTATION-AUDIT.md](PHASE4-IMPLEMENTATION-AUDIT.md) - Implementation status tracking

---

## Milestone Overview

| Milestone | FR | Title | Effort | Status | Notes |
|-----------|-----|-------|--------|--------|-------|
| **M1** | FR-006 | [Baseline ConflictDetector](M1-baseline-conflict-detector.md) | S/M | 📋 Planned | Prior work lost; ready to reimplement |
| M2 | FR-005 | ClauseBoundary + Coordination upgrade | M | 📋 Planned | |
| M3 | FR-005 | TermsOfArtResolver | S | 📋 Planned | Core types sketched |
| M4 | FR-006 | Precedence resolution + richer conflicts | M | 📋 Planned | |
| M5 | FR-006 | Metalinguistic references + deictic integration | M | 📋 Planned | |
| M6 | FR-005 | PP & relative clause attachment | M/L | 📋 Planned | |
| M7 | FR-005 | Negation & quantifier scope | M | 📋 Planned | |
| M8 | FR-006 | Semantic roles + obligation equivalence | L | 📋 Planned | |

---

## Recommended Implementation Order

### Immediate Value (Start Here)

1. **M1: Baseline ConflictDetector** (FR-006)
   - Immediate value: detect contradictory provisions
   - Uses existing FR-001/FR-004 infrastructure
   - Plan complete, ready for implementation

2. **M3: TermsOfArtResolver** (FR-005)
   - Cheap accuracy boost
   - Prevents false positives from legal terms of art
   - Small scope, quick win

### Unlocks Further Work

3. **M2: ClauseBoundaryResolver** (FR-005)
   - Unblocks scope resolution for M6/M7
   - Improves clause-level analysis

4. **M4: Precedence Resolution** (FR-006)
   - Builds on M1 conflicts
   - Adds precedence clause awareness

### Advanced (Later)

5. **M5**: Metalinguistic + deictic (FR-006)
6. **M6**: PP/relative attachment (FR-005)
7. **M7**: Negation/quantifier scope (FR-005)
8. **M8**: Semantic roles + equivalence (FR-006)

---

## Current Focus

**M1 (Baseline ConflictDetector)** is the current priority:
- Plan: [M1-baseline-conflict-detector.md](M1-baseline-conflict-detector.md)
- Status: Plan complete, implementation not started
- Prior attempt: Gates 0-3 complete, Gate 4 failing (code lost)
- Learnings documented in plan Appendix C

---

## Dependencies

```
M1 (ConflictDetector) ─────────────────────────────────┐
                                                        │
M3 (TermsOfArt) ───────────────────────────────────────┤
                                                        │
M2 (ClauseBoundary) ─────┬─────────────────────────────┤
                         │                              │
                         ├──► M6 (PP/relative)          │
                         │                              │
                         └──► M7 (Negation/scope)       │
                                                        │
M1 ──► M4 (Precedence) ─────────────────────────────────┤
                                                        │
M5 (Metalinguistic) ────────────────────────────────────┤
                                                        │
M1 + M4 + M5 ──► M8 (Semantic roles) ──────────────────┘
```

---

## Integration Points

### With Existing Infrastructure

| Component | Used By | Description |
|-----------|---------|-------------|
| `ObligationPhrase` (FR-001) | M1, M4, M8 | Source of obligations to analyze |
| `DocumentStructure` (FR-004) | M2, M4 | Section/clause boundaries |
| `PrecedenceClause` (FR-004) | M4 | Precedence rule detection |
| `TemporalExpression` | M1, M4 | Timing conflict detection |
| `Pipeline` (FR-008) | All | Resolver orchestration |

### New Components Per Milestone

| Milestone | New Components |
|-----------|----------------|
| M1 | `ConflictDetector`, `Conflict`, `ConflictType`, `ObligationNormalizer`, `TopicClassifier` |
| M2 | `ClauseBoundaryResolver`, enhanced `CoordinationResolver` |
| M3 | `TermsOfArtResolver`, `LegalTerm` |
| M4 | `PrecedenceResolver`, `ConflictResolution` |
| M5 | `MetalinguisticResolver`, deictic context integration |
| M6 | PP attachment heuristics, relative clause parser |
| M7 | `NegationScope`, `QuantifierScope` |
| M8 | `SemanticRole`, `ObligationEquivalence` |
