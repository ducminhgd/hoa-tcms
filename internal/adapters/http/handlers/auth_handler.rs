//! HTTP handlers for authentication (login, logout).

use actix_web::{HttpRequest, HttpResponse, cookie, web};
use std::sync::Arc;
use uuid::Uuid;

use crate::application::dto::auth::{LoginRequest, LoginResponse};
use crate::application::services::auth_service::AuthService;
use crate::application::services::errors::ServiceError;
use hoa_tcms_pkg::errors::{ApiError, FieldError};

/// The session cookie name (must match the middleware's lookup).
const SESSION_COOKIE: &str = "session";

/// HTTP handler for authentication endpoints.
pub struct AuthHandler {
    service: Arc<AuthService>,
}

impl AuthHandler {
    /// Create a new `AuthHandler`.
    pub fn new(service: Arc<AuthService>) -> Self {
        Self { service }
    }

    /// `POST /api/v1/auth/login` — authenticate and create a session cookie.
    pub async fn login(&self, req: HttpRequest, body: web::Json<LoginRequest>) -> HttpResponse {
        if let Err(errors) = body.validate() {
            let details: Vec<FieldError> = errors
                .into_iter()
                .map(|m| FieldError {
                    field: "body".into(),
                    message: m,
                })
                .collect();
            return HttpResponse::UnprocessableEntity()
                .json(ApiError::validation("invalid request", details));
        }

        // Optional device fingerprint used for session-theft detection.
        let fingerprint = req
            .headers()
            .get("x-device-fingerprint")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        match self
            .service
            .login(
                &body.username_or_email,
                &body.password,
                fingerprint.as_deref(),
            )
            .await
        {
            Ok((user, session)) => {
                let resp = LoginResponse {
                    id: user.id,
                    username: user.username,
                    email: user.email,
                    fullname: user.fullname,
                    status: user.status.to_string(),
                };
                let session_cookie =
                    cookie::Cookie::build(SESSION_COOKIE, session.session_id.to_string())
                        .path("/")
                        .http_only(true)
                        .same_site(cookie::SameSite::Lax)
                        .max_age(cookie::time::Duration::days(1))
                        .finish();
                HttpResponse::Ok()
                    .cookie(session_cookie)
                    .json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_error(e),
        }
    }

    /// `POST /api/v1/auth/logout` — destroy the session and clear the cookie.
    ///
    /// Idempotent: succeeds even when no (valid) session is present.
    pub async fn logout(&self, req: HttpRequest) -> HttpResponse {
        if let Some(session_id) = req
            .cookie(SESSION_COOKIE)
            .and_then(|c| Uuid::parse_str(c.value()).ok())
            && let Err(e) = self.service.logout(&session_id).await
        {
            tracing::error!(error = %e, "failed to destroy session on logout");
        }

        let clear_cookie = cookie::Cookie::build(SESSION_COOKIE, "")
            .path("/")
            .http_only(true)
            .same_site(cookie::SameSite::Lax)
            .max_age(cookie::time::Duration::seconds(0))
            .finish();

        HttpResponse::NoContent().cookie(clear_cookie).finish()
    }
}

/// Map service errors to HTTP responses.
///
/// Both "unknown user" and "wrong password" are surfaced as a generic
/// `401 Unauthorized` to prevent username enumeration.
fn map_error(e: ServiceError) -> HttpResponse {
    match e {
        ServiceError::NotFound => HttpResponse::Unauthorized().json(ApiError {
            code: "INVALID_CREDENTIALS".into(),
            message: "invalid credentials".into(),
            details: None,
        }),
        ServiceError::PermissionDenied(msg) => {
            HttpResponse::Forbidden().json(ApiError::forbidden(msg))
        }
        ServiceError::Validation(msg) => {
            HttpResponse::UnprocessableEntity().json(ApiError::validation(msg, vec![]))
        }
        ServiceError::Cache(details) => {
            tracing::error!(error.details = %details, "cache error in auth handler");
            HttpResponse::ServiceUnavailable().json(ApiError {
                code: "SERVICE_UNAVAILABLE".into(),
                message: "temporarily unavailable".into(),
                details: None,
            })
        }
        ServiceError::Database(details) => {
            tracing::error!(error.details = %details, "database error in auth handler");
            HttpResponse::InternalServerError()
                .json(ApiError::internal("an internal error occurred"))
        }
        ServiceError::Conflict(msg) | ServiceError::Internal(msg) => {
            HttpResponse::InternalServerError().json(ApiError::internal(msg))
        }
    }
}
