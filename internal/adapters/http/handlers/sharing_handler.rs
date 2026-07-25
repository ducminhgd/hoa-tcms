//! HTTP handlers for object sharing.

use actix_web::{HttpRequest, HttpResponse, web};
use std::sync::Arc;

use crate::adapters::http::middleware::auth;
use crate::application::dto::object_sharing::{
    CreateShareRequest, ObjectSharingResponse, UpdateShareRequest,
};
use crate::application::services::errors::ServiceError;
use crate::application::services::sharing_service::SharingService;
use hoa_tcms_pkg::errors::ApiError;

pub struct SharingHandler {
    service: Arc<SharingService>,
}

impl SharingHandler {
    pub fn new(service: Arc<SharingService>) -> Self {
        Self { service }
    }
}

fn map_error(e: ServiceError) -> HttpResponse {
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
            tracing::error!(error.details = %details, "database error in sharing handler");
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

impl SharingHandler {
    /// `GET /api/v1/share/{resource_type}/{resource_id}` — list shares.
    pub async fn list(&self, req: HttpRequest, path: web::Path<(String, i64)>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            _ => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
        };
        let (resource_type, resource_id) = path.into_inner();
        match self
            .service
            .list(user_id, &resource_type, resource_id)
            .await
        {
            Ok(items) => {
                let data: Vec<ObjectSharingResponse> = items
                    .into_iter()
                    .map(|s| ObjectSharingResponse {
                        id: s.id,
                        user_id: s.user_id,
                        username: s.username,
                        fullname: s.fullname,
                        resource_type: s.resource_type,
                        resource_id: s.resource_id,
                        role: s.role,
                        created_by: s.created_by,
                        created_at: s.created_at,
                    })
                    .collect();
                HttpResponse::Ok().json(serde_json::json!({ "data": data }))
            }
            Err(e) => map_error(e),
        }
    }

    /// `POST /api/v1/share` — create share.
    pub async fn create(
        &self,
        req: HttpRequest,
        body: web::Json<CreateShareRequest>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            _ => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
        };
        let resource_type = body.resource_type.clone();
        let resource_id = body.resource_id;
        match self
            .service
            .share(
                user_id,
                body.user_id,
                &resource_type,
                resource_id,
                &body.role,
            )
            .await
        {
            Ok(s) => {
                let resp = ObjectSharingResponse {
                    id: s.id,
                    user_id: s.user_id,
                    username: String::new(),
                    fullname: String::new(),
                    resource_type: s.resource_type.clone(),
                    resource_id: s.resource_id,
                    role: s.role.clone(),
                    created_by: s.created_by,
                    created_at: s.created_at,
                };
                HttpResponse::Created()
                    .insert_header((
                        "Location",
                        format!("/api/v1/share/{}/{}", resource_type, resource_id),
                    ))
                    .json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_error(e),
        }
    }

    /// `PATCH /api/v1/share/{id}` — update role.
    pub async fn update(
        &self,
        req: HttpRequest,
        path: web::Path<i64>,
        body: web::Json<UpdateShareRequest>,
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
            .update_role(user_id, path.into_inner(), &body.role)
            .await
        {
            Ok(()) => HttpResponse::NoContent().finish(),
            Err(e) => map_error(e),
        }
    }

    /// `DELETE /api/v1/share/{id}` — remove share.
    pub async fn delete(&self, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            _ => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
        };
        match self.service.remove(user_id, path.into_inner()).await {
            Ok(()) => HttpResponse::NoContent().finish(),
            Err(e) => map_error(e),
        }
    }
}
