//! TestRun entity — a planned execution cycle within a project.
//!
//! Each test run belongs to a project, groups test cases for execution,
//! and can optionally be linked to a test plan. Runs track metadata like
//! assigned tester, planning dates, and version info.
//!
//! This is a pure domain entity with **no ORM or framework imports**.

use chrono::{DateTime, NaiveDate, Utc};

/// A test run — a planned execution cycle.
///
/// Maps to the `test_runs` table. The `id` field is `0` until persisted.
#[derive(Clone)]
pub struct TestRun {
    pub id: i64,
    pub summary: String,
    pub report_to_user_id: Option<i64>,
    pub default_tester_id: i64,
    pub project_id: i64,
    pub plan_id: Option<i64>,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub planned_start: Option<NaiveDate>,
    pub planned_stop: Option<NaiveDate>,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_by: i64,
    pub updated_at: DateTime<Utc>,
    pub deleted_by: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for TestRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestRun")
            .field("id", &self.id)
            .field("summary", &self.summary)
            .field("project_id", &self.project_id)
            .field("plan_id", &self.plan_id)
            .field("default_tester_id", &self.default_tester_id)
            .field("created_by", &self.created_by)
            .field("created_at", &self.created_at)
            .field("updated_by", &self.updated_by)
            .field("updated_at", &self.updated_at)
            .field("deleted_by", &self.deleted_by)
            .field("deleted_at", &self.deleted_at)
            .finish()
    }
}

impl TestRun {
    /// Create a new test run.
    ///
    /// # Panics
    ///
    /// Panics via `debug_assert!` if `summary` is empty (callers must validate).
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        summary: String,
        report_to_user_id: Option<i64>,
        default_tester_id: i64,
        project_id: i64,
        plan_id: Option<i64>,
        version: Option<String>,
        notes: Option<String>,
        planned_start: Option<NaiveDate>,
        planned_stop: Option<NaiveDate>,
        created_by: i64,
    ) -> Self {
        debug_assert!(!summary.trim().is_empty(), "run summary must not be empty");

        let now = Utc::now();
        Self {
            id: 0,
            summary,
            report_to_user_id,
            default_tester_id,
            project_id,
            plan_id,
            version,
            notes,
            planned_start,
            planned_stop,
            created_by,
            created_at: now,
            updated_by: created_by,
            updated_at: now,
            deleted_by: None,
            deleted_at: None,
        }
    }

    /// Returns `true` if the test run has been soft-deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
