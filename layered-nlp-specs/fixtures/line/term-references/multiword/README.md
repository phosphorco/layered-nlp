# Term References (Multiword)

Description: Fixtures that test term-reference detection for multiword defined terms, where the reference should span multiple tokens.

References:
- https://en.wikipedia.org/wiki/Defined_term
- https://en.wikipedia.org/wiki/Multiword_expression

Examples:
- "Service Levels" (defined) → "Service Levels" (reference)

Edge cases:
- Hyphenated or punctuated multiword terms ("Service-Level Agreement").
- Mixed case in the reference ("service levels").
- Articles preceding the reference ("the Service Levels").

Difficulty:
- Matching a multiword span without overmatching adjacent words or skipping required whitespace.
