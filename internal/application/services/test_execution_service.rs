//! `TestExecutionService` — use case orchestration for test execution CRUD,
//! test case import, and result updates.
//!
//! Each method checks system permissions via [`AuthorizationService`],
//! validates access (project membership or tester/creator),
//! delegates to the repository layer, and returns domain entities or DTOs.
//! This service is agnostic to HTTP or database details.

use std::collections::HashMap;
use std::sync::Arc;

use crate::application::repositories::test_case_result_repository::TestCaseResultRepository;
use crate::application::repositories::test_execution_repository::{
    TestExecutionRepository, TesterInfoRow,
};
use crate::application::services::authorization::AuthorizationService;
use crate::application::services::errors::ServiceError;
use crate::domain::entities::test_case_result::TestCaseResult;
use crate::domain::entities::test_execution::TestExecution;
use crate::domain::value_objects::test_result_status::TestResultStatus;

/// Orchestrates test execution and test case result management use cases.
pub struct TestExecutionService {
    execution_repo: Box<dyn TestExecutionRepository>,
    result_repo: Box<dyn TestCaseResultRepository>,
    auth: Arc<AuthorizationService>,
}

impl TestExecutionService {
    /// Create a new `TestExecutionService`.
    pub fn new(
        execution_repo: Box<dyn TestExecutionRepository>,
        result_repo: Box<dyn TestCaseResultRepository>,
        auth: Arc<AuthorizationService>,
    ) -> Self {
        Self {
            execution_repo,
            result_repo,
            auth,
        }
    }

    /// Create a new test execution with assigned testers.
    ///
    /// The caller must hold the `test_execution:create` system permission.
    pub async fn create(
        &self,
        user_id: i64,
        name: String,
        test_run_id: i64,
        tester_ids: Vec<i64>,
    ) -> Result<TestExecution, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_execution:create")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // Validate that test_run_id exists before creating the execution.
        if !self.execution_repo.test_run_exists(test_run_id).await? {
            return Err(ServiceError::Validation(
                "test_run_id does not exist".into(),
            ));
        }

        let execution = TestExecution::create(name, test_run_id, user_id);

        self.execution_repo
            .create(&execution, &tester_ids)
            .await
            .map_err(ServiceError::from)
    }

    /// List test executions with pagination, optionally filtered by
    /// `test_run_id`.
    ///
    /// The caller must hold `test_execution:read_list`.
    pub async fn list(
        &self,
        user_id: i64,
        page: u32,
        limit: u32,
        test_run_id: Option<i64>,
    ) -> Result<(Vec<TestExecution>, u64), ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_execution:read_list")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // Admin sees all executions; regular users see only accessible ones.
        let is_admin = self.auth.is_admin(user_id).await?;
        if is_admin {
            self.execution_repo
                .list(page, limit, test_run_id)
                .await
                .map_err(ServiceError::from)
        } else {
            self.execution_repo
                .list_by_user(user_id, page, limit, test_run_id)
                .await
                .map_err(ServiceError::from)
        }
    }

    /// Get a single test execution.
    ///
    /// The caller must hold `test_execution:read` and have access to the
    /// execution (unless System Admin).
    pub async fn get(
        &self,
        execution_id: i64,
        user_id: i64,
    ) -> Result<TestExecution, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_execution:read")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let execution = self
            .execution_repo
            .find_by_id(execution_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let is_admin = self.auth.is_admin(user_id).await?;
        if !is_admin
            && !self
                .execution_repo
                .user_can_access(execution_id, user_id)
                .await
                .map_err(ServiceError::from)?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        Ok(execution)
    }

    /// Update a test execution's name and/or testers.
    ///
    /// The caller must hold `test_execution:update` and have access to the
    /// execution (unless System Admin).
    pub async fn update(
        &self,
        execution_id: i64,
        user_id: i64,
        name: Option<String>,
        tester_ids: Option<Vec<i64>>,
    ) -> Result<TestExecution, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_execution:update")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let mut execution = self
            .execution_repo
            .find_by_id(execution_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let is_admin = self.auth.is_admin(user_id).await?;
        if !is_admin
            && !self
                .execution_repo
                .user_can_access(execution_id, user_id)
                .await
                .map_err(ServiceError::from)?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        if let Some(new_name) = name {
            execution.name = new_name;
        }

        execution.updated_by = user_id;

        // When testers are provided, use the atomic update_with_testers.
        // Otherwise, just update the execution metadata.
        if let Some(new_tester_ids) = tester_ids {
            self.execution_repo
                .update_with_testers(&execution, &new_tester_ids)
                .await
                .map_err(ServiceError::from)
        } else {
            self.execution_repo
                .update(&execution)
                .await
                .map_err(ServiceError::from)
        }
    }

    /// Soft-delete a test execution.
    ///
    /// The caller must hold `test_execution:delete` and have access to the
    /// execution (unless System Admin).
    pub async fn delete(&self, execution_id: i64, user_id: i64) -> Result<(), ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_execution:delete")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // Verify the execution exists and is not already deleted.
        self.execution_repo
            .find_by_id(execution_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let is_admin = self.auth.is_admin(user_id).await?;
        if !is_admin
            && !self
                .execution_repo
                .user_can_access(execution_id, user_id)
                .await
                .map_err(ServiceError::from)?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        self.execution_repo
            .soft_delete(execution_id, user_id)
            .await
            .map_err(ServiceError::from)
    }

    /// Import test cases into an execution.
    ///
    /// For each `test_case_id`:
    /// - If the case has not already been imported: inserts a new
    ///   `TestCaseResult` with snapshot data (summary, description, priority).
    /// - If the case has already been imported: refreshes the snapshot fields
    ///   while preserving `result`, `logs`, and `tested_by`.
    ///
    /// Uses a single transaction via `batch_import_cases`.
    /// Returns `(imported_count, refreshed_count)`.
    ///
    /// The caller must hold `test_execution:update` and have access to the
    /// execution (unless System Admin).
    pub async fn import_cases(
        &self,
        execution_id: i64,
        user_id: i64,
        case_ids: &[i64],
    ) -> Result<(u32, u32), ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_execution:update")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // Verify the execution exists and retrieve its test_run_id.
        let execution = self
            .execution_repo
            .find_by_id(execution_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let is_admin = self.auth.is_admin(user_id).await?;
        if !is_admin
            && !self
                .execution_repo
                .user_can_access(execution_id, user_id)
                .await
                .map_err(ServiceError::from)?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // Validate that all test cases belong to the execution's test run.
        for &case_id in case_ids {
            if !self
                .result_repo
                .test_case_in_run(execution.test_run_id, case_id)
                .await?
            {
                return Err(ServiceError::Validation(format!(
                    "test case {} does not belong to the execution's test run",
                    case_id
                )));
            }
        }

        self.result_repo
            .batch_import_cases(execution_id, user_id, case_ids)
            .await
            .map_err(ServiceError::from)
    }

    /// Update a test case result's outcome (result status and/or logs).
    ///
    /// If transitioning from `NOT_TESTED` to something else, `tested_by` is
    /// set to `user_id` (once, immutable thereafter).
    ///
    /// The caller must hold `test_execution:update`.
    pub async fn update_result(
        &self,
        result_id: i64,
        user_id: i64,
        new_result: Option<String>,
        logs: Option<String>,
    ) -> Result<TestCaseResult, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_execution:update")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let mut result = self
            .result_repo
            .find_by_id(result_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        if let Some(ref status_str) = new_result {
            let status = TestResultStatus::parse(status_str).ok_or_else(|| {
                ServiceError::Validation(format!("invalid result status: {}", status_str))
            })?;

            result.result = status.clone();

            // Set tested_by only on first transition away from NOT_TESTED.
            if result.tested_by.is_none() && status != TestResultStatus::NotTested {
                result.tested_by = Some(user_id);
            }
        }

        if let Some(new_logs) = logs {
            result.logs = Some(new_logs);
        }

        result.updated_by = user_id;

        self.result_repo
            .update(&result)
            .await
            .map_err(ServiceError::from)
    }

    /// List test case results for an execution.
    ///
    /// The caller must hold `test_execution:read`.
    pub async fn list_results(
        &self,
        execution_id: i64,
        user_id: i64,
        page: u32,
        limit: u32,
    ) -> Result<(Vec<TestCaseResult>, u64), ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_execution:read")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let is_admin = self.auth.is_admin(user_id).await?;
        if !is_admin
            && !self
                .execution_repo
                .user_can_access(execution_id, user_id)
                .await
                .map_err(ServiceError::from)?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        self.result_repo
            .list_by_execution(execution_id, page, limit)
            .await
            .map_err(ServiceError::from)
    }

    /// Get tester counts for the given execution IDs.
    pub async fn get_tester_counts(
        &self,
        execution_ids: &[i64],
    ) -> Result<HashMap<i64, u64>, ServiceError> {
        self.execution_repo
            .get_tester_counts(execution_ids)
            .await
            .map_err(ServiceError::from)
    }

    /// Get detailed tester info for an execution.
    pub async fn get_tester_info(
        &self,
        execution_id: i64,
    ) -> Result<Vec<TesterInfoRow>, ServiceError> {
        self.execution_repo
            .get_tester_info(execution_id)
            .await
            .map_err(ServiceError::from)
    }
}
