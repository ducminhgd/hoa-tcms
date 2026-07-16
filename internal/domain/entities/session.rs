//! Session entity — represents an authenticated user session.
//!
//! Sessions are created on successful login and used for request
//! authentication. This is a pure domain entity with **no framework imports**.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An authenticated user session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: Uuid,
    pub user_id: i64,
    pub created_at: DateTime<Utc>,
    pub fingerprint: Option<String>,
}

impl Session {
    /// Create a new session for the given user.
    ///
    /// Generates a UUID v4 as the session identifier and sets the creation
    /// timestamp to the current time.
    pub fn new(user_id: i64, fingerprint: Option<String>) -> Self {
        Self {
            session_id: Uuid::new_v4(),
            user_id,
            created_at: Utc::now(),
            fingerprint,
        }
    }
}
