//! `ServiceError` — typed errors returned by application services.
//!
//! Application services (session store, permission resolver, authorization)
//! use this enum instead of raw `String` to enable typed error handling and
//! consistent error propagation.

use crate::application::repositories::RepositoryError;

/// Errors originating from application services.
///
/// Variants are kept broad because services have limited domain context:
/// a session store knows it had a cache failure, but cannot determine the
/// root cause (network, auth, timeout, etc.). Callers that need finer
/// granularity should map these variants to their own error types.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ServiceError {
    /// A database operation failed.
    #[error("database error: {0}")]
    Database(String),

    /// A cache (Redis) operation failed.
    #[error("cache error: {0}")]
    Cache(String),

    /// The requested action is not permitted.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// The requested resource was not found.
    #[error("not found")]
    NotFound,

    /// A uniqueness constraint was violated (conflict).
    #[error("conflict: {0}")]
    Conflict(String),

    /// Input validation failed.
    #[error("validation error: {0}")]
    Validation(String),

    /// An unexpected internal error occurred.
    #[error("internal error: {0}")]
    Internal(String),
}

// Central conversion from RepositoryError → ServiceError.
// This is used by all application services via `.map_err(ServiceError::from)`.
impl From<RepositoryError> for ServiceError {
    fn from(e: RepositoryError) -> Self {
        match e {
            RepositoryError::NotFound => ServiceError::NotFound,
            RepositoryError::Duplicate(msg) => ServiceError::Conflict(msg),
            RepositoryError::Forbidden(msg) => ServiceError::PermissionDenied(msg),
            RepositoryError::Database(details) => {
                tracing::error!(error.details = %details, "repository database error");
                ServiceError::Database(details)
            }
            RepositoryError::Connection(details) => {
                tracing::error!(error.details = %details, "repository connection error");
                ServiceError::Internal("an internal error occurred".into())
            }
        }
    }
}
