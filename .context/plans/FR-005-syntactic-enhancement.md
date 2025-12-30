# FR-005: Syntactic Structure Enhancement

**Status: Phase 4 — Milestones M2, M3, M6, M7**

## Summary

Introduce lightweight syntactic structure awareness to handle attachment ambiguity, garden path sentences, long-distance dependencies, and terms of art.

## Phase 4 Integration

FR-005 is implemented across 4 milestones, interleaved with FR-006. See [FR-007](FR-007-phase4-plan.md) for the full roadmap.

| Milestone | Component | Status |
|-----------|-----------|--------|
| M2 | ClauseBoundaryResolver + CoordinationResolver upgrade | 🔲 Todo |
| M3 | TermsOfArtResolver | 🔲 Todo |
| M6 | PP & relative clause attachment | 🔲 Todo |
| M7 | Negation & quantifier scope | 🔲 Todo |

**Recommended start:** M3 (TermsOfArtResolver) is lowest complexity with high value, followed by M2 (clause boundaries) to unblock scope resolution.

---

## Current State

The system uses linear left-to-right matching with no syntactic parsing. Token distance is the primary measure for reference resolution. This misses:
- Correct attachment sites for modifiers
- Re-analysis needed for garden path sentences
- Deeply embedded clause boundaries
- Non-compositional multi-word expressions

## Proposed Improvements

### 1. Clause Boundary Detection

**Problem:** No awareness of clause structure affects attachment and scope.

**Example:**
```
If the Contractor fails to deliver the goods that the Buyer ordered
before the due date, the Buyer may terminate.
```
(Does "before the due date" modify "deliver" or "ordered"?)

**Ask:**
- Integrate with `layered-clauses` for subordinate clause detection
- Mark clause boundaries in token stream
- Use clause depth for attachment scoring
- Prefer attachments within same clause

### 2. Coordination Structure Detection

**Problem:** "X and Y" coordination isn't parsed structurally.

**Example:**
```
old men and women
```
(Are women old? Ambiguous without structure.)

**Ask:**
- Detect coordination patterns (X and Y, X, Y, and Z)
- Determine scope of shared modifiers
- Mark coordinated spans for reference resolution
- Enable FR-002 split antecedent resolution

### 3. Relative Clause Attachment

**Problem:** Relative clauses aren't bracketed or attached to heads.

**Example:**
```
The agreement the Seller prepared contains terms.
```

**Ask:**
- Detect relative clause patterns (NP [that/which/who] VP)
- Detect reduced relatives (NP VP-past-participle)
- Mark relative clause spans
- Attach to correct head noun

### 4. PP Attachment Heuristics

**Problem:** Prepositional phrases attach ambiguously.

**Example:**
```
The Company shall deliver products to the Buyer in January.
```
(Is "in January" when delivery happens, or describing "Buyer in January"?)

**Ask:**
- Apply PP attachment heuristics (verb affinity, noun affinity)
- Use semantic compatibility (dates attach to events, not entities)
- Score multiple interpretations
- Prefer high-attachment for temporal PPs

### 5. Terms of Art / Multi-Word Expressions

**Problem:** Idiomatic legal phrases are parsed compositionally.

**Example:**
```
The Company shall indemnify and hold harmless the Buyer.
```
("indemnify and hold harmless" is one concept, not two obligations)

```
Payment shall be made net 30.
```
("net 30" is a payment term, not a number)

**Ask:**
- Build term-of-art dictionary for legal domain
- Detect multi-word expressions as atomic units
- Mark non-compositional spans
- Prevent splitting in obligation extraction

### 6. Negation Scope

**Problem:** Negation scope isn't computed.

**Example:**
```
The Company shall not disclose information except as required by law.
```

**Ask:**
- Detect negation markers (not, never, no)
- Compute scope of negation
- Distinguish "not [disclose information]" from "[not disclose] information"
- Handle double negatives

### 7. Quantifier Scope

**Problem:** Quantifier-negation interactions aren't resolved.

**Example:**
```
Each party shall not disclose any information.
```
(∀x.¬P(x) vs ¬∀x.P(x) - different meanings)

**Ask:**
- Detect quantifiers (each, every, all, any, no)
- Compute quantifier scope relative to negation
- Generate correct per-party obligations
- Flag scope ambiguity for review

## Success Criteria

- [ ] Clause boundaries detected and used for attachment
- [ ] Coordination structures parsed with modifier scope
- [ ] Relative clauses attached to head nouns
- [ ] PP attachment scored with semantic heuristics
- [ ] Terms of art detected as atomic units
- [ ] Negation scope computed correctly
- [ ] Quantifier scope resolved or flagged

## Edge Cases

This section catalogs syntactic complexity patterns found in legal text, with examples and detection strategies.

### 1. Long-Distance PP Attachment

**Pattern:** Prepositional phrases that attach to heads far from their surface position.

**Examples:**
```
"indemnify for losses arising from breach of contract by the Contractor"
             ╰─────PP1─────╯           ╰───────PP2─────────╯           ╰─PP3─╯
```
- Does "by the Contractor" attach to "breach" (who breached) or "indemnify" (who indemnifies)?
- "arising from breach" could attach to "losses" or "indemnify"

```
"deliver products to the Buyer in January under the terms of the Agreement"
                   ╰────PP1────╯ ╰───PP2───╯ ╰───────────PP3────────────╯
```
- Stacked PPs create exponential attachment ambiguity

**Detection strategy:** Use preposition lists + verb/noun affinity heuristics. Temporal PPs ("in January") prefer verb attachment; entity PPs ("to the Buyer") prefer argument slots.

### 2. Coordination Ambiguity

**Pattern:** Shared modifiers with "and"/"or" conjunctions.

**Examples:**
```
"buy and sell securities and commodities"
 ╰──╯     ╰──╯ ╰─────────────────────╯
```
- Flat parse: [[buy], [sell securities and commodities]]
- Nested parse: [[buy and sell] [securities and commodities]]

```
"old men and women"
 ╰─╯ ╰─╯     ╰───╯
```
- Does "old" modify just "men" or also "women"?

```
"the Seller's obligations and the Buyer's rights under this Agreement"
```
- Does "under this Agreement" scope over both or just "rights"?

**Detection strategy:** Identify coordination markers, track modifier attachment via POS tags. Prefer parallel structure (same POS categories coordinate).

### 3. Terms of Art with Unusual Internal Syntax

**Pattern:** Non-compositional multi-word expressions that should not be parsed.

**Examples:**
| Term | Issue |
|------|-------|
| `force majeure` | French syntax, not "force" modifying "majeure" |
| `time is of the essence` | Idiomatic; "of the essence" is not a standard PP |
| `indemnify and hold harmless` | Single obligation, not two obligations |
| `pro rata` | Latin, no internal decomposition |
| `res judicata` | Legal Latin |
| `pari passu` | Ranking term |
| `net 30` / `net 60` | Payment term idiom |
| `bona fide` | Adjectival idiom |
| `mutatis mutandis` | "with necessary changes" |
| `inter alia` | "among other things" |
| `covenant not to compete` | Single concept |
| `right of first refusal` | Single concept |
| `material adverse change` | MAC clause term |

**Detection strategy:** Dictionary lookup for known MWEs. Mark as atomic spans to prevent splitting in obligation extraction.

### 4. Relative Clause Attachment

**Pattern:** Ambiguous attachment of relative clauses to potential head nouns.

**Examples:**
```
"the party that breached the agreement in 2020"
     ╰─N1─╯                ╰───N2───╯
```
- "that breached..." attaches to "party" (high attachment)
- "in 2020" could attach to "breached" (when) or "agreement" (which agreement)

```
"the agreement the Seller prepared contains terms"
     ╰───N1───╯     ╰─SUBJ─╯               ╰─N2─╯
```
- Reduced relative clause (no "that/which")

```
"the documents relating to the acquisition that were delivered"
     ╰───N1────╯              ╰────N2────╯
```
- Does "that were delivered" attach to "documents" or "acquisition"?

**Detection strategy:** Detect relative pronouns (that/which/who/whom/whose) and participial phrases (relating/arising/concerning). Score attachment by distance, semantic compatibility, and intervening clause boundaries.

### 5. Scope of Negation

**Pattern:** Negation markers with ambiguous scope.

**Examples:**
```
"shall not unreasonably withhold consent"
       ╰─╯ ╰──────────────────────────╯
```
- Negation scopes over "unreasonably withhold" → withholding allowed if reasonable
- NOT: "shall [not unreasonably] [withhold consent]"

```
"shall not disclose information except as required by law"
       ╰─╯                       ╰─────────────────────╯
```
- Exception modifies the negated action

```
"The Company does not warrant and shall not be liable"
              ╰────────╯                    ╰─────╯
```
- Two separate negations, not one scoping over conjunction

**Detection strategy:** Mark negation markers (not/never/no/neither/nor). Compute rightward scope to next clause boundary or coordinating conjunction. Handle adverb interveners.

### 6. Garden Path Sentences

**Pattern:** Sentences where initial parse fails and requires reanalysis.

**Examples:**
```
"The Contractor the Company hired shall deliver goods"
                 ╰──────RC──────╯
```
- Initial parse expects "Contractor" as subject of main verb
- Must reanalyze: "Contractor" is head of reduced relative clause

```
"The goods delivered late shall be rejected"
           ╰──VP-ed──╯
```
- "delivered" could be main verb (active) or modifier (passive/reduced relative)

```
"The horse raced past the barn fell"
           ╰──VP-ed────────╯
```
- Classic garden path: "raced" looks like main verb but is reduced relative

**Detection strategy:** Detect reduced relatives via POS (NP + VP-past-participle). Flag sentences with multiple potential main verbs. Use modal presence as disambiguation anchor.

### 7. Nominalization Chains

**Pattern:** Deep embedding via nominalizations that obscure predicate-argument structure.

**Examples:**
```
"the determination of the calculation of the amount of the damages"
     ╰───N1───────╯      ╰───N2───────╯     ╰──N3──╯     ╰──N4──╯
```
- Four levels of nominalization
- Original predicates: someone determines, calculates, amounts, damages

```
"upon the completion of the delivery of the goods by the Seller"
           ╰───N1────╯      ╰───N2───╯      ╰─N3─╯      ╰──N4──╯
```
- Argument roles obscured: who completes? who delivers?

**Detection strategy:** Detect nominalization suffixes (-tion, -ment, -ance, -ence, -ity). Track "of" chains. Optionally flag for human review when depth > 2.

### 8. Quantifier-Negation Interaction

**Pattern:** Scope ambiguity between quantifiers and negation.

**Examples:**
```
"Each party shall not disclose any information"
 ╰──Q1──╯            ╰─╯       ╰──Q2─╯
```
- Surface: ∀party.¬∃info.disclose(party,info) (no party discloses anything)
- Possible: ¬∀party.∀info.disclose(party,info) (not every party discloses everything)

```
"All documents shall not be destroyed for five years"
 ╰─Q1─╯                   ╰─────temporal scope─────╯
```
- "All" scopes over "documents" or over the entire obligation?

**Detection strategy:** Mark universal quantifiers (each/every/all) and existential (any/some). Determine relative scope with negation based on surface order and legal conventions (surface scope usually intended).

### 9. Ellipsis and Gapping

**Pattern:** Missing constituents recovered from context.

**Examples:**
```
"The Company shall deliver goods and the Buyer [shall] [deliver] payment"
                                              ╰─────elided─────╯
```
- Gapped verb phrase

```
"The Seller may, and the Buyer shall, comply with regulations"
             ╰──╯       ╰─────modal gap──────────╯
```
- Different modals, shared VP

**Detection strategy:** Detect coordinated subject phrases with missing VP. Infer elided material from parallel structure.

### 10. Scope of Modifiers Across Conjunctions

**Pattern:** Adverbial and prepositional modifiers with ambiguous scope.

**Examples:**
```
"The Company shall promptly deliver goods and provide services"
                   ╰───ADV──╯
```
- Does "promptly" scope over both "deliver" and "provide"?

```
"within 30 days of receipt of notice deliver goods and pay invoices"
╰────────────────PP─────────────────╯
```
- Does the time frame apply to both actions?

**Detection strategy:** Use clause boundaries to limit scope. Default: modifiers scope over adjacent conjunct only unless fronted.

## Engineering Approach

This section details concrete Rust types and resolver strategies for implementing syntactic enhancements.

### Design Principles

1. **Heuristic-first:** Avoid full syntactic parsing. Use pattern-matching and scoring.
2. **Composability:** Each resolver handles one phenomenon; chain them.
3. **Multiple interpretations:** When ambiguous, emit all interpretations with confidence scores.
4. **Fail gracefully:** Unknown patterns should not block downstream resolvers.

### Core Types

```rust
/// Confidence-scored syntactic interpretation.
#[derive(Debug, Clone)]
pub struct SyntacticSpan {
    /// What kind of syntactic unit this represents
    pub span_type: SyntacticSpanType,
    /// Confidence in this interpretation (0.0 - 1.0)
    pub confidence: f64,
    /// Alternative interpretations, if ambiguous
    pub alternatives: Vec<SyntacticSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntacticSpanType {
    /// Clause with its type
    Clause(ClauseType),
    /// Coordinated structure
    Coordination(CoordinationType),
    /// Multi-word expression / term of art
    MultiWordExpression,
    /// Relative clause
    RelativeClause { head_noun_offset: i32 },
    /// Prepositional phrase with attachment
    PrepPhrase { attachment: AttachmentSite },
    /// Negation with computed scope
    NegationScope,
    /// Nominalization chain
    NominalizationChain { depth: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClauseType {
    Main,
    Subordinate,
    Relative,
    Conditional,
    Participial,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinationType {
    /// "X and Y"
    Conjunction,
    /// "X or Y"
    Disjunction,
    /// "X, Y, and Z"
    ListConjunction,
    /// "X, Y, or Z"  
    ListDisjunction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentSite {
    /// Attached to nearest verb
    Verb,
    /// Attached to nearest noun
    Noun,
    /// Attached to clause (sentential modifier)
    Clause,
    /// Multiple possible attachments
    Ambiguous(Vec<Box<AttachmentSite>>),
}
```

### Resolver 1: ClauseBoundaryResolver

Extends `layered-clauses` with finer-grained boundary detection.

```rust
/// Markers that signal clause boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClauseBoundary {
    /// Subordinating conjunction: if, when, unless, provided, etc.
    Subordinator(String),
    /// Relative pronoun: that, which, who, whom, whose
    RelativePronoun(String),
    /// Coordinating conjunction at clause level: and, or, but
    Coordinator(String),
    /// Punctuation: comma, semicolon, colon, dash
    Punctuation(char),
    /// Infinitive marker: "to" before verb
    Infinitive,
}

pub struct ClauseBoundaryResolver {
    subordinators: HashSet<&'static str>,
    relative_pronouns: HashSet<&'static str>,
    coordinators: HashSet<&'static str>,
}

impl Default for ClauseBoundaryResolver {
    fn default() -> Self {
        Self {
            subordinators: ["if", "when", "unless", "provided", "although",
                           "because", "since", "while", "after", "before",
                           "until", "whereas", "whereby"].into_iter().collect(),
            relative_pronouns: ["that", "which", "who", "whom", "whose", "where"].into_iter().collect(),
            coordinators: ["and", "or", "but", "nor", "yet"].into_iter().collect(),
        }
    }
}

impl Resolver for ClauseBoundaryResolver {
    type Attr = ClauseBoundary;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        for (sel, text) in selection.find_by(&x::token_text()) {
            let lower = text.to_lowercase();

            if self.subordinators.contains(lower.as_str()) {
                results.push(sel.finish_with_attr(
                    ClauseBoundary::Subordinator(lower)
                ));
            } else if self.relative_pronouns.contains(lower.as_str()) {
                results.push(sel.finish_with_attr(
                    ClauseBoundary::RelativePronoun(lower)
                ));
            } else if self.coordinators.contains(lower.as_str()) {
                results.push(sel.finish_with_attr(
                    ClauseBoundary::Coordinator(lower)
                ));
            }
        }

        // Also mark punctuation boundaries
        for (sel, _) in selection.find_by(&x::attr_eq(&TextTag::PUNC)) {
            if let Some((_, text)) = sel.find_first_by(&x::token_text()) {
                if let Some(c) = text.chars().next() {
                    if matches!(c, ',' | ';' | ':' | '—' | '–') {
                        results.push(sel.finish_with_attr(
                            ClauseBoundary::Punctuation(c)
                        ));
                    }
                }
            }
        }

        results
    }
}
```

### Resolver 2: CoordinationResolver

Detects "X and Y" patterns and determines shared modifier scope.

```rust
/// A coordinated structure with its conjuncts.
#[derive(Debug, Clone)]
pub struct CoordinatedSpan {
    pub coord_type: CoordinationType,
    /// Offsets to each conjunct's start token
    pub conjunct_spans: Vec<SpanRef>,
    /// Whether a preceding modifier likely scopes over all conjuncts
    pub shared_modifier_likely: bool,
}

pub struct CoordinationResolver {
    conjunctions: HashMap<&'static str, CoordinationType>,
}

impl Default for CoordinationResolver {
    fn default() -> Self {
        let mut conjunctions = HashMap::new();
        conjunctions.insert("and", CoordinationType::Conjunction);
        conjunctions.insert("or", CoordinationType::Disjunction);
        Self { conjunctions }
    }
}

impl Resolver for CoordinationResolver {
    type Attr = CoordinatedSpan;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        for (conj_sel, text) in selection.find_by(&x::token_text()) {
            let lower = text.to_lowercase();
            if let Some(&coord_type) = self.conjunctions.get(lower.as_str()) {
                // Look backwards for first conjunct (skip articles/adjectives)
                // Look forwards for second conjunct
                if let Some(span) = self.extract_coordination(&selection, &conj_sel, coord_type.clone()) {
                    results.push(conj_sel.finish_with_attr(span));
                }
            }
        }

        results
    }
}

impl CoordinationResolver {
    fn extract_coordination(
        &self,
        selection: &LLSelection,
        conj_sel: &LLSelection,
        coord_type: CoordinationType,
    ) -> Option<CoordinatedSpan> {
        // Find the noun/VP before conjunction
        let left_conjunct = self.find_conjunct_backwards(selection, conj_sel)?;
        // Find the noun/VP after conjunction
        let right_conjunct = self.find_conjunct_forwards(conj_sel)?;

        // Heuristic: shared modifier likely if both conjuncts have same POS category
        let shared_modifier_likely = self.same_pos_category(&left_conjunct, &right_conjunct);

        Some(CoordinatedSpan {
            coord_type,
            conjunct_spans: vec![left_conjunct, right_conjunct],
            shared_modifier_likely,
        })
    }

    fn find_conjunct_backwards(&self, _sel: &LLSelection, _conj: &LLSelection) -> Option<SpanRef> {
        // Implementation: walk backwards skipping whitespace, find WORD/NP
        todo!()
    }

    fn find_conjunct_forwards(&self, _conj: &LLSelection) -> Option<SpanRef> {
        // Implementation: walk forwards skipping whitespace, find WORD/NP
        todo!()
    }

    fn same_pos_category(&self, _a: &SpanRef, _b: &SpanRef) -> bool {
        // Check if both are nouns, both are verbs, etc.
        todo!()
    }
}
```

### Resolver 3: TermsOfArtResolver

Dictionary-based MWE detection with curated legal terms.

```rust
/// A multi-word expression that should be treated as atomic.
#[derive(Debug, Clone)]
pub struct TermOfArt {
    /// The canonical form of the term
    pub canonical: String,
    /// Category for downstream processing
    pub category: TermOfArtCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermOfArtCategory {
    /// "force majeure", "res judicata" - legal doctrine
    LegalDoctrine,
    /// "indemnify and hold harmless" - obligation phrase
    ObligationPhrase,
    /// "net 30", "COD" - payment term
    PaymentTerm,
    /// "material adverse change", "change of control"
    ContractMechanism,
    /// "pro rata", "pari passu" - ranking/allocation
    AllocationTerm,
    /// "time is of the essence" - interpretive phrase
    InterpretivePhrase,
}

pub struct TermsOfArtResolver {
    /// Maps first word → list of (remaining words, TermOfArt)
    dictionary: HashMap<String, Vec<(Vec<String>, TermOfArt)>>,
}

impl TermsOfArtResolver {
    pub fn new() -> Self {
        let mut resolver = Self { dictionary: HashMap::new() };
        resolver.add_defaults();
        resolver
    }

    fn add_defaults(&mut self) {
        // Legal doctrines
        self.add("force majeure", TermOfArtCategory::LegalDoctrine);
        self.add("res judicata", TermOfArtCategory::LegalDoctrine);
        self.add("stare decisis", TermOfArtCategory::LegalDoctrine);
        self.add("prima facie", TermOfArtCategory::LegalDoctrine);
        self.add("ex parte", TermOfArtCategory::LegalDoctrine);

        // Obligation phrases
        self.add("indemnify and hold harmless", TermOfArtCategory::ObligationPhrase);
        self.add("represent and warrant", TermOfArtCategory::ObligationPhrase);
        self.add("covenant not to compete", TermOfArtCategory::ObligationPhrase);
        self.add("acknowledge and agree", TermOfArtCategory::ObligationPhrase);

        // Payment terms
        self.add("net 30", TermOfArtCategory::PaymentTerm);
        self.add("net 60", TermOfArtCategory::PaymentTerm);
        self.add("net 90", TermOfArtCategory::PaymentTerm);
        self.add("cash on delivery", TermOfArtCategory::PaymentTerm);

        // Contract mechanisms
        self.add("material adverse change", TermOfArtCategory::ContractMechanism);
        self.add("material adverse effect", TermOfArtCategory::ContractMechanism);
        self.add("change of control", TermOfArtCategory::ContractMechanism);
        self.add("right of first refusal", TermOfArtCategory::ContractMechanism);
        self.add("right of first offer", TermOfArtCategory::ContractMechanism);

        // Allocation terms
        self.add("pro rata", TermOfArtCategory::AllocationTerm);
        self.add("pari passu", TermOfArtCategory::AllocationTerm);
        self.add("mutatis mutandis", TermOfArtCategory::AllocationTerm);
        self.add("inter alia", TermOfArtCategory::AllocationTerm);

        // Interpretive phrases
        self.add("time is of the essence", TermOfArtCategory::InterpretivePhrase);
        self.add("without prejudice", TermOfArtCategory::InterpretivePhrase);
        self.add("for the avoidance of doubt", TermOfArtCategory::InterpretivePhrase);
    }

    fn add(&mut self, phrase: &str, category: TermOfArtCategory) {
        let words: Vec<String> = phrase.split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();

        if let Some(first) = words.first() {
            let remaining = words[1..].to_vec();
            let term = TermOfArt {
                canonical: phrase.to_string(),
                category,
            };

            self.dictionary
                .entry(first.clone())
                .or_default()
                .push((remaining, term));
        }
    }
}

impl Resolver for TermsOfArtResolver {
    type Attr = TermOfArt;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        for (sel, text) in selection.find_by(&x::token_text()) {
            let lower = text.to_lowercase();

            if let Some(candidates) = self.dictionary.get(&lower) {
                for (remaining, term) in candidates {
                    if let Some(extended) = self.try_match_phrase(&sel, remaining) {
                        results.push(extended.finish_with_attr(term.clone()));
                    }
                }
            }
        }

        results
    }
}

impl TermsOfArtResolver {
    /// Try to match remaining words after the first word.
    fn try_match_phrase(&self, start: &LLSelection, remaining: &[String]) -> Option<LLSelection> {
        let mut current = start.clone();

        for expected in remaining {
            // Skip whitespace
            let (ws_sel, _) = current.match_first_forwards(&x::whitespace())?;
            current = ws_sel;

            // Match next word
            let (word_sel, text) = current.match_first_forwards(&x::token_text())?;
            if text.to_lowercase() != *expected {
                return None;
            }
            current = word_sel;
        }

        Some(current)
    }
}
```

### Resolver 4: NegationScopeResolver

Computes the scope of negation markers.

```rust
/// A negation with its computed scope.
#[derive(Debug, Clone)]
pub struct NegationScope {
    /// The negation marker text ("not", "never", etc.)
    pub marker: String,
    /// Whether scope extends to end of clause (rightward unbounded)
    pub scope_to_clause_end: bool,
    /// Explicit scope boundary if detectable
    pub scope_end_hint: Option<ScopeEndReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeEndReason {
    /// "except", "unless" introduces exception
    Exception,
    /// Clause boundary punctuation
    ClauseBoundary,
    /// Another modal verb
    ModalVerb,
    /// Coordinating conjunction
    Conjunction,
}

pub struct NegationScopeResolver {
    negation_markers: HashSet<&'static str>,
    exception_markers: HashSet<&'static str>,
}

impl Default for NegationScopeResolver {
    fn default() -> Self {
        Self {
            negation_markers: ["not", "never", "no", "neither", "nor"].into_iter().collect(),
            exception_markers: ["except", "unless", "save", "but"].into_iter().collect(),
        }
    }
}
```

### Resolver Chaining Order

The recommended execution order for syntactic resolvers:

```rust
let line = create_line_from_string(text)
    // Phase 1: Tokenization and POS (external)
    .run(&pos_resolver)

    // Phase 2: Clause structure (before content analysis)
    .run(&ClauseBoundaryResolver::default())
    .run(&ClauseResolver::default())

    // Phase 3: MWEs (prevents splitting terms of art)
    .run(&TermsOfArtResolver::new())

    // Phase 4: Coordination (after MWEs so "indemnify and hold harmless" is atomic)
    .run(&CoordinationResolver::default())

    // Phase 5: Scope resolution (uses clause boundaries)
    .run(&NegationScopeResolver::default())

    // Phase 6: Contract-specific (uses all above)
    .run(&ContractKeywordResolver::new())
    .run(&ObligationPhraseResolver::new());
```

### Trade-offs

#### Heuristics vs External Parsers

| Approach | Pros | Cons |
|----------|------|------|
| **Heuristics (recommended)** | Fast, predictable, domain-tunable, no external deps | May miss complex cases, requires manual patterns |
| **External parser** | Handles full syntax, less code | Slow, opaque errors, domain mismatch for legal text, dependency burden |

**Recommendation:** Start with heuristics. Add optional external parser integration only if:
- Recall on edge cases drops below 80%
- Performance budget allows 10-50ms per sentence
- A domain-adapted parser (e.g., legal-specific treebank) is available

#### Handling Ambiguity

1. **Single-best with confidence:** Return highest-scored interpretation with `confidence` field
2. **N-best list:** Store `alternatives: Vec<SyntacticSpan>` for downstream disambiguation
3. **Flag for review:** When confidence < threshold, add `AmbiguityFlag` attribute

**Default policy:** Return single-best interpretation; store top-3 alternatives if confidence spread is narrow (< 0.1 between top-2).

#### Performance Considerations

| Document Size | Target Latency | Strategy |
|--------------|----------------|----------|
| < 10 pages | < 100ms | All resolvers, no optimization needed |
| 10-100 pages | < 1s | Parallelize per-paragraph, lazy MWE matching |
| 100+ pages | < 10s | Batch processing, skip low-priority resolvers (e.g., nominalization detection) |

**Optimizations:**
1. **Dictionary structure:** Use `HashMap<first_word, Vec<...>>` for O(1) MWE lookup
2. **Early termination:** Stop scope resolution at clause boundaries
3. **Parallelism:** Resolvers are pure; process lines in parallel with Rayon

## Dependencies

- Enhances FR-001 (obligation extraction accuracy)
- Enhances FR-002 (reference resolution with structural distance)
- May require FR-003 (cross-line spans for long sentences)

## Implementation Notes

This is the most complex FR and may be implemented incrementally:
1. Phase 1: Clause boundaries + coordination detection
2. Phase 2: Terms of art dictionary
3. Phase 3: PP/relative clause attachment heuristics
4. Phase 4: Scope resolution (negation, quantifiers)

---

## Future: Multi-Perspective Support

FR-005 syntactic resolvers produce `Scored<T>` with alternatives. This naturally supports the **multi-perspective architecture** being developed:

- AI produces syntactic analysis with confidence + alternatives
- Human reviewers can accept, reject, or provide different interpretations
- All perspectives coexist — spans stack at the same position

See [../ARCHITECTURE-ITERATIVE-ENRICHMENT.md](../ARCHITECTURE-ITERATIVE-ENRICHMENT.md) for the vision of layered interpretation where multiple viewers contribute their own analysis.
