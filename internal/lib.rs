//! HOA TCMS — Library root (core crate).
//!
//! This crate implements the Clean Architecture layers:
//!
//! - **Domain**      — Entities, value objects, domain errors (innermost).
//! - **Application** — Use cases, repository interfaces, DTOs.
//! - **Adapters**    — HTTP handlers, middleware, messaging.
//! - **Infrastructure** — PostgreSQL repositories, Redis, config loading.
//!
//! Shared utilities live in the `hoa-tcms-pkg` crate.

pub mod adapters;
pub mod application;
pub mod domain;
pub mod infrastructure;

use actix_web::web;
use std::sync::Arc;

use crate::adapters::http::handlers::auth_handler::AuthHandler;
use crate::adapters::http::handlers::project_handler::ProjectHandler;
use crate::adapters::http::handlers::sharing_handler::SharingHandler;
use crate::adapters::http::handlers::test_case_file_handler::TestCaseFileHandler;
use crate::adapters::http::handlers::test_execution_handler::TestExecutionHandler;
use crate::adapters::http::handlers::test_plan_handler::TestPlanHandler;
use crate::adapters::http::handlers::test_run_handler::TestRunHandler;
use crate::application::services::session_store::SessionStore;

/// Register all application routes on the given `ServiceConfig`.
///
/// Called by `cmd/server/main.rs` when building the `HttpServer`.
/// Services are injected via `app_data` so handlers can access them.
#[allow(clippy::too_many_arguments)]
pub fn configure_app(
    cfg: &mut web::ServiceConfig,
    auth_handler: Arc<AuthHandler>,
    project_handler: Arc<ProjectHandler>,
    test_execution_handler: Arc<TestExecutionHandler>,
    test_case_file_handler: Arc<TestCaseFileHandler>,
    test_plan_handler: Arc<TestPlanHandler>,
    test_run_handler: Arc<TestRunHandler>,
    sharing_handler: Arc<SharingHandler>,
    session_store: Arc<dyn SessionStore>,
) {
    // Register services as app data for handler extraction.
    cfg.app_data(web::Data::new(auth_handler));
    cfg.app_data(web::Data::new(project_handler));
    cfg.app_data(web::Data::new(test_execution_handler));
    cfg.app_data(web::Data::new(test_case_file_handler));
    cfg.app_data(web::Data::new(test_plan_handler));
    cfg.app_data(web::Data::new(test_run_handler));
    cfg.app_data(web::Data::new(sharing_handler));
    cfg.app_data(web::Data::new(session_store));

    // Register routes.
    adapters::http::router::configure(cfg);
}
