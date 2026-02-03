# Temporal Obligations

Description: Fixtures that test obligations with explicit timing phrases (e.g., "within 30 days") in a single line or sentence.

References:
- https://en.wikipedia.org/wiki/Deontic_modality
- https://en.wikipedia.org/wiki/Temporal_expression

Examples:
- "The Company shall pay the invoice within thirty (30) days." (modal=shall, action=pay)

Edge cases:
- Multiple timing phrases in one sentence ("within 30 days and no later than... ").
- Relative deadlines that rely on triggers ("within 10 days after notice").
- Ambiguous temporal modifiers that attach to the wrong verb.

Difficulty:
- Keeping the obligation span stable while timing phrases vary in format.
