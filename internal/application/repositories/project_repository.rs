//! `ProjectRepository` trait — persistence contract for the [`Project`] entity.
//!
//! Defines the data-access operations the application layer requires for project
//! management. Implementations live in `infrastructure::postgres::repositories`.

use async_trait::async_trait;

use crate::application::repositories::RepositoryResult;
use crate::domain::entities::project::Project;

/// Repository interface for [`Project`] persistence.
///
/// All methods return [`RepositoryResult`] and are `Send + Sync` so they
/// can be called from async handlers behind `Arc<dyn ProjectRepository>`.
#[async_trait]
pub trait ProjectRepository: Send + Sync {
    /// Look up a project by primary key.
    ///
    /// Returns `None` when no active (non-deleted) project exists with the given `id`.
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<Project>>;

    /// Look up a project by name (case-insensitive).
    ///
    /// Returns `None` when no active project exists with the given `name`.
    async fn find_by_name(&self, name: &str) -> RepositoryResult<Option<Project>>;

    /// Insert a new project row.
    ///
    /// The returned [`Project`] will have the `id` field populated by the database.
    async fn insert(&self, project: &Project) -> RepositoryResult<Project>;

    /// Persist changes to an existing project.
    ///
    /// Returns the updated row. Returns `NotFound` if the project is soft-deleted.
    async fn update(&self, project: &Project) -> RepositoryResult<Project>;

    /// Soft-delete a project by setting `deleted_at` and `deleted_by`.
    ///
    /// Returns `NotFound` if the project is already soft-deleted or does not exist.
    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()>;

    /// Return a paginated list of projects the user is a member of, with total count.
    ///
    /// If `status_filter` is provided, only projects matching that status are returned.
    /// For System Admin, returns all non-deleted projects regardless of membership.
    async fn list_by_user(
        &self,
        user_id: i64,
        page: u32,
        limit: u32,
        status_filter: Option<&str>,
    ) -> RepositoryResult<(Vec<Project>, u64)>;

    /// Return all non-deleted projects (System Admin bypass).
    async fn list_all(
        &self,
        page: u32,
        limit: u32,
        status_filter: Option<&str>,
    ) -> RepositoryResult<(Vec<Project>, u64)>;

    /// Count the number of active (non-deleted) members in a project.
    async fn member_count(&self, project_id: i64) -> RepositoryResult<u64>;
}
