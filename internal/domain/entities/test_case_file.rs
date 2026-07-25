//! `TestCaseFile` entity — a file attachment on a test case.
//!
//! Each file belongs to a single test case and is stored on the server
//! filesystem. The entity tracks upload metadata and soft-delete state.
//!
//! This is a pure domain entity with **no ORM or framework imports**.

use chrono::{DateTime, Utc};

/// A file attached to a test case.
///
/// Maps to the `test_case_files` table. The `id` field is `0` until the
/// entity is persisted.
#[derive(Clone)]
pub struct TestCaseFile {
    pub id: i64,
    pub test_case_id: i64,
    pub file_name: String,
    pub file_path: String,
    pub file_size: i64,
    pub mime_type: String,
    pub uploaded_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_by: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for TestCaseFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestCaseFile")
            .field("id", &self.id)
            .field("test_case_id", &self.test_case_id)
            .field("file_name", &self.file_name)
            .field("file_size", &self.file_size)
            .field("mime_type", &self.mime_type)
            .field("uploaded_by", &self.uploaded_by)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .field("deleted_by", &self.deleted_by)
            .field("deleted_at", &self.deleted_at)
            .finish()
    }
}

impl TestCaseFile {
    /// Create a new `TestCaseFile`.
    ///
    /// The `id` is left at `0` and will be assigned by the persistence layer.
    ///
    /// # Panics
    ///
    /// Panics if `file_name` is empty, if `file_path` is empty, if `file_size`
    /// is zero, or if `mime_type` is empty (callers must validate before
    /// calling).
    pub fn create(
        test_case_id: i64,
        file_name: String,
        file_path: String,
        file_size: i64,
        mime_type: String,
        uploaded_by: i64,
    ) -> Self {
        debug_assert!(!file_name.trim().is_empty(), "file_name must not be empty");
        debug_assert!(!file_path.trim().is_empty(), "file_path must not be empty");
        debug_assert!(file_size > 0, "file_size must be positive");
        debug_assert!(!mime_type.trim().is_empty(), "mime_type must not be empty");

        let now = Utc::now();
        Self {
            id: 0,
            test_case_id,
            file_name,
            file_path,
            file_size,
            mime_type,
            uploaded_by,
            created_at: now,
            updated_at: now,
            deleted_by: None,
            deleted_at: None,
        }
    }

    /// Returns `true` if the file has been soft-deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
