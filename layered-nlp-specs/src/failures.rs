//! Expected failures tracking via TOML file.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Loaded expected failures configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExpectedFailures {
    /// Known failures (won't fix soon).
    #[serde(default)]
    pub known: Vec<FailureEntry>,
    /// Pending failures (awaiting fix).
    #[serde(default)]
    pub pending: Vec<FailureEntry>,
}

/// A single expected failure entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureEntry {
    /// Fixture file name.
    pub fixture: String,
    /// Assertion reference (e.g., "S3.[6]" for paragraph 3, span 6).
    pub assertion: String,
    /// Human-readable reason.
    #[serde(default)]
    pub reason: Option<String>,
    /// Date added (YYYY-MM-DD).
    #[serde(default)]
    pub added: Option<String>,
    /// Related issue URL.
    #[serde(default)]
    pub issue: Option<String>,
}

/// Failure lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureState {
    /// Known limitation, won't fix soon.
    Known,
    /// Awaiting fix, not blocking.
    Pending,
    /// Expected to pass - failure is a regression.
    Regression,
}

impl ExpectedFailures {
    /// Load from a TOML file.
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        toml::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
    }

    /// Check if a failure is expected.
    pub fn is_expected(&self, fixture: &str, assertion_ref: &str) -> FailureState {
        for entry in &self.known {
            if entry.fixture == fixture && entry.assertion == assertion_ref {
                return FailureState::Known;
            }
        }

        for entry in &self.pending {
            if entry.fixture == fixture && entry.assertion == assertion_ref {
                return FailureState::Pending;
            }
        }

        FailureState::Regression
    }

    /// Format an assertion reference from span/paragraph info.
    pub fn format_ref(paragraph_idx: usize, span_id: usize) -> String {
        format!("S{}.[{}]", paragraph_idx, span_id)
    }

    /// Get all expected failure fixtures.
    pub fn all_fixtures(&self) -> Vec<&str> {
        let mut fixtures: Vec<_> = self
            .known
            .iter()
            .chain(self.pending.iter())
            .map(|e| e.fixture.as_str())
            .collect();
        fixtures.sort();
        fixtures.dedup();
        fixtures
    }

    /// Count total expected failures.
    pub fn count(&self) -> usize {
        self.known.len() + self.pending.len()
    }

    /// Get entry for a specific failure (if expected).
    pub fn get_entry(&self, fixture: &str, assertion_ref: &str) -> Option<&FailureEntry> {
        self.known
            .iter()
            .chain(self.pending.iter())
            .find(|e| e.fixture == fixture && e.assertion == assertion_ref)
    }
}

/// Result of running the harness.
#[derive(Debug, Clone)]
pub struct HarnessResult {
    /// Total assertions checked.
    pub total: usize,
    /// Passed assertions.
    pub passed: usize,
    /// Expected failures (known + pending).
    pub expected_failures: usize,
    /// Regressions (unexpected failures).
    pub regressions: usize,
}

impl HarnessResult {
    pub fn new() -> Self {
        Self {
            total: 0,
            passed: 0,
            expected_failures: 0,
            regressions: 0,
        }
    }

    /// Get the exit code (0 = pass, 1 = regressions).
    pub fn exit_code(&self) -> i32 {
        if self.regressions > 0 {
            1
        } else {
            0
        }
    }

    /// Check if all tests passed (no regressions).
    pub fn success(&self) -> bool {
        self.regressions == 0
    }

    /// Record a passed assertion.
    pub fn record_pass(&mut self) {
        self.total += 1;
        self.passed += 1;
    }

    /// Record a failed assertion with its state.
    pub fn record_failure(&mut self, state: FailureState) {
        self.total += 1;
        match state {
            FailureState::Known | FailureState::Pending => {
                self.expected_failures += 1;
            }
            FailureState::Regression => {
                self.regressions += 1;
            }
        }
    }
}

impl Default for HarnessResult {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_empty() {
        let failures = ExpectedFailures::default();
        assert_eq!(failures.count(), 0);
    }

    #[test]
    fn test_is_expected_regression() {
        let failures = ExpectedFailures::default();
        assert_eq!(
            failures.is_expected("test.nlp", "S0.[1]"),
            FailureState::Regression
        );
    }

    #[test]
    fn test_is_expected_known() {
        let failures = ExpectedFailures {
            known: vec![FailureEntry {
                fixture: "test.nlp".to_string(),
                assertion: "S0.[1]".to_string(),
                reason: Some("Known issue".to_string()),
                added: None,
                issue: None,
            }],
            pending: vec![],
        };

        assert_eq!(
            failures.is_expected("test.nlp", "S0.[1]"),
            FailureState::Known
        );
    }

    #[test]
    fn test_is_expected_pending() {
        let failures = ExpectedFailures {
            known: vec![],
            pending: vec![FailureEntry {
                fixture: "test.nlp".to_string(),
                assertion: "S0.[1]".to_string(),
                reason: Some("Pending fix".to_string()),
                added: Some("2025-01-06".to_string()),
                issue: None,
            }],
        };

        assert_eq!(
            failures.is_expected("test.nlp", "S0.[1]"),
            FailureState::Pending
        );
    }

    #[test]
    fn test_format_ref() {
        assert_eq!(ExpectedFailures::format_ref(2, 5), "S2.[5]");
        assert_eq!(ExpectedFailures::format_ref(0, 1), "S0.[1]");
    }

    #[test]
    fn test_harness_result_exit_code() {
        let mut result = HarnessResult::new();
        assert_eq!(result.exit_code(), 0);

        result.regressions = 1;
        assert_eq!(result.exit_code(), 1);
    }

    #[test]
    fn test_harness_result_record() {
        let mut result = HarnessResult::new();

        result.record_pass();
        assert_eq!(result.total, 1);
        assert_eq!(result.passed, 1);

        result.record_failure(FailureState::Known);
        assert_eq!(result.total, 2);
        assert_eq!(result.expected_failures, 1);

        result.record_failure(FailureState::Regression);
        assert_eq!(result.total, 3);
        assert_eq!(result.regressions, 1);

        assert!(!result.success());
    }

    #[test]
    fn test_load_from_toml() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[[pending]]
fixture = "test.nlp"
assertion = "S0.[1]"
reason = "Awaiting implementation"
added = "2025-01-06"

[[known]]
fixture = "other.nlp"
assertion = "S1.[2]"
reason = "Known limitation"
issue = "https://github.com/example/issues/123"
"#
        )
        .unwrap();

        let failures = ExpectedFailures::load(file.path()).unwrap();
        assert_eq!(failures.count(), 2);
        assert_eq!(failures.pending.len(), 1);
        assert_eq!(failures.known.len(), 1);

        assert_eq!(
            failures.is_expected("test.nlp", "S0.[1]"),
            FailureState::Pending
        );
        assert_eq!(
            failures.is_expected("other.nlp", "S1.[2]"),
            FailureState::Known
        );
    }

    #[test]
    fn test_load_nonexistent_returns_empty() {
        let failures = ExpectedFailures::load(Path::new("/nonexistent/path.toml")).unwrap();
        assert_eq!(failures.count(), 0);
    }

    #[test]
    fn test_all_fixtures() {
        let failures = ExpectedFailures {
            known: vec![
                FailureEntry {
                    fixture: "a.nlp".to_string(),
                    assertion: "S0.[1]".to_string(),
                    reason: None,
                    added: None,
                    issue: None,
                },
                FailureEntry {
                    fixture: "b.nlp".to_string(),
                    assertion: "S0.[2]".to_string(),
                    reason: None,
                    added: None,
                    issue: None,
                },
            ],
            pending: vec![FailureEntry {
                fixture: "a.nlp".to_string(),
                assertion: "S1.[1]".to_string(),
                reason: None,
                added: None,
                issue: None,
            }],
        };

        let fixtures = failures.all_fixtures();
        assert_eq!(fixtures, vec!["a.nlp", "b.nlp"]);
    }

    #[test]
    fn test_get_entry() {
        let failures = ExpectedFailures {
            known: vec![FailureEntry {
                fixture: "test.nlp".to_string(),
                assertion: "S0.[1]".to_string(),
                reason: Some("Known issue".to_string()),
                added: None,
                issue: None,
            }],
            pending: vec![],
        };

        let entry = failures.get_entry("test.nlp", "S0.[1]");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().reason, Some("Known issue".to_string()));

        let missing = failures.get_entry("test.nlp", "S0.[2]");
        assert!(missing.is_none());
    }
}
