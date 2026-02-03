# Term References (Cross-Paragraph)

Description: Fixtures that test term-reference detection when a term is defined in one paragraph and referenced in a later paragraph.

References:
- https://en.wikipedia.org/wiki/Defined_term
- https://en.wikipedia.org/wiki/Anaphora_(linguistics)

Examples:
- Paragraph 1 defines "Service Levels"; paragraph 2 references Service Levels.

Edge cases:
- Multiword terms with mixed casing in later paragraphs.
- References that include articles ("the Service Levels").
- Intervening paragraphs with unrelated terms.

Difficulty:
- Maintaining links across paragraph boundaries without overmatching common words.
