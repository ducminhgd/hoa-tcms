//! Validation helpers — input sanitisation and format checks.

use std::fmt;
use validator::ValidateEmail;

/// Validate that a string is a syntactically valid email address.
pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    if email.is_empty() {
        return Err(ValidationError("email must not be empty".into()));
    }
    if email.len() > 254 {
        return Err(ValidationError(
            "email must not exceed 254 characters".into(),
        ));
    }
    if !email.validate_email() {
        return Err(ValidationError(
            "email must be a valid email address".into(),
        ));
    }
    Ok(())
}

/// Validate password strength.
pub fn validate_password(password: &str) -> Result<(), ValidationError> {
    if password.len() < 8 {
        return Err(ValidationError(
            "password must be at least 8 characters".into(),
        ));
    }
    if password.len() > 128 {
        return Err(ValidationError(
            "password must not exceed 128 characters".into(),
        ));
    }
    Ok(())
}

/// Validate that a string is non-empty and within length bounds.
pub fn validate_length(
    value: &str,
    field_name: &str,
    min: usize,
    max: usize,
) -> Result<(), ValidationError> {
    let len = value.len();
    if len < min {
        return Err(ValidationError(format!(
            "{} must be at least {} characters",
            field_name, min
        )));
    }
    if len > max {
        return Err(ValidationError(format!(
            "{} must not exceed {} characters",
            field_name, max
        )));
    }
    Ok(())
}

/// Validate that a username contains only permitted characters and is within length bounds.
pub fn validate_username(username: &str) -> Result<(), ValidationError> {
    validate_length(username, "username", 3, 100)?;
    if !username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(ValidationError(
            "username may only contain letters, digits, underscores, hyphens, and dots".into(),
        ));
    }
    Ok(())
}

/// A simple validation error with a human-readable message.
#[derive(Debug, Clone)]
pub struct ValidationError(pub String);

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ValidationError {}
