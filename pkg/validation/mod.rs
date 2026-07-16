//! Validation helpers — input sanitisation and format checks.

use std::fmt;

/// Validate that a string is a syntactically valid email address.
///
/// This is a basic check; Phase 2 may replace it with a dedicated
/// validation library.
pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    if email.is_empty() {
        return Err(ValidationError("email must not be empty".into()));
    }
    // Minimal check: contains exactly one '@' with non-empty local and domain parts.
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(ValidationError(
            "email must be a valid email address".into(),
        ));
    }
    let domain = parts[1];
    if !domain.contains('.') {
        return Err(ValidationError("email domain must contain a dot".into()));
    }
    Ok(())
}

/// Validate minimum password length.
pub fn validate_password(password: &str) -> Result<(), ValidationError> {
    if password.len() < 8 {
        return Err(ValidationError(
            "password must be at least 8 characters".into(),
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

/// Validate that a username contains only permitted characters.
pub fn validate_username(username: &str) -> Result<(), ValidationError> {
    if username.is_empty() {
        return Err(ValidationError("username must not be empty".into()));
    }
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
