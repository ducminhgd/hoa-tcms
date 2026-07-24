//! TestCaseResult entity — the outcome of a single test case within a
//! test execution.
//!
//! Summary, description, and priority are **snapshots** cloned from the
//! source `TestCase` at import time. Re-import updates these fields but
//! preserves the result, logs, and tested_by.
//!
//! `tested_by` is set once when the result first transitions away from
//! `NOT_TESTED` and is never modified thereafter.
//!
//! This is a pure domain entity with **no ORM or framework imports**.

use chrono::{DateTime, Utc};

use crate::domain::value_objects::test_result_status::TestResultStatus;

/// The result of a single test case within a test execution.
///
/// Maps to the `test_case_results` table. The `id` field is `0` until the
/// entity is persisted.
#[derive(Clone)]
pub struct TestCaseResult {
    pub id: i64,
    pub execution_id: i64,
    pub test_case_id: i64,
    pub summary: String,
    pub description: Option<String>,
    pub priority: String,
    pub result: TestResultStatus,
    pub logs: Option<String>,
    pub tested_by: Option<i64>,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_by: i64,
    pub updated_at: DateTime<Utc>,
    pub deleted_by: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for TestCaseResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestCaseResult")
            .field("id", &self.id)
            .field("execution_id", &self.execution_id)
            .field("test_case_id", &self.test_case_id)
            .field("summary", &self.summary)
            .field("result", &self.result)
            .field("tested_by", &self.tested_by)
            .field("created_by", &self.created_by)
            .field("created_at", &self.created_at)
            .field("updated_by", &self.updated_by)
            .field("updated_at", &self.updated_at)
            .field("deleted_by", &self.deleted_by)
            .field("deleted_at", &self.deleted_at)
            .finish()
    }
}

impl TestCaseResult {
    /// Create a new test case result with status `NotTested`.
    ///
    /// The `id` is left at `0` and will be assigned by the persistence layer.
    /// Both `created_by` and `updated_by` are set to the same user.
    ///
    /// # Panics
    ///
    /// Panics if `summary` is empty (callers must validate before calling).
    pub fn create(
        execution_id: i64,
        test_case_id: i64,
        summary: String,
        description: Option<String>,
        priority: String,
        created_by: i64,
    ) -> Self {
        debug_assert!(!summary.trim().is_empty(), "summary must not be empty");

        let now = Utc::now();
        Self {
            id: 0,
            execution_id,
            test_case_id,
            summary,
            description,
            priority,
            result: TestResultStatus::NotTested,
            logs: None,
            tested_by: None,
            created_by,
            created_at: now,
            updated_by: created_by,
            updated_at: now,
            deleted_by: None,
            deleted_at: None,
        }
    }
}
