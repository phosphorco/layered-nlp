# FR-012: Token-Level Comparison Infrastructure

**Interesting artifacts and learnings must be written back to this document.**

## Overview

Build foundational infrastructure for **token-level comparability across analyzed documents**. This goes beyond visual diff rendering - it establishes the primitives needed for:

1. **Version Comparison** - Diff two versions of the same document (current use case)
2. **Cross-Document Comparison** - Compare similar sections across different contracts
3. **Pattern Extraction** - Identify common/standard language vs deviations
4. **Corpus Analysis** - Aggregate analysis across multiple documents

The immediate deliverable is token-level diff for the contract viewer, but the architecture must support these broader use cases.

### Architectural Principle

**Tokens are the fundamental unit of comparison.** All higher-level comparisons (sections, obligations, terms) should be derivable from token-level alignment. This creates a consistent, composable comparison model.

### Current State
- **Rust**: Tokenizes text via `LLLine` with position info (`pos_starts_at`, `pos_ends_at`)
- **Rust**: `SemanticDiffEngine` detects meaning changes (shall→may, term redefinitions)
- **WASM**: Returns raw text strings (`original_texts`, `revised_texts`)
- **JavaScript**: Re-tokenizes and diffs using `diff_match_patch` library

### Target State
- **Rust**: Token-level comparison as a first-class primitive
- **Rust**: `TokenAligner` that can compare token sequences from any source
- **WASM**: Returns `token_alignment` data that enables any comparison visualization
- **JavaScript**: Renders comparisons using Rust-provided alignment (no re-tokenization)

### Key Files
- `layered-nlp/src/ll_line.rs` - Token infrastructure
- `layered-contracts/src/semantic_diff.rs` - Existing semantic diff
- `layered-contracts/src/document_aligner.rs` - Section alignment
- `layered-nlp-demo-wasm/src/lib.rs` - WASM API
- `web/contract-diff.html` - Frontend rendering

---

## Gate 0: Research & Design

### Objectives
- Understand existing tokenization infrastructure deeply
- Evaluate diff algorithm options
- Design data structures that integrate with existing architecture
- Document design decisions

### Scope
- Read-only exploration of codebase
- Algorithm research and selection
- Type design (no implementation)

### Dependencies
- None

### Tasks

- [ ] **0.1** Document `LLToken` structure and available metadata
  - *Acceptance*: Written summary of token fields, `TextTag` variants, position semantics

- [ ] **0.2** Document how `DocumentAligner` computes similarity
  - *Acceptance*: Flow diagram of alignment process, similarity metrics used

- [ ] **0.3** Research diff algorithms suitable for token sequences
  - *Acceptance*: Comparison table of Myers vs Patience vs Hunt-McIlroy with pros/cons

- [ ] **0.4** Design `TokenDiff` and related types
  - *Acceptance*: Type definitions in this document, reviewed against existing patterns

- [ ] **0.5** Decide on whitespace handling strategy
  - *Acceptance*: Documented decision on how whitespace tokens affect diff output

### Verification
- Design review: Types must follow existing patterns (`Scored<T>`, span conventions)
- Algorithm choice must be justified with complexity analysis
- No code changes in this gate

### Artifacts

#### Token Structure (from exploration)
```rust
pub struct LLToken {
    pub token_idx: usize,
    pub pos_starts_at: usize,  // Character position in source
    pub pos_ends_at: usize,
    pub token: LToken,
}

pub enum LToken {
    Text(String, TextTag),
    Value,
}

pub enum TextTag {
    NATN,   // Numbers
    PUNC,   // Punctuation
    SYMB,   // Symbols
    SPACE,  // Whitespace
    WORD,   // Words
}
```

#### Proposed Types (to be refined)

**Design Principle**: These are comparison primitives, not rendering instructions. The alignment data should be queryable for various purposes (visual diff, similarity scoring, pattern extraction).

```rust
/// Token-level alignment between two token sequences.
/// This is a queryable data structure, not just rendering output.
pub struct TokenAlignment {
    /// The aligned token pairs with their relationship
    pub pairs: Vec<AlignedTokenPair>,
    /// Summary statistics derived from the alignment
    pub stats: AlignmentStats,
    /// Similarity score (0.0 = completely different, 1.0 = identical)
    pub similarity: f64,
}

impl TokenAlignment {
    /// Query: Get all tokens that were added
    pub fn added(&self) -> impl Iterator<Item = &AlignedTokenPair>;

    /// Query: Get all tokens that were removed
    pub fn removed(&self) -> impl Iterator<Item = &AlignedTokenPair>;

    /// Query: Get all tokens that changed (added, removed, or modified)
    pub fn changes(&self) -> impl Iterator<Item = &AlignedTokenPair>;

    /// Query: Get unchanged tokens (for context)
    pub fn unchanged(&self) -> impl Iterator<Item = &AlignedTokenPair>;

    /// Query: Get tokens matching a predicate
    pub fn filter<F>(&self, predicate: F) -> impl Iterator<Item = &AlignedTokenPair>
    where F: Fn(&AlignedTokenPair) -> bool;

    /// Compute: Similarity between the two sequences
    pub fn compute_similarity(&self) -> f64;
}

/// A single aligned pair of tokens (or one-sided for add/remove)
pub struct AlignedTokenPair {
    /// Token from the "left" (original/source) sequence
    pub left: Option<TokenRef>,
    /// Token from the "right" (revised/target) sequence
    pub right: Option<TokenRef>,
    /// The relationship between left and right
    pub relation: TokenRelation,
}

/// Reference to a token with its metadata
pub struct TokenRef {
    pub text: String,
    pub position: TokenPosition,
    pub tag: TextTag,
    /// Index in the source sequence (for back-reference)
    pub index: usize,
}

/// Position in source text (character-level)
pub struct TokenPosition {
    pub start: usize,
    pub end: usize,
    /// Optional: line number if multi-line
    pub line: Option<usize>,
}

/// How two tokens relate to each other
pub enum TokenRelation {
    /// Identical token (same text)
    Identical,
    /// Token only exists on the left (was removed)
    LeftOnly,
    /// Token only exists on the right (was added)
    RightOnly,
    /// Tokens are similar but not identical (for fuzzy matching)
    Similar { similarity: f64 },
    /// Whitespace-equivalent (differ only in whitespace handling)
    WhitespaceEquivalent,
}

pub struct AlignmentStats {
    pub total_left: usize,
    pub total_right: usize,
    pub identical: usize,
    pub added: usize,
    pub removed: usize,
    pub similar: usize,
}
```

**Usage Examples**:
```rust
// Version comparison (current use case)
let alignment = TokenAligner::align(&original_tokens, &revised_tokens);
let changes = alignment.changes().collect::<Vec<_>>();

// Cross-document comparison
let alignment = TokenAligner::align(&contract_a_section, &contract_b_section);
if alignment.similarity > 0.9 {
    // Very similar clauses
}

// Pattern extraction
let alignments: Vec<TokenAlignment> = sections.iter()
    .map(|s| TokenAligner::align(&template, s))
    .collect();
let deviations: Vec<_> = alignments.iter()
    .flat_map(|a| a.changes())
    .collect();
```

---

## Gate 1: Core Token Diff Module

### Objectives
- Implement core token diff types in Rust
- Create `TokenDiffEngine` with basic diff computation
- Establish test patterns for token diff

### Scope
- New module: `layered-contracts/src/token_diff.rs`
- Unit tests for diff computation
- No WASM integration yet

### Dependencies
- Gate 0 complete (design finalized)

### Tasks

- [ ] **1.1** Create `token_diff.rs` module with type definitions
  - *Acceptance*: Module compiles, types exported from `lib.rs`

- [ ] **1.2** Implement token extraction from `LLLine`
  - *Acceptance*: Function converts `LLLine` → `Vec<DiffToken>` preserving positions

- [ ] **1.3** Implement basic diff algorithm on token sequences
  - *Acceptance*: Given two token sequences, produces correct diff spans

- [ ] **1.4** Implement whitespace normalization option
  - *Acceptance*: Config flag to collapse/preserve whitespace tokens in diff

- [ ] **1.5** Implement diff stats computation
  - *Acceptance*: Accurate counts of unchanged/added/removed tokens

### Verification

**Test file**: `layered-contracts/src/tests/token_diff.rs`

**Test scenarios**:
1. Identical texts → all spans Unchanged, stats show 0 changes
2. Single word addition → correct Added span with positions
3. Single word removal → correct Removed span with positions
4. Word replacement → one Removed + one Added span (or Modified if fuzzy)
5. Whitespace-only changes with normalization → no visible diff
6. Whitespace-only changes without normalization → spans reflect whitespace
7. Multi-word insertion in middle of sentence → correct span sequence
8. Punctuation changes → correctly tagged with TextTag::PUNC

**Coverage requirement**: >90% line coverage for `token_diff.rs`

**Pass criteria**: All tests pass, `cargo test -p layered-contracts token_diff`

---

## Gate 2: Integration with Aligned Pairs

### Objectives
- Integrate token diff into section alignment flow
- Compute token diffs for Modified aligned pairs
- Ensure performance is acceptable

### Scope
- Modify `FrontendAlignedPair` to include token diff
- Call token diff during alignment processing
- Performance benchmarking

### Dependencies
- Gate 1 complete

### Tasks

- [ ] **2.1** Add `token_diffs` field to internal alignment structures
  - *Acceptance*: Field added, defaults to None for non-Modified pairs

- [ ] **2.2** Compute token diff for Modified pairs during alignment
  - *Acceptance*: Token diffs populated for all Modified pairs

- [ ] **2.3** Handle multi-section pairs (Split/Merged alignments)
  - *Acceptance*: Token diff works when pair has multiple sections on one/both sides

- [ ] **2.4** Add configuration for token diff (enable/disable, options)
  - *Acceptance*: Config struct allows controlling token diff behavior

- [ ] **2.5** Benchmark token diff performance
  - *Acceptance*: Document time cost per section, ensure <10ms for typical sections

### Verification

**Test file**: `layered-contracts/src/tests/token_diff_integration.rs`

**Test scenarios**:
1. Modified pair → has token_diffs populated
2. ExactMatch pair → token_diffs is None or all Unchanged
3. Inserted pair → token_diffs shows all Added
4. Deleted pair → token_diffs shows all Removed
5. Split pair (1 original → 2 revised) → token diffs for each revised section
6. Large section (1000+ tokens) → completes within performance budget

**Performance test**: `cargo test -p layered-contracts --release token_diff_benchmark`
- Must complete 100 section diffs in <1 second

**Pass criteria**: All tests pass, performance within budget

---

## Gate 3: WASM Serialization

### Objectives
- Expose token diff data through WASM API
- Design efficient serialization for browser consumption
- Update `FrontendAlignedPair` structure

### Scope
- Modify `layered-nlp-demo-wasm/src/lib.rs`
- Add serializable token diff types
- Update JavaScript type definitions

### Dependencies
- Gate 2 complete

### Tasks

- [ ] **3.1** Create WASM-compatible token diff types
  - *Acceptance*: Types implement `Serialize`, minimize payload size

- [ ] **3.2** Add `token_diffs` to `FrontendAlignedPair`
  - *Acceptance*: Field populated during pair construction

- [ ] **3.3** Implement efficient serialization strategy
  - *Acceptance*: Document size impact, consider compression if needed

- [ ] **3.4** Add TypeScript type definitions
  - *Acceptance*: Types documented in WASM module comments or separate .d.ts

- [ ] **3.5** Test round-trip serialization
  - *Acceptance*: Rust → JSON → JS parsing produces correct data

### Verification

**Test file**: `layered-nlp-demo-wasm/src/tests/token_diff_serialization.rs`

**Test scenarios**:
1. Empty token diff → serializes to empty array
2. Simple diff → JSON structure matches expected format
3. Large diff → serialization completes, size is reasonable
4. Position data preserved → original_pos/revised_pos correct in JSON
5. TextTag preserved → tag field present in serialized spans

**Integration test**: Browser console validation
- Load sample NDA, verify `result.aligned_pairs[*].token_diffs` structure

**Pass criteria**: Serialization tests pass, JSON structure verified in browser

---

## Gate 4: Frontend Rendering

### Objectives
- Replace JavaScript diff logic with Rust-provided spans
- Render token diffs using position data
- Remove `diff_match_patch` dependency

### Scope
- Modify `web/contract-diff.html`
- Update `renderWordDiff` function
- Remove unused JS diff code

### Dependencies
- Gate 3 complete

### Tasks

- [ ] **4.1** Update `renderWordDiff` to use token_diffs from WASM
  - *Acceptance*: Function renders spans based on Rust data, not JS diff

- [ ] **4.2** Handle fallback when token_diffs is null/empty
  - *Acceptance*: Graceful degradation for edge cases

- [ ] **4.3** Render position-accurate highlights
  - *Acceptance*: Highlights align with actual character positions

- [ ] **4.4** Remove `diff_match_patch` library and related code
  - *Acceptance*: No references to `dmp`, `computeTokenDiff`, etc.

- [ ] **4.5** Visual regression testing
  - *Acceptance*: Screenshot comparison shows correct diff rendering

### Verification

**Test approach**: Manual + automated visual testing

**Test scenarios**:
1. Sample NDA → "technical" shows as only addition in Section 1.1
2. "shall" → "may" → correct strikethrough and highlight
3. "two (2)" → "three (3)" → numbers and words both highlighted
4. Added section → all text shows as added (green)
5. Deleted section → all text shows as removed (strikethrough)
6. Long section with multiple changes → all changes visible, scrollable

**Visual regression**: Compare screenshots before/after migration
- Diff rendering should be identical or improved
- No regressions in existing functionality

**Pass criteria**: All manual tests pass, no visual regressions

---

## Gate 5: Polish & Documentation

### Objectives
- Optimize performance if needed
- Add comprehensive documentation
- Update architecture docs

### Scope
- Performance tuning
- Documentation updates
- Code cleanup

### Dependencies
- Gate 4 complete

### Tasks

- [ ] **5.1** Profile and optimize if token diff is slow
  - *Acceptance*: Document any optimizations made

- [ ] **5.2** Add rustdoc comments to public types
  - *Acceptance*: `cargo doc` generates useful documentation

- [ ] **5.3** Update CLAUDE.md with token diff information
  - *Acceptance*: Usage examples for token diff API

- [ ] **5.4** Update .context/ARCHITECTURE.md
  - *Acceptance*: Token diff fits into documented architecture

- [ ] **5.5** Write learnings back to this document
  - *Acceptance*: Artifacts section populated with insights

### Verification

**Documentation review**:
- Rustdoc generates without warnings
- Examples in docs compile and run
- Architecture diagram updated

**Pass criteria**: Documentation complete, no outstanding TODOs

---

## Appendix A: Algorithm Comparison

| Algorithm | Time Complexity | Space | Pros | Cons |
|-----------|----------------|-------|------|------|
| Myers | O(ND) | O(N) | Minimal edit distance | Can be slow for very different texts |
| Patience | O(N log N) | O(N) | Better for code-like text | More complex implementation |
| Hunt-McIlroy | O(N log N) | O(N) | Fast for similar texts | Less optimal for dissimilar |

**Recommendation**: Start with Myers (simpler), optimize to Patience if needed.

## Appendix B: JSON Schema for Token Diff

```json
{
  "token_diffs": [
    {
      "text": "technical",
      "status": "Added",
      "original_pos": null,
      "revised_pos": [45, 54],
      "tag": "WORD"
    },
    {
      "text": " ",
      "status": "Unchanged",
      "original_pos": [35, 36],
      "revised_pos": [54, 55],
      "tag": "SPACE"
    }
  ]
}
```

## Appendix C: Learnings & Artifacts

### Gate 0 Learnings
- `LLToken` fields are `pub(crate)`, required adding public accessor methods to `layered-nlp/src/ll_line.rs`
- DocumentAligner uses Kuhn-Munkres (Hungarian) for section-level matching - overkill for token sequences
- Existing `pathfinding` crate doesn't have Myers diff, custom implementation needed

### Gate 1 Learnings
- **Strategic shortcut**: Used LCS (Longest Common Subsequence) algorithm instead of Myers diff
  - LCS is O(N*M) vs Myers O(ND), but simpler and more robust
  - Myers backtracking is error-prone (integer underflow issues encountered)
  - For typical contract sections (<500 tokens), LCS performance is acceptable
- **Design decision**: Kept function name `myers_diff` even though using LCS (could rename in future)
- **API exposed**: Module is `pub mod token_diff` for full access to types and `TokenAligner`
- **Whitespace handling**: Three modes work well - Normalize (default), Preserve, Ignore
- **Position data**: Added `pos_starts_at()`, `pos_ends_at()`, `token_idx()` accessors to `LLToken`
- **Type system**: `TokenTag` is a serializable version of `TextTag` for WASM export

### Gate 2 Learnings
-

### Gate 3 Learnings
-

### Gate 4 Learnings
-

### Gate 5 Learnings
-
