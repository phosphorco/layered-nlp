# Cross-Paragraph

Description: Context that carries across paragraphs (discourse-level coreference).

References:
- https://en.wikipedia.org/wiki/Discourse
- https://en.wikipedia.org/wiki/Coreference

Examples:
- Paragraph 1 defines "Tenant"; paragraph 2 uses "It shall pay".
- Paragraph 1 defines "Property"; paragraph 2 refers to "Property".

Edge cases:
- Multiple entities introduced in prior sections with similar names.

Difficulty:
- Maintaining the right context window without leaking entities across unrelated sections.
