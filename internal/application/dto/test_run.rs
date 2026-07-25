//! DTOs for test run requests and responses.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListTestRunsQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
    pub plan_id: Option<i64>,
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
    "-id".to_string()
}

#[derive(Debug, Deserialize)]
pub struct CreateTestRunRequest {
    pub summary: String,
    pub report_to: Option<i64>,
    #[serde(default)]
    pub default_tester: Option<i64>,
    pub plan_id: Option<i64>,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub planned_start_date: Option<String>,
    pub planned_end_date: Option<String>,
    pub case_ids: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTestRunRequest {
    pub summary: Option<String>,
    pub report_to: Option<i64>,
    pub default_tester: Option<i64>,
    pub plan_id: Option<i64>,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub planned_start_date: Option<String>,
    pub planned_end_date: Option<String>,
    pub case_ids: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
pub struct ManageCasesRequest {
    pub add_case_ids: Option<Vec<i64>>,
    pub remove_case_ids: Option<Vec<i64>>,
}

// ---------------------------------------------------------------------------
// Response
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct TestRunResponse {
    pub id: i64,
    pub summary: String,
    pub report_to_user_id: Option<i64>,
    pub default_tester_id: i64,
    pub project_id: i64,
    pub plan_id: Option<i64>,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub planned_start: Option<String>,
    pub planned_stop: Option<String>,
    pub case_ids: Vec<i64>,
    pub execution_ids: Vec<i64>,
    pub created_by: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_by: i64,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct TestRunListItemResponse {
    pub id: i64,
    pub summary: String,
    pub report_to_user_id: Option<i64>,
    pub default_tester_id: i64,
    pub project_id: i64,
    pub plan_id: Option<i64>,
    pub version: Option<String>,
    pub planned_start: Option<String>,
    pub planned_stop: Option<String>,
    pub case_count: i64,
    pub created_by: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct TestRunStatisticsResponse {
    pub total: i64,
    pub not_tested: i64,
    pub in_progress: i64,
    pub pass: i64,
    pub fail: i64,
    pub warning: i64,
    pub ignore: i64,
}
