# Permissions vs Prohibitions (Document)

Description: Fixtures that test how permissions and prohibitions are distinguished across multiple sentences or paragraphs.

References:
- https://en.wikipedia.org/wiki/Deontic_modality
- https://en.wikipedia.org/wiki/Modal_verb

Examples:
- "The Tenant may access the records." vs "The Tenant shall not access the records."

Edge cases:
- Negated permissions ("may not") that behave like prohibitions.
- Multiple modals acting on the same verb in adjacent sentences.
- Cross-paragraph context that could bias modal interpretation.
- Exception clauses that should not flip prohibition polarity ("except as required by law").

Difficulty:
- Preserving correct polarity when modals are close together and actions repeat.
