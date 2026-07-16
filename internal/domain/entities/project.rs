//! Project entity — represents a workspace that scopes test cases, plans, runs,
//! and metadata.
//!
//! This is a pure domain entity with **no ORM or framework imports**.

use chrono::{DateTime, Utc};

use crate::domain::value_objects::project_status::ProjectStatus;

/// A project workspace.
///
/// Maps to the `projects` table. The `id` field is `0` until the entity is
/// persisted. Each project acts as a scope container for test cases, test
/// plans, test runs, and per-project metadata (categories, priorities,
/// templates, plan types).
#[derive(Clone)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub status: ProjectStatus,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_by: i64,
    pub updated_at: DateTime<Utc>,
    pub deleted_by: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Project")
            .field("id", &self.id)
            .field("name", &self.name)
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

impl Project {
    /// Create a new active project.
    ///
    /// The `id` is left at `0` and will be assigned by the persistence layer.
    /// Both `created_by` and `updated_by` are set to the same user.
    ///
    /// # Panics
    ///
    /// Panics if `name` is empty or `description` exceeds 2000 characters
    /// (callers must validate before calling).
    pub fn create(
        name: String,
        description: Option<String>,
        status: ProjectStatus,
        created_by: i64,
    ) -> Self {
        debug_assert!(!name.trim().is_empty(), "project name must not be empty");
        debug_assert!(
            description.as_ref().is_none_or(|d| d.len() <= 2000),
            "description must not exceed 2000 characters"
        );

        let now = Utc::now();
        Self {
            id: 0,
            name,
            description,
            status,
            created_by,
            created_at: now,
            updated_by: created_by,
            updated_at: now,
            deleted_by: None,
            deleted_at: None,
        }
    }

    /// Returns `true` if the project is active and not soft-deleted.
    pub fn is_active(&self) -> bool {
        self.status == ProjectStatus::Active && self.deleted_at.is_none()
    }

    /// Returns `true` if the project has been soft-deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
