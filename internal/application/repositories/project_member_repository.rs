//! `ProjectMemberRepository` trait — persistence for project membership.
//!
//! Manages the many-to-many relationship between users and projects stored in
//! the `project_members` junction table.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::application::repositories::RepositoryResult;

/// A project member record as returned by the repository.
#[derive(Clone, Debug)]
pub struct ProjectMemberRow {
    pub project_id: i64,
    pub user_id: i64,
    pub username: String,
    pub fullname: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// Manages the many-to-many relationship between users and projects.
///
/// Uses hard delete — no soft-delete on junction table records.
#[async_trait]
pub trait ProjectMemberRepository: Send + Sync {
    /// Add a user as a member of a project with the given role.
    ///
    /// # Errors
    ///
    /// Returns `Duplicate` if the user is already a member.
    async fn add_member(&self, project_id: i64, user_id: i64, role: &str) -> RepositoryResult<()>;

    /// Remove a user from a project.
    ///
    /// This is a no-op if the user is not a member (idempotent).
    async fn remove_member(&self, project_id: i64, user_id: i64) -> RepositoryResult<()>;

    /// Change a member's role.
    ///
    /// Returns `NotFound` if the user is not a member of the project.
    async fn change_role(&self, project_id: i64, user_id: i64, role: &str) -> RepositoryResult<()>;

    /// List all members of a project with their user details.
    ///
    /// Returns only active, non-deleted users.
    async fn list_by_project(&self, project_id: i64) -> RepositoryResult<Vec<ProjectMemberRow>>;

    /// Get a single member's role.
    ///
    /// Returns `None` if the user is not a member.
    async fn get_member(
        &self,
        project_id: i64,
        user_id: i64,
    ) -> RepositoryResult<Option<ProjectMemberRow>>;

    /// Count members with a specific role in a project.
    async fn count_by_role(&self, project_id: i64, role: &str) -> RepositoryResult<u64>;

    /// Count all members in a project.
    async fn count_all(&self, project_id: i64) -> RepositoryResult<u64>;
}
