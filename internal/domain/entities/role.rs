//! `Role` entity — a named set of permissions that can be assigned to groups
//! (or directly to users).
//!
//! Maps to the `roles` table. Roles are the unit of authorization —
//! permissions are granted to roles, roles are assigned to groups/users.

use chrono::{DateTime, Utc};

use crate::domain::value_objects::role_status::RoleStatus;

/// A named set of permissions.
///
/// The built-in `"System Admin"` role is protected by the `is_system` flag —
/// it cannot be deleted or modified through the application API.
#[derive(Debug, Clone)]
pub struct Role {
    /// Auto-increment primary key (assigned by the database).
    pub id: i64,

    /// Unique human-readable name (e.g. `"System Admin"`, `"Tester"`).
    pub name: String,

    /// Role lifecycle status. Defaults to `Active`.
    pub status: RoleStatus,

    /// Whether this is a system-protected role. System roles cannot be
    /// deleted or have their permissions modified through the application API.
    /// Set via the `is_system` column in the database.
    pub is_system: bool,

    /// Human-readable summary of the role's purpose.
    pub description: Option<String>,

    /// ID of the user who created this role. `None` for pre-seeded roles.
    pub created_by: Option<i64>,

    /// Timestamp of creation.
    pub created_at: DateTime<Utc>,

    /// ID of the user who last updated this role, if any.
    pub updated_by: Option<i64>,

    /// Timestamp of the last update.
    pub updated_at: DateTime<Utc>,

    /// ID of the user who soft-deleted this role, if any.
    pub deleted_by: Option<i64>,

    /// Timestamp of soft-deletion, if any.
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Role {
    /// Create a new role.
    ///
    /// The `id` field is set to `0` — the repository assigns the real value
    /// when persisting to the database. The `is_system` flag is `false` by
    /// default — only seeded roles set it to `true`.
    ///
    /// # Arguments
    ///
    /// * `name` — Unique role name.
    /// * `description` — Optional human-readable summary.
    /// * `created_by` — ID of the creating user, or `None` for seeded roles.
    pub fn create(name: String, description: Option<String>, created_by: Option<i64>) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            name,
            description,
            status: RoleStatus::Active,
            is_system: false,
            created_by,
            created_at: now,
            updated_by: created_by,
            updated_at: now,
            deleted_by: None,
            deleted_at: None,
        }
    }

    /// Returns `true` if this is a system-protected role.
    ///
    /// System roles are identified by the `is_system` column in the database,
    /// not by name. This is immune to renames.
    pub fn is_protected(&self) -> bool {
        self.is_system
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_role_with_creator() {
        let role = Role::create("Tester".into(), Some("Test executor".into()), Some(1));
        assert_eq!(role.name, "Tester");
        assert_eq!(role.status, RoleStatus::Active);
        assert!(!role.is_system);
        assert_eq!(role.created_by, Some(1));
        assert_eq!(role.updated_by, Some(1));
    }

    #[test]
    fn create_role_without_creator() {
        let role = Role::create("Seeded Role".into(), None, None);
        assert!(role.created_by.is_none());
        assert!(role.updated_by.is_none());
    }

    #[test]
    fn system_role_is_protected() {
        let mut role = Role::create("Any Name".into(), None, None);
        role.is_system = true;
        assert!(role.is_protected());
    }

    #[test]
    fn non_system_role_is_not_protected() {
        let role = Role::create("Tester".into(), None, None);
        assert!(!role.is_protected());
    }
}
