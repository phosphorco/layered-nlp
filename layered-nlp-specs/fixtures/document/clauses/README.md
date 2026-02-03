# Clauses (Document)

Description: Multi-sentence or multi-paragraph clause recognition, focused on clause type boundaries across sentence punctuation.

References:
- https://en.wikipedia.org/wiki/Clause
- https://en.wikipedia.org/wiki/Conditional_sentence

Examples:
- "If payment is late, the Company shall charge a fee. The Tenant shall pay interest."

Edge cases:
- Sentence boundaries after a condition clause.
- Exception clauses introduced by "unless" in the same paragraph.

Difficulty:
- Clause detection is line-level, so sentence boundaries can leak condition state into later clauses.
