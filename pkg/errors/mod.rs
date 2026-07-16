//! Shared error types and helpers.

use std::fmt;

/// Standard API error response body.
#[derive(Debug, serde::Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<FieldError>>,
}

/// A single field-level validation error.
#[derive(Debug, serde::Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

impl ApiError {
    /// Create a validation error with field-level details.
    pub fn validation(message: impl Into<String>, details: Vec<FieldError>) -> Self {
        Self {
            code: "VALIDATION_ERROR".into(),
            message: message.into(),
            details: Some(details),
        }
    }

    /// Create a generic bad-request error.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            code: "BAD_REQUEST".into(),
            message: message.into(),
            details: None,
        }
    }

    /// Create a "not found" error.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: "NOT_FOUND".into(),
            message: message.into(),
            details: None,
        }
    }

    /// Create a "forbidden" error.
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            code: "FORBIDDEN".into(),
            message: message.into(),
            details: None,
        }
    }

    /// Create a "conflict" error.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            code: "CONFLICT".into(),
            message: message.into(),
            details: None,
        }
    }

    /// Create an internal-server-error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: "INTERNAL_ERROR".into(),
            message: message.into(),
            details: None,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}
