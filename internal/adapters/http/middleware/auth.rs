//! Authentication helpers — session validation and user context extraction.
//!
//! Handlers call [`extract_user_id`] to validate the session cookie and
//! retrieve the authenticated user's ID. This is a utility module, not
//! Actix middleware — it's called explicitly by each handler that needs auth.

use actix_web::{HttpRequest, web};
use std::sync::Arc;
use uuid::Uuid;

use crate::application::services::session_store::SessionStore;
use hoa_tcms_pkg::errors::ApiError;

/// Public paths that do not require authentication.
const PUBLIC_PATHS: &[(&str, &str)] = &[("/api/v1/health", "GET"), ("/api/v1/auth/login", "POST")];

/// Returns `true` if the path is public (no auth required).
pub fn is_public_path(path: &str, method: &str) -> bool {
    PUBLIC_PATHS.iter().any(|(p, m)| path == *p && method == *m)
}

/// Extract the authenticated user ID from the request's session cookie.
///
/// Returns `Ok(user_id)` if a valid session exists.
/// Returns `Err(ApiError)` with `401 Unauthorized` if no valid session is found.
pub async fn extract_user_id(req: &HttpRequest) -> Result<i64, ApiError> {
    // Check for public paths first.
    if is_public_path(req.path(), req.method().as_str()) {
        return Ok(0); // sentinel — caller should handle public paths
    }

    // Get session store from app data.
    let store = req
        .app_data::<web::Data<Arc<dyn SessionStore>>>()
        .ok_or_else(|| {
            tracing::error!("SessionStore not found in app data");
            ApiError::internal("authentication service unavailable")
        })?;

    // Extract session cookie.
    let session_id = req
        .cookie("session")
        .and_then(|c| Uuid::parse_str(c.value()).ok())
        .ok_or_else(|| ApiError {
            code: "NOT_AUTHENTICATED".into(),
            message: "not authenticated".into(),
            details: None,
        })?;

    // Validate session.
    match store.get_session(&session_id).await {
        Ok(Some(session)) => Ok(session.user_id),
        Ok(None) => Err(ApiError {
            code: "NOT_AUTHENTICATED".into(),
            message: "session expired or invalid".into(),
            details: None,
        }),
        Err(e) => {
            tracing::error!(error = %e, "session store error");
            Err(ApiError::internal("session validation failed"))
        }
    }
}
