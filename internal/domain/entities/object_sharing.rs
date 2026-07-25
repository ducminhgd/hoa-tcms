//! ObjectSharing entity — per-object sharing with specific users.
//!
//! Overrides project membership role for a specific object. Uses hard delete
//! (no soft-delete on junction records).

use chrono::{DateTime, Utc};

/// An object sharing record — grants a specific user access to an object.
#[derive(Clone, Debug)]
pub struct ObjectSharing {
    pub id: i64,
    pub user_id: i64,
    pub resource_type: String,
    pub resource_id: i64,
    pub role: String,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_by: i64,
    pub updated_at: DateTime<Utc>,
}

impl ObjectSharing {
    pub fn create(
        user_id: i64,
        resource_type: String,
        resource_id: i64,
        role: String,
        created_by: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            user_id,
            resource_type,
            resource_id,
            role,
            created_by,
            created_at: now,
            updated_by: created_by,
            updated_at: now,
        }
    }

    /// Valid resource types for sharing.
    pub const RESOURCE_TYPES: &'static [&'static str] = &[
        "project",
        "test_plan",
        "test_case",
        "test_run",
        "test_execution",
    ];

    /// Valid sharing roles.
    pub const SHARING_ROLES: &'static [&'static str] = &["Editor", "Contributor", "Viewer"];

    pub fn validate_resource_type(rt: &str) -> bool {
        Self::RESOURCE_TYPES.contains(&rt)
    }

    pub fn validate_role(role: &str) -> bool {
        Self::SHARING_ROLES.contains(&role)
    }
}
