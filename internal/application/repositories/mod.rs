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

// ---------------------------------------------------------------------------
// Conversion to API errors
// ---------------------------------------------------------------------------
// The `From` impl is valid here because `RepositoryError` (the type parameter
// of `From`) is defined in this crate, satisfying the orphan rule even though
// `ApiError` is defined in the `hoa-tcms-pkg` crate.
//
// Sensitive database details (table names, SQL state codes) from the
// `Database` and `Connection` variants are logged and replaced with a
// generic message to prevent information leakage in HTTP responses.

impl From<RepositoryError> for hoa_tcms_pkg::errors::ApiError {
    fn from(err: RepositoryError) -> Self {
        match err {
            RepositoryError::NotFound => {
                hoa_tcms_pkg::errors::ApiError::not_found("resource not found")
            }
            RepositoryError::Duplicate(msg) => hoa_tcms_pkg::errors::ApiError::conflict(msg),
            RepositoryError::Database(details) => {
                tracing::error!(
                    error.details = %details,
                    error.variant = "database",
                    "repository error mapped to generic API error"
                );
                hoa_tcms_pkg::errors::ApiError::internal("an internal error occurred")
            }
            RepositoryError::Connection(details) => {
                tracing::error!(
                    error.details = %details,
                    error.variant = "connection",
                    "repository error mapped to generic API error"
                );
                hoa_tcms_pkg::errors::ApiError::internal("an internal error occurred")
            }
        }
    }
}
