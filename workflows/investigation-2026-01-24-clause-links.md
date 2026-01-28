# Investigation Report (2026-01-24): Clause Links

Focus: Systematic gaps in clause link emission for list links, cross-reference links, and relative links.

## Cluster: List link emission (ListItem/ListContainer)
Symptoms:
- List item links not emitted across list marker variants.
- Container links target mismatch or point at combined list spans.

Fixtures:
- document/clause-links/list-items-basic.nlp
- document/clause-links/list-items-numbered.nlp
- document/clause-links/list-items-roman.nlp
- document/clause-links/list-items-numbered-paren.nlp
- document/clause-links/list-items-marker-included.nlp
- document/clause-links/list-items-multiline.nlp
- document/clause-links/list-items-cross-paragraph.nlp
- document/clause-links/list-items-mixed-markers.nlp
- document/clause-links/list-items-sentence-container.nlp
- document/clause-links/list-items-semicolons.nlp
- document/clause-links/list-items-next-sentence.nlp
- document/clause-links/list-items-semicolons-coordination.nlp
- document/clause-links/list-items-coordination.nlp

Likely cause:
- Unit mismatch in ClauseLinkResolver list detection: ListMarker ranges are byte positions
  from LLLineFind.range(), but DocSpan uses token indices. Comparisons like
  marker_start >= clause_start_token are invalid, so list items are often missed.
- Span extraction in runner uses DocSpan token indices as byte offsets
  (span_text_for_docspan), which drops or mislabels links even if they are emitted.
- Secondary: ClauseResolver spans often exclude list markers, and the list detector
  only checks markers within/after the clause start.

Suggested fix:
- Convert list marker detection to use token index ranges (e.g., LLLine::query or
  map byte positions to token indices using LLLine tokens) before comparing to DocSpan.
- Update span extraction in runner to convert DocSpan token indices to byte offsets
  using LLLine token positions.
- Consider allowing markers immediately before clause start (start - 1/2 tokens).

Next fixture:
- list-items-basic.nlp is a minimal repro once span extraction is fixed.

## Cluster: Cross-reference links (ClauseRole::CrossReference)
Symptoms:
- Clause cross-reference links not emitted for Section/Exhibit/Schedule/Appendix references.
- Multiple cross-reference links from a single clause not emitted.
- Punctuation-trailing references ignored.

Fixtures:
- document/clause-links/cross-reference-basic.nlp
- document/clause-links/cross-reference-subject-to.nlp
- document/clause-links/cross-reference-punctuation.nlp
- document/clause-links/cross-reference-multiple.nlp
- document/clause-links/cross-reference-schedule-appendix.nlp

Likely cause:
- Unit mismatch in ClauseLinkResolver cross-reference detection: SectionReference
  ranges from LLLineFind.range() are byte positions but are treated as token indices
  for DocSpan construction and comparisons.
- Runner span extraction uses DocSpan token indices as byte offsets, dropping
  emitted links when anchor/target text cannot be extracted.

Suggested fix:
- Use token ranges for SectionReference (LLLine::query) or convert byte ranges to
  token indices before creating DocSpan and checking containment.
- Fix span_text_for_docspan to map DocSpan token indices to byte offsets using
  LLLine tokens for the relevant line.
- Re-check multi-reference parsing (SectionReferenceResolver already emits multiple
  references for a single clause).

Next fixture:
- cross-reference-basic.nlp is the minimal repro.

## Cluster: Relative links (ClauseRole::Relative)
Symptoms:
- Relative clause links not emitted for explicit "who" clauses.
- Zero-relative clauses not linked to head nouns.

Fixtures:
- document/clause-links/relative-clause-basic.nlp
- document/clause-links/relative-clause-zero.nlp

Likely cause:
- No resolver currently emits ClauseRole::Relative links. RelativeClauseDetector
  exists but is not wired into ClauseLinkResolver or a document-level resolver.

Suggested fix:
- Add a RelativeClauseLinkResolver (or extend ClauseLinkResolver) to detect
  relative clauses using RelativeClauseDetector and emit ClauseLink role Relative.
- Ensure pipeline and runner include this resolver.

Next fixture:
- relative-clause-basic.nlp is the minimal repro.

## Cross-cutting note: span extraction
The fixture runner converts DocSpan token indices into byte slices directly in
span_text_for_docspan(), which is invalid for document-level spans. This causes
link extraction to silently drop anchors/targets and makes emitted links look
missing. Fixing span extraction should precede any expansion work so that
failures reflect true resolver gaps.
