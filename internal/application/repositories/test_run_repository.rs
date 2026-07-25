//! `TestRunRepository` trait — persistence contract for the [`TestRun`] entity.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::application::repositories::RepositoryResult;
use crate::domain::entities::test_run::TestRun;

/// A test run row returned by list queries.
#[derive(Clone, Debug, serde::Serialize)]
pub struct TestRunListItem {
    pub id: i64,
    pub summary: String,
    pub report_to_user_id: Option<i64>,
    pub default_tester_id: i64,
    pub project_id: i64,
    pub plan_id: Option<i64>,
    pub version: Option<String>,
    pub planned_start: Option<chrono::NaiveDate>,
    pub planned_stop: Option<chrono::NaiveDate>,
    pub case_count: i64,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Statistics aggregated from linked executions.
#[derive(Clone, Debug, serde::Serialize)]
pub struct TestRunStatistics {
    pub total: i64,
    pub not_tested: i64,
    pub in_progress: i64,
    pub pass: i64,
    pub fail: i64,
    pub warning: i64,
    pub ignore: i64,
}

#[async_trait]
pub trait TestRunRepository: Send + Sync {
    /// Look up a test run by primary key.
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<TestRun>>;

    /// List test runs scoped to a project.
    async fn list_by_project(
        &self,
        project_id: i64,
        page: u32,
        limit: u32,
        plan_id: Option<i64>,
        search: Option<&str>,
        sort: &str,
    ) -> RepositoryResult<(Vec<TestRunListItem>, u64)>;

    /// Find a non-deleted test run by summary in a project (case-insensitive).
    async fn find_by_summary(
        &self,
        project_id: i64,
        summary: &str,
    ) -> RepositoryResult<Option<TestRun>>;

    /// Get the test case IDs linked to a run.
    async fn get_case_ids(&self, run_id: i64) -> RepositoryResult<Vec<i64>>;

    /// Get the execution IDs linked to a run.
    async fn get_execution_ids(&self, run_id: i64) -> RepositoryResult<Vec<i64>>;

    /// Compute aggregated statistics from linked executions.
    async fn compute_statistics(&self, run_id: i64) -> RepositoryResult<TestRunStatistics>;

    /// Create a test run with optional case assignments in a transaction.
    async fn create(&self, run: &TestRun, case_ids: &[i64]) -> RepositoryResult<TestRun>;

    /// Update a test run and optionally replace case assignments.
    async fn update(&self, run: &TestRun, case_ids: Option<&[i64]>) -> RepositoryResult<TestRun>;

    /// Soft-delete a test run.
    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()>;
}
