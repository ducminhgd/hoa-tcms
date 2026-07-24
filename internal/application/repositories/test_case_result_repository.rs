//! `TestCaseResultRepository` trait — persistence contract for the
//! [`TestCaseResult`] entity.
//!
//! Defines the data-access operations the application layer requires for test
//! case result management. Implementations live in
//! `infrastructure::postgres::repositories`.

use async_trait::async_trait;

use crate::application::repositories::RepositoryResult;
use crate::domain::entities::test_case_result::TestCaseResult;

/// Snapshot data from a `test_case` row used for import.
#[derive(Clone, Debug)]
pub struct TestCaseImportData {
    pub test_case_id: i64,
    pub summary: String,
    pub description: Option<String>,
    pub priority: String,
}

/// Repository interface for [`TestCaseResult`] persistence.
///
/// All methods return [`RepositoryResult`] and are `Send + Sync` so they
/// can be called from async handlers behind `Arc<dyn TestCaseResultRepository>`.
#[async_trait]
pub trait TestCaseResultRepository: Send + Sync {
    /// Create a new test case result.
    async fn create(&self, result: &TestCaseResult) -> RepositoryResult<TestCaseResult>;

    /// Look up a test case result by primary key.
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<TestCaseResult>>;

    /// Persist changes to an existing test case result.
    async fn update(&self, result: &TestCaseResult) -> RepositoryResult<TestCaseResult>;

    /// Soft-delete a test case result.
    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()>;

    /// List results for an execution with pagination.
    async fn list_by_execution(
        &self,
        execution_id: i64,
        page: u32,
        limit: u32,
    ) -> RepositoryResult<(Vec<TestCaseResult>, u64)>;

    /// Find an existing result for a test case within an execution.
    ///
    /// Used during import to detect whether a case has already been imported.
    async fn find_by_execution_and_test_case(
        &self,
        execution_id: i64,
        test_case_id: i64,
    ) -> RepositoryResult<Option<TestCaseResult>>;

    /// Check whether a test case belongs to a test run.
    async fn test_case_in_run(&self, run_id: i64, test_case_id: i64) -> RepositoryResult<bool>;

    /// Get test case snapshot data for import.
    async fn get_test_case_for_import(
        &self,
        test_case_id: i64,
    ) -> RepositoryResult<Option<TestCaseImportData>>;

    /// List test cases in a test run with pagination (for the import dialog).
    async fn list_test_cases_in_run(
        &self,
        run_id: i64,
        page: u32,
        limit: u32,
    ) -> RepositoryResult<(Vec<TestCaseImportData>, u64)>;

    /// Atomically import multiple test cases into an execution.
    /// Uses a single transaction. Returns (imported_count, refreshed_count).
    async fn batch_import_cases(
        &self,
        execution_id: i64,
        user_id: i64,
        case_ids: &[i64],
    ) -> RepositoryResult<(u32, u32)>;
}
