//! `UserRepository` trait — persistence contract for the [`User`] entity.
//!
//! Defines the data-access operations the application layer requires for user
//! management. Implementations live in `infrastructure::postgres::repositories`.

use async_trait::async_trait;

use crate::application::repositories::RepositoryResult;
use crate::domain::entities::user::User;

/// Repository interface for [`User`] persistence.
///
/// All methods return [`RepositoryResult`] and are `Send + Sync` so they
/// can be called from async handlers behind `Arc<dyn UserRepository>`.
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Look up a user by primary key.
    ///
    /// Returns `None` when no user exists with the given `id`.
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<User>>;

    /// Look up a user by unique username.
    ///
    /// Returns `None` when no user exists with the given `username`.
    async fn find_by_username(&self, username: &str) -> RepositoryResult<Option<User>>;

    /// Look up a user by unique email address.
    ///
    /// Returns `None` when no user exists with the given `email`.
    async fn find_by_email(&self, email: &str) -> RepositoryResult<Option<User>>;

    /// Insert a new user row.
    ///
    /// The returned [`User`] will have the `id` field populated by the database.
    async fn create(&self, user: &User) -> RepositoryResult<User>;

    /// Persist changes to an existing user.
    ///
    /// Returns the updated row (with `updated_at` bumped by the database).
    async fn update(&self, user: &User) -> RepositoryResult<User>;

    /// Soft-delete a user by setting `deleted_at` and `deleted_by`.
    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()>;

    /// Return a paginated list of users together with the total count.
    async fn list_paginated(&self, page: u32, limit: u32) -> RepositoryResult<(Vec<User>, u64)>;

    /// Replace the `password_hash` for the user identified by `id`.
    ///
    /// This is the only method that mutates a single column in isolation.
    async fn update_password_hash(&self, id: i64, password_hash: &str) -> RepositoryResult<()>;
}
