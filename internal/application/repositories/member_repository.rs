//! `MemberRepository` trait — manages user-group membership.
//!
//! Groups contain users via a many-to-many relationship stored in the
//! `user_groups` junction table. This repository handles that relationship.

use crate::application::repositories::RepositoryResult;
use async_trait::async_trait;

/// Manages the many-to-many relationship between users and groups.
///
/// Users can belong to multiple groups, and groups contain multiple users.
/// This repository only handles the membership itself — user and group
/// CRUD is handled by their respective repositories.
#[async_trait]
pub trait MemberRepository: Send + Sync {
    /// Add a user to a group.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Duplicate` if the user is already a member.
    /// Returns `RepositoryError::NotFound` if either the user or group does
    /// not exist (when FK constraints are enforced).
    async fn add(&self, group_id: i64, user_id: i64) -> RepositoryResult<()>;

    /// Remove a user from a group.
    ///
    /// This is a no-op if the user is not a member of the group.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` on unexpected database failures.
    async fn remove(&self, group_id: i64, user_id: i64) -> RepositoryResult<()>;

    /// List all member IDs and usernames for a group.
    ///
    /// Returns a list of `(user_id, username)` tuples. Includes only active,
    /// non-deleted users who belong to an active group.
    ///
    /// Returns an empty `Vec` if the group has no members or does not exist.
    async fn find_by_group(&self, group_id: i64) -> RepositoryResult<Vec<(i64, String)>>;

    /// Count the number of members in a group.
    ///
    /// Returns `0` if the group has no members or does not exist.
    async fn count_by_group(&self, group_id: i64) -> RepositoryResult<u64>;

    /// Validate that a set of user IDs exist in the system.
    ///
    /// Returns the subset of `user_ids` that correspond to existing,
    /// non-deleted users. IDs that do not match any user are silently
    /// excluded from the result.
    async fn validate_users_exist(&self, user_ids: &[i64]) -> RepositoryResult<Vec<i64>>;
}
