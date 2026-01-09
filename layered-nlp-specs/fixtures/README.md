# Fixture Guide

These fixtures are small, readable tests for the layered NLP pipeline. They are meant for
technical contributors who know contract language but may not be deep in NLP terminology.

## Layout

- `line/`: Single-line or single-sentence fixtures (no cross-paragraph context).
- `document/`: Multi-sentence or multi-paragraph fixtures that need wider context.
- `integration/`: End-to-end scenarios that exercise multiple capabilities together.
- `expected_failures.toml`: Known gaps we accept for now (update when behavior changes).

## Fixture Format (quick view)

Fixtures use inline spans and expectations:

```
# Test: Simple Obligation Detection

The Tenant «1:shall pay» rent of $2,000 to the Landlord monthly.

> [1]: Obligation(modal=shall)
```

- `«1:... »` marks the span under test.
- `> [1]: ...` declares the expected extraction or label for that span.

## Coverage Group READMEs

Each coverage group folder should include a short README with:

- Description in plain language (what the group is testing).
- References (links to short primers, not paywalled material).
- Examples and edge cases.
- Difficulty note (what makes the cases tricky).

Suggested template:

```
# <Group Name>

Description: ...

References:
- https://example.com

Examples:
- "..."

Edge cases:
- "..."

Difficulty:
- ...
```

## Glossary (plain-language)

- Obligation: a required action (e.g., "shall", "must").
- Permission: an allowed action (e.g., "may", "is permitted to").
- Prohibition: a forbidden action (e.g., "shall not", "must not").
- Defined term: a named label assigned to a concept (e.g., `"Company" means ABC Corp`).
- Term reference: later use of a defined term.
- Coreference: linking a later mention (like "it" or "they") to an earlier entity.
- Semantic role: who does what to whom (agent, patient, beneficiary).

## Contribution Notes

- Keep fixtures minimal: one or two key phenomena per file.
- Use realistic contract phrasing; avoid toy grammar.
- If a fixture is expected to fail, record it in `expected_failures.toml`.
