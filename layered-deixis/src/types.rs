//! Core types for deictic reference detection.
//!
//! Deixis refers to words whose meaning depends on context - pronouns (I, you),
//! demonstratives (this, that), temporal references (now, then), spatial references
//! (here, there), and discourse markers (however, therefore).

/// A unified deictic reference detected in text.
///
/// This struct "flattens" various domain-specific deictic types into
/// a normalized representation suitable for cross-domain analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct DeicticReference {
    /// The deictic category (Person, Place, Time, Discourse, Social)
    pub category: DeicticCategory,

    /// Subcategory providing finer classification within the category
    pub subcategory: DeicticSubcategory,

    /// The surface text of the deictic expression
    pub surface_text: String,

    /// Optional resolved antecedent/referent (for anaphoric deixis)
    /// e.g., for "it" -> "the Company"
    pub resolved_referent: Option<ResolvedReferent>,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,

    /// Source that produced this deictic reference
    pub source: DeicticSource,
}

impl DeicticReference {
    /// Create a new DeicticReference with basic fields.
    pub fn new(
        category: DeicticCategory,
        subcategory: DeicticSubcategory,
        surface_text: impl Into<String>,
        source: DeicticSource,
    ) -> Self {
        Self {
            category,
            subcategory,
            surface_text: surface_text.into(),
            resolved_referent: None,
            confidence: 1.0,
            source,
        }
    }

    /// Set the confidence score.
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }

    /// Set the resolved referent.
    pub fn with_referent(mut self, referent: ResolvedReferent) -> Self {
        self.resolved_referent = Some(referent);
        self
    }
}

/// The five major categories of deixis in linguistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeicticCategory {
    /// Person deixis - reference to participants (I, you, we, they, etc.)
    Person,
    /// Place/spatial deixis - reference to location (here, there, this, that)
    Place,
    /// Time/temporal deixis - reference to time (now, then, today, yesterday)
    Time,
    /// Discourse deixis - reference to parts of the discourse (above, herein, this Section)
    Discourse,
    /// Social deixis - reference to social relationships (honorifics, formal/informal).
    /// Note: No resolvers currently implement Social deixis; it's included for completeness
    /// and future domain-specific implementations (e.g., formal legal language detection).
    Social,
}

/// Subcategories provide finer-grained classification within each deictic category.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeicticSubcategory {
    // === Person subcategories ===
    /// First person singular: I, me, my, mine, myself
    PersonFirst,
    /// First person plural: we, us, our, ours, ourselves
    PersonFirstPlural,
    /// Second person: you, your, yours, yourself
    PersonSecond,
    /// Second person plural: you (plural), yourselves
    PersonSecondPlural,
    /// Third person singular: he, she, it, him, her, his, hers, its
    PersonThirdSingular,
    /// Third person plural: they, them, their, theirs, themselves
    PersonThirdPlural,
    /// Relative pronouns: who, whom, whose, which, that
    PersonRelative,

    // === Place subcategories ===
    /// Proximal spatial: here, this (place)
    PlaceProximal,
    /// Distal spatial: there, that (place)
    PlaceDistal,
    /// Other spatial: elsewhere, somewhere, anywhere, nowhere, everywhere
    PlaceOther,

    // === Time subcategories ===
    /// Present temporal: now, currently, presently, today
    TimePresent,
    /// Past temporal: then, yesterday, previously, formerly, earlier
    TimePast,
    /// Future temporal: tomorrow, later, subsequently, hereafter
    TimeFuture,
    /// Event-relative temporal: upon, following, prior to, during
    TimeRelative,
    /// Reference to defined date terms: "the Effective Date", "the Termination Date"
    TimeDefinedTerm,
    /// Duration expressions: "30 days", "five years"
    TimeDuration,
    /// Deadline expressions: "within 30 days", "by December 31"
    TimeDeadline,

    // === Discourse subcategories ===
    /// This-document references: herein, hereof, hereby, hereunder
    DiscourseThisDocument,
    /// Section references: "this Section", "Section 3.1 above"
    DiscourseSectionRef,
    /// Anaphoric (backward-pointing): "the foregoing", "the above", "aforementioned"
    DiscourseAnaphoric,
    /// Cataphoric (forward-pointing): "the following", "as described below"
    DiscourseCataphoric,
    /// Discourse connectives: however, therefore, thus, moreover, furthermore
    DiscourseMarker,

    // === Social subcategories (reserved for future use) ===
    /// Formal register markers (e.g., "hereby", "wherefore" in legal text)
    SocialFormal,
    /// Informal register markers
    SocialInformal,

    /// Catch-all for unclassified deictics
    Other,
}

/// The resolved referent/antecedent for an anaphoric deictic expression.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedReferent {
    /// The text of the resolved referent
    pub text: String,
    /// Token span of the referent (start, end) if in same line/document
    pub span: Option<(usize, usize)>,
    /// Confidence of the resolution
    pub resolution_confidence: f64,
}

impl ResolvedReferent {
    /// Create a new resolved referent.
    pub fn new(text: impl Into<String>, resolution_confidence: f64) -> Self {
        Self {
            text: text.into(),
            span: None,
            resolution_confidence,
        }
    }

    /// Set the token span.
    pub fn with_span(mut self, start: usize, end: usize) -> Self {
        self.span = Some((start, end));
        self
    }
}

/// Indicates the source/origin of a deictic detection.
#[derive(Debug, Clone, PartialEq)]
pub enum DeicticSource {
    /// From a simple word-list match
    WordList {
        /// The pattern/word that matched
        pattern: &'static str,
    },
    /// Mapped from a PronounReference resolver
    PronounResolver,
    /// Mapped from a TemporalExpression resolver
    TemporalResolver,
    /// Mapped from a SectionReference/RelativeReference resolver
    SectionReferenceResolver,
    /// Mapped from POS tag analysis
    POSTag,
    /// Derived/composed from multiple sources
    Derived,
}
