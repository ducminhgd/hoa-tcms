//! Test execution DTOs — request/response shapes for test execution and
//! test case result endpoints.
//!
//! Request DTOs define the expected JSON shape for test execution endpoints.
//! Response DTOs define the public representation of test execution data.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------
// Request DTOs
// ---------------------------------------------------------------

/// Create a new test execution.
#[derive(Debug, Deserialize)]
pub struct CreateTestExecutionRequest {
    pub name: String,
    pub test_run_id: i64,
    #[serde(default)]
    pub tester_ids: Vec<i64>,
}

impl CreateTestExecutionRequest {
    /// Validate all fields, returning a list of field-level error messages.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let name = self.name.trim();
        if name.is_empty() {
            errors.push("name is required".to_string());
        } else if name.len() > 255 {
            errors.push("name must not exceed 255 characters".to_string());
        }
        if self.test_run_id <= 0 {
            errors.push("test_run_id must be a positive integer".to_string());
        }
        for (i, &tid) in self.tester_ids.iter().enumerate() {
            if tid <= 0 {
                errors.push(format!("tester_ids[{}] must be a positive integer", i));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Update an existing test execution.
///
/// All fields are optional — only provided fields will be updated.
#[derive(Debug, Deserialize)]
pub struct UpdateTestExecutionRequest {
    pub name: Option<String>,
    pub tester_ids: Option<Vec<i64>>,
}

impl UpdateTestExecutionRequest {
    /// Validate the optional fields if they are present.
    ///
    /// Returns `Ok(())` if at least one field was provided and all provided
    /// fields are valid.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let mut has_field = false;

        if let Some(ref name) = self.name {
            has_field = true;
            if name.trim().is_empty() {
                errors.push("name must not be empty".to_string());
            } else if name.len() > 255 {
                errors.push("name must not exceed 255 characters".to_string());
            }
        }
        if let Some(ref tester_ids) = self.tester_ids {
            has_field = true;
            for (i, &tid) in tester_ids.iter().enumerate() {
                if tid <= 0 {
                    errors.push(format!("tester_ids[{}] must be a positive integer", i));
                }
            }
        }

        if !has_field {
            errors.push("at least one field must be provided".to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Import test cases into an execution.
#[derive(Debug, Deserialize)]
pub struct ImportCasesRequest {
    pub case_ids: Vec<i64>,
}

impl ImportCasesRequest {
    /// Validate the request body.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.case_ids.is_empty() {
            errors.push("case_ids must not be empty".to_string());
        }
        for (i, &cid) in self.case_ids.iter().enumerate() {
            if cid <= 0 {
                errors.push(format!("case_ids[{}] must be a positive integer", i));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Update a test case result's outcome.
#[derive(Debug, Deserialize)]
pub struct UpdateTestCaseResultRequest {
    pub result: Option<String>,
    pub logs: Option<String>,
}

impl UpdateTestCaseResultRequest {
    /// Validate the request body.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let mut has_field = false;

        if let Some(ref result) = self.result {
            has_field = true;
            let upper = result.to_uppercase();
            if !matches!(
                upper.as_str(),
                "NOT_TESTED" | "IN_PROGRESS" | "PASS" | "FAIL" | "WARNING" | "IGNORE"
            ) {
                errors.push(
                    "result must be one of: NOT_TESTED, IN_PROGRESS, PASS, FAIL, WARNING, IGNORE"
                        .to_string(),
                );
            }
        }
        if self.logs.is_some() {
            has_field = true;
        }

        if !has_field {
            errors.push("at least one of 'result' or 'logs' must be provided".to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Query parameters for listing test executions.
#[derive(Debug, Deserialize)]
pub struct ListTestExecutionsQuery {
    #[serde(default)]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
    pub test_run_id: Option<i64>,
}

fn default_limit() -> u32 {
    25
}

// ---------------------------------------------------------------
// Response DTOs
// ---------------------------------------------------------------

/// Test execution data returned in list responses.
#[derive(Debug, Serialize)]
pub struct TestExecutionListItem {
    pub id: i64,
    pub name: String,
    pub test_run_id: i64,
    pub tester_count: u64,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single tester's brief info.
#[derive(Debug, Serialize)]
pub struct TesterInfo {
    pub user_id: i64,
    pub username: String,
    pub fullname: String,
}

/// Full test execution data returned in single-resource responses.
#[derive(Debug, Serialize)]
pub struct TestExecutionResponse {
    pub id: i64,
    pub name: String,
    pub test_run_id: i64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub testers: Vec<TesterInfo>,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_by: i64,
    pub updated_at: DateTime<Utc>,
}

/// A test case result as exposed in the API.
#[derive(Debug, Serialize)]
pub struct TestCaseResultResponse {
    pub id: i64,
    pub execution_id: i64,
    pub test_case_id: i64,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub priority: String,
    pub result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tested_by: Option<i64>,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_by: i64,
    pub updated_at: DateTime<Utc>,
}

/// Response returned after importing cases into an execution.
#[derive(Debug, Serialize)]
pub struct ImportCasesResponse {
    pub imported_count: u32,
    pub refreshed_count: u32,
}
