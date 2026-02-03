# Modal Prohibitions

Description: Fixtures that test prohibitions triggered by modal phrases like "shall not" and "must not" in a single line or sentence.

References:
- https://en.wikipedia.org/wiki/Deontic_modality
- https://en.wikipedia.org/wiki/Modal_verb

Examples:
- "The Tenant shall not disclose Confidential Information to third parties." (modal=shall not, action=disclose)

Edge cases:
- Negation scope that crosses conjunctions ("shall not disclose or use").
- Mixed modal/negation forms ("may not" vs "shall not").
- Clauses with exceptions ("shall not disclose, except as required...").

Difficulty:
- Capturing prohibition polarity without breaking the action span.
