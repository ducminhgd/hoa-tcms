//! Configuration — application state and environment variable loading.

use sea_orm::DatabaseConnection;

/// Shared application state available to all HTTP handlers.
///
/// Created during server startup in `cmd/server/main.rs` and registered
/// with Actix-Web's `web::Data<AppState>` so handlers can access the
/// database pool, Redis client, and other shared resources.
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub db: DatabaseConnection,
    pub redis_client: redis::Client,
}
