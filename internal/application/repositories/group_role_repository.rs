//! `GroupRoleRepository` trait — manages role assignments to groups.
//!
//! Roles are assigned to groups via the `group_roles` junction table.
//! Users inherit the roles of every group they belong to.

use crate::application::repositories::RepositoryResult;
use async_trait::async_trait;

/// Manages the many-to-many relationship between groups and roles.
///
/// Roles define permissions; groups are the unit of assignment. When a role
/// is assigned to a group, every member of that group inherits the role's
/// permissions. This is separate from direct role-to-user assignments.
#[async_trait]
pub trait GroupRoleRepository: Send + Sync {
    /// Assign a role to a group.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Duplicate` if the role is already assigned
    /// to the group.
    /// Returns `RepositoryError::NotFound` if either the group or role does
    /// not exist (when FK constraints are enforced).
    async fn add(&self, group_id: i64, role_id: i64) -> RepositoryResult<()>;

    /// Remove a role assignment from a group.
    ///
    /// This is a no-op if the role is not assigned to the group.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` on unexpected database failures.
    async fn remove(&self, group_id: i64, role_id: i64) -> RepositoryResult<()>;

    /// List all roles assigned to a group.
    ///
    /// Returns a list of `(role_id, role_name)` tuples for roles currently
    /// assigned to the group.
    ///
    /// Returns an empty `Vec` if the group exists but has no role assignments.
    async fn find_by_group(&self, group_id: i64) -> RepositoryResult<Vec<(i64, String)>>;

    /// Count the number of roles assigned to a group.
    ///
    /// Returns `0` if the group has no role assignments or does not exist.
    async fn count_by_group(&self, group_id: i64) -> RepositoryResult<u64>;

    /// Validate that a set of role IDs exist in the system.
    ///
    /// Returns the subset of `role_ids` that correspond to existing,
    /// non-deleted roles. IDs that do not match any role are silently
    /// excluded from the result.
    async fn validate_roles_exist(&self, role_ids: &[i64]) -> RepositoryResult<Vec<i64>>;
}
