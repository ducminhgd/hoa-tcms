//! TestPlan entity — represents a test plan that can span multiple projects.
//!
//! A test plan groups test cases into a cohesive testing effort. Each plan
//! has a lifecycle status with validated transitions and is associated with
//! one or more projects through the `test_plan_projects` junction table.
//!
//! This is a pure domain entity with **no ORM or framework imports**.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_objects::test_plan_status::TestPlanStatus;

/// A test plan — a multi-project testing initiative.
///
/// Maps to the `test_plans` table. The `id` field is `0` until persisted.
#[derive(Clone)]
pub struct TestPlan {
    pub id: i64,
    pub name: String,
    pub version: String,
    /// Plan types stored as JSONB array of strings (e.g. `["REGRESSION", "API"]`).
    pub types: serde_json::Value,
    pub description: Option<String>,
    pub status: TestPlanStatus,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_by: i64,
    pub updated_at: DateTime<Utc>,
    pub deleted_by: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for TestPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestPlan")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("version", &self.version)
            .field("types", &self.types)
            .field("description", &self.description)
            .field("status", &self.status)
            .field("created_by", &self.created_by)
            .field("created_at", &self.created_at)
            .field("updated_by", &self.updated_by)
            .field("updated_at", &self.updated_at)
            .field("deleted_by", &self.deleted_by)
            .field("deleted_at", &self.deleted_at)
            .finish()
    }
}

impl TestPlan {
    /// Create a new test plan.
    ///
    /// The `id` is left at `0` and will be assigned by the persistence layer.
    /// Status defaults to `TO_DO`. Both `created_by` and `updated_by` are set
    /// to the same user.
    ///
    /// # Panics
    ///
    /// Panics via `debug_assert!` if `name` is empty (callers must validate).
    pub fn create(
        name: String,
        version: String,
        types: serde_json::Value,
        description: Option<String>,
        created_by: i64,
    ) -> Self {
        debug_assert!(!name.trim().is_empty(), "plan name must not be empty");

        let now = Utc::now();
        Self {
            id: 0,
            name,
            version,
            types,
            description,
            status: TestPlanStatus::ToDo,
            created_by,
            created_at: now,
            updated_by: created_by,
            updated_at: now,
            deleted_by: None,
            deleted_at: None,
        }
    }

    /// Returns `true` if the test plan has been soft-deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Transition this test plan to a new status.
    ///
    /// Returns `Err` with the current status if the transition is invalid.
    pub fn transition_status(
        &mut self,
        new_status: TestPlanStatus,
    ) -> Result<(), (TestPlanStatus, TestPlanStatus)> {
        if !self.status.can_transition_to(&new_status) {
            return Err((self.status.clone(), new_status));
        }
        self.status = new_status;
        Ok(())
    }
}

/// A compact representation for selection dropdowns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPlanSelectItem {
    pub id: i64,
    pub name: String,
}
