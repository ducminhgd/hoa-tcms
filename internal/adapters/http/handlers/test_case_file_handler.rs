//! HTTP handlers for test case file CRUD (upload, list, download, soft-delete).
//!
//! Each handler validates input, calls [`TestCaseFileService`], and maps
//! results to HTTP responses with the correct status codes, content types,
//! and content-disposition headers.

use actix_multipart::Multipart;
use actix_web::{HttpRequest, HttpResponse, web};
use futures_util::StreamExt;
use mime;
use std::sync::Arc;

use crate::adapters::http::middleware::auth;
use crate::application::dto::test_case_file::{ListFilesQuery, TestCaseFileResponse};
use crate::application::services::errors::ServiceError;
use crate::application::services::test_case_file_service::TestCaseFileService;
use hoa_tcms_pkg::errors::ApiError;

/// Shared handler state.
pub struct TestCaseFileHandler {
    service: Arc<TestCaseFileService>,
}

impl TestCaseFileHandler {
    pub fn new(service: Arc<TestCaseFileService>) -> Self {
        Self { service }
    }
}

// ---------------------------------------------------------------------------
// Helper: map ServiceError -> ApiError -> HttpResponse
// ---------------------------------------------------------------------------

fn map_service_error(e: ServiceError) -> HttpResponse {
    match e {
        ServiceError::PermissionDenied(_) => {
            HttpResponse::Forbidden().json(ApiError::forbidden("forbidden"))
        }
        ServiceError::NotFound => HttpResponse::NotFound().json(ApiError::not_found("not found")),
        ServiceError::Conflict(msg) => HttpResponse::Conflict().json(ApiError::conflict(msg)),
        ServiceError::Validation(msg) => {
            // Distinguish FILE_TOO_LARGE and FILE_TYPE_NOT_ALLOWED from
            // generic VALIDATION_ERROR using message content.
            let lower = msg.to_lowercase();
            if lower.contains("file type") {
                return HttpResponse::UnprocessableEntity().json(ApiError {
                    code: "FILE_TYPE_NOT_ALLOWED".into(),
                    message: msg.clone(),
                    details: Some(vec![hoa_tcms_pkg::errors::FieldError {
                        field: "file".into(),
                        message: msg,
                    }]),
                });
            }
            if lower.contains("file size") {
                return HttpResponse::PayloadTooLarge().json(ApiError {
                    code: "FILE_TOO_LARGE".into(),
                    message: msg.clone(),
                    details: Some(vec![hoa_tcms_pkg::errors::FieldError {
                        field: "file".into(),
                        message: msg,
                    }]),
                });
            }
            HttpResponse::UnprocessableEntity().json(ApiError::validation(msg, vec![]))
        }
        ServiceError::Database(details) => {
            tracing::error!(error.details = %details, "database error in test case file handler");
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
// Helper: infer MIME type from file extension
// ---------------------------------------------------------------------------

/// Infer a MIME type from the given filename extension.
///
/// # TODO: Use magic bytes for MIME detection
///
/// Currently this function maps known extensions to MIME types as a fallback
/// when the multipart Content-Type header is not trustworthy. The spec requires
/// MIME detection from file content (magic bytes). Once a magic-bytes library
/// (e.g., `tree_magic` or `infer`) is available, replace this function with
/// content-based detection and validate the result against the allowlist.
fn infer_mime_from_extension(file_name: &str) -> Option<String> {
    let ext = file_name.rsplit('.').next()?.to_lowercase();
    match ext.as_str() {
        "pdf" => Some("application/pdf".into()),
        "png" => Some("image/png".into()),
        "jpg" | "jpeg" => Some("image/jpeg".into()),
        "gif" => Some("image/gif".into()),
        "webp" => Some("image/webp".into()),
        "txt" => Some("text/plain".into()),
        "csv" => Some("text/csv".into()),
        "json" => Some("application/json".into()),
        "zip" => Some("application/zip".into()),
        "tar" => Some("application/x-tar".into()),
        "gz" | "gzip" => Some("application/gzip".into()),
        "xlsx" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into()),
        "docx" => {
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document".into())
        }
        "xls" => Some("application/vnd.ms-excel".into()),
        "ppt" | "pptx" => Some("application/vnd.ms-powerpoint".into()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

impl TestCaseFileHandler {
    /// `GET /api/v1/projects/{project_id}/test-cases/{id}/files` — list files.
    pub async fn list(
        &self,
        req: HttpRequest,
        path: web::Path<(i64, i64)>,
        query: web::Query<ListFilesQuery>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            Ok(_) => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let (project_id, test_case_id) = path.into_inner();

        let search_trimmed = query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());

        // Validate search length.
        if let Some(s) = search_trimmed
            && s.len() > 255
        {
            return HttpResponse::UnprocessableEntity().json(ApiError::validation(
                "search exceeds maximum length of 255 characters",
                vec![],
            ));
        }

        match self
            .service
            .list_files(
                project_id,
                test_case_id,
                search_trimmed,
                &query.sort,
                user_id,
            )
            .await
        {
            Ok(items) => {
                let data: Vec<TestCaseFileResponse> = items
                    .into_iter()
                    .map(|item| TestCaseFileResponse {
                        id: item.id,
                        file_name: item.file_name,
                        file_size: item.file_size,
                        mime_type: item.mime_type,
                        uploaded_by: item.uploaded_by,
                        created_at: item.created_at,
                        updated_at: item.updated_at,
                        test_case_id,
                    })
                    .collect();
                HttpResponse::Ok().json(serde_json::json!({ "data": data }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `POST /api/v1/projects/{project_id}/test-cases/{id}/files` — upload a
    /// file.
    ///
    /// The request must use `multipart/form-data` encoding with a single file
    /// part named `file`. All authorization values are fetched server-side
    /// from the database — client input is **never** trusted.
    pub async fn upload(
        &self,
        req: HttpRequest,
        path: web::Path<(i64, i64)>,
        body: Multipart,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            Ok(_) => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let (project_id, test_case_id) = path.into_inner();

        // Parse multipart: extract the "file" part.
        let mut file_data: Option<Vec<u8>> = None;
        let mut file_name: Option<String> = None;
        let mut content_type: Option<String> = None;

        let mut body = body;
        while let Some(Ok(mut field)) = body.next().await {
            let field_name = field
                .content_disposition()
                .as_ref()
                .and_then(|cd| cd.get_name())
                .unwrap_or("")
                .to_string();

            if field_name == "file" {
                file_name = field
                    .content_disposition()
                    .as_ref()
                    .and_then(|cd| cd.get_filename())
                    .map(|s| s.to_string());

                content_type = field.content_type().map(|m| m.to_string());

                let mut data = Vec::new();
                while let Some(Ok(chunk)) = field.next().await {
                    data.extend_from_slice(&chunk);
                }
                file_data = Some(data);
            } else {
                // Drain and discard other parts.
                while let Some(Ok(_chunk)) = field.next().await {}
            }
        }

        let file_data = match file_data {
            Some(d) => d,
            None => {
                return HttpResponse::UnprocessableEntity().json(ApiError::validation(
                    "request must contain a 'file' part",
                    vec![hoa_tcms_pkg::errors::FieldError {
                        field: "file".into(),
                        message: "file part is required".into(),
                    }],
                ));
            }
        };

        let file_name = match file_name {
            Some(name) if !name.trim().is_empty() => name,
            _ => {
                return HttpResponse::UnprocessableEntity().json(ApiError::validation(
                    "file name is required and must not be empty",
                    vec![hoa_tcms_pkg::errors::FieldError {
                        field: "file".into(),
                        message: "file name is required".into(),
                    }],
                ));
            }
        };

        let file_size = file_data.len() as i64;

        // TODO: Use magic bytes for MIME detection instead of trusting the
        // client-supplied Content-Type. For now, prefer the Content-Type from
        // the multipart header, falling back to extension-based inference.
        let mime_type = content_type
            .filter(|ct| !ct.is_empty() && ct != "application/octet-stream")
            .or_else(|| infer_mime_from_extension(&file_name))
            .unwrap_or_else(|| "application/octet-stream".into());

        match self
            .service
            .upload_file(
                project_id,
                test_case_id,
                file_data,
                file_name.clone(),
                mime_type,
                file_size,
                user_id,
            )
            .await
        {
            Ok(saved) => {
                let resp = TestCaseFileResponse {
                    id: saved.id,
                    file_name: saved.file_name,
                    file_size: saved.file_size,
                    mime_type: saved.mime_type,
                    uploaded_by: saved.uploaded_by,
                    created_at: saved.created_at,
                    updated_at: saved.updated_at,
                    test_case_id: saved.test_case_id,
                };
                let location = format!(
                    "/api/v1/projects/{}/test-cases/{}/files/{}",
                    project_id, test_case_id, saved.id
                );
                HttpResponse::Created()
                    .insert_header(("Location", location))
                    .json(serde_json::json!({ "data": resp }))
            }
            Err(e) => map_service_error(e),
        }
    }

    /// `GET /api/v1/projects/{project_id}/test-cases/{id}/files/{file_id}`
    /// — download a file.
    ///
    /// Streams the file from disk via `NamedFile` so the entire file is never
    /// buffered in memory, preventing resource-exhaustion DoS under concurrent
    /// download requests.
    pub async fn download(
        &self,
        req: HttpRequest,
        path: web::Path<(i64, i64, i64)>,
    ) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            Ok(_) => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let (project_id, test_case_id, file_id) = path.into_inner();

        let (file, abs_path) = match self
            .service
            .download_file(project_id, test_case_id, file_id, user_id)
            .await
        {
            Ok(r) => r,
            Err(e) => return map_service_error(e),
        };

        let mime_type: mime::Mime = file
            .mime_type
            .parse()
            .unwrap_or(mime::APPLICATION_OCTET_STREAM);

        match actix_files::NamedFile::open(abs_path) {
            Ok(named) => named
                .set_content_type(mime_type)
                .set_content_disposition(actix_web::http::header::ContentDisposition {
                    disposition: actix_web::http::header::DispositionType::Attachment,
                    parameters: vec![actix_web::http::header::DispositionParam::Filename(
                        file.file_name.clone(),
                    )],
                })
                .into_response(&req),
            Err(e) => {
                tracing::error!(
                    file.id = file.id,
                    error = %e,
                    "failed to open file for download"
                );
                HttpResponse::NotFound().json(ApiError::not_found("not found"))
            }
        }
    }

    /// `DELETE /api/v1/projects/{project_id}/test-cases/{id}/files/{file_id}`
    /// — soft-delete a file.
    pub async fn delete(&self, req: HttpRequest, path: web::Path<(i64, i64, i64)>) -> HttpResponse {
        let user_id = match auth::extract_user_id(&req).await {
            Ok(id) if id > 0 => id,
            Ok(_) => {
                return HttpResponse::Unauthorized()
                    .json(ApiError::forbidden("authentication required"));
            }
            Err(e) => return HttpResponse::Unauthorized().json(e),
        };

        let (project_id, test_case_id, file_id) = path.into_inner();

        match self
            .service
            .delete_file(project_id, test_case_id, file_id, user_id)
            .await
        {
            Ok(()) => HttpResponse::NoContent().finish(),
            Err(e) => map_service_error(e),
        }
    }
}
