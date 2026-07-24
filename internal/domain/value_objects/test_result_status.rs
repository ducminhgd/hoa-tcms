//! Test result status value object.
//!
//! Represents the execution state of an individual test case within a
//! test execution: `NotTested`, `InProgress`, `Pass`, `Fail`, `Warning`,
//! or `Ignore`.

use std::fmt;

/// The execution state of a test case result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TestResultStatus {
    /// Test has not yet been executed.
    NotTested,
    /// Test is currently being executed.
    InProgress,
    /// Test passed.
    Pass,
    /// Test failed.
    Fail,
    /// Test completed with a warning (non-blocking issue).
    Warning,
    /// Test was skipped/ignored.
    Ignore,
}

impl TestResultStatus {
    /// Parse a string into a `TestResultStatus`.
    ///
    /// Matching is case-insensitive. Returns `None` for unrecognised values.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "NOT_TESTED" => Some(Self::NotTested),
            "IN_PROGRESS" => Some(Self::InProgress),
            "PASS" => Some(Self::Pass),
            "FAIL" => Some(Self::Fail),
            "WARNING" => Some(Self::Warning),
            "IGNORE" => Some(Self::Ignore),
            _ => None,
        }
    }

    /// Return the canonical string representation (DB value).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotTested => "NOT_TESTED",
            Self::InProgress => "IN_PROGRESS",
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Warning => "WARNING",
            Self::Ignore => "IGNORE",
        }
    }
}

impl fmt::Display for TestResultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Error returned when parsing an invalid test result status string.
#[derive(Debug, Clone)]
pub struct InvalidTestResultStatus(pub String);

impl fmt::Display for InvalidTestResultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid test result status: {}", self.0)
    }
}

impl std::error::Error for InvalidTestResultStatus {}
