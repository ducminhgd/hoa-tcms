//! Test Case File DTOs — request/response shapes for file endpoints.
//!
//! Request DTOs define the expected query parameter shapes for file list
//! endpoints. Response DTOs define the public representation of file metadata.

use chrono::{DateTime, Utc};

/// Query parameters for listing files attached to a test case.
#[derive(Debug, serde::Deserialize)]
pub struct ListFilesQuery {
    /// Optional case-insensitive substring search on `file_name`.
    #[serde(default)]
    pub search: Option<String>,

    /// Sort field with optional `-` prefix for descending order.
    ///
    /// Acceptable values: `file_name`, `-file_name`, `file_size`,
    /// `-file_size`, `created_at`, `-created_at`.
    #[serde(default = "default_sort")]
    pub sort: String,
}

fn default_sort() -> String {
    "-created_at".to_string()
}

/// Public file metadata returned in API responses.
///
/// The `file_path` and soft-delete columns are intentionally excluded.
#[derive(Debug, serde::Serialize)]
pub struct TestCaseFileResponse {
    pub id: i64,
    pub file_name: String,
    pub file_size: i64,
    pub mime_type: String,
    pub uploaded_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub test_case_id: i64,
}
