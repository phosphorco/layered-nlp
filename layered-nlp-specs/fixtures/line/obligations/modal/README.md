# Modal Obligations

Description: Fixtures that test obligation detection triggered by modal verbs like "shall" and "must" in a single line or sentence.

References:
- https://en.wikipedia.org/wiki/Deontic_modality
- https://en.wikipedia.org/wiki/Modal_verb

Examples:
- "The Tenant shall pay rent on the first business day of each month." (modal=shall, bearer=Tenant, action=pay)

Edge cases:
- Multi-word actions ("shall deliver goods") where only the first verb anchors the span.
- Embedded conditions ("shall pay if") that can distort span boundaries.
- Subject nouns with articles or modifiers ("The Primary Tenant") that affect bearer extraction.

Difficulty:
- Correctly separating the modal phrase from the action and obligor in real contract sentences.
