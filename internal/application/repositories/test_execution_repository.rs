//! `TestExecutionRepository` trait — persistence contract for the
//! [`TestExecution`] entity.
//!
//! Defines the data-access operations the application layer requires for test
//! execution management. Implementations live in
//! `infrastructure::postgres::repositories`.

use async_trait::async_trait;

use crate::application::repositories::RepositoryResult;
use crate::domain::entities::test_execution::TestExecution;

/// Tester identifier with display name information.
#[derive(Clone, Debug)]
pub struct TesterInfoRow {
    pub user_id: i64,
    pub username: String,
    pub fullname: String,
}

/// Repository interface for [`TestExecution`] persistence.
///
/// All methods return [`RepositoryResult`] and are `Send + Sync` so they
/// can be called from async handlers behind `Arc<dyn TestExecutionRepository>`.
#[async_trait]
pub trait TestExecutionRepository: Send + Sync {
    /// Create a new test execution and assign testers — all within a single
    /// database transaction.
    async fn create(
        &self,
        execution: &TestExecution,
        tester_ids: &[i64],
    ) -> RepositoryResult<TestExecution>;

    /// Look up a test execution by primary key.
    ///
    /// Returns `None` when no active (non-deleted) execution exists with the
    /// given `id`.
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<TestExecution>>;

    /// Persist changes to an existing test execution.
    ///
    /// Returns the updated row.
    async fn update(&self, execution: &TestExecution) -> RepositoryResult<TestExecution>;

    /// Update execution metadata and replace testers in a single transaction.
    async fn update_with_testers(
        &self,
        execution: &TestExecution,
        tester_ids: &[i64],
    ) -> RepositoryResult<TestExecution>;

    /// Soft-delete a test execution by setting `deleted_at` and `deleted_by`.
    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()>;

    /// Return a paginated list of test executions, optionally filtered by
    /// `test_run_id`, with total count.
    async fn list(
        &self,
        page: u32,
        limit: u32,
        test_run_id: Option<i64>,
    ) -> RepositoryResult<(Vec<TestExecution>, u64)>;

    /// Return a paginated list of test executions accessible by a user
    /// (creator, tester, or project member).
    async fn list_by_user(
        &self,
        user_id: i64,
        page: u32,
        limit: u32,
        test_run_id: Option<i64>,
    ) -> RepositoryResult<(Vec<TestExecution>, u64)>;

    /// Check whether a user can access a test execution.
    ///
    /// Access is granted if the user is the creator, a tester, or a member
    /// of the project that owns the test run (via the run's project_id).
    async fn user_can_access(&self, execution_id: i64, user_id: i64) -> RepositoryResult<bool>;

    /// Get the tester user IDs for an execution.
    async fn get_tester_ids(&self, execution_id: i64) -> RepositoryResult<Vec<i64>>;

    /// Set testers for an execution (replaces all existing entries).
    async fn set_testers(&self, execution_id: i64, tester_ids: &[i64]) -> RepositoryResult<()>;

    /// Get tester counts for the given execution IDs.
    async fn get_tester_counts(
        &self,
        execution_ids: &[i64],
    ) -> RepositoryResult<std::collections::HashMap<i64, u64>>;

    /// Get detailed tester info (username, fullname) for an execution.
    async fn get_tester_info(&self, execution_id: i64) -> RepositoryResult<Vec<TesterInfoRow>>;

    /// Check whether a test run exists and is not soft-deleted.
    async fn test_run_exists(&self, run_id: i64) -> RepositoryResult<bool>;
}
