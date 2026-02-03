# Temporal Deadlines (Document)

Description: Fixtures that test obligations with timing phrases across multiple paragraphs.

References:
- https://en.wikipedia.org/wiki/Temporal_expression
- https://en.wikipedia.org/wiki/Deontic_modality

Examples:
- "The Company shall deliver the report within ten (10) business days." (modal=shall, action=deliver)

Edge cases:
- Deadlines that depend on triggers ("within 10 days after notice").
- Multiple timing phrases in the same paragraph.
- Timing phrases separated from the modal by intervening clauses.

Difficulty:
- Preserving action span extraction when temporal modifiers vary widely.
