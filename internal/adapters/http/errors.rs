//! Error mapping from domain/repository errors to HTTP API errors.
//!
//! This module owns the conversion from internal error types to
//! `ApiError` — the HTTP presentation concern. The application
//! layer returns domain/repository errors; this layer translates them.

use hoa_tcms_pkg::errors::ApiError;

use crate::application::repositories::RepositoryError;

impl From<RepositoryError> for ApiError {
    fn from(err: RepositoryError) -> Self {
        match err {
            RepositoryError::NotFound => ApiError::not_found("resource not found"),
            RepositoryError::Duplicate(msg) => ApiError::conflict(msg),
            RepositoryError::Database(details) => {
                tracing::error!(
                    error.details = %details,
                    error.variant = "database",
                    "repository error mapped to generic API error"
                );
                ApiError::internal("an internal error occurred")
            }
            RepositoryError::Connection(details) => {
                tracing::error!(
                    error.details = %details,
                    error.variant = "connection",
                    "repository error mapped to generic API error"
                );
                ApiError::internal("an internal error occurred")
            }
        }
    }
}
