//! HTTP handlers for project CRUD and member management.
//!
//! Each handler validates input, calls [`ProjectService`], and maps
//! results to HTTP responses with the correct status codes.

use actix_web::{HttpRequest, HttpResponse, web};
use std::sync::Arc;

use crate::adapters::http::middleware::auth;
use crate::application::dto::project::{
    ChangeMemberRoleRequest, CreateProjectRequest, ListProjectsQuery, ManageMembersRequest,
    MemberResponse, ProjectListItem, ProjectResponse, UpdateProjectRequest,
};
use crate::application::services::errors::ServiceError;
use crate::application::services::project_service::ProjectService;
use crate::domain::value_objects::project_status::ProjectStatus;
use hoa_tcms_pkg::errors::ApiError;
use hoa_tcms_pkg::pagination::{PaginatedResponse, PaginationParams};

/// Shared handler state.
pub struct ProjectHandler {
    service: Arc<ProjectService>,
}

impl ProjectHandler {
    pub fn new(service: Arc<ProjectService>) -> Self {
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
            tracing::error!(error.details = %details, "database error in project handler");
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

impl ProjectHandler {
    /// `GET /api/v1/projects` — list projects.
    pub async fn list(
        &self,
        req: HttpRequest,
        query: web::Query<ListProjectsQuery>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let (projects, total) = match self
            .service
            .list_projects(user_id, query.page, query.limit, query.status.as_deref())
            .await
        {
            Ok(r) => r,
            Err(e) => return map_service_error(e),
        };

        let items: Vec<ProjectListItem> = projects
            .into_iter()
            .map(|p| ProjectListItem {
                id: p.id,
                name: p.name,
                description: p.description,
                status: p.status.to_string(),
                member_count: 0, // not computed in list view; use GET /projects/{id} for member details
                created_by: p.created_by,
                created_at: p.created_at,
                updated_at: p.updated_at,
            })
            .collect();

        let params = PaginationParams {
            page: query.page,
            limit: query.limit,
        };

        HttpResponse::Ok().json(PaginatedResponse::new(items, total, &params))
    }

    /// `POST /api/v1/projects` — create a project.
    pub async fn create(
        &self,
        req: HttpRequest,
        body: web::Json<CreateProjectRequest>,
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

        let status = ProjectStatus::from_str(&body.status).unwrap_or(ProjectStatus::Active);

        match self
            .service
            .create_project(
                body.name.trim().to_string(),
                body.description.clone(),
                status,
                user_id,
            )
            .await
        {
            Ok(project) => {
                let resp = ProjectResponse {
                    id: project.id,
                    name: project.name,
                    description: project.description,
                    status: project.status.to_string(),
                    created_by: project.created_by,
                    created_at: project.created_at,
                    updated_by: project.updated_by,
                    updated_at: project.updated_at,
                    members: None,
                };
                HttpResponse::Created()
                    .insert_header(("Location", format!("/api/v1/projects/{}", project.id)))
                    .json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `GET /api/v1/projects/{id}` — get project detail with members.
    pub async fn get(&self, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let project_id = path.into_inner();

        match self.service.get_project(project_id, user_id).await {
            Ok((project, members)) => {
                let member_list: Vec<MemberResponse> = members
                    .into_iter()
                    .map(|m| MemberResponse {
                        user_id: m.user_id,
                        username: m.username,
                        fullname: m.fullname,
                        role: m.role,
                    })
                    .collect();

                let resp = ProjectResponse {
                    id: project.id,
                    name: project.name,
                    description: project.description,
                    status: project.status.to_string(),
                    created_by: project.created_by,
                    created_at: project.created_at,
                    updated_by: project.updated_by,
                    updated_at: project.updated_at,
                    members: Some(member_list),
                };
                HttpResponse::Ok().json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `PATCH /api/v1/projects/{id}` — update a project.
    pub async fn update(
        &self,
        req: HttpRequest,
        path: web::Path<i64>,
        body: web::Json<UpdateProjectRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let project_id = path.into_inner();

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
            .update_project(
                project_id,
                body.name.as_deref().map(str::trim).map(String::from),
                body.description.clone(),
                body.status.clone(),
                user_id,
            )
            .await
        {
            Ok(project) => {
                let resp = ProjectResponse {
                    id: project.id,
                    name: project.name,
                    description: project.description,
                    status: project.status.to_string(),
                    created_by: project.created_by,
                    created_at: project.created_at,
                    updated_by: project.updated_by,
                    updated_at: project.updated_at,
                    members: None,
                };
                HttpResponse::Ok().json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `DELETE /api/v1/projects/{id}` — soft-delete a project.
    pub async fn delete(&self, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let project_id = path.into_inner();

        match self.service.delete_project(project_id, user_id).await {
            Ok(()) => HttpResponse::NoContent().finish(),
            Err(e) => map_service_error(e),
        }
    }

    // ------------------------------------------------------------------
    // Member management handlers
    // ------------------------------------------------------------------

    /// `GET /api/v1/projects/{id}/members` — list project members.
    pub async fn list_members(&self, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let project_id = path.into_inner();

        match self.service.list_members(project_id, user_id).await {
            Ok(members) => {
                let member_list: Vec<MemberResponse> = members
                    .into_iter()
                    .map(|m| MemberResponse {
                        user_id: m.user_id,
                        username: m.username,
                        fullname: m.fullname,
                        role: m.role,
                    })
                    .collect();
                HttpResponse::Ok().json(serde_json::json!({ "data": member_list }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `POST /api/v1/projects/{id}/members` — bulk add/remove members.
    pub async fn manage_members(
        &self,
        req: HttpRequest,
        path: web::Path<i64>,
        body: web::Json<ManageMembersRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let project_id = path.into_inner();

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
            .manage_members(project_id, &body.add, &body.remove, user_id)
            .await
        {
            Ok(outcome) => {
                let added: Vec<MemberResponse> = outcome
                    .added
                    .into_iter()
                    .map(|m| MemberResponse {
                        user_id: m.user_id,
                        username: m.username,
                        fullname: m.fullname,
                        role: m.role,
                    })
                    .collect();
                HttpResponse::Ok().json(serde_json::json!({
                    "data": {
                        "added": added,
                        "removed": outcome.removed,
                    }
                }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `PATCH /api/v1/projects/{id}/members/{user_id}` — change a member's role.
    pub async fn change_member_role(
        &self,
        req: HttpRequest,
        path: web::Path<(i64, i64)>,
        body: web::Json<ChangeMemberRoleRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) => id,
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let (project_id, target_user_id) = path.into_inner();

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
            .change_member_role(project_id, target_user_id, &body.role, user_id)
            .await
        {
            Ok(member) => {
                let resp = MemberResponse {
                    user_id: member.user_id,
                    username: member.username,
                    fullname: member.fullname,
                    role: member.role,
                };
                HttpResponse::Ok().json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }
}
