# M3: TermsOfArtResolver

**FR:** FR-005 – Syntactic Structure Enhancement  
**Status:** ✅ Complete  
**Effort:** S (1-2 hours)  
**Last Updated:** 2024-12-30

> **Summary:** M3 TermsOfArtResolver detects multi-word legal expressions that should
> be treated as atomic units rather than parsed compositionally. Prevents downstream
> resolvers from misinterpreting terms like "indemnify and hold harmless" as two
> separate obligations.

---

## Overview

Legal contracts contain many multi-word expressions with non-compositional meaning:

- **Legal doctrines**: "force majeure", "res judicata", "prima facie"
- **Obligation phrases**: "indemnify and hold harmless", "represent and warrant"
- **Payment terms**: "net 30", "cash on delivery"
- **Contract mechanisms**: "material adverse change", "right of first refusal"
- **Allocation terms**: "pro rata", "pari passu"
- **Interpretive phrases**: "time is of the essence", "without prejudice"

The TermsOfArtResolver marks these as atomic spans to prevent splitting in obligation
extraction and other downstream processing.

---

## Gates

### Gate 1: Types and Dictionary Skeleton
**Status:** ✅ Complete

**Deliverables:**
- [x] `TermOfArt` struct with `canonical: String`, `category: TermOfArtCategory`
- [x] `TermOfArtCategory` enum with 6 variants
- [x] `TermsOfArtResolver` struct with dictionary `HashMap<String, Vec<(Vec<String>, TermOfArt)>>`
- [x] `add_defaults()` method with ~45 predefined terms
- [x] `new()`, `empty()`, `add()`, `len()`, `is_empty()` methods

**Verification:**
- [x] 6 unit tests pass for types and dictionary structure

---

### Gate 2: Core Matching Logic
**Status:** ✅ Complete

**Deliverables:**
- [x] `try_match_phrase(&LLSelection, &[String]) -> Option<LLSelection>` method
- [x] `Resolver` trait implementation with `type Attr = TermOfArt`
- [x] Case-insensitive matching (lowercased comparison)
- [x] Whitespace handling between words

**Verification:**
- [x] 15 integration tests covering all categories and edge cases

---

### Gate 3: Pipeline Integration
**Status:** ✅ Complete

**Deliverables:**
- [x] Added `TermsOfArt` to `ResolverType` enum in `pipeline/mod.rs`
- [x] Added to `Pipeline::standard()` after ContractKeyword, before DefinedTerm
- [x] Exported `TermOfArt`, `TermOfArtCategory`, `TermsOfArtResolver` from `lib.rs`

**Rationale:** Run after ContractKeyword (modal detection) but before Obligation 
extraction so that "indemnify and hold harmless" is recognized as a single term.

**Verification:**
- [x] 2 pipeline integration tests pass
- [x] All 345 crate tests pass

---

### Gate 4: Edge Cases and Category Coverage
**Status:** ✅ Complete

**Deliverables:**
- [x] Comprehensive tests for all 45 default terms
- [x] Tests for each category (6 category coverage tests)
- [x] Edge case tests: start/end of line, mixed case, punctuation, non-matches

**Verification:**
- [x] 36 total tests pass

---

### Gate 5: Documentation and Audit Updates
**Status:** ✅ Complete

**Deliverables:**
- [x] Module documentation with examples
- [x] `cargo doc` builds without warnings
- [x] PHASE4-IMPLEMENTATION-AUDIT.md updated
- [x] This plan document created

---

## API Reference

### Types

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermOfArt {
    pub canonical: String,
    pub category: TermOfArtCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TermOfArtCategory {
    LegalDoctrine,     // "force majeure", "res judicata"
    ObligationPhrase,  // "indemnify and hold harmless"
    PaymentTerm,       // "net 30", "cash on delivery"
    ContractMechanism, // "material adverse change"
    AllocationTerm,    // "pro rata", "pari passu"
    InterpretivePhrase, // "time is of the essence"
}
```

### Usage

```rust
use layered_contracts::{TermsOfArtResolver, TermOfArt};
use layered_nlp::{create_line_from_string, x};

let line = create_line_from_string("Force majeure shall excuse performance.")
    .run(&TermsOfArtResolver::new());

for f in line.find(&x::attr::<TermOfArt>()) {
    println!("{}: {:?}", f.attr().canonical, f.attr().category);
}
// Output: force majeure: LegalDoctrine
```

### Custom Terms

```rust
let mut resolver = TermsOfArtResolver::empty();
resolver.add("custom legal phrase", TermOfArtCategory::LegalDoctrine);
```

---

## Design Decisions

1. **Dictionary keyed by first word**: O(1) lookup for first word, then linear scan
   of candidates. Efficient for typical term counts (~50).

2. **Case-insensitive matching**: All dictionary words are lowercased; input text
   is lowercased during comparison but canonical form preserves original casing.

3. **No Scored<T> wrapper**: Terms of art are deterministic dictionary matches,
   not probabilistic. Confidence is implicit 1.0.

4. **Pipeline position**: After ContractKeyword, before DefinedTerm. This ensures
   "indemnify and hold harmless" is marked before obligation extraction runs.

---

## Future Enhancements

- [ ] Add variant detection ("force majeur" without 'e')
- [ ] Add abbreviation support ("MAC" for "material adverse change")
- [ ] Consider loading terms from external dictionary file
- [ ] Add span association to link back to matched tokens
