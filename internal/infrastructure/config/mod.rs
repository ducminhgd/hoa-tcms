//! Configuration — application state and environment variable loading.

pub mod metadata_seeder;

use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::adapters::http::handlers::project_handler::ProjectHandler;
use crate::adapters::http::handlers::test_case_file_handler::TestCaseFileHandler;
use crate::adapters::http::handlers::test_execution_handler::TestExecutionHandler;
use crate::application::services::session_store::SessionStore;

/// Shared application state available to all HTTP handlers.
///
/// Created during server startup in `cmd/server/main.rs` and registered
/// with Actix-Web's `web::Data<AppState>` so handlers can access the
/// database pool, Redis client, and other shared resources.
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub db: DatabaseConnection,
    pub redis_client: redis::Client,
    /// Session store for auth validation.
    pub session_store: Arc<dyn SessionStore>,
    /// Project handler (Milestone 3).
    pub project_handler: Arc<ProjectHandler>,
    /// Test execution handler (Milestone 4).
    pub test_execution_handler: Arc<TestExecutionHandler>,
    /// Test case file handler (Milestone 5).
    pub test_case_file_handler: Arc<TestCaseFileHandler>,
}
