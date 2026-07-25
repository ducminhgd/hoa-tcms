//! `TestCaseFileService` — use case orchestration for file attachment CRUD.
//!
//! Each method checks system permissions via [`AuthorizationService`],
//! validates project membership and role-based access (Contributor/Editor/Owner),
//! then delegates to the repository and file-storage backends.
//!
//! **Security:** All authorization values (`test_case.created_by`,
//! `test_case.project_id`) are fetched server-side from the database — client
//! input is **never** trusted for access-control decisions.
//!
//! This service is agnostic to HTTP or database details.

use std::sync::Arc;

use uuid::Uuid;

use crate::application::repositories::file_storage::FileStorage;
use crate::application::repositories::project_member_repository::ProjectMemberRepository;
use crate::application::repositories::test_case_file_repository::{
    TestCaseFileListItem, TestCaseFileRepository, TestCaseInfo,
};
use crate::application::services::authorization::AuthorizationService;
use crate::application::services::errors::ServiceError;
use crate::domain::entities::test_case_file::TestCaseFile;

/// Orchestrates test case file management use cases.
pub struct TestCaseFileService {
    file_repo: Box<dyn TestCaseFileRepository>,
    file_storage: Box<dyn FileStorage>,
    auth: Arc<AuthorizationService>,
    member_repo: Box<dyn ProjectMemberRepository>,
    upload_max_size: u64,
    allowed_mime_types: Vec<String>,
}

impl TestCaseFileService {
    /// Create a new `TestCaseFileService`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        file_repo: Box<dyn TestCaseFileRepository>,
        file_storage: Box<dyn FileStorage>,
        auth: Arc<AuthorizationService>,
        member_repo: Box<dyn ProjectMemberRepository>,
        upload_max_size: u64,
        allowed_mime_types: Vec<String>,
    ) -> Self {
        Self {
            file_repo,
            file_storage,
            auth,
            member_repo,
            upload_max_size,
            allowed_mime_types,
        }
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Validate that `test_case_id` belongs to `project_id`.
    ///
    /// Fetches test case metadata **from the database** — never trusts client
    /// input. Returns the server-side `TestCaseInfo` for use in further
    /// authorization checks (e.g. Contributor ownership).
    async fn validate_test_case_access(
        &self,
        project_id: i64,
        test_case_id: i64,
    ) -> Result<TestCaseInfo, ServiceError> {
        let info = self
            .file_repo
            .get_test_case_info(test_case_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        // Cross-project guard: if the test case belongs to a project, it must
        // match the project_id from the URL path.  This prevents a member of
        // Project A from accessing test cases in Project B by swapping the URL.
        if let Some(tc_project_id) = info.project_id
            && tc_project_id != project_id
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }
        // Orphaned test cases (project_id IS NULL) are owned only by their
        // creator.  We still allow access here; the caller can apply further
        // creator-based restrictions as needed.

        Ok(info)
    }

    // ------------------------------------------------------------------
    // Upload
    // ------------------------------------------------------------------

    /// Upload a file attachment to a test case.
    ///
    /// The caller must hold `test_case_file:upload`, be at least a Contributor
    /// of the project, and (if Contributor) own the test case.
    #[allow(clippy::too_many_arguments)]
    pub async fn upload_file(
        &self,
        project_id: i64,
        test_case_id: i64,
        file_data: Vec<u8>,
        file_name: String,
        mime_type: String,
        file_size: i64,
        current_user_id: i64,
    ) -> Result<TestCaseFile, ServiceError> {
        // 1. Check system permission (System Admin bypasses).
        if !self
            .auth
            .check_permission(current_user_id, "test_case_file:upload")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // 2. Fetch test case info from DB — cross-project + server-side
        //    ownership check.
        let tc_info = self
            .validate_test_case_access(project_id, test_case_id)
            .await?;

        // 3. Check project membership — get role.
        let member = self
            .member_repo
            .get_member(project_id, current_user_id)
            .await
            .map_err(ServiceError::from)?;

        let role = match member {
            Some(m) => m.role,
            None => return Err(ServiceError::PermissionDenied("forbidden".into())),
        };

        // 3b. Viewer cannot upload. System Admin already bypassed.
        if role == "Viewer" {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // 4. If Contributor: verify test case ownership using the DB-backed
        //    `created_by` value — NOT a client-supplied parameter.
        if role == "Contributor" && tc_info.created_by != current_user_id {
            return Err(ServiceError::PermissionDenied(
                "Contributors can only upload files to their own test cases".into(),
            ));
        }

        // 5. Validate file size.
        let file_size_u64 = file_size as u64;
        if file_size_u64 > self.upload_max_size {
            return Err(ServiceError::Validation(format!(
                "file size exceeds the maximum allowed size of {} bytes",
                self.upload_max_size
            )));
        }

        // 6. Validate MIME type against allowlist.
        let mime_trimmed = mime_type.trim().to_lowercase();
        if !self.allowed_mime_types.contains(&mime_trimmed) {
            return Err(ServiceError::Validation(format!(
                "file type '{}' is not allowed. Allowed types: {}",
                mime_trimmed,
                self.allowed_mime_types.join(", "),
            )));
        }

        // 7. Sanitize file_name: trim, check length, reject path traversal.
        let file_name = file_name.trim().to_string();
        if file_name.is_empty() || file_name.len() > 255 {
            return Err(ServiceError::Validation(
                "file name must be between 1 and 255 characters".into(),
            ));
        }
        if file_name.contains("../")
            || file_name.contains("..\\")
            || file_name.contains('/')
            || file_name.contains('\\')
        {
            return Err(ServiceError::Validation(
                "file name contains invalid characters".into(),
            ));
        }

        // 8. Generate storage path: {project_id}/{test_case_id}/{uuid}.{ext}
        let ext = file_name
            .rsplit('.')
            .next()
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
                    .to_lowercase()
            })
            .filter(|s| !s.is_empty())
            .map(|s| format!(".{}", s))
            .unwrap_or_default();

        let uuid = Uuid::new_v4();
        let storage_path = format!("{}/{}/{}{}", project_id, test_case_id, uuid, ext);

        // 9. Store file to storage backend.
        self.file_storage
            .store(&storage_path, &file_data)
            .await
            .map_err(|e| ServiceError::Internal(format!("file storage error: {}", e)))?;

        // 10. Create domain entity and persist.
        let entity = TestCaseFile::create(
            test_case_id,
            file_name.to_string(),
            storage_path.clone(),
            file_size,
            mime_trimmed.clone(),
            current_user_id,
        );

        let saved = match self.file_repo.save(&entity).await {
            Ok(f) => f,
            Err(e) => {
                // Clean up orphaned file from storage on DB save failure.
                let _ = self.file_storage.delete(&storage_path).await;
                return Err(ServiceError::from(e));
            }
        };

        Ok(saved)
    }

    // ------------------------------------------------------------------
    // List
    // ------------------------------------------------------------------

    /// List non-deleted files attached to a test case.
    ///
    /// The caller must hold `test_case_file:read` and be a project member
    /// (any role).
    pub async fn list_files(
        &self,
        project_id: i64,
        test_case_id: i64,
        search: Option<&str>,
        sort: &str,
        current_user_id: i64,
    ) -> Result<Vec<TestCaseFileListItem>, ServiceError> {
        // 1. Check system permission.
        if !self
            .auth
            .check_permission(current_user_id, "test_case_file:read")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // 2. Cross-project check + test case existence.
        self.validate_test_case_access(project_id, test_case_id)
            .await?;

        // 3. Check project membership (any role).
        let member = self
            .member_repo
            .get_member(project_id, current_user_id)
            .await
            .map_err(ServiceError::from)?;

        if member.is_none() {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // 4. Delegate to repository.
        self.file_repo
            .find_by_test_case(test_case_id, search, sort)
            .await
            .map_err(ServiceError::from)
    }

    // ------------------------------------------------------------------
    // Download
    // ------------------------------------------------------------------

    /// Download a file attachment.
    ///
    /// Returns the file metadata and the resolved **absolute** filesystem path
    /// for streaming. The path is internal and **must never** be returned to
    /// API clients. The caller (HTTP handler) uses `NamedFile` to stream the
    /// response so the entire file is never buffered in memory.
    ///
    /// The caller must hold `test_case_file:read` and be a project member
    /// (any role).
    pub async fn download_file(
        &self,
        project_id: i64,
        test_case_id: i64,
        file_id: i64,
        current_user_id: i64,
    ) -> Result<(TestCaseFile, String), ServiceError> {
        // 1. Check system permission.
        if !self
            .auth
            .check_permission(current_user_id, "test_case_file:read")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // 2. Cross-project check + test case existence.
        self.validate_test_case_access(project_id, test_case_id)
            .await?;

        // 3. Check project membership (any role).
        let member = self
            .member_repo
            .get_member(project_id, current_user_id)
            .await
            .map_err(ServiceError::from)?;

        if member.is_none() {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // 4. Fetch file by ID.
        let file = self
            .file_repo
            .find_by_id(file_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        // 5. Defence-in-depth: verify file belongs to the expected test case.
        if file.test_case_id != test_case_id {
            return Err(ServiceError::NotFound);
        }

        // 6. Verify physical file exists.
        if !self
            .file_storage
            .exists(&file.file_path)
            .await
            .map_err(|e| ServiceError::Internal(format!("file storage error: {}", e)))?
        {
            tracing::error!(file.id = file.id, "physical file missing from storage");
            return Err(ServiceError::NotFound);
        }

        // 7. Resolve absolute path for streaming (never returned to client).
        let abs_path = self.file_storage.resolve_path(&file.file_path);

        Ok((file, abs_path))
    }

    // ------------------------------------------------------------------
    // Delete
    // ------------------------------------------------------------------

    /// Soft-delete a file attachment.
    ///
    /// The caller must hold `test_case_file:delete`, be at least a Contributor
    /// of the project, and (if Contributor) own the file (`uploaded_by`
    /// matches).
    pub async fn delete_file(
        &self,
        project_id: i64,
        test_case_id: i64,
        file_id: i64,
        current_user_id: i64,
    ) -> Result<(), ServiceError> {
        // 1. Check system permission.
        if !self
            .auth
            .check_permission(current_user_id, "test_case_file:delete")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // 2. Cross-project check + test case existence.
        self.validate_test_case_access(project_id, test_case_id)
            .await?;

        // 3. Check project membership.
        let member = self
            .member_repo
            .get_member(project_id, current_user_id)
            .await
            .map_err(ServiceError::from)?;

        let role = match member {
            Some(m) => m.role,
            None => return Err(ServiceError::PermissionDenied("forbidden".into())),
        };

        // 3b. Viewer cannot delete.
        if role == "Viewer" {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // 4. Fetch file by ID.
        let file = self
            .file_repo
            .find_by_id(file_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        // 5. Defence-in-depth: verify file belongs to the expected test case.
        if file.test_case_id != test_case_id {
            return Err(ServiceError::NotFound);
        }

        // 6. If Contributor: verify file ownership (DB-backed, not client input).
        if role == "Contributor" && file.uploaded_by != current_user_id {
            return Err(ServiceError::PermissionDenied(
                "Contributors can only delete files they uploaded".into(),
            ));
        }

        // 7. Soft-delete.
        self.file_repo
            .soft_delete(file_id, current_user_id)
            .await
            .map_err(ServiceError::from)
    }
}
