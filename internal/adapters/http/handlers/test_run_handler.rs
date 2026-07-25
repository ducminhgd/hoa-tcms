//! HTTP handlers for test run CRUD, case management, and statistics.

use actix_web::{HttpRequest, HttpResponse, web};
use std::sync::Arc;

use crate::adapters::http::middleware::auth;
use crate::application::dto::test_run::{
    CreateTestRunRequest, ListTestRunsQuery, ManageCasesRequest, TestRunListItemResponse,
    TestRunResponse, TestRunStatisticsResponse, UpdateTestRunRequest,
};
use crate::application::services::errors::ServiceError;
use crate::application::services::test_run_service::TestRunService;
use hoa_tcms_pkg::errors::ApiError;
use hoa_tcms_pkg::pagination::{PaginatedResponse, PaginationParams};

pub struct TestRunHandler {
    service: Arc<TestRunService>,
}

impl TestRunHandler {
    pub fn new(service: Arc<TestRunService>) -> Self {
        Self { service }
    }
}

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
            tracing::error!(error.details = %details, "database error in test run handler");
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

fn fmt_date(d: &Option<chrono::NaiveDate>) -> Option<String> {
    d.map(|d| d.format("%Y-%m-%d").to_string())
}

impl TestRunHandler {
    /// `GET /api/v1/projects/{project_id}/test-runs` — list.
    pub async fn list(
        &self,
        req: HttpRequest,
        path: web::Path<i64>,
        query: web::Query<ListTestRunsQuery>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            _ => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
        };
        let project_id = path.into_inner();

        match self
            .service
            .list(
                user_id,
                project_id,
                query.page,
                query.limit,
                query.plan_id,
                query.search.clone(),
                query.sort.clone(),
            )
            .await
        {
            Ok((items, total)) => {
                let data: Vec<TestRunListItemResponse> = items
                    .into_iter()
                    .map(|i| TestRunListItemResponse {
                        id: i.id,
                        summary: i.summary,
                        report_to_user_id: i.report_to_user_id,
                        default_tester_id: i.default_tester_id,
                        project_id: i.project_id,
                        plan_id: i.plan_id,
                        version: i.version,
                        planned_start: fmt_date(&i.planned_start),
                        planned_stop: fmt_date(&i.planned_stop),
                        case_count: i.case_count,
                        created_by: i.created_by,
                        created_at: i.created_at,
                        updated_at: i.updated_at,
                    })
                    .collect();
                let params = PaginationParams {
                    page: query.page,
                    limit: query.limit,
                };
                HttpResponse::Ok().json(PaginatedResponse::new(data, total, &params))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `POST /api/v1/projects/{project_id}/test-runs` — create.
    pub async fn create(
        &self,
        req: HttpRequest,
        path: web::Path<i64>,
        body: web::Json<CreateTestRunRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            _ => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
        };
        let project_id = path.into_inner();

        match self
            .service
            .create(
                user_id,
                project_id,
                body.summary.clone(),
                body.report_to,
                body.default_tester,
                body.plan_id,
                body.version.clone(),
                body.notes.clone(),
                body.planned_start_date.clone(),
                body.planned_end_date.clone(),
                body.case_ids.clone().unwrap_or_default(),
            )
            .await
        {
            Ok(run) => {
                let resp = TestRunResponse {
                    id: run.id,
                    summary: run.summary,
                    report_to_user_id: run.report_to_user_id,
                    default_tester_id: run.default_tester_id,
                    project_id: run.project_id,
                    plan_id: run.plan_id,
                    version: run.version,
                    notes: run.notes,
                    planned_start: fmt_date(&run.planned_start),
                    planned_stop: fmt_date(&run.planned_stop),
                    case_ids: vec![],
                    execution_ids: vec![],
                    created_by: run.created_by,
                    created_at: run.created_at,
                    updated_by: run.updated_by,
                    updated_at: run.updated_at,
                };
                HttpResponse::Created()
                    .insert_header((
                        "Location",
                        format!("/api/v1/projects/{}/test-runs/{}", project_id, run.id),
                    ))
                    .json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `GET /api/v1/projects/{project_id}/test-runs/{id}` — detail.
    pub async fn get(&self, req: HttpRequest, path: web::Path<(i64, i64)>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            _ => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
        };
        let (_project_id, run_id) = path.into_inner();

        match self.service.get(run_id, user_id).await {
            Ok(run) => {
                let resp = TestRunResponse {
                    id: run.id,
                    summary: run.summary,
                    report_to_user_id: run.report_to_user_id,
                    default_tester_id: run.default_tester_id,
                    project_id: run.project_id,
                    plan_id: run.plan_id,
                    version: run.version,
                    notes: run.notes,
                    planned_start: fmt_date(&run.planned_start),
                    planned_stop: fmt_date(&run.planned_stop),
                    case_ids: vec![],
                    execution_ids: vec![],
                    created_by: run.created_by,
                    created_at: run.created_at,
                    updated_by: run.updated_by,
                    updated_at: run.updated_at,
                };
                HttpResponse::Ok().json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `PATCH /api/v1/projects/{project_id}/test-runs/{id}` — update.
    pub async fn update(
        &self,
        req: HttpRequest,
        path: web::Path<(i64, i64)>,
        body: web::Json<UpdateTestRunRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            _ => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
        };
        let (_project_id, run_id) = path.into_inner();

        match self
            .service
            .update(
                run_id,
                user_id,
                body.summary.clone(),
                body.report_to,
                body.default_tester,
                body.plan_id,
                body.version.clone(),
                body.notes.clone(),
                body.planned_start_date.clone(),
                body.planned_end_date.clone(),
                body.case_ids.clone(),
            )
            .await
        {
            Ok(run) => {
                let resp = TestRunResponse {
                    id: run.id,
                    summary: run.summary,
                    report_to_user_id: run.report_to_user_id,
                    default_tester_id: run.default_tester_id,
                    project_id: run.project_id,
                    plan_id: run.plan_id,
                    version: run.version,
                    notes: run.notes,
                    planned_start: fmt_date(&run.planned_start),
                    planned_stop: fmt_date(&run.planned_stop),
                    case_ids: vec![],
                    execution_ids: vec![],
                    created_by: run.created_by,
                    created_at: run.created_at,
                    updated_by: run.updated_by,
                    updated_at: run.updated_at,
                };
                HttpResponse::Ok().json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `DELETE /api/v1/projects/{project_id}/test-runs/{id}` — soft-delete.
    pub async fn delete(&self, req: HttpRequest, path: web::Path<(i64, i64)>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            _ => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
        };
        let (_project_id, run_id) = path.into_inner();

        match self.service.delete(run_id, user_id).await {
            Ok(()) => HttpResponse::NoContent().finish(),
            Err(e) => map_service_error(e),
        }
    }

    /// `GET /api/v1/test-runs/{id}/cases` — list case IDs.
    pub async fn list_cases(&self, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            _ => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
        };
        match self.service.list_cases(path.into_inner(), user_id).await {
            Ok(ids) => HttpResponse::Ok().json(serde_json::json!({ "data": ids })),
            Err(e) => map_service_error(e),
        }
    }

    /// `POST /api/v1/test-runs/{id}/cases` — manage case assignments.
    pub async fn manage_cases(
        &self,
        req: HttpRequest,
        path: web::Path<i64>,
        body: web::Json<ManageCasesRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            _ => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
        };
        match self
            .service
            .manage_cases(
                path.into_inner(),
                user_id,
                body.add_case_ids.clone().unwrap_or_default(),
                body.remove_case_ids.clone().unwrap_or_default(),
            )
            .await
        {
            Ok(()) => HttpResponse::NoContent().finish(),
            Err(e) => map_service_error(e),
        }
    }

    /// `GET /api/v1/test-runs/{id}/statistics` — aggregated stats.
    pub async fn statistics(&self, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            _ => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
        };
        match self.service.statistics(path.into_inner(), user_id).await {
            Ok(s) => {
                let resp = TestRunStatisticsResponse {
                    total: s.total,
                    not_tested: s.not_tested,
                    in_progress: s.in_progress,
                    pass: s.pass,
                    fail: s.fail,
                    warning: s.warning,
                    ignore: s.ignore,
                };
                HttpResponse::Ok().json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }
}
