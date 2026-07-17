//! TestExecution entity — represents a single execution run against a set of
//! test cases within a test run.
//!
//! A test execution tracks who ran the tests, when, and which testers were
//! assigned. The actual per-case results are modelled as `TestCaseResult`
//! children.
//!
//! This is a pure domain entity with **no ORM or framework imports**.

use chrono::{DateTime, Utc};

/// A test execution — an instance of executing a test run.
///
/// Maps to the `test_executions` table. The `id` field is `0` until the
/// entity is persisted.
#[derive(Clone)]
pub struct TestExecution {
    pub id: i64,
    pub name: String,
    pub test_run_id: i64,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_by: i64,
    pub updated_at: DateTime<Utc>,
    pub deleted_by: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for TestExecution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestExecution")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("test_run_id", &self.test_run_id)
            .field("created_by", &self.created_by)
            .field("created_at", &self.created_at)
            .field("updated_by", &self.updated_by)
            .field("updated_at", &self.updated_at)
            .field("deleted_by", &self.deleted_by)
            .field("deleted_at", &self.deleted_at)
            .finish()
    }
}

impl TestExecution {
    /// Create a new test execution.
    ///
    /// The `id` is left at `0` and will be assigned by the persistence layer.
    /// Both `created_by` and `updated_by` are set to the same user.
    ///
    /// # Panics
    ///
    /// Panics if `name` is empty (callers must validate before calling).
    pub fn create(name: String, test_run_id: i64, created_by: i64) -> Self {
        debug_assert!(!name.trim().is_empty(), "execution name must not be empty");

        let now = Utc::now();
        Self {
            id: 0,
            name,
            test_run_id,
            created_by,
            created_at: now,
            updated_by: created_by,
            updated_at: now,
            deleted_by: None,
            deleted_at: None,
        }
    }

    /// Returns `true` if the execution has been soft-deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
