//! HTTP handlers for test execution CRUD, case import, and result updates.
//!
//! Each handler validates input, calls [`TestExecutionService`], and maps
//! results to HTTP responses with the correct status codes.

use actix_web::{HttpRequest, HttpResponse, web};
use std::sync::Arc;

use crate::adapters::http::middleware::auth;
use crate::application::dto::test_execution::{
    CreateTestExecutionRequest, ImportCasesRequest, ListTestExecutionsQuery,
    TestCaseResultResponse, TestExecutionListItem, TestExecutionResponse, TesterInfo,
    UpdateTestCaseResultRequest, UpdateTestExecutionRequest,
};
use crate::application::services::errors::ServiceError;
use crate::application::services::test_execution_service::TestExecutionService;
use hoa_tcms_pkg::errors::ApiError;
use hoa_tcms_pkg::pagination::{PaginatedResponse, PaginationParams};

/// Shared handler state.
pub struct TestExecutionHandler {
    service: Arc<TestExecutionService>,
}

impl TestExecutionHandler {
    pub fn new(service: Arc<TestExecutionService>) -> Self {
        Self { service }
    }
}

// ---------------------------------------------------------------------------
// Helper: map ServiceError → ApiError → HttpResponse
// ---------------------------------------------------------------------------

fn map_service_error(e: ServiceError) -> HttpResponse {
    match e {
        ServiceError::PermissionDenied(_) => {
            HttpResponse::Forbidden().json(ApiError::forbidden("forbidden"))
        }
        ServiceError::NotFound => HttpResponse::NotFound().json(ApiError::not_found("not found")),
        ServiceError::Conflict(msg) => HttpResponse::Conflict().json(ApiError::conflict(msg)),
        ServiceError::Validation(msg) => {
            HttpResponse::UnprocessableEntity().json(ApiError::validation(msg, vec![]))
        }
        ServiceError::Database(details) => {
            tracing::error!(error.details = %details, "database error in test execution handler");
            HttpResponse::InternalServerError()
                .json(ApiError::internal("an internal error occurred"))
        }
        ServiceError::Internal(msg) => {
            HttpResponse::InternalServerError().json(ApiError::internal(msg))
        }
        ServiceError::Cache(_) => HttpResponse::ServiceUnavailable().json(ApiError {
            code: "SERVICE_UNAVAILABLE".into(),
            message: "temporarily unavailable".into(),
            details: None,
        }),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

impl TestExecutionHandler {
    /// `GET /api/v1/test-executions` — list test executions.
    pub async fn list(
        &self,
        req: HttpRequest,
        query: web::Query<ListTestExecutionsQuery>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let (executions, total) = match self
            .service
            .list(user_id, query.page, query.limit, query.test_run_id)
            .await
        {
            Ok(r) => r,
            Err(e) => return map_service_error(e),
        };

        // Populate tester counts for all returned executions.
        let execution_ids: Vec<i64> = executions.iter().map(|e| e.id).collect();
        let counts = if execution_ids.is_empty() {
            std::collections::HashMap::new()
        } else {
            match self.service.get_tester_counts(&execution_ids).await {
                Ok(c) => c,
                Err(e) => return map_service_error(e),
            }
        };

        let items: Vec<TestExecutionListItem> = executions
            .into_iter()
            .map(|e| TestExecutionListItem {
                id: e.id,
                name: e.name,
                test_run_id: e.test_run_id,
                tester_count: counts.get(&e.id).copied().unwrap_or(0),
                created_by: e.created_by,
                created_at: e.created_at,
                updated_at: e.updated_at,
            })
            .collect();

        let params = PaginationParams {
            page: query.page,
            limit: query.limit,
        };

        HttpResponse::Ok().json(PaginatedResponse::new(items, total, &params))
    }

    /// `POST /api/v1/test-executions` — create a test execution.
    pub async fn create(
        &self,
        req: HttpRequest,
        body: web::Json<CreateTestExecutionRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        // Validate input.
        if let Err(errors) = body.validate() {
            let details: Vec<hoa_tcms_pkg::errors::FieldError> = errors
                .into_iter()
                .map(|msg| hoa_tcms_pkg::errors::FieldError {
                    field: "body".into(),
                    message: msg,
                })
                .collect();
            return HttpResponse::UnprocessableEntity()
                .json(ApiError::validation("validation error", details));
        }

        match self
            .service
            .create(
                user_id,
                body.name.trim().to_string(),
                body.test_run_id,
                body.tester_ids.clone(),
            )
            .await
        {
            Ok(execution) => {
                let resp = TestExecutionResponse {
                    id: execution.id,
                    name: execution.name,
                    test_run_id: execution.test_run_id,
                    testers: vec![],
                    created_by: execution.created_by,
                    created_at: execution.created_at,
                    updated_by: execution.updated_by,
                    updated_at: execution.updated_at,
                };
                HttpResponse::Created()
                    .insert_header((
                        "Location",
                        format!("/api/v1/test-executions/{}", execution.id),
                    ))
                    .json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `GET /api/v1/test-executions/{id}` — get a test execution detail.
    pub async fn get(&self, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let execution_id = path.into_inner();

        match self.service.get(execution_id, user_id).await {
            Ok(execution) => {
                // Populate tester info for the single execution.
                let testers = match self.service.get_tester_info(execution_id).await {
                    Ok(info) => info
                        .into_iter()
                        .map(|ti| TesterInfo {
                            user_id: ti.user_id,
                            username: ti.username,
                            fullname: ti.fullname,
                        })
                        .collect(),
                    Err(e) => return map_service_error(e),
                };

                let resp = TestExecutionResponse {
                    id: execution.id,
                    name: execution.name,
                    test_run_id: execution.test_run_id,
                    testers,
                    created_by: execution.created_by,
                    created_at: execution.created_at,
                    updated_by: execution.updated_by,
                    updated_at: execution.updated_at,
                };
                HttpResponse::Ok().json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `PATCH /api/v1/test-executions/{id}` — update a test execution.
    pub async fn update(
        &self,
        req: HttpRequest,
        path: web::Path<i64>,
        body: web::Json<UpdateTestExecutionRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let execution_id = path.into_inner();

        // Validate input.
        match body.validate() {
            Ok(_) => {}
            Err(errors) => {
                let details: Vec<hoa_tcms_pkg::errors::FieldError> = errors
                    .into_iter()
                    .map(|msg| hoa_tcms_pkg::errors::FieldError {
                        field: "body".into(),
                        message: msg,
                    })
                    .collect();
                return HttpResponse::UnprocessableEntity()
                    .json(ApiError::validation("validation error", details));
            }
        }

        match self
            .service
            .update(
                execution_id,
                user_id,
                body.name.as_deref().map(str::trim).map(String::from),
                body.tester_ids.clone(),
            )
            .await
        {
            Ok(execution) => {
                let testers = match self.service.get_tester_info(execution_id).await {
                    Ok(info) => info
                        .into_iter()
                        .map(|ti| TesterInfo {
                            user_id: ti.user_id,
                            username: ti.username,
                            fullname: ti.fullname,
                        })
                        .collect(),
                    Err(e) => return map_service_error(e),
                };

                let resp = TestExecutionResponse {
                    id: execution.id,
                    name: execution.name,
                    test_run_id: execution.test_run_id,
                    testers,
                    created_by: execution.created_by,
                    created_at: execution.created_at,
                    updated_by: execution.updated_by,
                    updated_at: execution.updated_at,
                };
                HttpResponse::Ok().json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `DELETE /api/v1/test-executions/{id}` — soft-delete a test execution.
    pub async fn delete(&self, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let execution_id = path.into_inner();

        match self.service.delete(execution_id, user_id).await {
            Ok(()) => HttpResponse::NoContent().finish(),
            Err(e) => map_service_error(e),
        }
    }

    /// `POST /api/v1/test-executions/{id}/import-cases` — import test cases
    /// into an execution.
    pub async fn import_cases(
        &self,
        req: HttpRequest,
        path: web::Path<i64>,
        body: web::Json<ImportCasesRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let execution_id = path.into_inner();

        // Validate input.
        if let Err(errors) = body.validate() {
            let details: Vec<hoa_tcms_pkg::errors::FieldError> = errors
                .into_iter()
                .map(|msg| hoa_tcms_pkg::errors::FieldError {
                    field: "body".into(),
                    message: msg,
                })
                .collect();
            return HttpResponse::UnprocessableEntity()
                .json(ApiError::validation("validation error", details));
        }

        match self
            .service
            .import_cases(execution_id, user_id, &body.case_ids)
            .await
        {
            Ok((imported_count, refreshed_count)) => HttpResponse::Ok().json(serde_json::json!({
                "data": {
                    "imported_count": imported_count,
                    "refreshed_count": refreshed_count,
                }
            })),
            Err(e) => map_service_error(e),
        }
    }

    /// `PATCH /api/v1/test-case-results/{id}` — update a test case result's
    /// outcome.
    pub async fn update_result(
        &self,
        req: HttpRequest,
        path: web::Path<i64>,
        body: web::Json<UpdateTestCaseResultRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let result_id = path.into_inner();

        // Validate input.
        match body.validate() {
            Ok(_) => {}
            Err(errors) => {
                let details: Vec<hoa_tcms_pkg::errors::FieldError> = errors
                    .into_iter()
                    .map(|msg| hoa_tcms_pkg::errors::FieldError {
                        field: "body".into(),
                        message: msg,
                    })
                    .collect();
                return HttpResponse::UnprocessableEntity()
                    .json(ApiError::validation("validation error", details));
            }
        }

        match self
            .service
            .update_result(result_id, user_id, body.result.clone(), body.logs.clone())
            .await
        {
            Ok(result) => {
                let resp = TestCaseResultResponse {
                    id: result.id,
                    execution_id: result.execution_id,
                    test_case_id: result.test_case_id,
                    summary: result.summary,
                    description: result.description,
                    priority: result.priority,
                    result: result.result.to_string(),
                    logs: result.logs,
                    tested_by: result.tested_by,
                    created_by: result.created_by,
                    created_at: result.created_at,
                    updated_by: result.updated_by,
                    updated_at: result.updated_at,
                };
                HttpResponse::Ok().json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }
}
