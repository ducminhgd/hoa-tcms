//! `SessionStore` trait — persistence of authenticated user sessions.
//!
//! Sessions are created on successful login and used to authenticate
//! subsequent requests. This trait abstracts over the storage backend
//! (e.g. Redis, in-memory cache, database).

use async_trait::async_trait;
use uuid::Uuid;

use crate::application::services::errors::ServiceError;
use crate::domain::entities::session::Session;

/// Persistence operations for user sessions.
///
/// Sessions are short-lived (typically a few hours) and should be stored
/// in a fast, distributed cache. The `Session` entity uses uuid `Uuid`
/// as its session identifier.
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Create a new session for the given user.
    ///
    /// Generates a new `Session` entity (with a fresh UUID and timestamp),
    /// persists it, and returns the entity back to the caller.
    ///
    /// `fingerprint` is an optional device/browser fingerprint string used
    /// for session theft detection.
    ///
    /// # Errors
    ///
    /// Returns `Err(ServiceError::Cache)` if the session could not be
    /// persisted (e.g. cache is unreachable).
    async fn create_session(
        &self,
        user_id: i64,
        fingerprint: Option<&str>,
    ) -> Result<Session, ServiceError>;

    /// Retrieve a session by its ID.
    ///
    /// Returns `Ok(Some(session))` if the session exists and has not expired.
    /// Returns `Ok(None)` if the session does not exist or has expired.
    ///
    /// # Errors
    ///
    /// Returns `Err(ServiceError::Cache)` if the lookup could not be
    /// completed (e.g. cache is unreachable).
    async fn get_session(&self, session_id: &Uuid) -> Result<Option<Session>, ServiceError>;

    /// Delete a single session.
    ///
    /// This is used on logout. It is a no-op if the session does not exist.
    ///
    /// # Errors
    ///
    /// Returns `Err(ServiceError::Cache)` if the deletion could not be completed.
    async fn delete_session(&self, session_id: &Uuid) -> Result<(), ServiceError>;

    /// Delete all sessions belonging to a user.
    ///
    /// This is used when a user changes their password or when an admin
    /// forcibly terminates a user's sessions.
    ///
    /// Returns the number of sessions that were deleted.
    ///
    /// # Errors
    ///
    /// Returns `Err(ServiceError::Cache)` if the deletion could not be completed.
    async fn delete_all_user_sessions(&self, user_id: i64) -> Result<u64, ServiceError>;
}
