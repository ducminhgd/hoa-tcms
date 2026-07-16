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
    ///
    /// This variant wraps [`RepositoryError`] so callers can recover the
    /// typed repository error if needed.
    #[error("database error: {0}")]
    Database(#[from] RepositoryError),

    /// A cache (Redis) operation failed.
    #[error("cache error: {0}")]
    Cache(String),

    /// The requested action is not permitted.
    #[error("permission denied: {0}")]
    PermissionDenied(String),
}
