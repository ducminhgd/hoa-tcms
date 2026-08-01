//! Auth DTOs — login request/response contracts.
//!
//! These DTOs represent the authentication boundary. The login response
//! returns a flat user representation (no password hash, no timestamps).

use serde::{Deserialize, Serialize};

/// Login request payload.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Username or email address used for authentication.
    pub username_or_email: String,
    /// Plain-text password.
    pub password: String,
}

impl LoginRequest {
    /// Maximum accepted password length, in bytes.
    ///
    /// Bounds the cost of PBKDF2 verification (which scales with input size) so
    /// an unauthenticated caller cannot send a multi-megabyte password and tie
    /// up a blocking worker for seconds per request.
    pub const MAX_PASSWORD_LEN: usize = 1024;

    /// Validate that required fields are present and within bounds.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.username_or_email.trim().is_empty() {
            errors.push("username_or_email is required".to_string());
        }
        if self.password.is_empty() {
            errors.push("password is required".to_string());
        } else if self.password.len() > Self::MAX_PASSWORD_LEN {
            errors.push(format!(
                "password must be at most {} characters",
                Self::MAX_PASSWORD_LEN
            ));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Login response payload.
///
/// Returned on successful authentication. Excludes the password hash and
/// audit timestamps.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub fullname: String,
    pub status: String,
}
