# Pronoun Ambiguity (Document)

Description: Fixtures that test pronoun recognition when multiple antecedents are present.

References:
- https://en.wikipedia.org/wiki/Coreference
- https://en.wikipedia.org/wiki/Pronoun

Examples:
- "The Tenant and the Landlord met. They shall cooperate." (They -> ambiguous)

Edge cases:
- Multiple entities of the same grammatical number.
- Pronouns at long distance from candidates.
- Pronouns that refer to groups vs. single entities.

Difficulty:
- Recognizing the pronoun type reliably even when target resolution is ambiguous.
