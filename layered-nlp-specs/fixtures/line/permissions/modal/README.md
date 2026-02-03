# Modal Permissions

Description: Fixtures that test permissions triggered by modal verbs like "may" in a single line or sentence.

References:
- https://en.wikipedia.org/wiki/Deontic_modality
- https://en.wikipedia.org/wiki/Modal_verb

Examples:
- "The Tenant may enter the premises during business hours." (modal=may, action=enter)

Edge cases:
- Permissions with embedded conditions ("may enter if...").
- Multiword actions ("may disclose confidential information").
- Negative permissions that are actually prohibitions ("may not disclose").

Difficulty:
- Distinguishing permission vs prohibition and keeping action spans stable.
