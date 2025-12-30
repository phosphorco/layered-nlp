//! Temporal expression detection for contract text.
//!
//! This module provides `TemporalExpressionResolver` which detects time-related
//! expressions common in contracts:
//!
//! - **Dates**: "December 31, 2024", "the Effective Date"
//! - **Durations**: "thirty (30) days", "six months", "one year"
//! - **Deadlines**: "within 30 days", "no later than December 31"
//! - **Relative times**: "upon termination", "following receipt"

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver, TextTag};

/// A temporal expression detected in contract text.
#[derive(Debug, Clone, PartialEq)]
pub struct TemporalExpression {
    /// The type of temporal expression
    pub temporal_type: TemporalType,
    /// Raw text of the expression
    pub text: String,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
}

/// The type of temporal expression.
#[derive(Debug, Clone, PartialEq)]
pub enum TemporalType {
    /// A specific date: "December 31, 2024", "1/1/2025"
    Date {
        year: Option<u32>,
        month: Option<u8>,
        day: Option<u8>,
    },
    /// A duration/period: "30 days", "six months"
    Duration {
        value: u32,
        unit: DurationUnit,
        /// Written form if present, e.g., "thirty" in "thirty (30) days"
        written_form: Option<String>,
    },
    /// A deadline: "within 30 days", "by December 31"
    Deadline {
        deadline_type: DeadlineType,
        /// The duration or date this deadline references
        reference: Box<TemporalType>,
    },
    /// A defined term reference: "the Effective Date", "the Termination Date"
    DefinedDate {
        term: String,
    },
    /// Relative to an event: "upon termination", "following receipt"
    RelativeTime {
        trigger: String,
        relation: TimeRelation,
    },
}

/// Unit of time for durations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationUnit {
    Days,
    Weeks,
    Months,
    Years,
    BusinessDays,
}

impl DurationUnit {
    fn from_text(text: &str) -> Option<Self> {
        match text.to_lowercase().as_str() {
            "day" | "days" => Some(DurationUnit::Days),
            "week" | "weeks" => Some(DurationUnit::Weeks),
            "month" | "months" => Some(DurationUnit::Months),
            "year" | "years" => Some(DurationUnit::Years),
            _ => None,
        }
    }

    /// Check if text is a business/working days prefix
    fn is_business_prefix(text: &str) -> bool {
        matches!(text.to_lowercase().as_str(), "business" | "working")
    }
}

/// Type of deadline expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineType {
    /// "within 30 days"
    Within,
    /// "by December 31"
    By,
    /// "no later than"
    NoLaterThan,
    /// "before the Effective Date"
    Before,
    /// "after the Termination Date"
    After,
    /// "on or before"
    OnOrBefore,
    /// "promptly following"
    PromptlyFollowing,
}

/// Relation for relative time expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeRelation {
    /// "upon termination"
    Upon,
    /// "following receipt"
    Following,
    /// "prior to closing"
    PriorTo,
    /// "during the term"
    During,
    /// "at the time of"
    AtTimeOf,
}

/// Resolver for detecting temporal expressions in contract text.
#[derive(Debug, Clone)]
pub struct TemporalExpressionResolver {
    /// Confidence for date patterns
    date_confidence: f64,
    /// Confidence for duration patterns
    duration_confidence: f64,
    /// Confidence for deadline patterns
    deadline_confidence: f64,
    /// Confidence for defined date terms
    defined_date_confidence: f64,
}

impl Default for TemporalExpressionResolver {
    fn default() -> Self {
        Self {
            date_confidence: 0.95,
            duration_confidence: 0.90,
            deadline_confidence: 0.85,
            defined_date_confidence: 0.80,
        }
    }
}

impl TemporalExpressionResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a month name to its number (1-12).
    fn parse_month(text: &str) -> Option<u8> {
        match text.to_lowercase().as_str() {
            "january" | "jan" => Some(1),
            "february" | "feb" => Some(2),
            "march" | "mar" => Some(3),
            "april" | "apr" => Some(4),
            "may" => Some(5),
            "june" | "jun" => Some(6),
            "july" | "jul" => Some(7),
            "august" | "aug" => Some(8),
            "september" | "sep" | "sept" => Some(9),
            "october" | "oct" => Some(10),
            "november" | "nov" => Some(11),
            "december" | "dec" => Some(12),
            _ => None,
        }
    }

    /// Parse a written number to its numeric value.
    fn parse_written_number(text: &str) -> Option<u32> {
        match text.to_lowercase().as_str() {
            "one" | "a" | "an" => Some(1),
            "two" => Some(2),
            "three" => Some(3),
            "four" => Some(4),
            "five" => Some(5),
            "six" => Some(6),
            "seven" => Some(7),
            "eight" => Some(8),
            "nine" => Some(9),
            "ten" => Some(10),
            "eleven" => Some(11),
            "twelve" => Some(12),
            "thirteen" => Some(13),
            "fourteen" => Some(14),
            "fifteen" => Some(15),
            "sixteen" => Some(16),
            "seventeen" => Some(17),
            "eighteen" => Some(18),
            "nineteen" => Some(19),
            "twenty" => Some(20),
            "thirty" => Some(30),
            "forty" | "fourty" => Some(40),
            "fifty" => Some(50),
            "sixty" => Some(60),
            "seventy" => Some(70),
            "eighty" => Some(80),
            "ninety" => Some(90),
            "hundred" => Some(100),
            _ => None,
        }
    }

    /// Check if text is a deadline keyword and return its type.
    fn parse_deadline_keyword(text: &str) -> Option<DeadlineType> {
        match text.to_lowercase().as_str() {
            "within" => Some(DeadlineType::Within),
            "by" => Some(DeadlineType::By),
            "before" => Some(DeadlineType::Before),
            "after" => Some(DeadlineType::After),
            _ => None,
        }
    }

    /// Check if text is a relative time keyword and return its relation.
    fn parse_time_relation(text: &str) -> Option<TimeRelation> {
        match text.to_lowercase().as_str() {
            "upon" => Some(TimeRelation::Upon),
            "following" => Some(TimeRelation::Following),
            "during" => Some(TimeRelation::During),
            _ => None,
        }
    }

    /// Try to parse a duration starting from the current position.
    /// Returns (duration_type, final_selection, raw_text) if found.
    fn try_parse_duration(
        &self,
        selection: &LLSelection,
    ) -> Option<(TemporalType, LLSelection, String)> {
        let mut current = selection.clone();
        let mut raw_text = String::new();
        let mut value: Option<u32> = None;
        let mut written_form: Option<String> = None;

        // Try to match a number (written or numeric)
        if let Some((num_sel, (_, num_text))) =
            current.match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            if let Some(num) = Self::parse_written_number(num_text) {
                value = Some(num);
                written_form = Some(num_text.to_string());
                raw_text.push_str(num_text);
                current = num_sel;
            }
        }

        // If no written number, try numeric
        if value.is_none() {
            if let Some((num_sel, (_, num_text))) = current
                .match_first_forwards(&x::all((x::attr_eq(&TextTag::NATN), x::token_text())))
            {
                if let Ok(num) = num_text.parse::<u32>() {
                    value = Some(num);
                    raw_text.push_str(num_text);
                    current = num_sel;
                }
            }
        }

        let value = value?;

        // Skip whitespace
        if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
            raw_text.push(' ');
            current = ws_sel;
        }

        // Check for parenthetical confirmation: "(30)"
        if let Some((paren_sel, _)) = current.match_first_forwards(&x::attr_eq(&'(')) {
            if let Some((num_sel, (_, num_text))) = paren_sel
                .match_first_forwards(&x::all((x::attr_eq(&TextTag::NATN), x::token_text())))
            {
                if let Some((close_sel, _)) = num_sel.match_first_forwards(&x::attr_eq(&')')) {
                    // Verify the parenthetical number matches
                    if let Ok(paren_num) = num_text.parse::<u32>() {
                        if paren_num == value {
                            raw_text.push_str(&format!("({}) ", num_text));
                            current = close_sel;
                            // Skip whitespace after close paren
                            if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace())
                            {
                                current = ws_sel;
                            }
                        }
                    }
                }
            }
        }

        // Now look for the unit (days, months, years, etc.)
        if let Some((unit_sel, (_, unit_text))) =
            current.match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            // Check for "business days" / "working days" (two-token pattern)
            if DurationUnit::is_business_prefix(unit_text) {
                let mut check_sel = unit_sel.clone();
                raw_text.push_str(unit_text);

                // Skip whitespace
                if let Some((ws_sel, _)) = check_sel.match_first_forwards(&x::whitespace()) {
                    raw_text.push(' ');
                    check_sel = ws_sel;
                }

                // Look for "day" or "days"
                if let Some((day_sel, (_, day_text))) = check_sel
                    .match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
                {
                    if day_text.to_lowercase() == "day" || day_text.to_lowercase() == "days" {
                        raw_text.push_str(day_text);
                        return Some((
                            TemporalType::Duration {
                                value,
                                unit: DurationUnit::BusinessDays,
                                written_form,
                            },
                            day_sel,
                            raw_text,
                        ));
                    }
                }
                // If not followed by day/days, fall through (might be "business" used differently)
            }

            if let Some(unit) = DurationUnit::from_text(unit_text) {
                raw_text.push_str(unit_text);
                return Some((
                    TemporalType::Duration {
                        value,
                        unit,
                        written_form,
                    },
                    unit_sel,
                    raw_text,
                ));
            }
        }

        None
    }
}

impl Resolver for TemporalExpressionResolver {
    type Attr = TemporalExpression;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut assignments = Vec::new();

        // Pattern 1: "Month Day, Year" (e.g., "December 31, 2024")
        for (sel, (_, month_text)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            if let Some(month) = Self::parse_month(month_text) {
                let mut current = sel.clone();
                let mut raw_text = month_text.to_string();

                // Skip whitespace
                if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                    current = ws_sel;
                } else {
                    continue;
                }

                // Match day number
                if let Some((day_sel, (_, day_text))) = current
                    .match_first_forwards(&x::all((x::attr_eq(&TextTag::NATN), x::token_text())))
                {
                    let day: u8 = match day_text.parse() {
                        Ok(d) if d >= 1 && d <= 31 => d,
                        _ => continue,
                    };
                    raw_text.push(' ');
                    raw_text.push_str(day_text);
                    current = day_sel;

                    // Optional comma
                    if let Some((comma_sel, _)) = current.match_first_forwards(&x::attr_eq(&',')) {
                        raw_text.push(',');
                        current = comma_sel;
                    }

                    // Skip whitespace
                    if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                        current = ws_sel;
                    }

                    // Match year (optional)
                    let mut year: Option<u32> = None;
                    let mut final_sel = current.clone();
                    if let Some((year_sel, (_, year_text))) = current
                        .match_first_forwards(&x::all((x::attr_eq(&TextTag::NATN), x::token_text())))
                    {
                        if let Ok(y) = year_text.parse::<u32>() {
                            if y >= 1900 && y <= 2100 {
                                year = Some(y);
                                raw_text.push(' ');
                                raw_text.push_str(year_text);
                                final_sel = year_sel;
                            }
                        }
                    }

                    assignments.push(final_sel.finish_with_attr(TemporalExpression {
                        temporal_type: TemporalType::Date {
                            year,
                            month: Some(month),
                            day: Some(day),
                        },
                        text: raw_text,
                        confidence: self.date_confidence,
                    }));
                }
            }
        }

        // Pattern 2: Deadline + Duration (e.g., "within thirty (30) days")
        for (sel, (_, keyword_text)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            if let Some(deadline_type) = Self::parse_deadline_keyword(keyword_text) {
                let mut current = sel.clone();
                let mut raw_text = keyword_text.to_string();

                // Skip whitespace
                if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                    raw_text.push(' ');
                    current = ws_sel;
                } else {
                    continue;
                }

                // Try to parse a duration
                if let Some((duration_type, final_sel, duration_text)) =
                    self.try_parse_duration(&current)
                {
                    raw_text.push_str(&duration_text);

                    assignments.push(final_sel.finish_with_attr(TemporalExpression {
                        temporal_type: TemporalType::Deadline {
                            deadline_type,
                            reference: Box::new(duration_type),
                        },
                        text: raw_text,
                        confidence: self.deadline_confidence,
                    }));
                }
            }
        }

        // Pattern 2b: Multi-word deadline phrases (e.g., "no later than", "on or before")
        for (sel, (_, first_word)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            let lower = first_word.to_lowercase();
            let deadline_type = match lower.as_str() {
                "no" => {
                    // "no later than"
                    let mut check = sel.clone();
                    let mut matched = false;
                    if let Some((ws, _)) = check.match_first_forwards(&x::whitespace()) {
                        if let Some((later_sel, (_, later))) = ws.match_first_forwards(
                            &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                        ) {
                            if later.to_lowercase() == "later" {
                                if let Some((ws2, _)) =
                                    later_sel.match_first_forwards(&x::whitespace())
                                {
                                    if let Some((than_sel, (_, than))) = ws2.match_first_forwards(
                                        &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                                    ) {
                                        if than.to_lowercase() == "than" {
                                            check = than_sel;
                                            matched = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if matched {
                        Some((DeadlineType::NoLaterThan, check, "no later than"))
                    } else {
                        None
                    }
                }
                "on" => {
                    // "on or before"
                    let mut check = sel.clone();
                    let mut matched = false;
                    if let Some((ws, _)) = check.match_first_forwards(&x::whitespace()) {
                        if let Some((or_sel, (_, or_text))) = ws.match_first_forwards(
                            &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                        ) {
                            if or_text.to_lowercase() == "or" {
                                if let Some((ws2, _)) =
                                    or_sel.match_first_forwards(&x::whitespace())
                                {
                                    if let Some((before_sel, (_, before))) = ws2
                                        .match_first_forwards(
                                            &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                                        )
                                    {
                                        if before.to_lowercase() == "before" {
                                            check = before_sel;
                                            matched = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if matched {
                        Some((DeadlineType::OnOrBefore, check, "on or before"))
                    } else {
                        None
                    }
                }
                "promptly" => {
                    // "promptly following"
                    let mut check = sel.clone();
                    let mut matched = false;
                    if let Some((ws, _)) = check.match_first_forwards(&x::whitespace()) {
                        if let Some((following_sel, (_, following))) = ws.match_first_forwards(
                            &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                        ) {
                            if following.to_lowercase() == "following" {
                                check = following_sel;
                                matched = true;
                            }
                        }
                    }
                    if matched {
                        Some((DeadlineType::PromptlyFollowing, check, "promptly following"))
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some((dtype, phrase_end_sel, phrase_text)) = deadline_type {
                let mut current = phrase_end_sel;
                let mut raw_text = phrase_text.to_string();

                // Skip whitespace
                if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                    raw_text.push(' ');
                    current = ws_sel;
                } else {
                    continue;
                }

                // Try to parse a duration or date
                if let Some((duration_type, final_sel, duration_text)) =
                    self.try_parse_duration(&current)
                {
                    raw_text.push_str(&duration_text);
                    assignments.push(final_sel.finish_with_attr(TemporalExpression {
                        temporal_type: TemporalType::Deadline {
                            deadline_type: dtype,
                            reference: Box::new(duration_type),
                        },
                        text: raw_text,
                        confidence: self.deadline_confidence,
                    }));
                }
            }
        }

        // Pattern 3: Standalone durations with written numbers (e.g., "thirty (30) days", "five years")
        for (sel, (_, first_text)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            // Only match if it's a written number
            if let Some(value) = Self::parse_written_number(first_text) {
                let mut current = sel.clone();
                let mut raw_text = first_text.to_string();
                let written_form = Some(first_text.to_string());

                // Skip whitespace
                if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                    raw_text.push(' ');
                    current = ws_sel;
                } else {
                    continue;
                }

                // Check for parenthetical confirmation: "(30)"
                if let Some((paren_sel, _)) = current.match_first_forwards(&x::attr_eq(&'(')) {
                    if let Some((num_sel, (_, num_text))) = paren_sel
                        .match_first_forwards(&x::all((x::attr_eq(&TextTag::NATN), x::token_text())))
                    {
                        if let Some((close_sel, _)) = num_sel.match_first_forwards(&x::attr_eq(&')'))
                        {
                            if let Ok(paren_num) = num_text.parse::<u32>() {
                                if paren_num == value {
                                    raw_text.push_str(&format!("({}) ", num_text));
                                    current = close_sel;
                                    if let Some((ws_sel, _)) =
                                        current.match_first_forwards(&x::whitespace())
                                    {
                                        current = ws_sel;
                                    }
                                }
                            }
                        }
                    }
                }

                // Match the unit (including "business days" / "working days")
                if let Some((unit_sel, (_, unit_text))) = current
                    .match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
                {
                    // Check for "business days" / "working days"
                    if DurationUnit::is_business_prefix(unit_text) {
                        let mut check_sel = unit_sel.clone();
                        let mut check_text = raw_text.clone();
                        check_text.push_str(unit_text);

                        if let Some((ws_sel, _)) = check_sel.match_first_forwards(&x::whitespace())
                        {
                            check_text.push(' ');
                            check_sel = ws_sel;
                        }

                        if let Some((day_sel, (_, day_text))) = check_sel.match_first_forwards(
                            &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                        ) {
                            let lower = day_text.to_lowercase();
                            if lower == "day" || lower == "days" {
                                check_text.push_str(day_text);
                                assignments.push(day_sel.finish_with_attr(TemporalExpression {
                                    temporal_type: TemporalType::Duration {
                                        value,
                                        unit: DurationUnit::BusinessDays,
                                        written_form,
                                    },
                                    text: check_text,
                                    confidence: self.duration_confidence,
                                }));
                                continue;
                            }
                        }
                    }

                    if let Some(unit) = DurationUnit::from_text(unit_text) {
                        raw_text.push_str(unit_text);
                        assignments.push(unit_sel.finish_with_attr(TemporalExpression {
                            temporal_type: TemporalType::Duration {
                                value,
                                unit,
                                written_form,
                            },
                            text: raw_text,
                            confidence: self.duration_confidence,
                        }));
                    }
                }
            }
        }

        // Pattern 4: Standalone numeric durations (e.g., "30 days", "5 business days")
        for (sel, (_, num_text)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::NATN), x::token_text())))
        {
            if let Ok(value) = num_text.parse::<u32>() {
                let mut current = sel.clone();
                let mut raw_text = num_text.to_string();

                // Skip whitespace
                if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                    raw_text.push(' ');
                    current = ws_sel;
                } else {
                    continue;
                }

                // Match the unit (including "business days" / "working days")
                if let Some((unit_sel, (_, unit_text))) = current
                    .match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
                {
                    // Check for "business days" / "working days"
                    if DurationUnit::is_business_prefix(unit_text) {
                        let mut check_sel = unit_sel.clone();
                        let mut check_text = raw_text.clone();
                        check_text.push_str(unit_text);

                        if let Some((ws_sel, _)) = check_sel.match_first_forwards(&x::whitespace())
                        {
                            check_text.push(' ');
                            check_sel = ws_sel;
                        }

                        if let Some((day_sel, (_, day_text))) = check_sel.match_first_forwards(
                            &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                        ) {
                            let lower = day_text.to_lowercase();
                            if lower == "day" || lower == "days" {
                                check_text.push_str(day_text);
                                assignments.push(day_sel.finish_with_attr(TemporalExpression {
                                    temporal_type: TemporalType::Duration {
                                        value,
                                        unit: DurationUnit::BusinessDays,
                                        written_form: None,
                                    },
                                    text: check_text,
                                    confidence: self.duration_confidence,
                                }));
                                continue;
                            }
                        }
                    }

                    if let Some(unit) = DurationUnit::from_text(unit_text) {
                        raw_text.push_str(unit_text);
                        assignments.push(unit_sel.finish_with_attr(TemporalExpression {
                            temporal_type: TemporalType::Duration {
                                value,
                                unit,
                                written_form: None,
                            },
                            text: raw_text,
                            confidence: self.duration_confidence,
                        }));
                    }
                }
            }
        }

        // Pattern 5: "the [Something] Date" (defined date terms)
        for (sel, (_, the_text)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            if the_text.to_lowercase() != "the" {
                continue;
            }

            let mut current = sel.clone();
            let mut raw_text = the_text.to_string();
            let mut term_parts = Vec::new();

            // Skip whitespace
            if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                current = ws_sel;
            } else {
                continue;
            }

            // Collect capitalized words until we hit "Date"
            loop {
                if let Some((word_sel, (_, word_text))) = current
                    .match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
                {
                    if word_text.to_lowercase() == "date" {
                        if !term_parts.is_empty() {
                            raw_text.push(' ');
                            raw_text.push_str(&term_parts.join(" "));
                            raw_text.push(' ');
                            raw_text.push_str(word_text);

                            assignments.push(word_sel.finish_with_attr(TemporalExpression {
                                temporal_type: TemporalType::DefinedDate {
                                    term: format!("the {} Date", term_parts.join(" ")),
                                },
                                text: raw_text.clone(),
                                confidence: self.defined_date_confidence,
                            }));
                        }
                        break;
                    } else if word_text.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                        term_parts.push(word_text.to_string());
                        current = word_sel;

                        // Skip whitespace
                        if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                            current = ws_sel;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        // Pattern 6: Relative time expressions (e.g., "upon termination", "following receipt")
        for (sel, (_, keyword_text)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            if let Some(relation) = Self::parse_time_relation(keyword_text) {
                let mut current = sel.clone();
                let mut raw_text = keyword_text.to_string();

                // Skip whitespace
                if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                    current = ws_sel;
                } else {
                    continue;
                }

                // Match the trigger word(s)
                let mut trigger_words = Vec::new();
                loop {
                    if let Some((word_sel, (_, word_text))) = current.match_first_forwards(
                        &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                    ) {
                        trigger_words.push(word_text.to_string());
                        current = word_sel;

                        // Check for whitespace to continue
                        if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                            // Check if next word continues the trigger phrase
                            if let Some((_, (_, peek_text))) = ws_sel.match_first_forwards(
                                &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                            ) {
                                // Continue if it's "of", "the", or starts with uppercase
                                let lower = peek_text.to_lowercase();
                                if lower == "of" || lower == "the"
                                    || peek_text.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                                {
                                    current = ws_sel;
                                    continue;
                                }
                            }
                        }
                        break;
                    } else {
                        break;
                    }
                }

                if !trigger_words.is_empty() {
                    raw_text.push(' ');
                    raw_text.push_str(&trigger_words.join(" "));

                    assignments.push(current.finish_with_attr(TemporalExpression {
                        temporal_type: TemporalType::RelativeTime {
                            trigger: trigger_words.join(" "),
                            relation,
                        },
                        text: raw_text,
                        confidence: self.duration_confidence * 0.9,
                    }));
                }
            }
        }

        // Pattern 6b: Multi-word time relations ("prior to", "at the time of")
        for (sel, (_, first_word)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            let lower = first_word.to_lowercase();
            let relation_match = match lower.as_str() {
                "prior" => {
                    // "prior to"
                    let check = sel.clone();
                    if let Some((ws, _)) = check.match_first_forwards(&x::whitespace()) {
                        if let Some((to_sel, (_, to_text))) = ws.match_first_forwards(
                            &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                        ) {
                            if to_text.to_lowercase() == "to" {
                                Some((TimeRelation::PriorTo, to_sel, "prior to"))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                "at" => {
                    // "at the time of"
                    let mut check = sel.clone();
                    let mut matched = false;
                    if let Some((ws1, _)) = check.match_first_forwards(&x::whitespace()) {
                        if let Some((the_sel, (_, the_text))) = ws1.match_first_forwards(
                            &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                        ) {
                            if the_text.to_lowercase() == "the" {
                                if let Some((ws2, _)) =
                                    the_sel.match_first_forwards(&x::whitespace())
                                {
                                    if let Some((time_sel, (_, time_text))) = ws2
                                        .match_first_forwards(
                                            &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                                        )
                                    {
                                        if time_text.to_lowercase() == "time" {
                                            if let Some((ws3, _)) =
                                                time_sel.match_first_forwards(&x::whitespace())
                                            {
                                                if let Some((of_sel, (_, of_text))) = ws3
                                                    .match_first_forwards(&x::all((
                                                        x::attr_eq(&TextTag::WORD),
                                                        x::token_text(),
                                                    )))
                                                {
                                                    if of_text.to_lowercase() == "of" {
                                                        check = of_sel;
                                                        matched = true;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if matched {
                        Some((TimeRelation::AtTimeOf, check, "at the time of"))
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some((relation, phrase_end_sel, phrase_text)) = relation_match {
                let mut current = phrase_end_sel;
                let mut raw_text = phrase_text.to_string();

                // Skip whitespace
                if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                    current = ws_sel;
                } else {
                    continue;
                }

                // Match the trigger word(s)
                let mut trigger_words = Vec::new();
                loop {
                    if let Some((word_sel, (_, word_text))) = current.match_first_forwards(
                        &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                    ) {
                        trigger_words.push(word_text.to_string());
                        current = word_sel;

                        if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                            if let Some((_, (_, peek_text))) = ws_sel.match_first_forwards(
                                &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                            ) {
                                let lower_peek = peek_text.to_lowercase();
                                if lower_peek == "of"
                                    || lower_peek == "the"
                                    || peek_text
                                        .chars()
                                        .next()
                                        .map(|c| c.is_uppercase())
                                        .unwrap_or(false)
                                {
                                    current = ws_sel;
                                    continue;
                                }
                            }
                        }
                        break;
                    } else {
                        break;
                    }
                }

                if !trigger_words.is_empty() {
                    raw_text.push(' ');
                    raw_text.push_str(&trigger_words.join(" "));

                    assignments.push(current.finish_with_attr(TemporalExpression {
                        temporal_type: TemporalType::RelativeTime {
                            trigger: trigger_words.join(" "),
                            relation,
                        },
                        text: raw_text,
                        confidence: self.duration_confidence * 0.9,
                    }));
                }
            }
        }

        assignments
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layered_nlp::{create_line_from_string, LLLineDisplay};

    fn detect_temporal(text: &str) -> Vec<TemporalExpression> {
        let line = create_line_from_string(text).run(&TemporalExpressionResolver::new());
        line.find(&x::attr::<TemporalExpression>())
            .into_iter()
            .map(|found| (*found.attr()).clone())
            .collect()
    }

    #[test]
    fn test_date_full() {
        let exprs = detect_temporal("Payment is due on December 31, 2024.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::Date { year: Some(2024), month: Some(12), day: Some(31) }
            )),
            "Expected December 31, 2024. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_date_without_year() {
        let exprs = detect_temporal("The deadline is January 15.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::Date { year: None, month: Some(1), day: Some(15) }
            )),
            "Expected January 15 without year. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_duration_numeric() {
        let exprs = detect_temporal("The term is 30 days from signing.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::Duration { value: 30, unit: DurationUnit::Days, .. }
            )),
            "Expected 30 days duration. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_duration_written() {
        let exprs = detect_temporal("The term shall be five years.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::Duration { value: 5, unit: DurationUnit::Years, written_form: Some(_) }
            )),
            "Expected five years duration. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_duration_with_parenthetical() {
        let exprs = detect_temporal("within thirty (30) days of receipt");
        assert!(
            exprs.iter().any(|e| {
                if let TemporalType::Deadline { reference, .. } = &e.temporal_type {
                    matches!(
                        reference.as_ref(),
                        TemporalType::Duration { value: 30, unit: DurationUnit::Days, written_form: Some(w) } if w == "thirty"
                    )
                } else {
                    false
                }
            }),
            "Expected thirty (30) days. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_deadline_within() {
        let exprs = detect_temporal("The Company shall respond within 10 days.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::Deadline { deadline_type: DeadlineType::Within, .. }
            )),
            "Expected 'within' deadline. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_deadline_by() {
        let exprs = detect_temporal("Submit the report by 30 days.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::Deadline { deadline_type: DeadlineType::By, .. }
            )),
            "Expected 'by' deadline. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_defined_date() {
        let exprs = detect_temporal("This Agreement commences on the Effective Date.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::DefinedDate { term } if term.contains("Effective")
            )),
            "Expected 'the Effective Date'. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_defined_date_termination() {
        let exprs = detect_temporal("Obligations continue until the Termination Date.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::DefinedDate { term } if term.contains("Termination")
            )),
            "Expected 'the Termination Date'. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_relative_upon() {
        let exprs = detect_temporal("Payment is due upon termination.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::RelativeTime { relation: TimeRelation::Upon, .. }
            )),
            "Expected 'upon termination'. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_relative_following() {
        let exprs = detect_temporal("Notify the Company following receipt of notice.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::RelativeTime { relation: TimeRelation::Following, .. }
            )),
            "Expected 'following receipt'. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_business_days() {
        let exprs = detect_temporal("Response required within 5 business days.");
        assert!(
            exprs.iter().any(|e| {
                if let TemporalType::Deadline { reference, .. } = &e.temporal_type {
                    matches!(
                        reference.as_ref(),
                        TemporalType::Duration { value: 5, unit: DurationUnit::BusinessDays, .. }
                    )
                } else {
                    false
                }
            }),
            "Expected 5 business days. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_working_days() {
        let exprs = detect_temporal("Complete the task in 10 working days.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::Duration { value: 10, unit: DurationUnit::BusinessDays, .. }
            )),
            "Expected 10 working days. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_no_later_than() {
        let exprs = detect_temporal("Payment due no later than 30 days after invoice.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::Deadline { deadline_type: DeadlineType::NoLaterThan, .. }
            )),
            "Expected 'no later than' deadline. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_on_or_before() {
        let exprs = detect_temporal("Complete on or before 60 days from signing.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::Deadline { deadline_type: DeadlineType::OnOrBefore, .. }
            )),
            "Expected 'on or before' deadline. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_promptly_following() {
        let exprs = detect_temporal("Notify promptly following 5 business days.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::Deadline { deadline_type: DeadlineType::PromptlyFollowing, .. }
            )),
            "Expected 'promptly following' deadline. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_prior_to() {
        let exprs = detect_temporal("Complete all deliverables prior to closing.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::RelativeTime { relation: TimeRelation::PriorTo, .. }
            )),
            "Expected 'prior to' relation. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_at_time_of() {
        let exprs = detect_temporal("Payment required at the time of execution.");
        assert!(
            exprs.iter().any(|e| matches!(
                &e.temporal_type,
                TemporalType::RelativeTime { relation: TimeRelation::AtTimeOf, .. }
            )),
            "Expected 'at the time of' relation. Found: {:?}",
            exprs
        );
    }

    #[test]
    fn test_multiple_expressions() {
        let exprs = detect_temporal(
            "Payment of thirty (30) days after the Effective Date but before December 31, 2024.",
        );
        // Should find: duration, defined date, and date
        assert!(exprs.len() >= 3, "Expected at least 3 expressions. Found: {:?}", exprs);
    }

    #[test]
    fn test_display_snapshot() {
        let line = create_line_from_string("Payment due within thirty (30) days of the Effective Date")
            .run(&TemporalExpressionResolver::new());
        let mut display = LLLineDisplay::new(&line);
        display.include::<TemporalExpression>();
        insta::assert_snapshot!(display.to_string());
    }
}
