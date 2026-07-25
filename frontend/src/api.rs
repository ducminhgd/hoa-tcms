//! Typed API client for the HOA TCMS REST API.
//!
//! Uses `gloo_net::http::Request` to call the backend. All functions are async
//! and return `Result<T, String>` for use in Leptos resources.

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

const BASE: &str = "/api/v1";

// ---------------------------------------------------------------------------
// Generic helpers
// ---------------------------------------------------------------------------

async fn get<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    let resp = Request::get(url).send().await.map_err(|e| e.to_string())?;
    if resp.ok() {
        resp.json::<T>().await.map_err(|e| e.to_string())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

async fn post<B: Serialize, T: for<'de> Deserialize<'de>>(
    url: &str,
    body: &B,
) -> Result<T, String> {
    let resp = Request::post(url)
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        resp.json::<T>().await.map_err(|e| e.to_string())
    } else {
        Err(resp.text().await.unwrap_or_default())
    }
}

async fn patch<B: Serialize, T: for<'de> Deserialize<'de>>(
    url: &str,
    body: &B,
) -> Result<T, String> {
    let resp = Request::patch(url)
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        resp.json::<T>().await.map_err(|e| e.to_string())
    } else {
        Err(resp.text().await.unwrap_or_default())
    }
}

async fn delete(url: &str) -> Result<(), String> {
    let resp = Request::delete(url).send().await.map_err(|e| e.to_string())?;
    if resp.ok() || resp.status() == 204 {
        Ok(())
    } else {
        Err(resp.text().await.unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// Paginated response wrapper
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub data: Vec<T>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationMeta {
    pub total: u64,
    pub page: u32,
    pub limit: u32,
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username_or_email: String,
    pub password: String,
}

pub async fn login(body: &LoginRequest) -> Result<(), String> {
    let _ = post::<_, serde_json::Value>(&format!("{}/auth/login", BASE), body).await?;
    Ok(())
}

pub async fn logout() -> Result<(), String> {
    let _ = post::<(), serde_json::Value>(&format!("{}/auth/logout", BASE), &()).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRow {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub description: Option<String>,
    pub status: String,
}

pub async fn list_projects(
    page: u32,
    limit: u32,
) -> Result<Paginated<ProjectRow>, String> {
    get(&format!(
        "{}/projects?page={}&limit={}",
        BASE, page, limit
    ))
    .await
}

pub async fn create_project(body: &CreateProject) -> Result<ProjectRow, String> {
    post::<_, serde_json::Value>(&format!("{}/projects", BASE), body)
        .await
        .map(|v| serde_json::from_value(v["data"].clone()).unwrap())
}

// ---------------------------------------------------------------------------
// Test Plans
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPlanRow {
    pub id: i64,
    pub name: String,
    pub version: String,
    pub types: Vec<String>,
    pub status: String,
    pub project_ids: Vec<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTestPlan {
    pub name: String,
    pub project_ids: Vec<i64>,
    pub types: Vec<String>,
    pub version: String,
    pub description: Option<String>,
}

pub async fn list_test_plans(
    page: u32,
    limit: u32,
) -> Result<Paginated<TestPlanRow>, String> {
    get(&format!(
        "{}/test-plans?page={}&limit={}",
        BASE, page, limit
    ))
    .await
}

pub async fn create_test_plan(body: &CreateTestPlan) -> Result<TestPlanRow, String> {
    post::<_, serde_json::Value>(&format!("{}/test-plans", BASE), body)
        .await
        .map(|v| serde_json::from_value(v["data"].clone()).unwrap())
}

pub async fn transition_plan_status(plan_id: i64, status: &str) -> Result<(), String> {
    let body = serde_json::json!({ "status": status });
    let _ = post::<_, serde_json::Value>(
        &format!("{}/test-plans/{}/transition-status", BASE, plan_id),
        &body,
    )
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Test Runs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunRow {
    pub id: i64,
    pub summary: String,
    pub project_id: i64,
    pub plan_id: Option<i64>,
    pub version: Option<String>,
    pub case_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTestRun {
    pub summary: String,
    pub report_to: Option<i64>,
    pub default_tester: Option<i64>,
    pub plan_id: Option<i64>,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub planned_start_date: Option<String>,
    pub planned_end_date: Option<String>,
}

pub async fn list_test_runs(
    project_id: i64,
    page: u32,
    limit: u32,
) -> Result<Paginated<TestRunRow>, String> {
    get(&format!(
        "{}/projects/{}/test-runs?page={}&limit={}",
        BASE, project_id, page, limit
    ))
    .await
}

pub async fn create_test_run(
    project_id: i64,
    body: &CreateTestRun,
) -> Result<TestRunRow, String> {
    post::<_, serde_json::Value>(&format!("{}/projects/{}/test-runs", BASE, project_id), body)
        .await
        .map(|v| serde_json::from_value(v["data"].clone()).unwrap())
}

// ---------------------------------------------------------------------------
// Test Executions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExecutionRow {
    pub id: i64,
    pub name: String,
    pub test_run_id: i64,
    pub tester_count: u64,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn list_test_executions(
    page: u32,
    limit: u32,
) -> Result<Paginated<TestExecutionRow>, String> {
    get(&format!(
        "{}/test-executions?page={}&limit={}",
        BASE, page, limit
    ))
    .await
}

// ---------------------------------------------------------------------------
// Users (IAM)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRow {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub fullname: String,
    pub status: String,
}

pub async fn list_users(page: u32, limit: u32) -> Result<Paginated<UserRow>, String> {
    get(&format!("{}/users?page={}&limit={}", BASE, page, limit)).await
}
