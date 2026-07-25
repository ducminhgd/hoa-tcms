//! DTOs for test plan requests and responses.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Request DTOs
// ---------------------------------------------------------------------------

/// Query parameters for listing test plans.
#[derive(Debug, Deserialize)]
pub struct ListTestPlansQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
    pub status: Option<String>,
    #[serde(rename = "plan_type")]
    pub plan_type: Option<String>,
    pub project_id: Option<i64>,
    pub search: Option<String>,
    #[serde(default = "default_sort")]
    pub sort: String,
}

fn default_page() -> u32 {
    1
}
fn default_limit() -> u32 {
    25
}
fn default_sort() -> String {
    "-updated_at".to_string()
}

/// Request body for creating a test plan.
#[derive(Debug, Deserialize)]
pub struct CreateTestPlanRequest {
    pub name: String,
    pub project_ids: Vec<i64>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default = "default_version")]
    pub version: String,
    pub description: Option<String>,
}

fn default_version() -> String {
    "UNSPECIFIED".to_string()
}

/// Request body for updating a test plan (all fields optional).
#[derive(Debug, Deserialize)]
pub struct UpdateTestPlanRequest {
    pub name: Option<String>,
    pub version: Option<String>,
    pub types: Option<Vec<String>>,
    pub description: Option<String>,
    pub project_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub status: Option<String>,
}

/// Request body for status transition.
#[derive(Debug, Deserialize)]
pub struct TransitionStatusRequest {
    pub status: String,
}

// ---------------------------------------------------------------------------
// Response DTOs
// ---------------------------------------------------------------------------

/// Full test plan detail response.
#[derive(Debug, Serialize)]
pub struct TestPlanResponse {
    pub id: i64,
    pub name: String,
    pub version: String,
    pub types: Vec<String>,
    pub status: String,
    pub description: Option<String>,
    pub project_ids: Vec<i64>,
    pub created_by: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_by: i64,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Compact list item for paginated list responses.
#[derive(Debug, Serialize)]
pub struct TestPlanListItemResponse {
    pub id: i64,
    pub name: String,
    pub version: String,
    pub types: Vec<String>,
    pub status: String,
    pub project_ids: Vec<i64>,
    pub created_by: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Dropdown select item (id + name only).
#[derive(Debug, Serialize)]
pub struct TestPlanSelectResponse {
    pub id: i64,
    pub name: String,
}
