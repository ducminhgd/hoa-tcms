//! Central router — all HTTP routes registered here.
//!
//! This module wires handlers, middleware, and path prefixes into the
//! Actix-Web application.

use actix_web::web;

/// Register all API and SSR routes on the given `ServiceConfig`.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            // Health check (no auth required)
            .route("/health", web::get().to(health_check))
            // Project routes (Milestone 3)
            .route("/projects", web::get().to(list_projects))
            .route("/projects", web::post().to(create_project))
            .route("/projects/{id}", web::get().to(get_project))
            .route("/projects/{id}", web::patch().to(update_project))
            .route("/projects/{id}", web::delete().to(delete_project))
            // Project member routes (Milestone 3)
            .route(
                "/projects/{id}/members",
                web::get().to(list_project_members),
            )
            .route(
                "/projects/{id}/members",
                web::post().to(manage_project_members),
            )
            .route(
                "/projects/{project_id}/members/{user_id}",
                web::patch().to(change_member_role),
            )
            // Test execution routes (Milestone 4)
            .route("/test-executions", web::get().to(list_test_executions))
            .route("/test-executions", web::post().to(create_test_execution))
            .route("/test-executions/{id}", web::get().to(get_test_execution))
            .route(
                "/test-executions/{id}",
                web::patch().to(update_test_execution),
            )
            .route(
                "/test-executions/{id}",
                web::delete().to(delete_test_execution),
            )
            .route(
                "/test-executions/{id}/import-cases",
                web::post().to(import_test_cases),
            )
            // Test case result routes (Milestone 4)
            .route(
                "/test-case-results/{id}",
                web::patch().to(update_test_case_result),
            )
            // Test case file routes (Milestone 5)
            .route(
                "/projects/{project_id}/test-cases/{id}/files",
                web::get().to(list_test_case_files),
            )
            .route(
                "/projects/{project_id}/test-cases/{id}/files",
                web::post().to(upload_test_case_file),
            )
            .route(
                "/projects/{project_id}/test-cases/{id}/files/{file_id}",
                web::get().to(download_test_case_file),
            )
            .route(
                "/projects/{project_id}/test-cases/{id}/files/{file_id}",
                web::delete().to(delete_test_case_file),
            ),
    );
}

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

/// Health-check endpoint.
async fn health_check(
    state: web::Data<crate::infrastructure::config::AppState>,
) -> actix_web::HttpResponse {
    let db_healthy = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();

    let redis_healthy = state
        .redis_client
        .get_connection()
        .and_then(|mut conn| redis::cmd("PING").query::<String>(&mut conn))
        .is_ok();

    let all_healthy = db_healthy && redis_healthy;

    let mut status = if all_healthy {
        actix_web::HttpResponse::Ok()
    } else {
        actix_web::HttpResponse::ServiceUnavailable()
    };

    status.json(serde_json::json!({
        "status": if all_healthy { "ok" } else { "error" },
        "version": env!("CARGO_PKG_VERSION"),
        "checks": {
            "database": if db_healthy { "healthy" } else { "unreachable" },
            "redis": if redis_healthy { "healthy" } else { "unreachable" },
        }
    }))
}

// ---------------------------------------------------------------------------
// Project handlers — delegates to ProjectHandler
// ---------------------------------------------------------------------------

use crate::adapters::http::handlers::project_handler::ProjectHandler;
use crate::application::dto::project::{
    ChangeMemberRoleRequest, CreateProjectRequest, ListProjectsQuery, ManageMembersRequest,
    UpdateProjectRequest,
};

async fn list_projects(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<ProjectHandler>>,
    query: web::Query<ListProjectsQuery>,
) -> actix_web::HttpResponse {
    handler.list(req, query).await
}

async fn create_project(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<ProjectHandler>>,
    body: web::Json<CreateProjectRequest>,
) -> actix_web::HttpResponse {
    handler.create(req, body).await
}

async fn get_project(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<ProjectHandler>>,
    path: web::Path<i64>,
) -> actix_web::HttpResponse {
    handler.get(req, path).await
}

async fn update_project(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<ProjectHandler>>,
    path: web::Path<i64>,
    body: web::Json<UpdateProjectRequest>,
) -> actix_web::HttpResponse {
    handler.update(req, path, body).await
}

async fn delete_project(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<ProjectHandler>>,
    path: web::Path<i64>,
) -> actix_web::HttpResponse {
    handler.delete(req, path).await
}

async fn list_project_members(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<ProjectHandler>>,
    path: web::Path<i64>,
) -> actix_web::HttpResponse {
    handler.list_members(req, path).await
}

async fn manage_project_members(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<ProjectHandler>>,
    path: web::Path<i64>,
    body: web::Json<ManageMembersRequest>,
) -> actix_web::HttpResponse {
    handler.manage_members(req, path, body).await
}

async fn change_member_role(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<ProjectHandler>>,
    path: web::Path<(i64, i64)>,
    body: web::Json<ChangeMemberRoleRequest>,
) -> actix_web::HttpResponse {
    handler.change_member_role(req, path, body).await
}

// ---------------------------------------------------------------------------
// Test execution handlers — delegates to TestExecutionHandler
// ---------------------------------------------------------------------------

use crate::adapters::http::handlers::test_execution_handler::TestExecutionHandler;
use crate::application::dto::test_execution::{
    CreateTestExecutionRequest, ImportCasesRequest, ListTestExecutionsQuery,
    UpdateTestCaseResultRequest, UpdateTestExecutionRequest,
};

async fn list_test_executions(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<TestExecutionHandler>>,
    query: web::Query<ListTestExecutionsQuery>,
) -> actix_web::HttpResponse {
    handler.list(req, query).await
}

async fn create_test_execution(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<TestExecutionHandler>>,
    body: web::Json<CreateTestExecutionRequest>,
) -> actix_web::HttpResponse {
    handler.create(req, body).await
}

async fn get_test_execution(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<TestExecutionHandler>>,
    path: web::Path<i64>,
) -> actix_web::HttpResponse {
    handler.get(req, path).await
}

async fn update_test_execution(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<TestExecutionHandler>>,
    path: web::Path<i64>,
    body: web::Json<UpdateTestExecutionRequest>,
) -> actix_web::HttpResponse {
    handler.update(req, path, body).await
}

async fn delete_test_execution(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<TestExecutionHandler>>,
    path: web::Path<i64>,
) -> actix_web::HttpResponse {
    handler.delete(req, path).await
}

async fn import_test_cases(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<TestExecutionHandler>>,
    path: web::Path<i64>,
    body: web::Json<ImportCasesRequest>,
) -> actix_web::HttpResponse {
    handler.import_cases(req, path, body).await
}

async fn update_test_case_result(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<TestExecutionHandler>>,
    path: web::Path<i64>,
    body: web::Json<UpdateTestCaseResultRequest>,
) -> actix_web::HttpResponse {
    handler.update_result(req, path, body).await
}

// ---------------------------------------------------------------------------
// Test case file handlers — delegates to TestCaseFileHandler
// ---------------------------------------------------------------------------

use crate::adapters::http::handlers::test_case_file_handler::TestCaseFileHandler;
use crate::application::dto::test_case_file::ListFilesQuery;
use actix_multipart::Multipart;

async fn list_test_case_files(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<TestCaseFileHandler>>,
    path: web::Path<(i64, i64)>,
    query: web::Query<ListFilesQuery>,
) -> actix_web::HttpResponse {
    handler.list(req, path, query).await
}

async fn upload_test_case_file(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<TestCaseFileHandler>>,
    path: web::Path<(i64, i64)>,
    body: Multipart,
) -> actix_web::HttpResponse {
    handler.upload(req, path, body).await
}

async fn download_test_case_file(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<TestCaseFileHandler>>,
    path: web::Path<(i64, i64, i64)>,
) -> actix_web::HttpResponse {
    handler.download(req, path).await
}

async fn delete_test_case_file(
    req: actix_web::HttpRequest,
    handler: web::Data<std::sync::Arc<TestCaseFileHandler>>,
    path: web::Path<(i64, i64, i64)>,
) -> actix_web::HttpResponse {
    handler.delete(req, path).await
}
