//! Trait-based assertion system for span type checking.

use std::fmt;
use thiserror::Error;

/// Implemented by each span type that supports fixture assertions.
pub trait SpanAssertion: Sized {
    /// The parsed assertion specification for this type.
    type Assertion: AssertionSpec;

    /// Parse assertion syntax specific to this span type.
    ///
    /// Input is the body string inside Type(...) - e.g., for
    /// `Obligation(modal=shall, bearer=§T)` the input is `modal=shall, bearer=§T`.
    fn parse_assertion(input: &str) -> Result<Self::Assertion, ParseError>;

    /// Check if this span satisfies the assertion.
    fn check(&self, assertion: &Self::Assertion) -> Result<(), AssertionMismatch>;

    /// Type name for error messages (e.g., "ObligationPhrase").
    fn span_type_name() -> &'static str;
}

/// Describes an assertion for error reporting.
pub trait AssertionSpec: fmt::Debug + Clone {
    /// Human-readable description of what was asserted.
    fn describe(&self) -> String;

    /// List of fields this assertion constrains.
    fn constrained_fields(&self) -> Vec<&'static str>;
}

/// Error parsing assertion syntax.
#[derive(Debug, Clone, Error)]
#[error("Parse error at position {position}: {message}")]
pub struct ParseError {
    pub message: String,
    pub position: usize,
    /// Suggestions for valid alternatives.
    pub suggestions: Vec<String>,
    /// Valid fields for this span type.
    pub valid_fields: Vec<&'static str>,
}

impl ParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            position: 0,
            suggestions: Vec::new(),
            valid_fields: Vec::new(),
        }
    }

    pub fn at(mut self, position: usize) -> Self {
        self.position = position;
        self
    }

    pub fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions = suggestions;
        self
    }

    pub fn with_valid_fields(mut self, fields: Vec<&'static str>) -> Self {
        self.valid_fields = fields;
        self
    }
}

/// Assertion check failed.
#[derive(Debug, Clone, Error)]
pub struct AssertionMismatch {
    /// The text that was marked in the fixture.
    pub span_text: String,
    /// Original assertion source for context.
    pub assertion_source: String,
    /// Field-level mismatches.
    pub fields: Vec<FieldMismatch>,
}

impl fmt::Display for AssertionMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Assertion mismatch for span: \"{}\"", self.span_text)?;
        writeln!(f, "Assertion: {}", self.assertion_source)?;
        for field in &self.fields {
            writeln!(f, "  {}", field)?;
        }
        Ok(())
    }
}

impl AssertionMismatch {
    pub fn new(span_text: impl Into<String>, assertion_source: impl Into<String>) -> Self {
        Self {
            span_text: span_text.into(),
            assertion_source: assertion_source.into(),
            fields: Vec::new(),
        }
    }

    pub fn with_field(mut self, field: FieldMismatch) -> Self {
        self.fields.push(field);
        self
    }

    pub fn has_hard_mismatch(&self) -> bool {
        self.fields.iter().any(|f| f.severity == MismatchSeverity::Hard)
    }

    pub fn has_soft_mismatch(&self) -> bool {
        self.fields.iter().any(|f| f.severity == MismatchSeverity::Soft)
    }
}

/// A single field mismatch.
#[derive(Debug, Clone)]
pub struct FieldMismatch {
    /// The field name that didn't match.
    pub field: &'static str,
    /// What was expected (from assertion).
    pub expected: String,
    /// What was found (in the span).
    pub actual: String,
    /// How serious is this mismatch.
    pub severity: MismatchSeverity,
}

impl fmt::Display for FieldMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let severity_marker = match self.severity {
            MismatchSeverity::Hard => "✗",
            MismatchSeverity::Soft => "~",
            MismatchSeverity::Info => "ℹ",
        };
        write!(
            f,
            "{} {}: expected {}, got {}",
            severity_marker, self.field, self.expected, self.actual
        )
    }
}

impl FieldMismatch {
    pub fn hard(field: &'static str, expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self {
            field,
            expected: expected.into(),
            actual: actual.into(),
            severity: MismatchSeverity::Hard,
        }
    }

    pub fn soft(field: &'static str, expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self {
            field,
            expected: expected.into(),
            actual: actual.into(),
            severity: MismatchSeverity::Soft,
        }
    }

    pub fn info(field: &'static str, expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self {
            field,
            expected: expected.into(),
            actual: actual.into(),
            severity: MismatchSeverity::Info,
        }
    }
}

/// How serious is a field mismatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MismatchSeverity {
    /// Semantic mismatch - definitely wrong.
    Hard,
    /// Threshold not met (e.g., confidence).
    Soft,
    /// Informational difference.
    Info,
}

/// A parsed field check from the assertion body.
///
/// Reused from fixture parsing but extended for type-specific use.
#[derive(Debug, Clone)]
pub struct TypeFieldCheck {
    pub field: String,
    pub operator: FieldOperator,
    pub value: FieldValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldOperator {
    Equals,
    Gte,
    Lte,
    Arrow,  // -> for reference resolution
}

#[derive(Debug, Clone)]
pub enum FieldValue {
    String(String),
    Number(f64),
    EntityRef(String),  // §Entity reference
}

impl TypeFieldCheck {
    /// Parse a field check from "field=value" or "-> §Entity" syntax.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let input = input.trim();

        // Handle arrow syntax: -> §Entity
        if input.starts_with("->") {
            let target = input[2..].trim();
            if target.starts_with('§') {
                return Ok(Self {
                    field: "target".to_string(),
                    operator: FieldOperator::Arrow,
                    value: FieldValue::EntityRef(target[2..].to_string()), // Skip the § char
                });
            }
            return Err(ParseError::new(format!("Arrow target must be §Entity, got: {}", target)));
        }

        // Try operators in order of specificity
        if let Some(pos) = input.find(">=") {
            let (field, value) = (&input[..pos], &input[pos + 2..]);
            return Ok(Self {
                field: field.trim().to_string(),
                operator: FieldOperator::Gte,
                value: parse_value(value.trim())?,
            });
        }

        if let Some(pos) = input.find("<=") {
            let (field, value) = (&input[..pos], &input[pos + 2..]);
            return Ok(Self {
                field: field.trim().to_string(),
                operator: FieldOperator::Lte,
                value: parse_value(value.trim())?,
            });
        }

        if let Some(pos) = input.find('=') {
            let (field, value) = (&input[..pos], &input[pos + 1..]);
            return Ok(Self {
                field: field.trim().to_string(),
                operator: FieldOperator::Equals,
                value: parse_value(value.trim())?,
            });
        }

        Err(ParseError::new(format!("Invalid field check syntax: {}", input)))
    }
}

fn parse_value(input: &str) -> Result<FieldValue, ParseError> {
    // Check for entity reference
    if input.starts_with('§') {
        // Handle both §ID and §ID format (the § char can be followed by the ID)
        let id = input.chars().skip(1).collect::<String>();
        return Ok(FieldValue::EntityRef(id));
    }

    // Try to parse as number
    if let Ok(n) = input.parse::<f64>() {
        return Ok(FieldValue::Number(n));
    }

    // Otherwise it's a string
    Ok(FieldValue::String(input.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_equals() {
        let check = TypeFieldCheck::parse("modal=shall").unwrap();
        assert_eq!(check.field, "modal");
        assert_eq!(check.operator, FieldOperator::Equals);
        assert!(matches!(check.value, FieldValue::String(s) if s == "shall"));
    }

    #[test]
    fn test_parse_gte() {
        let check = TypeFieldCheck::parse("confidence >= 0.8").unwrap();
        assert_eq!(check.field, "confidence");
        assert_eq!(check.operator, FieldOperator::Gte);
        assert!(matches!(check.value, FieldValue::Number(n) if (n - 0.8).abs() < 0.001));
    }

    #[test]
    fn test_parse_entity_ref() {
        let check = TypeFieldCheck::parse("bearer=§T").unwrap();
        assert_eq!(check.field, "bearer");
        assert!(matches!(check.value, FieldValue::EntityRef(ref s) if s == "T"));
    }

    #[test]
    fn test_parse_arrow() {
        let check = TypeFieldCheck::parse("-> §Tenant").unwrap();
        assert_eq!(check.field, "target");
        assert_eq!(check.operator, FieldOperator::Arrow);
        assert!(matches!(check.value, FieldValue::EntityRef(ref s) if s == "Tenant"));
    }

    #[test]
    fn test_field_mismatch_display() {
        let m = FieldMismatch::hard("modal", "shall", "may");
        assert!(m.to_string().contains("modal"));
        assert!(m.to_string().contains("shall"));
        assert!(m.to_string().contains("may"));
    }
}
