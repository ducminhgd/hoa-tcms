//! TestPlanStatus — lifecycle states for test plans.
//!
//! Statuses follow a validated state machine. Transitions are enforced at
//! the application layer with a `CHECK` constraint providing defence-in-depth
//! at the database level.

use serde::{Deserialize, Serialize};

/// The lifecycle status of a test plan.
///
/// Maps to `test_plans.status` (VARCHAR with CHECK constraint).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TestPlanStatus {
    /// Initial state; no work has started.
    #[serde(rename = "TO_DO")]
    ToDo,
    /// Work is actively in progress.
    InProgress,
    /// All work is completed.
    Done,
    /// The plan has been cancelled.
    Cancel,
}

impl TestPlanStatus {
    /// Parse a status string (case-insensitive).
    ///
    /// Accepts both `TODO` and `TO_DO` for forward-compatibility.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "TO_DO" | "TODO" => Some(Self::ToDo),
            "IN_PROGRESS" | "INPROGRESS" => Some(Self::InProgress),
            "DONE" => Some(Self::Done),
            "CANCEL" | "CANCELED" => Some(Self::Cancel),
            _ => None,
        }
    }

    /// Return the canonical string representation for storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ToDo => "TO_DO",
            Self::InProgress => "IN_PROGRESS",
            Self::Done => "DONE",
            Self::Cancel => "CANCEL",
        }
    }

    /// Check whether a transition from `self` to `target` is valid.
    ///
    /// Self-transitions are accepted as no-ops.
    pub fn can_transition_to(&self, target: &Self) -> bool {
        if self == target {
            return true; // no-op
        }
        match self {
            Self::ToDo => matches!(target, Self::InProgress | Self::Cancel),
            Self::InProgress => matches!(target, Self::Done | Self::Cancel),
            Self::Done => matches!(target, Self::InProgress),
            Self::Cancel => matches!(target, Self::ToDo),
        }
    }
}

impl std::fmt::Display for TestPlanStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_variants() {
        assert_eq!(TestPlanStatus::parse("TO_DO"), Some(TestPlanStatus::ToDo));
        assert_eq!(TestPlanStatus::parse("TODO"), Some(TestPlanStatus::ToDo));
        assert_eq!(
            TestPlanStatus::parse("IN_PROGRESS"),
            Some(TestPlanStatus::InProgress)
        );
        assert_eq!(TestPlanStatus::parse("DONE"), Some(TestPlanStatus::Done));
        assert_eq!(
            TestPlanStatus::parse("CANCEL"),
            Some(TestPlanStatus::Cancel)
        );
        assert_eq!(TestPlanStatus::parse("unknown"), None);
    }

    #[test]
    fn allowed_transitions() {
        use TestPlanStatus::*;
        assert!(ToDo.can_transition_to(&InProgress));
        assert!(ToDo.can_transition_to(&Cancel));
        assert!(!ToDo.can_transition_to(&Done));
        assert!(InProgress.can_transition_to(&Done));
        assert!(InProgress.can_transition_to(&Cancel));
        assert!(!InProgress.can_transition_to(&ToDo));
        assert!(Done.can_transition_to(&InProgress));
        assert!(!Done.can_transition_to(&ToDo));
        assert!(Cancel.can_transition_to(&ToDo));
        assert!(!Cancel.can_transition_to(&InProgress));
        // Self-transitions are no-ops.
        assert!(ToDo.can_transition_to(&ToDo));
        assert!(Done.can_transition_to(&Done));
    }
}
