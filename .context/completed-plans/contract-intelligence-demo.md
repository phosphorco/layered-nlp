# Contract Intelligence Demo

**Status:** ✅ Complete
**Last Updated:** 2025-12-31
**Total Tests:** 741 passing across workspace
**Dependencies:** Foundation Complete (SpanLink, ScopeOperator, Conflicts, Semantic Roles)

## Objective

Build a compelling interactive demo that showcases contract intelligence capabilities:
- **SpanLink<R>** — Typed relations between spans (clause hierarchy, semantic roles, conflicts)
- **ScopeOperator<O>** — Scope-bearing operators (negation, quantifiers, precedence)
- **N-Best Ambiguity** — Preserved alternatives with confidence scores
- **Conflict Detection & Resolution** — Modal, temporal, party conflicts with precedence resolution

## Demo Scenarios

### Scenario 1: Obligation Extraction with Semantic Roles
**Input:** Complex obligation with multiple participants
```
Unless the Buyer provides written notice within 5 business days of receiving goods,
the Seller may, in its sole discretion, refuse to accept returns, unless the goods
are defective.
```

**Demonstrates:**
- Semantic role extraction (Agent, Theme, Beneficiary, Condition, Exception)
- SpanLink<SemanticRole> connecting predicates to arguments
- Pronoun resolution ("Seller" → Company, "Buyer" → Customer)
- Temporal expression extraction and normalization
- Multiple exception/condition layers

### Scenario 2: Conflict Detection & Precedence Resolution
**Input:** Two conflicting provisions
```
Section 3.1: The Company shall deliver goods within 30 days.
Section 3.2: The Company may deliver goods within 60 days.
Section 10: Notwithstanding anything to the contrary, Section 3.1 shall govern.
```

**Demonstrates:**
- ConflictDetector finding modal conflict (shall vs may)
- ScopeOperator<PrecedenceOp> from "Notwithstanding" clause
- Precedence resolution citing Section 10
- Visual display of conflict → resolution chain

### Scenario 3: Negation & Quantifier Scope
**Input:** Scope-sensitive obligation
```
No employee shall disclose any confidential information to third parties.
```

**Demonstrates:**
- ScopeOperator<NegationOp> with domain over VP
- ScopeOperator<QuantifierOp> ("any" → Universal quantifier)
- Scope interaction visualization (negation outscopes quantifier)
- Correct semantic reading: ∀x. ¬disclose(employee, x)

### Scenario 4: Clause Hierarchy & Coordination
**Input:** Complex clause structure
```
If the Seller fails to deliver, and the Buyer provides notice,
then the Seller shall refund the purchase price, or, at the Buyer's option,
redeliver the goods within 10 days.
```

**Demonstrates:**
- SpanLink<ClauseRole::Parent/Child/Conjunct/Exception>
- Coordination structure (and/or conjuncts)
- Nested conditions with visual tree display
- Alternative obligation branches

### Scenario 5: Obligation Equivalence Detection
**Input:** Two semantically equivalent obligations from different sections
```
Section 5.1: The Vendor must deliver within 30 days of purchase.
Section 8.3: Products shall be shipped within one month following the order date.
```

**Demonstrates:**
- Semantic role normalization (Vendor=Agent, deliver/ship=same action)
- Temporal equivalence (30 days ≈ one month)
- ObligationEquivalence with similarity score
- Duplicate/redundancy flagging

---

## Implementation Gates

### Gate 0: Demo Infrastructure Setup
**Effort:** S (1 day)
**Status:** Partial - UI Complete, WASM Blocked

**Deliverables:**
- [x] New HTML file: `web/contract-intelligence-demo.html`
- [x] Basic UI shell with scenario selector tabs (5 tabs)
- [x] Text input area + analysis output panel
- [x] Demo scenarios: `web/demo-scenarios.js`
- [ ] WASM bindings for contract intelligence types (BLOCKED - see Learning)

**Success Criteria:**
- ✅ Demo page loads with sample scenarios
- ⚠️ WASM bindings blocked by missing document-level APIs

**Learning (2025-12-31):**
The WASM bindings cannot be implemented yet because:
1. **Missing API:** `LayeredDocument` has no `query_all<T>()` method for document-level attributes
2. **Types exist but aren't queryable:** SpanLink, ScopeOperator, Conflict types are defined but the document resolver infrastructure to store/query them isn't integrated
3. **DocumentResolver trait exists** in plans but `run_document_resolver()` isn't implemented

**Options:**
- **Option A:** Build document-level query infrastructure first (adds M-size prerequisite)
- **Option B:** Use existing line-level APIs that DO work (ObligationPhrase, ContractKeyword, etc.)
- **Option C:** Mock data in UI for visualization development, connect real APIs later

**Decision:** Proceed with Option B+C - demo existing line-level features while developing visualization, then connect new APIs when available.

### Gate 0.5: Document Query Infrastructure (NEW - PREREQUISITE)
**Effort:** M (2-3 days)
**Status:** ✅ Complete (711+ tests passing)

**Exploration Findings (2025-12-31):**
1. **Types exist:** SpanLink<R>, ScopeOperator<O>, DocSpan, Scored<T>, Conflict
2. **Line-level query works:** `line.find(&x::attr::<T>())` via TypeId-indexed storage
3. **Function-based pattern exists:** `ClauseLinkResolver::resolve(&doc)` returns ephemeral `Vec<ClauseLink>`
4. **Missing:** Persistent document-level storage, trait interface, query method

**Implementation Approach:**
The simplest path is to extend `LayeredDocument` with document-level attribute storage following the line-level pattern:

```rust
// In layered-nlp-document/src/document.rs
pub struct LayeredDocument {
    lines: Vec<LLLine>,
    line_to_source: Vec<usize>,
    original_text: String,
    doc_attrs: DocAttrStore,  // NEW: document-level attributes
}

// Simple TypeId-indexed storage (mirrors line-level pattern)
struct DocAttrStore {
    attrs: HashMap<TypeId, Vec<Box<dyn Any + Send + Sync>>>,
}

impl LayeredDocument {
    // Store document-level attribute
    pub fn add_doc_attr<T: 'static + Send + Sync>(&mut self, attr: T) { ... }

    // Query all attributes of type T
    pub fn query_doc<T: 'static>(&self) -> Vec<&T> { ... }
}
```

**Deliverables:**

**Step 1: DocAttrStore** (layered-nlp-document) ✅
- [x] Add `DocAttrStore` struct with TypeId-indexed HashMap
- [x] Add `add_doc_attr<T>()` method to LayeredDocument
- [x] Add `query_doc<T>()` method to LayeredDocument
- [x] Write tests for storage/retrieval (5 tests)

**Step 2: DocumentResolver Trait** (layered-nlp-document) ✅
- [x] Define `DocumentResolver` trait with `run(&self, doc: &LayeredDocument) -> Vec<T>` signature
- [x] Add `run_document_resolver()` method that stores results in DocAttrStore
- [x] Write trait tests (2 tests)
- [x] Export in crate public API

**Step 3: Connect ConflictDetector** (layered-contracts) ✅
- [x] Implement `DocumentResolver for ConflictDetector`
- [x] Wrap existing `detect_in_document()` implementation
- [x] Add integration tests (2 tests)

**Step 4: WASM Bindings** (layered-nlp-demo-wasm) ✅
- [x] Add `detect_conflicts(text: &str) -> JsValue` using new API
- [x] Create WasmConflict and ConflictAnalysisResult structs
- [x] Serialize Conflict results to JavaScript
- [x] Add 5 WASM tests

**Success Criteria:**
- `cargo test -p layered-nlp-document` passes with new storage tests
- `doc.run_document_resolver(&ConflictDetector::new()).query_doc::<Scored<Conflict>>()` returns conflicts
- WASM `detect_conflicts()` function works in browser

### Gate 1: SpanLink Visualization
**Effort:** M (2 days)
**Status:** ✅ Complete

**Deliverables:**
- [x] WASM `get_span_links(text)` function returning ClauseLink relationships
- [x] Visual representation of SpanLink<R> relations with curved SVG arrows
- [x] Arrow/line drawing between linked spans (same-line and cross-line)
- [x] Role labels (Parent, Child, Conjunct, Exception)
- [x] Anchor/target highlighting on render
- [x] Legend for relation types with color coding
- [x] 7 WASM tests for span link extraction

**Success Criteria:**
- ✅ Clause relationships displayed as colored arrows
- ✅ Clause hierarchy visible in Clause Hierarchy tab
- ✅ Legend shows Parent (blue), Child (green), Conjunct (purple), Exception (amber)

### Gate 2: ScopeOperator Visualization
**Effort:** M (2 days)
**Status:** ✅ Complete

**Deliverables:**
- [x] WASM `extract_scope_operators(text)` function (13 tests)
- [x] Scope domain highlighting (shaded regions by type)
- [x] Trigger word highlighting (bold/underlined/colored)
- [x] Scope type indicators:
  - Negation = red
  - Universal quantifier = blue
  - Existential quantifier = green
  - Negative quantifier = purple
- [x] Legend and summary panel
- [x] Confidence score display

**Success Criteria:**
- ✅ "No employee shall" shows negation scope with red shading
- ✅ "Each party must" shows universal quantifier with blue shading
- ✅ Trigger words highlighted with dimension-specific colors

### Gate 3: Conflict Detection UI
**Effort:** M (2 days)
**Status:** ✅ Complete

**Deliverables:**
- [x] Conflict panel listing detected conflicts as cards
- [x] Conflict type badges (Modal=yellow, Temporal=blue, Party=pink, Scope=purple)
- [x] Both obligation spans shown with extracted text
- [x] Risk level indicator (Critical/High/Medium/Low based on confidence)
- [x] Summary panel with counts by type and risk level
- [x] Legend explaining conflict types
- [x] Sample contract with modal conflict scenario

**Success Criteria:**
- ✅ Shall vs may conflict detected and displayed
- ✅ Conflict cards show type, risk, explanation, and both spans
- ✅ Summary shows breakdown by type and risk level

### Gate 4: Interactive Exploration
**Effort:** M (2 days)
**Status:** ✅ Complete

**Deliverables:**
- [x] Click-to-focus any span for details panel
- [x] Tooltip system showing position and confidence on hover
- [x] Details panel with full metadata for selected items
- [x] Export analysis as JSON for all 5 tabs
- [x] Global result storage for export functionality
- [x] Clickable spans with hover/selected states

**Success Criteria:**
- ✅ Clicking any result shows details in side panel
- ✅ Confidence scores visible on hover as tooltips
- ✅ JSON export contains timestamped analysis with all results

### Gate 5: Documentation & Polish
**Effort:** S (1 day)
**Status:** ✅ Complete

**Deliverables:**
- [x] Help modal with feature descriptions and usage instructions
- [x] Updated header with tagline and GitHub link
- [x] Comprehensive sample contracts for all 5 scenarios
- [x] WASM loading overlay with spinner and error recovery
- [x] Enhanced error handling with suggestions
- [x] Mobile-responsive layout (768px/480px breakpoints)
- [x] Footer with version info and links

**Success Criteria:**
- ✅ New user can understand demo via Help modal
- ✅ All 5 scenarios have realistic sample contracts
- ✅ Loading states and error handling provide clear feedback

---

## Technical Notes

### WASM Bindings Needed

```rust
// New exports for layered-nlp-demo-wasm
#[wasm_bindgen]
pub fn analyze_semantic_roles(text: &str) -> JsValue;

#[wasm_bindgen]
pub fn detect_conflicts(text: &str) -> JsValue;

#[wasm_bindgen]
pub fn extract_scope_operators(text: &str) -> JsValue;

#[wasm_bindgen]
pub fn get_span_links(text: &str) -> JsValue;
```

### JSON Schema for UI

```typescript
interface SpanLinkDisplay {
  anchor: { start: number; end: number; text: string };
  target: { start: number; end: number; text: string };
  role: string;
  score: number;
}

interface ScopeOperatorDisplay {
  dimension: "Negation" | "Quantifier" | "Precedence" | "Deictic";
  trigger: { start: number; end: number; text: string };
  domain: { start: number; end: number; text: string };
  payload: object;
}

interface ConflictDisplay {
  type: "Modal" | "Temporal" | "Party" | "ScopeOverlap";
  left: ObligationDisplay;
  right: ObligationDisplay;
  risk: "Critical" | "High" | "Medium" | "Low";
  resolution?: { resolved_by: string; source_span: SpanDisplay };
}
```

---

## Success Metrics

1. **Clarity:** Non-technical user understands what contract intelligence adds within 2 minutes
2. **Interactivity:** All extractions explorable via click/hover
3. **Completeness:** All 5 scenarios demonstrate distinct contract intelligence capabilities
4. **Performance:** Analysis completes in <2s for typical contracts
5. **Shareability:** Demo links work for sharing specific analyses

---

## Future Extensions (Post-Demo)

- [ ] Side-by-side comparison view for two contract versions
- [ ] Obligation graph visualization (force-directed layout)
- [ ] Export to legal document format (annotations as comments)
- [ ] Integration with M5 (metalinguistic) when implemented
- [ ] Integration with M6 (PP attachment) when implemented

---

*This demo will serve as the canonical showcase of contract intelligence architectural improvements.*
