//! `TestPlanRepository` trait — persistence contract for the [`TestPlan`] entity.
//!
//! Defines data-access operations for test plans, including multi-project
//! membership filtering. Implementations live in
//! `infrastructure::postgres::repositories`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::application::repositories::RepositoryResult;
use crate::domain::entities::test_plan::{TestPlan, TestPlanSelectItem};

/// A test plan row as returned by list queries.
#[derive(Clone, Debug, serde::Serialize)]
pub struct TestPlanListItem {
    pub id: i64,
    pub name: String,
    pub version: String,
    pub types: serde_json::Value,
    pub status: String,
    pub project_ids: Vec<i64>,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Filters for the list query.
#[derive(Clone, Debug, Default)]
pub struct TestPlanFilters {
    pub status: Option<String>,
    pub plan_type: Option<String>,
    pub project_id: Option<i64>,
    pub search: Option<String>,
    pub sort: String, // defaults to "-updated_at"
}

/// Repository interface for [`TestPlan`] persistence.
#[async_trait]
pub trait TestPlanRepository: Send + Sync {
    /// Look up a test plan by primary key.
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<TestPlan>>;

    /// Find a non-deleted test plan by name (case-insensitive).
    async fn find_by_name(&self, name: &str) -> RepositoryResult<Option<TestPlan>>;

    /// Return paginated list of test plans accessible by a user.
    async fn find_accessible(
        &self,
        user_id: i64,
        is_admin: bool,
        page: u32,
        limit: u32,
        filters: &TestPlanFilters,
    ) -> RepositoryResult<(Vec<TestPlanListItem>, u64)>;

    /// Return all accessible test plans for dropdown (id + name only).
    async fn find_selectable(
        &self,
        user_id: i64,
        is_admin: bool,
    ) -> RepositoryResult<Vec<TestPlanSelectItem>>;

    /// Get the project IDs linked to a test plan.
    async fn get_project_ids(&self, plan_id: i64) -> RepositoryResult<Vec<i64>>;

    /// Check if any of the given project IDs are valid and non-deleted.
    async fn validate_project_ids(&self, project_ids: &[i64]) -> RepositoryResult<Vec<i64>>;

    /// Create a test plan with project associations in a transaction.
    async fn create(&self, plan: &TestPlan, project_ids: &[i64]) -> RepositoryResult<TestPlan>;

    /// Update a test plan and optionally replace project associations.
    async fn update(
        &self,
        plan: &TestPlan,
        project_ids: Option<&[i64]>,
    ) -> RepositoryResult<TestPlan>;

    /// Soft-delete a test plan.
    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()>;
}
