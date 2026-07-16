//! `RoleRepository` trait — persistence contract for the [`Role`] entity.
//!
//! Roles are the unit of authorization — permissions are granted to roles,
//! and roles are assigned to groups/users via the `group_roles` / `user_roles`
//! junction tables.

use async_trait::async_trait;

use crate::application::repositories::RepositoryResult;
use crate::domain::entities::permission::Permission;
use crate::domain::entities::role::Role;

/// Repository interface for [`Role`] persistence.
///
/// All methods return [`RepositoryResult`] and are `Send + Sync` so they
/// can be called from async handlers behind `Arc<dyn RoleRepository>`.
#[async_trait]
pub trait RoleRepository: Send + Sync {
    /// Look up a role by primary key.
    ///
    /// Returns `None` when no role exists with the given `id`.
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<Role>>;

    /// Look up a role by unique name.
    ///
    /// Returns `None` when no role exists with the given `name`.
    async fn find_by_name(&self, name: &str) -> RepositoryResult<Option<Role>>;

    /// Insert a new role row.
    ///
    /// The returned [`Role`] will have the `id` field populated by the database.
    async fn save(&self, role: &Role) -> RepositoryResult<Role>;

    /// Persist changes to an existing role.
    ///
    /// Returns the updated row (with `updated_at` bumped by the database).
    async fn update(&self, role: &Role) -> RepositoryResult<Role>;

    /// Return a paginated list of roles together with the total count.
    async fn find_all(&self, page: u32, limit: u32) -> RepositoryResult<(Vec<Role>, u64)>;

    /// Atomically replace the permission set assigned to the given role.
    ///
    /// This runs inside a transaction: all existing entries in the
    /// `role_permissions` junction table for this role are removed and the
    /// provided `permission_ids` are inserted.
    async fn set_permissions(&self, role_id: i64, permission_ids: &[i64]) -> RepositoryResult<()>;

    /// Return all permissions currently assigned to the given role.
    async fn get_permissions(&self, role_id: i64) -> RepositoryResult<Vec<Permission>>;
}
