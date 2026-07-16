//! Central router — all HTTP routes registered here.
//!
//! This module wires handlers, middleware, and path prefixes into the
//! Actix-Web application.

use crate::infrastructure::config::AppState;
use actix_web::web;

/// Register all API and SSR routes on the given `ServiceConfig`.
pub fn configure(cfg: &mut web::ServiceConfig) {
    // Placeholder — routes will be registered in Milestone 2+.
    cfg.service(
        web::scope("/api/v1")
            // Milestone 2: Auth routes
            // Milestone 2: User routes
            // Milestone 2: Group routes
            // Milestone 2: Role routes
            // Milestone 2: Permission routes
            // Milestone 3: Project routes
            // Milestone 4: Test case routes
            // Milestone 5: File routes
            // Milestone 6: Test plan routes
            // Milestone 7: Test run routes
            // Milestone 8: Test execution routes
            // Milestone 10: Sharing routes
            .route("/health", web::get().to(health_check)),
    );
}

/// Health-check endpoint.
///
/// Verifies database connectivity. Returns:
/// - `200 OK` when the database is reachable.
/// - `503 Service Unavailable` when the database is unreachable.
///   Load balancers should use this signal to avoid routing traffic to
///   a dead or degraded instance.
async fn health_check(state: web::Data<AppState>) -> actix_web::HttpResponse {
    let db_healthy = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();

    if db_healthy {
        actix_web::HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION"),
            "checks": {
                "database": "healthy"
            }
        }))
    } else {
        actix_web::HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "error",
            "version": env!("CARGO_PKG_VERSION"),
            "checks": {
                "database": "unreachable"
            }
        }))
    }
}
