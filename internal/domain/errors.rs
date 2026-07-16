//! Domain errors — typed errors for business-rule violations.

use thiserror::Error;

/// Generic domain error.
#[derive(Error, Debug)]
pub enum DomainError {
    /// The requested entity was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// A uniqueness constraint was violated.
    #[error("duplicate value: {0}")]
    Duplicate(String),

    /// An operation was attempted on an entity in an invalid state.
    #[error("invalid state: {0}")]
    InvalidState(String),

    /// Validation failed.
    #[error("validation error: {0}")]
    Validation(String),

    /// The caller lacks permission.
    #[error("forbidden: {0}")]
    Forbidden(String),

    /// The operation is not supported.
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}
