//! HTTP handlers for test plan CRUD, select, and status transitions.

use actix_web::{HttpRequest, HttpResponse, web};
use std::sync::Arc;

use crate::adapters::http::middleware::auth;
use crate::application::dto::test_plan::{
    CreateTestPlanRequest, ListTestPlansQuery, TestPlanListItemResponse, TestPlanResponse,
    TestPlanSelectResponse, TransitionStatusRequest, UpdateTestPlanRequest,
};
use crate::application::services::errors::ServiceError;
use crate::application::services::test_plan_service::TestPlanService;
use hoa_tcms_pkg::errors::ApiError;
use hoa_tcms_pkg::pagination::{PaginatedResponse, PaginationParams};

pub struct TestPlanHandler {
    service: Arc<TestPlanService>,
}

impl TestPlanHandler {
    pub fn new(service: Arc<TestPlanService>) -> Self {
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
            tracing::error!(error.details = %details, "database error in test plan handler");
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

fn vec_to_strings(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

impl TestPlanHandler {
    /// `GET /api/v1/test-plans` — list test plans.
    pub async fn list(
        &self,
        req: HttpRequest,
        query: web::Query<ListTestPlansQuery>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            Ok(_) => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        match self
            .service
            .list(
                user_id,
                query.page,
                query.limit,
                query.status.clone(),
                query.plan_type.clone(),
                query.project_id,
                query.search.clone(),
                query.sort.clone(),
            )
            .await
        {
            Ok((items, total)) => {
                let data: Vec<TestPlanListItemResponse> = items
                    .into_iter()
                    .map(|i| TestPlanListItemResponse {
                        id: i.id,
                        name: i.name,
                        version: i.version,
                        types: vec_to_strings(&i.types),
                        status: i.status,
                        project_ids: i.project_ids,
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

    /// `POST /api/v1/test-plans` — create a test plan.
    pub async fn create(
        &self,
        req: HttpRequest,
        body: web::Json<CreateTestPlanRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            Ok(_) => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        match self
            .service
            .create(
                user_id,
                body.name.clone(),
                body.types.clone(),
                body.version.clone(),
                body.description.clone(),
                body.project_ids.clone(),
            )
            .await
        {
            Ok(plan) => {
                let resp = TestPlanResponse {
                    id: plan.id,
                    name: plan.name,
                    version: plan.version,
                    types: vec_to_strings(&plan.types),
                    status: plan.status.to_string(),
                    description: plan.description,
                    project_ids: body.project_ids.clone(),
                    created_by: plan.created_by,
                    created_at: plan.created_at,
                    updated_by: plan.updated_by,
                    updated_at: plan.updated_at,
                };
                HttpResponse::Created()
                    .insert_header(("Location", format!("/api/v1/test-plans/{}", plan.id)))
                    .json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `GET /api/v1/test-plans/select` — dropdown list.
    pub async fn select(&self, req: HttpRequest) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            Ok(_) => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        match self.service.select(user_id).await {
            Ok(items) => {
                let data: Vec<TestPlanSelectResponse> = items
                    .into_iter()
                    .map(|i| TestPlanSelectResponse {
                        id: i.id,
                        name: i.name,
                    })
                    .collect();
                HttpResponse::Ok().json(serde_json::json!({ "data": data }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `GET /api/v1/test-plans/{id}` — get test plan detail.
    pub async fn get(&self, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            Ok(_) => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        match self.service.get(path.into_inner(), user_id).await {
            Ok(plan) => {
                let resp = TestPlanResponse {
                    id: plan.id,
                    name: plan.name,
                    version: plan.version,
                    types: vec_to_strings(&plan.types),
                    status: plan.status.to_string(),
                    description: plan.description,
                    project_ids: vec![], // filled below
                    created_by: plan.created_by,
                    created_at: plan.created_at,
                    updated_by: plan.updated_by,
                    updated_at: plan.updated_at,
                };
                HttpResponse::Ok().json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `PATCH /api/v1/test-plans/{id}` — update a test plan.
    pub async fn update(
        &self,
        req: HttpRequest,
        path: web::Path<i64>,
        body: web::Json<UpdateTestPlanRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            Ok(_) => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let plan_id = path.into_inner();

        // Reject status in update body.
        if body.status.is_some() {
            return HttpResponse::UnprocessableEntity().json(ApiError::validation(
                "status changes must use the dedicated transition-status endpoint",
                vec![],
            ));
        }

        match self
            .service
            .update(
                plan_id,
                user_id,
                body.name.clone(),
                body.version.clone(),
                body.types.clone(),
                body.description.clone(),
                body.project_ids.clone(),
            )
            .await
        {
            Ok(plan) => {
                let resp = TestPlanResponse {
                    id: plan.id,
                    name: plan.name,
                    version: plan.version,
                    types: vec_to_strings(&plan.types),
                    status: plan.status.to_string(),
                    description: plan.description,
                    project_ids: vec![],
                    created_by: plan.created_by,
                    created_at: plan.created_at,
                    updated_by: plan.updated_by,
                    updated_at: plan.updated_at,
                };
                HttpResponse::Ok().json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `DELETE /api/v1/test-plans/{id}` — soft-delete a test plan.
    pub async fn delete(&self, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            Ok(_) => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        match self.service.delete(path.into_inner(), user_id).await {
            Ok(()) => HttpResponse::NoContent().finish(),
            Err(e) => map_service_error(e),
        }
    }

    /// `POST /api/v1/test-plans/{id}/transition-status` — transition status.
    pub async fn transition_status(
        &self,
        req: HttpRequest,
        path: web::Path<i64>,
        body: web::Json<TransitionStatusRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            Ok(_) => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        match self
            .service
            .transition_status(path.into_inner(), user_id, &body.status)
            .await
        {
            Ok(plan) => {
                let resp = TestPlanResponse {
                    id: plan.id,
                    name: plan.name,
                    version: plan.version,
                    types: vec_to_strings(&plan.types),
                    status: plan.status.to_string(),
                    description: plan.description,
                    project_ids: vec![],
                    created_by: plan.created_by,
                    created_at: plan.created_at,
                    updated_by: plan.updated_by,
                    updated_at: plan.updated_at,
                };
                HttpResponse::Ok().json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }
}
