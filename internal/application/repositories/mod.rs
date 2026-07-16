//! Repository interfaces (ports).
//!
//! These traits define the contract between the application layer and
//! persistence. Implementations live in `infrastructure::postgres::repositories`.
//!
//! # Error handling
//!
//! `RepositoryError::Database` and `RepositoryError::Connection` carry raw
//! database error messages (table names, column names, SQL state codes).
//! These MUST be mapped to generic messages before reaching HTTP handlers
//! to avoid leaking internal database details to clients.
//!
//! Use the `From<RepositoryError> for ApiError` conversion defined below,
//! which logs the sensitive details and returns safe, generic messages.

/// Common repository result type.
pub type RepositoryResult<T> = Result<T, RepositoryError>;

/// Errors originating from the repository layer.
///
/// **IMPORTANT**: The `Database` and `Connection` variants carry raw
/// database error messages. Before converting to an HTTP response, use
/// the `From<RepositoryError> for ApiError` impl to map sensitive
/// details to safe, generic messages.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("not found")]
    NotFound,

    #[error("duplicate key: {0}")]
    Duplicate(String),

    #[error("database error: {0}")]
    Database(String),

    #[error("connection error: {0}")]
    Connection(String),
}

// The `From<RepositoryError> for ApiError` conversion lives in
// `internal/adapters/http/errors.rs` — the adapter layer is the correct
// place for mapping internal errors to HTTP presentation types.
