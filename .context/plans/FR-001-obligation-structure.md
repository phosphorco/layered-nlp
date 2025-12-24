# FR-001: Enhanced Obligation Structure Modeling

## Summary

Extend the obligation detection system to capture complex obligation structures including modal strength, nested conditions, exception hierarchies, and implicit obligations.

## Current State

The `ObligationPhraseResolver` detects obligations based on modal keywords (shall, may, must, shall not) and extracts a flat list of conditions. This misses:
- Gradations in obligation strength
- Hierarchical condition structures
- Exception nesting and precedence
- Obligations implied by non-modal patterns

## Proposed Improvements

### 1. Modal Strength Classification

**Problem:** "shall" and "should" are treated identically (or "should" is missed entirely).

**Example:**
```
The Company should endeavor to provide the Services, but shall not be liable for delays.
```

**Ask:** Classify obligations by binding strength:
- `Duty` - shall, must (absolute)
- `SoftDuty` - should, ought to (best effort)
- `Permission` - may, can
- `Prohibition` - shall not, must not
- `Discretionary` - may, might (truly optional)

### 2. Nested Condition Trees

**Problem:** Conditions are collected as a flat `Vec<ConditionRef>`, losing hierarchy.

**Example:**
```
If the Services are not delivered on time, the Client may deduct late fees,
unless the delay is caused by force majeure, in which case the Company
shall have an additional thirty days, provided that notice is given within
five business days.
```

**Ask:** Build condition trees with operator precedence:
- `If/When` → primary trigger
- `Unless` → exception to primary
- `In which case` → consequence of exception
- `Provided that` → sub-condition on consequence

### 3. Exception Hierarchies

**Problem:** Multiple exceptions are treated as a flat list.

**Example:**
```
The Company shall indemnify the Client against all claims, except claims
arising from gross negligence, and further except claims covered by
insurance, except to the extent insurance is exhausted.
```

**Ask:** Model nested exceptions with scope reduction:
- `FullCarveOut` - obligation doesn't apply
- `PartialReduction` - obligation reduced by amount
- `ConditionalReinstate` - exception doesn't apply if condition met
- `CappedLiability` - obligation capped at limit

### 4. Implicit Obligation Detection

**Problem:** Only explicit modal verbs trigger obligation detection.

**Example:**
```
The Company represents and warrants that it has full authority to enter
into this Agreement.
```

**Ask:** Detect implicit obligation patterns:
- "represents and warrants" → implicit duty to ensure truth
- "acknowledges" → implicit acceptance
- "agrees to" → soft obligation
- "covenants" → binding promise
- "undertakes to" → commitment

## Success Criteria

- [ ] Modal strength captured with confidence scoring
- [ ] Condition trees preserve if/unless/provided hierarchy
- [ ] Exception chains model scope reduction correctly
- [ ] Implicit obligations detected with lower confidence scores
- [ ] Existing tests continue to pass

## Edge Cases

### Modal Ambiguity

**1. Future vs Duty Ambiguity**
```
The Company shall deliver goods by January.   // Duty: must deliver
The hearing shall take place next month.      // Future: will take place
```
The word "shall" in legal contexts typically denotes obligation, but in procedural or scheduling contexts may indicate future tense. Detection heuristics:
- Subject animacy (companies have duties; events have schedules)
- Presence of duty-compatible verbs (deliver, pay, indemnify vs take place, occur)

**2. Epistemic vs Deontic "may"**
```
The Company may terminate this Agreement.     // Permission: is allowed to
The delay may cause damages.                  // Epistemic: might possibly
```
Disambiguation requires:
- Subject animacy (parties get permissions; events have probabilities)
- Verb type (volitional actions vs stative outcomes)

**3. "Should" Interpretation Variance**
```
The Company should provide notice.            // Soft duty (US contracts)
The Company should provide notice.            // Strong duty (UK interpretation)
```
Jurisdictional context affects modal strength interpretation.

### Nested Conditionals with Exception Layers

**4. Multi-Layer Exception Nesting**
```
The Company shall indemnify the Client,
  unless the claim arises from Client negligence,
    except where such negligence was caused by Company's instructions,
      in which case Company shall bear 50% of liability,
        provided that total liability shall not exceed $1M.
```
Exception tree:
```
Duty(indemnify)
├── Exception(client_negligence)
│   └── Counter-Exception(company_instructions)
│       └── Modified-Duty(50% liability)
│           └── Cap($1M)
```

**5. Overlapping Exception Scopes**
```
The Seller shall deliver, except on holidays and except during force majeure,
unless the Buyer provides 48 hours notice, in which case delivery shall proceed
notwithstanding any holiday.
```
The "notwithstanding" creates a counter-exception that only removes the holiday exception, not force majeure.

**6. Conditional Exception Reinstatement**
```
The Company shall not disclose Confidential Information, except as required by law,
provided that if such disclosure is required, the Company shall first notify Client,
unless notification would violate the legal requirement.
```
Creates a conditional that reinstates a modified form of the original duty.

### Implicit Obligations

**7. Representation as Continuing Duty**
```
The Seller represents that the goods are free from defects.
```
Implicit: Seller has ongoing duty to ensure truth of representation; breach triggers remedies.

**8. Warranty as Performance Standard**
```
The Contractor warrants workmanship for a period of two years.
```
Implicit: Contractor shall repair/replace defective work during warranty period.

**9. Covenant Obligations**
```
The Company hereby covenants to maintain insurance coverage.
```
"Covenants" creates binding promise without explicit modal verb.

**10. Agreement-to-Agree Obligations**
```
The parties agree to negotiate in good faith regarding pricing.
```
Creates implicit procedural obligations (timing, manner, scope of negotiation).

**11. Acknowledgment Creating Estoppel**
```
The Buyer acknowledges that the Seller makes no warranties.
```
Creates implicit waiver obligation—Buyer cannot later claim warranty breach.

### Temporal Scoping Issues

**12. Open-Ended Temporal Quantification**
```
The Company may at any time and from time to time inspect the premises.
```
Permission with unlimited temporal scope and iteration.

**13. Survival Clauses Creating Temporal Layers**
```
The obligations under Sections 5-7 shall survive termination.
```
Creates implicit temporal extension for subset of obligations.

**14. Retroactive Obligations**
```
The Company shall have been in compliance with all laws since the Effective Date.
```
Creates obligation with retroactive verification scope.

**15. Contingent Future Obligations**
```
Upon occurrence of an IPO, the Company shall offer Investor the right to participate.
```
Obligation exists but is dormant until trigger event.

### Obligation Chains and Dependencies

**16. Cascading Triggered Obligations**
```
If the Company fails to deliver, the Buyer may terminate.
Upon such termination, the Company shall refund all amounts paid.
The Buyer shall return all delivered goods within 30 days of refund.
```
Creates dependency chain: Company-failure → Buyer-termination → Company-refund → Buyer-return.

**17. Cross-Referenced Obligation Modification**
```
Subject to Section 5.2, the Company shall deliver goods monthly.
[Section 5.2]: Delivery obligations are suspended during force majeure.
```
Obligation modified by remote cross-reference.

**18. Reciprocal Obligations**
```
Each party shall notify the other of any material change.
```
Single statement creates two separate obligations with mirrored structure.

### Cross-Cultural/Jurisdictional Variations

**19. Civil Law Modal Patterns**
```
Le vendeur doit livrer...              // "doit" = shall/must (French)
Der Verkäufer hat zu liefern...        // "hat zu" = shall (German)
```
Non-English modal detection for international contracts.

**20. Common Law vs Civil Law "Best Efforts"**
```
The Company shall use best efforts to obtain approval.  // US: reasonable efforts
The Company shall use best efforts to obtain approval.  // UK: higher standard
```
Same phrase, different legal standard by jurisdiction.

### Complex Structural Patterns

**21. Disjunctive Obligations**
```
The Company shall either (a) deliver goods or (b) provide equivalent credit.
```
Obligation satisfied by either branch—must track as OR-linked.

**22. Conjunctive Obligations with Partial Exceptions**
```
The Company shall deliver goods and provide support, except that support
obligations shall not apply to discontinued products.
```
Exception applies to second conjunct only.

**23. Obligation with Cure Period**
```
The Company shall remedy any breach within 30 days of notice, failing which
the Buyer may terminate.
```
Primary obligation with grace period before consequence obligation activates.

## Engineering Approach

### Proposed Resolver Architecture

The enhanced obligation detection requires multiple composable resolvers that build incrementally:

```
Input Text
    │
    ▼
┌──────────────────────┐
│ ModalStrengthResolver │  ← Classifies modal verbs by deontic strength
└──────────────────────┘
    │
    ▼
┌──────────────────────┐
│ ImplicitDutyResolver  │  ← Detects non-modal obligation patterns
└──────────────────────┘
    │
    ▼
┌────────────────────────┐
│ ConditionStructResolver │  ← Parses condition/exception tree
└────────────────────────┘
    │
    ▼
┌──────────────────────────┐
│ ExceptionHierarchyResolver│  ← Models nested exception scopes
└──────────────────────────┘
    │
    ▼
┌───────────────────────┐
│ ObligationTreeResolver │  ← Assembles complete obligation structure
└───────────────────────┘
```

### Core Type Definitions

```rust
//! obligation_types.rs - Core types for obligation structure modeling

use layered_nlp::{Association, SpanRef};
use serde::{Deserialize, Serialize};

/// Gradation of obligation binding strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModalStrength {
    /// Absolute duty: shall, must
    Duty,
    /// Best effort: should, ought to
    SoftDuty,
    /// Permitted action: may, can (deontic)
    Permission,
    /// Forbidden action: shall not, must not
    Prohibition,
    /// Truly optional: may, might (discretionary)
    Discretionary,
    /// Epistemic possibility (not deontic): may cause, might result
    Epistemic,
    /// Future indication (not deontic): shall take place, will occur
    FutureIndicative,
}

/// Source of modal strength classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModalSource {
    /// Explicit modal verb detected
    ExplicitModal { verb: String },
    /// Implicit from performative pattern
    ImplicitPerformative { pattern: String },
    /// Inferred from context
    ContextualInference { reason: String },
}

/// Modal classification with confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct ModalClassification {
    pub strength: ModalStrength,
    pub source: ModalSource,
    pub confidence: f64,
    /// Alternative interpretations when ambiguous
    pub alternatives: Vec<(ModalStrength, f64)>,
}

/// Condition type in obligation structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionType {
    /// Primary trigger: if, when, where
    Trigger,
    /// Exception to primary: unless, except
    Exception,
    /// Proviso on exception: provided that, so long as
    Proviso,
    /// Counter-exception: notwithstanding, except that...except
    CounterException,
    /// Consequence of exception: in which case, whereupon
    Consequence,
    /// Scope limitation: subject to, limited to
    ScopeLimiter,
}

/// Operator for combining conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionOperator {
    And,
    Or,
}

/// Node in condition tree structure.
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionNode {
    pub condition_type: ConditionType,
    /// Text of the condition clause
    pub text: String,
    /// Span reference for provenance
    pub span: SpanRef,
    /// Child conditions (nested exceptions, provisos)
    pub children: Vec<ConditionNode>,
    /// Operator combining siblings
    pub sibling_operator: Option<ConditionOperator>,
}

/// Exception scope reduction pattern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExceptionEffect {
    /// Obligation completely inapplicable
    FullCarveOut,
    /// Obligation reduced by amount/percentage
    PartialReduction { amount: String },
    /// Exception doesn't apply if sub-condition met
    ConditionalReinstate { condition: String },
    /// Obligation capped at limit
    CappedLiability { cap: String },
    /// Different party/scope applies
    SubstitutedObligation { replacement: String },
}

/// Hierarchical exception with nested structure.
#[derive(Debug, Clone, PartialEq)]
pub struct ExceptionNode {
    pub effect: ExceptionEffect,
    pub scope_text: String,
    pub span: SpanRef,
    /// Nested sub-exceptions
    pub sub_exceptions: Vec<ExceptionNode>,
}

/// Pattern for implicit obligation detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImplicitPattern {
    /// "represents and warrants" → duty to ensure truth
    RepresentationWarranty,
    /// "covenants" → binding promise
    Covenant,
    /// "undertakes to" → commitment
    Undertaking,
    /// "agrees to" → soft obligation
    Agreement,
    /// "acknowledges" → acceptance creating estoppel
    Acknowledgment,
    /// "is entitled to" → correlative duty on counterparty
    Entitlement,
}

/// Complete obligation structure with tree-based conditions.
#[derive(Debug, Clone, PartialEq)]
pub struct ObligationTree {
    /// Who has the obligation
    pub obligor: ObligorReference,
    /// Classification of modal strength
    pub modal: ModalClassification,
    /// The action to be performed
    pub action: String,
    /// Span of the action phrase
    pub action_span: SpanRef,
    /// Hierarchical condition tree (if any)
    pub conditions: Option<ConditionNode>,
    /// Hierarchical exception tree (if any)
    pub exceptions: Option<ExceptionNode>,
    /// Temporal scope (when obligation applies)
    pub temporal_scope: Option<TemporalScope>,
    /// Whether this is an implicit obligation
    pub implicit: bool,
}

/// Temporal scoping for obligations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TemporalScope {
    /// Applies continuously during agreement term
    Continuous,
    /// One-time performance required
    OneTime,
    /// Recurring at specified intervals
    Periodic { interval: String },
    /// Triggered by event
    EventTriggered { trigger: String },
    /// Dormant until condition
    Contingent { condition: String },
    /// Survives termination
    Survival { sections: Vec<String> },
}

/// Reference to the obligor (from existing obligation.rs, extended).
#[derive(Debug, Clone, PartialEq)]
pub enum ObligorReference {
    TermRef { term_name: String, confidence: f64 },
    PronounRef { pronoun: String, resolved_to: String, is_defined_term: bool, confidence: f64 },
    NounPhrase { text: String },
    /// New: Quantified obligor (each party, all shareholders)
    Quantified { quantifier: String, entity: String },
    /// New: Disjunctive obligor (Buyer or its designee)
    Disjunctive { options: Vec<String> },
}
```

### Association Types for Relationships

```rust
//! obligation_associations.rs - Association types linking obligation components

use layered_nlp::Association;

/// Links obligation to its triggering condition span.
#[derive(Debug, Clone)]
pub struct TriggerCondition;
impl Association for TriggerCondition {
    fn label(&self) -> &'static str { "trigger_condition" }
    fn glyph(&self) -> Option<&'static str> { Some("?") }
}

/// Links obligation to its exception span.
#[derive(Debug, Clone)]
pub struct ExceptionSpan;
impl Association for ExceptionSpan {
    fn label(&self) -> &'static str { "exception" }
    fn glyph(&self) -> Option<&'static str> { Some("!") }
}

/// Links exception to its parent exception (for nesting).
#[derive(Debug, Clone)]
pub struct ParentException;
impl Association for ParentException {
    fn label(&self) -> &'static str { "parent_exception" }
    fn glyph(&self) -> Option<&'static str> { Some("↑") }
}

/// Links counter-exception to the exception it overrides.
#[derive(Debug, Clone)]
pub struct OverridesException;
impl Association for OverridesException {
    fn label(&self) -> &'static str { "overrides" }
    fn glyph(&self) -> Option<&'static str> { Some("⊘") }
}

/// Links proviso to the condition/exception it qualifies.
#[derive(Debug, Clone)]
pub struct QualifiesCondition;
impl Association for QualifiesCondition {
    fn label(&self) -> &'static str { "qualifies" }
    fn glyph(&self) -> Option<&'static str> { Some("~") }
}

/// Links implicit obligation to the pattern that creates it.
#[derive(Debug, Clone)]
pub struct ImplicitSource;
impl Association for ImplicitSource {
    fn label(&self) -> &'static str { "implicit_from" }
    fn glyph(&self) -> Option<&'static str> { Some("*") }
}

/// Links dependent obligation to triggering obligation in chain.
#[derive(Debug, Clone)]
pub struct TriggeredBy;
impl Association for TriggeredBy {
    fn label(&self) -> &'static str { "triggered_by" }
    fn glyph(&self) -> Option<&'static str> { Some("→") }
}

/// Links obligation to its temporal scope marker.
#[derive(Debug, Clone)]
pub struct TemporalMarker;
impl Association for TemporalMarker {
    fn label(&self) -> &'static str { "temporal" }
    fn glyph(&self) -> Option<&'static str> { Some("⏱") }
}
```

### Resolver Implementations

```rust
//! modal_strength_resolver.rs

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver, TextTag};
use layered_part_of_speech::Tag;

use crate::contract_keyword::ContractKeyword;
use crate::scored::Scored;
use crate::obligation_types::{ModalClassification, ModalStrength, ModalSource};

/// Resolver for classifying modal strength with disambiguation.
pub struct ModalStrengthResolver {
    /// Verbs that indicate event/state (not volitional action)
    stative_verbs: Vec<&'static str>,
    /// Subjects that indicate scheduling context
    event_subjects: Vec<&'static str>,
}

impl Default for ModalStrengthResolver {
    fn default() -> Self {
        Self {
            stative_verbs: vec![
                "take place", "occur", "happen", "result", "arise",
                "become", "remain", "exist", "consist",
            ],
            event_subjects: vec![
                "hearing", "meeting", "trial", "proceeding", "session",
                "event", "occurrence", "matter", "issue",
            ],
        }
    }
}

impl Resolver for ModalStrengthResolver {
    type Attr = Scored<ModalClassification>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        // Find existing ContractKeyword::Shall, May, ShallNot
        for (modal_sel, keyword) in selection.find_by(&x::attr::<ContractKeyword>()) {
            let (primary_strength, alternatives, confidence) = match keyword {
                ContractKeyword::Shall => {
                    // Disambiguate: duty vs future indicative
                    if self.is_future_context(&selection, &modal_sel) {
                        (ModalStrength::FutureIndicative, 
                         vec![(ModalStrength::Duty, 0.3)], 
                         0.7)
                    } else {
                        (ModalStrength::Duty, 
                         vec![(ModalStrength::FutureIndicative, 0.1)], 
                         0.9)
                    }
                }
                ContractKeyword::May => {
                    // Disambiguate: deontic permission vs epistemic possibility
                    if self.is_epistemic_context(&selection, &modal_sel) {
                        (ModalStrength::Epistemic,
                         vec![(ModalStrength::Permission, 0.2)],
                         0.75)
                    } else {
                        (ModalStrength::Permission,
                         vec![(ModalStrength::Epistemic, 0.1)],
                         0.85)
                    }
                }
                ContractKeyword::ShallNot => {
                    (ModalStrength::Prohibition, vec![], 0.95)
                }
                _ => continue,
            };

            let classification = ModalClassification {
                strength: primary_strength,
                source: ModalSource::ExplicitModal {
                    verb: format!("{:?}", keyword),
                },
                confidence,
                alternatives,
            };

            results.push(
                modal_sel.finish_with_attr(Scored::rule_based(
                    classification,
                    confidence,
                    "modal_strength",
                ))
            );
        }

        // Also detect "should", "ought to" for SoftDuty
        for (sel, text) in selection.find_by(&x::token_text()) {
            let lower = text.to_lowercase();
            if lower == "should" {
                let classification = ModalClassification {
                    strength: ModalStrength::SoftDuty,
                    source: ModalSource::ExplicitModal { verb: "should".into() },
                    confidence: 0.8,
                    alternatives: vec![],
                };
                results.push(sel.finish_with_attr(Scored::rule_based(
                    classification, 0.8, "modal_strength"
                )));
            }
        }

        results
    }
}

impl ModalStrengthResolver {
    /// Check if context suggests future indicative rather than duty.
    fn is_future_context(&self, selection: &LLSelection, modal_sel: &LLSelection) -> bool {
        // Look for stative verbs after modal
        // Look for event-type subjects before modal
        // Implementation would check POS tags and semantic patterns
        false // Placeholder
    }

    /// Check if context suggests epistemic rather than deontic.
    fn is_epistemic_context(&self, selection: &LLSelection, modal_sel: &LLSelection) -> bool {
        // Check for non-volitional predicates (cause, result, lead to)
        // Check for non-animate subjects
        false // Placeholder
    }
}
```

```rust
//! implicit_duty_resolver.rs

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver, TextTag};

use crate::scored::Scored;
use crate::obligation_types::{ImplicitPattern, ModalClassification, ModalStrength, ModalSource};
use crate::obligation_associations::ImplicitSource;

/// Detects implicit obligations from performative patterns.
pub struct ImplicitDutyResolver {
    /// Patterns and their associated strength
    patterns: Vec<(Vec<&'static str>, ImplicitPattern, ModalStrength, f64)>,
}

impl Default for ImplicitDutyResolver {
    fn default() -> Self {
        Self {
            patterns: vec![
                (vec!["represents", "and", "warrants"], 
                 ImplicitPattern::RepresentationWarranty, ModalStrength::Duty, 0.85),
                (vec!["covenants", "to"],
                 ImplicitPattern::Covenant, ModalStrength::Duty, 0.9),
                (vec!["covenants", "that"],
                 ImplicitPattern::Covenant, ModalStrength::Duty, 0.9),
                (vec!["undertakes", "to"],
                 ImplicitPattern::Undertaking, ModalStrength::Duty, 0.85),
                (vec!["agrees", "to"],
                 ImplicitPattern::Agreement, ModalStrength::SoftDuty, 0.75),
                (vec!["acknowledges", "that"],
                 ImplicitPattern::Acknowledgment, ModalStrength::SoftDuty, 0.7),
                (vec!["is", "entitled", "to"],
                 ImplicitPattern::Entitlement, ModalStrength::Permission, 0.8),
            ],
        }
    }
}

impl Resolver for ImplicitDutyResolver {
    type Attr = Scored<ModalClassification>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        for (pattern_words, pattern_type, strength, confidence) in &self.patterns {
            // Find multi-word pattern matches
            for match_result in self.find_pattern(&selection, pattern_words) {
                let classification = ModalClassification {
                    strength: *strength,
                    source: ModalSource::ImplicitPerformative {
                        pattern: format!("{:?}", pattern_type),
                    },
                    confidence: *confidence,
                    alternatives: vec![],
                };

                // Get the span of the pattern source for association
                let source_span = match_result.span_ref();

                results.push(
                    match_result
                        .assign(Scored::rule_based(classification, *confidence, "implicit_duty"))
                        .with_association(ImplicitSource, source_span)
                        .build()
                );
            }
        }

        results
    }
}

impl ImplicitDutyResolver {
    fn find_pattern<'a>(
        &self,
        selection: &'a LLSelection,
        pattern: &[&str],
    ) -> Vec<LLSelection> {
        // Multi-word phrase matching implementation
        // Similar to Pattern 2 in building-resolvers.md
        vec![] // Placeholder
    }
}
```

```rust
//! condition_struct_resolver.rs

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver, SpanRef};

use crate::contract_keyword::ContractKeyword;
use crate::scored::Scored;
use crate::obligation_types::{ConditionNode, ConditionType, ConditionOperator};
use crate::obligation_associations::{TriggerCondition, ExceptionSpan, QualifiesCondition};

/// Builds hierarchical condition trees from condition keywords.
pub struct ConditionStructResolver;

impl Resolver for ConditionStructResolver {
    type Attr = Scored<ConditionNode>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();
        
        // Map condition keywords to types
        let keyword_types = [
            (ContractKeyword::If, ConditionType::Trigger),
            (ContractKeyword::Unless, ConditionType::Exception),
            (ContractKeyword::Provided, ConditionType::Proviso),
            (ContractKeyword::SubjectTo, ConditionType::ScopeLimiter),
        ];

        for (keyword, cond_type) in keyword_types {
            for (cond_sel, _) in selection.find_by(&x::attr_eq(&keyword)) {
                let text = self.extract_condition_text(&cond_sel);
                let span = cond_sel.span_ref();

                // Find parent condition (for nesting)
                let parent = self.find_parent_condition(&selection, &cond_sel);

                let node = ConditionNode {
                    condition_type: cond_type,
                    text,
                    span,
                    children: vec![], // Children added in later pass
                    sibling_operator: self.detect_sibling_operator(&selection, &cond_sel),
                };

                let mut builder = cond_sel.assign(Scored::rule_based(node, 0.85, "condition_struct"));

                // Add association to parent if nested
                if let Some(parent_span) = parent {
                    builder = builder.with_association(QualifiesCondition, parent_span);
                }

                results.push(builder.build());
            }
        }

        // Detect "in which case" consequence patterns
        for sel in self.find_consequence_patterns(&selection) {
            let node = ConditionNode {
                condition_type: ConditionType::Consequence,
                text: self.extract_condition_text(&sel),
                span: sel.span_ref(),
                children: vec![],
                sibling_operator: None,
            };
            results.push(sel.finish_with_attr(Scored::rule_based(node, 0.8, "condition_struct")));
        }

        // Detect "notwithstanding" counter-exception patterns
        for sel in self.find_notwithstanding_patterns(&selection) {
            let node = ConditionNode {
                condition_type: ConditionType::CounterException,
                text: self.extract_condition_text(&sel),
                span: sel.span_ref(),
                children: vec![],
                sibling_operator: None,
            };
            results.push(sel.finish_with_attr(Scored::rule_based(node, 0.85, "condition_struct")));
        }

        results
    }
}

impl ConditionStructResolver {
    fn extract_condition_text(&self, sel: &LLSelection) -> String {
        // Extract text following condition keyword up to boundary
        String::new() // Placeholder
    }

    fn find_parent_condition(&self, selection: &LLSelection, child: &LLSelection) -> Option<SpanRef> {
        // Find enclosing condition for nesting
        None // Placeholder
    }

    fn detect_sibling_operator(&self, selection: &LLSelection, sel: &LLSelection) -> Option<ConditionOperator> {
        // Look for "and" or "or" before this condition
        None // Placeholder
    }

    fn find_consequence_patterns(&self, selection: &LLSelection) -> Vec<LLSelection> {
        vec![] // Placeholder
    }

    fn find_notwithstanding_patterns(&self, selection: &LLSelection) -> Vec<LLSelection> {
        vec![] // Placeholder
    }
}
```

### Integration Points

#### FR-003: Cross-Line Spans
The condition tree and exception hierarchy often span multiple lines:
```
If payment is not received within 30 days,      ← Line 1: Trigger
    and if the Buyer has not provided notice,   ← Line 2: Conjunctive condition
    the Seller may terminate,                   ← Line 3: Consequence
    unless such delay is excused.               ← Line 4: Exception
```

FR-003's `DocumentResolver` trait enables:
- `ConditionTreeDocResolver` that builds trees spanning line boundaries
- `SemanticSpan` covering full condition-obligation-exception blocks
- Linking via `DocPosition` rather than single-line `SpanRef`

#### FR-005: Syntactic Enhancement
Complex obligation parsing benefits from:
- **Clause boundary detection**: Determines exception scope ("unless" applies to which clause?)
- **PP attachment**: "within 30 days" attaches to "deliver" vs "terminate"
- **Coordination structure**: "shall deliver goods and provide support" → two actions or one compound?
- **Negation scope**: "shall not disclose information except..." → scope of negation vs exception

The `ModalStrengthResolver` can consume clause boundary attributes from FR-005:
```rust
// Query clause boundary from FR-005
for (sel, clause_boundary) in selection.find_by(&x::attr::<ClauseBoundary>()) {
    // Use clause depth for disambiguation
}
```

#### FR-006: Conflict Detection
FR-001's structured `ObligationTree` enables FR-006's semantic analysis:
- **Normalized comparison**: Compare `(obligor, modal_strength, action_normalized)` tuples
- **Conflict detection**: Two obligations with same obligor+action but different `ModalStrength` (Duty vs Prohibition)
- **Precedence application**: Exception hierarchy determines which obligation wins
- **Semantic roles**: Agent/Theme extraction from obligation actions

Example conflict detection integration:
```rust
pub fn detect_conflicts(obligations: &[ObligationTree]) -> Vec<Conflict> {
    // Compare obligations with overlapping scope
    // Check for contradictory modal strengths
    // Apply exception hierarchies to determine resolution
}
```

## Related Edge Cases

- Modal ambiguity (Contract #1)
- Nested conditions (Contract #2)
- Implicit obligations (Contract #3)
- Exception hierarchies (Contract #8)

## Dependencies

- **FR-003 (Cross-Line Spans)**: Required for multi-sentence conditions and exception chains
- **FR-005 (Syntactic Enhancement)**: Enhances disambiguation of modal verbs, attachment ambiguity, and scope resolution
- **FR-006 (Semantic Analysis)**: Consumes structured obligation output for conflict detection and precedence application
