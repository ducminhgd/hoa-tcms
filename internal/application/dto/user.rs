//! User DTOs — data transfer objects for user CRUD and profile management.
//!
//! Request DTOs define the expected JSON shape for user-related endpoints.
//! Response DTOs define the public representation of user data.

use chrono::{DateTime, Utc};
use hoa_tcms_pkg::validation::{validate_email, validate_password, validate_username};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------
// Request DTOs
// ---------------------------------------------------------------

/// Create a new user (admin only).
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub fullname: String,
}

/// Update a user's account details (admin only).
///
/// All fields are optional — only provided fields will be updated.
#[derive(Debug, Deserialize)]
pub struct UpdateAdminRequest {
    pub username: Option<String>,
    pub email: Option<String>,
    pub fullname: Option<String>,
    pub status: Option<String>,
}

/// Update the authenticated user's own profile.
///
/// All fields are optional — only provided fields will be updated.
#[derive(Debug, Deserialize)]
pub struct UpdateSelfRequest {
    pub email: Option<String>,
    pub fullname: Option<String>,
}

/// Change the authenticated user's password.
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

impl CreateUserRequest {
    /// Validate all fields, returning a list of field-level errors.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if let Err(e) = validate_username(&self.username) {
            errors.push(e.0);
        }
        if let Err(e) = validate_email(&self.email) {
            errors.push(e.0);
        }
        if let Err(e) = validate_password(&self.password) {
            errors.push(e.0);
        }
        if self.fullname.trim().is_empty() {
            errors.push("fullname is required".to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl UpdateAdminRequest {
    /// Validate optional fields if they are present.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if let Some(ref username) = self.username
            && let Err(e) = validate_username(username)
        {
            errors.push(e.0);
        }
        if let Some(ref email) = self.email
            && let Err(e) = validate_email(email)
        {
            errors.push(e.0);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl UpdateSelfRequest {
    /// Validate optional fields if they are present.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if let Some(ref email) = self.email
            && let Err(e) = validate_email(email)
        {
            errors.push(e.0);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl ChangePasswordRequest {
    /// Validate that the current password is present and the new password
    /// meets strength requirements.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.current_password.is_empty() {
            errors.push("current_password is required".to_string());
        }
        if let Err(e) = validate_password(&self.new_password) {
            errors.push(e.0);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ---------------------------------------------------------------
// Response DTOs
// ---------------------------------------------------------------

/// Full user profile returned in single-resource responses.
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub fullname: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Lightweight user data returned in list responses.
#[derive(Debug, Serialize)]
pub struct UserListItem {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub fullname: String,
    pub status: String,
}
