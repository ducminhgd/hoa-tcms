//! User entity — represents a registered user in the system.
//!
//! This is a pure domain entity with **no ORM or framework imports**.

use chrono::{DateTime, Utc};

use crate::domain::value_objects::user_status::UserStatus;

/// A registered user.
///
/// Maps to the `users` table in the database. The `id` field is `0` until
/// the entity is persisted.
#[derive(Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub fullname: String,
    pub status: UserStatus,
    pub created_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<i64>,
    pub updated_at: DateTime<Utc>,
    pub deleted_by: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("username", &self.username)
            .field("email", &self.email)
            .field("password_hash", &"<redacted>")
            .field("fullname", &self.fullname)
            .field("status", &self.status)
            .field("created_by", &self.created_by)
            .field("created_at", &self.created_at)
            .field("updated_by", &self.updated_by)
            .field("updated_at", &self.updated_at)
            .field("deleted_by", &self.deleted_by)
            .field("deleted_at", &self.deleted_at)
            .finish()
    }
}

impl User {
    /// Create a new active user.
    ///
    /// This is the only way to construct a `User` in the domain layer.
    /// The caller is responsible for hashing the password before passing it in.
    ///
    /// The user is created with status `Active` and timestamps set to the
    /// current time. The `id` is left at `0` and will be assigned by the
    /// persistence layer.
    pub fn create(
        username: String,
        email: String,
        password_hash: String,
        fullname: String,
        created_by: Option<i64>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            username,
            email,
            password_hash,
            fullname,
            status: UserStatus::Active,
            created_by,
            created_at: now,
            updated_by: created_by,
            updated_at: now,
            deleted_by: None,
            deleted_at: None,
        }
    }

    /// Returns `true` if the user is active and not soft-deleted.
    pub fn is_active(&self) -> bool {
        self.status == UserStatus::Active && self.deleted_at.is_none()
    }
}
