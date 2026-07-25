//! `TestPlanService` — use case orchestration for test plan CRUD and
//! status transitions.
//!
//! Each method checks system permissions via [`AuthorizationService`],
//! validates multi-project membership (Owner/Editor for mutations, any role
//! for reads), then delegates to the repository layer.

use std::sync::Arc;

use crate::application::repositories::project_member_repository::ProjectMemberRepository;
use crate::application::repositories::test_plan_repository::{
    TestPlanFilters, TestPlanListItem, TestPlanRepository,
};
use crate::application::services::authorization::AuthorizationService;
use crate::application::services::errors::ServiceError;
use crate::domain::entities::test_plan::{TestPlan, TestPlanSelectItem};
use crate::domain::value_objects::test_plan_status::TestPlanStatus;

/// Orchestrates test plan management use cases.
pub struct TestPlanService {
    plan_repo: Box<dyn TestPlanRepository>,
    member_repo: Box<dyn ProjectMemberRepository>,
    auth: Arc<AuthorizationService>,
}

impl TestPlanService {
    pub fn new(
        plan_repo: Box<dyn TestPlanRepository>,
        member_repo: Box<dyn ProjectMemberRepository>,
        auth: Arc<AuthorizationService>,
    ) -> Self {
        Self {
            plan_repo,
            member_repo,
            auth,
        }
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Verify the user is Owner or Editor in at least one of the given
    /// projects. System Admin bypasses this check.
    async fn require_owner_or_editor_in_any(
        &self,
        user_id: i64,
        project_ids: &[i64],
    ) -> Result<(), ServiceError> {
        let is_admin = self.auth.is_admin(user_id).await?;
        if is_admin {
            return Ok(());
        }
        for &pid in project_ids {
            let m = self
                .member_repo
                .get_member(pid, user_id)
                .await
                .map_err(ServiceError::from)?;
            if let Some(member) = m
                && (member.role == "Owner" || member.role == "Editor")
            {
                return Ok(());
            }
        }
        Err(ServiceError::PermissionDenied("forbidden".into()))
    }

    /// Verify the user is a member (any role) of at least one project.
    async fn require_member_in_any(
        &self,
        user_id: i64,
        project_ids: &[i64],
    ) -> Result<(), ServiceError> {
        let is_admin = self.auth.is_admin(user_id).await?;
        if is_admin {
            return Ok(());
        }
        for &pid in project_ids {
            let m = self
                .member_repo
                .get_member(pid, user_id)
                .await
                .map_err(ServiceError::from)?;
            if m.is_some() {
                return Ok(());
            }
        }
        Err(ServiceError::PermissionDenied("forbidden".into()))
    }

    // ------------------------------------------------------------------
    // Create
    // ------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        user_id: i64,
        name: String,
        types: Vec<String>,
        version: String,
        description: Option<String>,
        project_ids: Vec<i64>,
    ) -> Result<TestPlan, ServiceError> {
        // Permission.
        if !self
            .auth
            .check_permission(user_id, "test_plan:create")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // Validate input.
        let name = name.trim().to_string();
        if name.is_empty() || name.len() > 255 {
            return Err(ServiceError::Validation(
                "name must be between 1 and 255 characters".into(),
            ));
        }
        if project_ids.is_empty() {
            return Err(ServiceError::Validation(
                "at least one project is required".into(),
            ));
        }

        // Dedup project IDs.
        let project_ids: Vec<i64> = {
            let mut v = project_ids;
            v.sort_unstable();
            v.dedup();
            v
        };

        // Validate projects exist.
        let valid_ids = self
            .plan_repo
            .validate_project_ids(&project_ids)
            .await
            .map_err(ServiceError::from)?;
        if valid_ids.len() != project_ids.len() {
            return Err(ServiceError::Validation(
                "one or more projects are invalid".into(),
            ));
        }

        // Must be Owner/Editor in at least one.
        self.require_owner_or_editor_in_any(user_id, &project_ids)
            .await?;

        // Check duplicate name.
        if self
            .plan_repo
            .find_by_name(&name)
            .await
            .map_err(ServiceError::from)?
            .is_some()
        {
            return Err(ServiceError::Conflict(
                "a test plan with this name already exists".into(),
            ));
        }

        let types_json =
            serde_json::to_value(&types).map_err(|e| ServiceError::Internal(e.to_string()))?;
        let plan = TestPlan::create(name, version, types_json, description, user_id);

        self.plan_repo
            .create(&plan, &project_ids)
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
        page: u32,
        limit: u32,
        status: Option<String>,
        plan_type: Option<String>,
        project_id: Option<i64>,
        search: Option<String>,
        sort: String,
    ) -> Result<(Vec<TestPlanListItem>, u64), ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_plan:read_list")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let is_admin = self.auth.is_admin(user_id).await?;
        let filters = TestPlanFilters {
            status,
            plan_type,
            project_id,
            search,
            sort,
        };

        self.plan_repo
            .find_accessible(user_id, is_admin, page, limit, &filters)
            .await
            .map_err(ServiceError::from)
    }

    // ------------------------------------------------------------------
    // Get
    // ------------------------------------------------------------------

    pub async fn get(&self, plan_id: i64, user_id: i64) -> Result<TestPlan, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_plan:read")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let plan = self
            .plan_repo
            .find_by_id(plan_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let project_ids = self
            .plan_repo
            .get_project_ids(plan_id)
            .await
            .map_err(ServiceError::from)?;

        // Visibility gate: must be member of at least one linked project.
        if self
            .require_member_in_any(user_id, &project_ids)
            .await
            .is_err()
        {
            return Err(ServiceError::NotFound);
        }

        Ok(plan)
    }

    // ------------------------------------------------------------------
    // Update
    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        &self,
        plan_id: i64,
        user_id: i64,
        name: Option<String>,
        version: Option<String>,
        types: Option<Vec<String>>,
        description: Option<String>,
        project_ids: Option<Vec<i64>>,
    ) -> Result<TestPlan, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_plan:update")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let mut plan = self
            .plan_repo
            .find_by_id(plan_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let current_project_ids = self
            .plan_repo
            .get_project_ids(plan_id)
            .await
            .map_err(ServiceError::from)?;

        // Must be Owner/Editor in at least one CURRENT linked project.
        self.require_owner_or_editor_in_any(user_id, &current_project_ids)
            .await?;

        // If changing projects, must also be Owner/Editor in at least one NEW.
        if let Some(ref new_ids) = project_ids {
            if new_ids.is_empty() {
                return Err(ServiceError::Validation(
                    "at least one project is required".into(),
                ));
            }
            self.require_owner_or_editor_in_any(user_id, new_ids)
                .await?;
        }

        // Apply field updates.
        if let Some(new_name) = name {
            let trimmed = new_name.trim().to_string();
            if trimmed.is_empty() || trimmed.len() > 255 {
                return Err(ServiceError::Validation(
                    "name must be between 1 and 255 characters".into(),
                ));
            }
            if trimmed.to_lowercase() != plan.name.to_lowercase()
                && self
                    .plan_repo
                    .find_by_name(&trimmed)
                    .await
                    .map_err(ServiceError::from)?
                    .is_some()
            {
                return Err(ServiceError::Conflict(
                    "a test plan with this name already exists".into(),
                ));
            }
            plan.name = trimmed;
        }

        if let Some(v) = version {
            plan.version = v;
        }

        if let Some(t) = types {
            plan.types =
                serde_json::to_value(&t).map_err(|e| ServiceError::Internal(e.to_string()))?;
        }

        if description.is_some() {
            plan.description = description;
        }

        plan.updated_by = user_id;

        self.plan_repo
            .update(&plan, project_ids.as_deref())
            .await
            .map_err(ServiceError::from)
    }

    // ------------------------------------------------------------------
    // Delete
    // ------------------------------------------------------------------

    pub async fn delete(&self, plan_id: i64, user_id: i64) -> Result<(), ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_plan:delete")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let _plan = self
            .plan_repo
            .find_by_id(plan_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let project_ids = self
            .plan_repo
            .get_project_ids(plan_id)
            .await
            .map_err(ServiceError::from)?;

        // Visibility + ownership.
        self.require_owner_or_editor_in_any(user_id, &project_ids)
            .await?;

        self.plan_repo
            .soft_delete(plan_id, user_id)
            .await
            .map_err(ServiceError::from)
    }

    // ------------------------------------------------------------------
    // Select (dropdown)
    // ------------------------------------------------------------------

    pub async fn select(&self, user_id: i64) -> Result<Vec<TestPlanSelectItem>, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_plan:select")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let is_admin = self.auth.is_admin(user_id).await?;
        self.plan_repo
            .find_selectable(user_id, is_admin)
            .await
            .map_err(ServiceError::from)
    }

    // ------------------------------------------------------------------
    // Status transition
    // ------------------------------------------------------------------

    pub async fn transition_status(
        &self,
        plan_id: i64,
        user_id: i64,
        new_status_str: &str,
    ) -> Result<TestPlan, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "test_plan:update")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let mut plan = self
            .plan_repo
            .find_by_id(plan_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let project_ids = self
            .plan_repo
            .get_project_ids(plan_id)
            .await
            .map_err(ServiceError::from)?;
        self.require_owner_or_editor_in_any(user_id, &project_ids)
            .await?;

        let new_status = TestPlanStatus::parse(new_status_str).ok_or_else(|| {
            ServiceError::Validation(format!("invalid status: {}", new_status_str))
        })?;

        plan.transition_status(new_status).map_err(|(from, to)| {
            ServiceError::Validation(format!(
                "cannot transition from {} to {}",
                from.as_str(),
                to.as_str(),
            ))
        })?;

        plan.updated_by = user_id;

        self.plan_repo
            .update(&plan, None)
            .await
            .map_err(ServiceError::from)
    }
}
