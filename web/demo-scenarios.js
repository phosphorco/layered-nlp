export const DEMO_SCENARIOS = {
  semanticRoles: {
    name: "Obligation Analysis",
    description: "Complex obligation with multiple participants and conditions",
    contract: `Unless the Buyer provides written notice within 5 business days of receiving goods,
the Seller may, in its sole discretion, refuse to accept returns, unless the goods
are defective.`
  },

  conflictDetection: {
    name: "Conflict Detection",
    description: "Two conflicting provisions with precedence resolution",
    contract: `ARTICLE 3. DELIVERY

Section 3.1 Delivery Terms
The Company shall deliver goods within 30 days of the order date.

Section 3.2 Delivery Exceptions
The Company may deliver goods within 60 days if supply chain disruptions occur.

ARTICLE 10. GENERAL PROVISIONS

Section 10.1 Precedence
Notwithstanding anything to the contrary in this Agreement, Section 3.1 shall govern all delivery timelines.`
  },

  scopeOperators: {
    name: "Scope Operators",
    description: "Negation and quantifier scope visualization",
    contract: `ARTICLE 5. CONFIDENTIALITY

Section 5.1 Non-Disclosure
No employee shall disclose any confidential information to third parties.

Section 5.2 Exceptions
Nothing in this Section 5.1 shall prevent an employee from disclosing information required by law.`
  },

  clauseHierarchy: {
    name: "Clause Hierarchy",
    description: "Nested conditions and alternatives",
    contract: `ARTICLE 7. REMEDIES

Section 7.1 Seller's Remedies
If the Seller fails to deliver, and the Buyer provides written notice within 10 days,
then the Seller shall refund the full purchase price, or, at the Buyer's option,
redeliver the goods within 15 days, provided that the Buyer has not already obtained
substitute goods from another supplier.`
  },

  equivalence: {
    name: "Obligation Equivalence",
    description: "Semantically equivalent obligations from different sections",
    contract: `ARTICLE 5. DELIVERY

Section 5.1 Vendor Obligations
The Vendor must deliver Products within 30 days of purchase.

ARTICLE 8. FULFILLMENT

Section 8.3 Shipping Timeline
Products shall be shipped within one month following the order date.`
  }
};
