# Versioned Diff Architecture (Semantic Graph + Human-Verified Clarifications)

This document formalizes a long-horizon design for diffing contract versions. It treats each version as a semantic graph with explicit provenance and human-verified clarifications that can be replayed deterministically.

## Goals
- Stable semantic deltas across versions, resilient to paraphrase and reformatting.
- First-class human clarification inputs that improve alignment and meaning extraction.
- Deterministic re-runs for auditability (same inputs, same outputs).
- Support branching and merging of contract history.
- Incremental recomputation for long revision chains.

## Non-goals
- Replacing the existing section-based diff in the short term.
- Requiring LLMs in the pipeline (optional, gated).

## Key Concepts

**Contract Semantic Graph (CSG)**
A version-independent graph of contract meaning: obligations, permissions, prohibitions, definitions, parties, temporal constraints, references, and provenance (DocSpan links).

**SemanticSnapshot**
The CSG plus metadata (pipeline hash, resolver version map, config hash, source text hash).

**ClarificationManifest**
A structured, version-scoped file that injects HumanVerified semantic overrides. It reuses existing language:
- Scored attributes with `HumanVerified`

**AlignmentHintSet**
Version-pair scoped hints that guide section/structure alignment.

**AlignmentHintStore**
A separate store keyed by `(from_version, to_version)` that holds AlignmentHintSet values, so pair-scoped hints are not duplicated across VersionNodes.

**ChangeSet**
The semantic delta between two snapshots, including risk/impact annotations and provenance.

**VersionNode**
A node in the history DAG with pointers to parents, snapshot, clarifications, and diffs.

## Data Model (logical)

```
VersionNode
  id: VersionId
  parents: [VersionId]
  text_hash: Hash
  snapshot: SemanticSnapshot
  clarifications: ClarificationManifest
  diffs_to_parents: [ChangeSet]

SemanticSnapshot
  csg: ContractSemanticGraph
  pipeline_hash: Hash
  resolver_versions: Map<String, String>
  config_hash: Hash
  created_at: Timestamp

ClarificationManifest
  semantic_overrides: [HumanVerifiedAttribute]

AlignmentHintSet
  from: VersionId
  to: VersionId
  hints: [AlignmentHint]

AlignmentHint
  match: { original_section_id, revised_section_id }
  action: Force | Forbid | AdjustConfidence
  note: String?

HumanVerifiedAttribute
  target: { span_ref or node_id }
  type: String
  value: Any
  source: HumanVerified
  note: String?

ChangeSet
  from: VersionId
  to: VersionId
  changes: [SemanticChange]
  summary: ChangeSummary
```

Note: AlignmentHintSet values live in a pair-scoped store keyed by (from, to). They are not embedded in VersionNode to avoid duplication.

## Pipeline (per version)

1. Parse text into lines and spans.
2. Run resolver stack to build initial semantics.
3. Apply ClarificationManifest (semantic overrides):
   - HumanVerified attributes are injected into the span index (Scored<T> with source=HumanVerified).
4. Build SemanticSnapshot (CSG + metadata).

## Diff Pipeline (between versions)

1. Align structure using DocumentAligner + AlignmentHintSet from the pair-scoped hint store.
2. Align semantic nodes (obligations, definitions, conditions, temporal) using CSG matching.
3. Produce ChangeSet with provenance to both versions (DocSpan links).
4. Surface change impacts and risk.

## Clarifications are not a parallel concept
Clarifications should use the same language already in the system:
- **AlignmentHint** for alignment control.
- **Scored<T>** with **HumanVerified** for semantic overrides.

This keeps the architecture coherent and avoids competing models.

## Version History as a DAG
Contracts evolve via branches and merges. The history should support:
- Diff against parent.
- Diff against baseline (e.g., original contract).
- Diff between branches.

Each VersionNode stores its SemanticSnapshot and clarifications. Diffs are derived artifacts, not the source of truth.

## Determinism and Reproducibility
Outputs must be reproducible given:
- raw text
- ClarificationManifest
- resolver versions
- pipeline config

Store pipeline hashes and resolver version maps in SemanticSnapshot to guarantee re-run parity.

## Incremental Recompute
Cache snapshots and only recompute when inputs change:
- text changes -> rebuild snapshot
- clarifications change -> rebuild snapshot
- resolver version/config changes -> rebuild snapshot

Diffs can be cached for version pairs and recomputed on-demand.

## Merge Rules for Clarifications
When merging branches:
- Alignment hints are pair-scoped; keep them in the pair-scoped hint store and do not auto-merge unless both sides match the same pair.
- HumanVerified attributes can conflict; require explicit resolution or prefer latest timestamp with audit note.

## Compatibility with Existing Components
- DocumentStructureBuilder + SectionHeaderResolver: structure extraction.
- DocumentAligner: alignment (use hints).
- SemanticDiffEngine: change detection (extended to use real section spans and CSG nodes).
- TokenAligner: token-level diffs for UI.
- Scored<T> + HumanVerified: overrides.

This design does not discard existing components; it formalizes how they compose over time.

## UX Surfaces
- Change review view: semantic changes with provenance, click-through to source spans.
- Clarification editor: write alignment hints or semantic overrides, re-run diff.
- Audit log: show which changes were produced by human clarifications vs automated inference.

## Resolver Design Exercise
- See `docs/resolver-design-exercise.md` for a recipe + Rust-like pseudocode on designing resolvers that fit this architecture.

## Success Criteria
- A clarified diff re-runs identically on demand.
- Semantic changes persist even across large rewrites.
- Alignment corrections improve downstream diffs without manual rework each time.
- Branch merges preserve explicit human decisions.

## Open Questions
- Should clarified attributes be stored by span ref or by canonical node id?
- How do we represent CSG node identity across versions (hash of semantics vs explicit ids)?
- How do we integrate LLM canonicalization while keeping determinism?
