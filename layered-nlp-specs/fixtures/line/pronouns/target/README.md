# Pronoun Targets

Description: Fixtures that test pronoun target resolution (coreference), where a pronoun like "it" or "they" should link back to the correct earlier entity in the same line or sentence context.

References:
- https://en.wikipedia.org/wiki/Coreference
- https://en.wikipedia.org/wiki/Pronoun

Examples:
- "The Tenant shall maintain the premises. It shall keep the premises in good order." (It -> Tenant)

Edge cases:
- Multiple possible antecedents ("Tenant" vs "Landlord") near the pronoun.
- Pronouns that do not refer to a real entity (dummy "it" like "It is agreed that...").
- Distance across sentence boundaries where the target is less obvious.

Difficulty:
- Picking the correct target when there are competing candidates, and enforcing basic agreement (number/type) without overfitting to simple patterns.
