//! `GroupRepository` trait — persistence contract for the [`Group`] entity.
//!
//! Groups are the primary unit of access control — roles are assigned to
//! groups, and users inherit those roles through group membership.

use async_trait::async_trait;

use crate::application::repositories::RepositoryResult;
use crate::domain::entities::group::Group;
use crate::domain::value_objects::group_status::GroupStatus;

/// Repository interface for [`Group`] persistence.
///
/// All methods return [`RepositoryResult`] and are `Send + Sync` so they
/// can be called from async handlers behind `Arc<dyn GroupRepository>`.
#[async_trait]
pub trait GroupRepository: Send + Sync {
    /// Look up a group by primary key.
    ///
    /// Returns `None` when no group exists with the given `id`.
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<Group>>;

    /// Look up a group by unique name.
    ///
    /// Returns `None` when no group exists with the given `name`.
    async fn find_by_name(&self, name: &str) -> RepositoryResult<Option<Group>>;

    /// Insert a new group row.
    ///
    /// The returned [`Group`] will have the `id` field populated by the database.
    async fn save(&self, group: &Group) -> RepositoryResult<Group>;

    /// Persist changes to an existing group.
    ///
    /// Returns the updated row (with `updated_at` bumped by the database).
    async fn update(&self, group: &Group) -> RepositoryResult<Group>;

    /// Soft-delete a group by setting `deleted_at` and `deleted_by`.
    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()>;

    /// Return a filtered, paginated list of groups together with the total
    /// count.
    ///
    /// # Parameters
    ///
    /// * `page` — 1-based page number.
    /// * `limit` — page size (capped server-side to [`MAX_PAGE_SIZE`]).
    /// * `search` — optional substring filter applied against the group name.
    /// * `status_filter` — optional filter by group lifecycle status.
    async fn find_all(
        &self,
        page: u32,
        limit: u32,
        search: Option<&str>,
        status_filter: Option<GroupStatus>,
    ) -> RepositoryResult<(Vec<Group>, u64)>;
}
