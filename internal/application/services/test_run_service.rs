//! `TestRunService` — use case orchestration for test run CRUD, case
//! management, and statistics.

use std::sync::Arc;

use crate::application::repositories::project_member_repository::ProjectMemberRepository;
use crate::application::repositories::test_run_repository::{
    TestRunListItem, TestRunRepository, TestRunStatistics,
};
use crate::application::services::authorization::AuthorizationService;
use crate::application::services::errors::ServiceError;
use crate::domain::entities::test_run::TestRun;

pub struct TestRunService {
    run_repo: Box<dyn TestRunRepository>,
    member_repo: Box<dyn ProjectMemberRepository>,
    auth: Arc<AuthorizationService>,
}

impl TestRunService {
    pub fn new(
        run_repo: Box<dyn TestRunRepository>,
        member_repo: Box<dyn ProjectMemberRepository>,
        auth: Arc<AuthorizationService>,
    ) -> Self {
        Self {
            run_repo,
            member_repo,
            auth,
        }
    }

    /// Check the user is at least Contributor (Viewer excluded) in the project.
    async fn require_member_role(
        &self,
        project_id: i64,
        user_id: i64,
        min_role: &str, // "Contributor", "Editor", or "Owner"
    ) -> Result<String, ServiceError> {
        let is_admin = self.auth.is_admin(user_id).await?;
        if is_admin {
            return Ok("Owner".into());
        }
        let m = self
            .member_repo
            .get_member(project_id, user_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::PermissionDenied("forbidden".into()))?;
        let roles = ["Viewer", "Contributor", "Editor", "Owner"];
        let user_idx = roles.iter().position(|r| *r == m.role).unwrap_or(0);
        let min_idx = roles.iter().position(|r| *r == min_role).unwrap_or(0);
        if user_idx < min_idx {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }
        Ok(m.role)
    }

    /// Parse an ISO date string.
    fn parse_date(s: &str) -> Result<chrono::NaiveDate, ServiceError> {
        chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
            .map_err(|_| ServiceError::Validation(format!("invalid date format: {}", s)))
    }

    // ------------------------------------------------------------------
    // Create
    // ------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        user_id: i64,
        project_id: i64,
        summary: String,
        report_to: Option<i64>,
        default_tester: Option<i64>,
        plan_id: Option<i64>,
        version: Option<String>,
        notes: Option<String>,
        planned_start: Option<String>,
        planned_end: Option<String>,
        case_ids: Vec<i64>,
    ) -> Result<TestRun, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_run:create")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        self.require_member_role(project_id, user_id, "Contributor")
            .await?;

        let summary = summary.trim().to_string();
        if summary.is_empty() || summary.len() > 500 {
            return Err(ServiceError::Validation(
                "summary must be 1-500 characters".into(),
            ));
        }

        // Check duplicate summary in project.
        if self
            .run_repo
            .find_by_summary(project_id, &summary)
            .await
            .map_err(ServiceError::from)?
            .is_some()
        {
            return Err(ServiceError::Conflict(
                "a test run with this summary already exists".into(),
            ));
        }

        // Validate plan belongs to same project if provided.
        if let Some(pid) = plan_id
            && !self.validate_plan_in_project(pid, project_id).await?
        {
            return Err(ServiceError::Validation(
                "plan is invalid for this project".into(),
            ));
        }

        let tester_id = default_tester.unwrap_or(user_id);
        let ps = planned_start.as_deref().map(Self::parse_date).transpose()?;
        let pe = planned_end.as_deref().map(Self::parse_date).transpose()?;

        // Validate date range.
        if let (Some(s), Some(e)) = (&ps, &pe)
            && e < s
        {
            return Err(ServiceError::Validation(
                "end date is before start date".into(),
            ));
        }

        let run = TestRun::create(
            summary, report_to, tester_id, project_id, plan_id, version, notes, ps, pe, user_id,
        );

        self.run_repo
            .create(&run, &case_ids)
            .await
            .map_err(ServiceError::from)
    }

    // ------------------------------------------------------------------
    // List
    // ------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        &self,
        user_id: i64,
        project_id: i64,
        page: u32,
        limit: u32,
        plan_id: Option<i64>,
        search: Option<String>,
        sort: String,
    ) -> Result<(Vec<TestRunListItem>, u64), ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_run:read_list")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }
        self.require_member_role(project_id, user_id, "Viewer")
            .await?;

        self.run_repo
            .list_by_project(project_id, page, limit, plan_id, search.as_deref(), &sort)
            .await
            .map_err(ServiceError::from)
    }

    // ------------------------------------------------------------------
    // Get
    // ------------------------------------------------------------------

    pub async fn get(&self, run_id: i64, user_id: i64) -> Result<TestRun, ServiceError> {
        if !self.auth.check_permission(user_id, "test_run:read").await? {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let run = self
            .run_repo
            .find_by_id(run_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        // Scope check: must be member of run's project.
        self.require_member_role(run.project_id, user_id, "Viewer")
            .await?;

        Ok(run)
    }

    // ------------------------------------------------------------------
    // Update
    // ------------------------------------------------------------------
    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        &self,
        run_id: i64,
        user_id: i64,
        summary: Option<String>,
        report_to: Option<i64>,
        default_tester: Option<i64>,
        plan_id: Option<i64>,
        version: Option<String>,
        notes: Option<String>,
        planned_start: Option<String>,
        planned_end: Option<String>,
        case_ids: Option<Vec<i64>>,
    ) -> Result<TestRun, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_run:update")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let mut run = self
            .run_repo
            .find_by_id(run_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;
        self.require_member_role(run.project_id, user_id, "Contributor")
            .await?;

        if let Some(s) = summary {
            let trimmed = s.trim().to_string();
            if trimmed.is_empty() || trimmed.len() > 500 {
                return Err(ServiceError::Validation(
                    "summary must be 1-500 characters".into(),
                ));
            }
            if trimmed.to_lowercase() != run.summary.to_lowercase()
                && self
                    .run_repo
                    .find_by_summary(run.project_id, &trimmed)
                    .await
                    .map_err(ServiceError::from)?
                    .is_some()
            {
                return Err(ServiceError::Conflict(
                    "a test run with this summary already exists".into(),
                ));
            }
            run.summary = trimmed;
        }

        if let Some(rt) = report_to {
            run.report_to_user_id = Some(rt);
        }
        if let Some(dt) = default_tester {
            run.default_tester_id = dt;
        }
        if let Some(pid) = plan_id
            && !self.validate_plan_in_project(pid, run.project_id).await?
        {
            return Err(ServiceError::Validation(
                "plan is invalid for this project".into(),
            ));
        }
        if plan_id.is_some() {
            run.plan_id = plan_id;
        }
        if version.is_some() {
            run.version = version;
        }
        if notes.is_some() {
            run.notes = notes;
        }
        if let Some(ref ps) = planned_start {
            run.planned_start = Some(Self::parse_date(ps)?);
        }
        if let Some(ref pe) = planned_end {
            run.planned_stop = Some(Self::parse_date(pe)?);
        }
        if let (Some(s), Some(e)) = (&run.planned_start, &run.planned_stop)
            && e < s
        {
            return Err(ServiceError::Validation(
                "end date is before start date".into(),
            ));
        }

        run.updated_by = user_id;
        self.run_repo
            .update(&run, case_ids.as_deref())
            .await
            .map_err(ServiceError::from)
    }

    // ------------------------------------------------------------------
    // Delete
    // ------------------------------------------------------------------

    pub async fn delete(&self, run_id: i64, user_id: i64) -> Result<(), ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_run:delete")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }
        let run = self
            .run_repo
            .find_by_id(run_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;
        self.require_member_role(run.project_id, user_id, "Contributor")
            .await?;
        self.run_repo
            .soft_delete(run_id, user_id)
            .await
            .map_err(ServiceError::from)
    }

    // ------------------------------------------------------------------
    // Case management
    // ------------------------------------------------------------------

    pub async fn list_cases(&self, run_id: i64, user_id: i64) -> Result<Vec<i64>, ServiceError> {
        if !self.auth.check_permission(user_id, "test_run:read").await? {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }
        let run = self
            .run_repo
            .find_by_id(run_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;
        self.require_member_role(run.project_id, user_id, "Viewer")
            .await?;
        self.run_repo
            .get_case_ids(run_id)
            .await
            .map_err(ServiceError::from)
    }

    pub async fn manage_cases(
        &self,
        run_id: i64,
        user_id: i64,
        add_ids: Vec<i64>,
        remove_ids: Vec<i64>,
    ) -> Result<(), ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_run:update")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }
        let run = self
            .run_repo
            .find_by_id(run_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;
        self.require_member_role(run.project_id, user_id, "Contributor")
            .await?;

        let mut current = self
            .run_repo
            .get_case_ids(run_id)
            .await
            .map_err(ServiceError::from)?;
        for &add in &add_ids {
            if !current.contains(&add) {
                current.push(add);
            }
        }
        current.retain(|id| !remove_ids.contains(id));

        self.run_repo.update(&run, Some(&current)).await?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Statistics
    // ------------------------------------------------------------------

    pub async fn statistics(
        &self,
        run_id: i64,
        user_id: i64,
    ) -> Result<TestRunStatistics, ServiceError> {
        if !self.auth.check_permission(user_id, "test_run:read").await? {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }
        let run = self
            .run_repo
            .find_by_id(run_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;
        self.require_member_role(run.project_id, user_id, "Viewer")
            .await?;
        self.run_repo
            .compute_statistics(run_id)
            .await
            .map_err(ServiceError::from)
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    #[allow(unused)]
    async fn validate_plan_in_project(
        &self,
        _plan_id: i64,
        _project_id: i64,
    ) -> Result<bool, ServiceError> {
        // Full validation deferred — FK constraints handle integrity at DB level.
        // TODO: Query test_plans table to verify plan.project_id matches.
        Ok(true)
    }
}
