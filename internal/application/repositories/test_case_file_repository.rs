//! `TestCaseFileRepository` trait — persistence contract for the
//! [`TestCaseFile`] entity.
//!
//! Defines the data-access operations the application layer requires for
//! test case file management. Implementations live in
//! `infrastructure::postgres::repositories`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::application::repositories::RepositoryResult;
use crate::domain::entities::test_case_file::TestCaseFile;

/// Metadata about a test case fetched server-side for authorization checks.
///
/// Never derived from client input — always queried from the database.
#[derive(Clone, Debug)]
pub struct TestCaseInfo {
    /// The user who created the test case.
    pub created_by: i64,
    /// The project the test case belongs to (`None` for orphaned cases).
    pub project_id: Option<i64>,
}

/// A summary row for listing files attached to a test case.
///
/// Excludes internal fields (`file_path`, `deleted_at`, `deleted_by`).
#[derive(Clone, Debug, serde::Serialize)]
pub struct TestCaseFileListItem {
    pub id: i64,
    pub file_name: String,
    pub file_size: i64,
    pub mime_type: String,
    pub uploaded_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Repository interface for [`TestCaseFile`] persistence.
///
/// All methods return [`RepositoryResult`] and are `Send + Sync` so they
/// can be called from async handlers behind
/// `Arc<dyn TestCaseFileRepository>`.
#[async_trait]
pub trait TestCaseFileRepository: Send + Sync {
    /// Look up a file by primary key.
    ///
    /// Returns `None` when no active (non-deleted) file exists with the
    /// given `id`.
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<TestCaseFile>>;

    /// List all non-deleted files for a test case.
    ///
    /// Supports optional case-insensitive `search` on `file_name` and a
    /// `sort` parameter. The `sort` value is validated against a whitelist
    /// (`file_name`, `file_size`, `created_at` with optional `-` prefix).
    async fn find_by_test_case(
        &self,
        test_case_id: i64,
        search: Option<&str>,
        sort: &str,
    ) -> RepositoryResult<Vec<TestCaseFileListItem>>;

    /// Persist a new file record.
    ///
    /// Returns the saved entity with the assigned `id`, `created_at`, and
    /// `updated_at`.
    async fn save(&self, file: &TestCaseFile) -> RepositoryResult<TestCaseFile>;

    /// Soft-delete a file by setting `deleted_at` and `deleted_by`.
    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()>;

    /// Fetch test case metadata for server-side authorization checks.
    ///
    /// Returns `None` if the test case does not exist or is soft-deleted.
    /// This is the **only** source of `created_by` and `project_id` used in
    /// authorization decisions — the service never trusts client input for
    /// these values.
    async fn get_test_case_info(&self, test_case_id: i64)
    -> RepositoryResult<Option<TestCaseInfo>>;
}
