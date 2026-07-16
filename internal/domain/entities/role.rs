//! `Role` entity — a named set of permissions that can be assigned to groups
//! (or directly to users).
//!
//! Maps to the `roles` table. Roles are the unit of authorization —
//! permissions are granted to roles, roles are assigned to groups/users.

use chrono::{DateTime, Utc};

/// A named set of permissions.
///
/// The built-in `"System Admin"` role is protected by the domain — it cannot
/// be deleted or modified through the application API.
#[derive(Debug, Clone)]
pub struct Role {
    /// Auto-increment primary key (assigned by the database).
    pub id: i64,

    /// Unique human-readable name (e.g. `"System Admin"`, `"Tester"`).
    pub name: String,

    /// Role status (`"ACTIVE"`, `"INACTIVE"`, etc.). Defaults to `"ACTIVE"`.
    pub status: String,

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
    /// when persisting to the database.
    ///
    /// # Arguments
    ///
    /// * `name` — Unique role name.
    /// * `created_by` — ID of the creating user, or `None` for seeded roles.
    pub fn create(name: String, created_by: Option<i64>) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            name,
            status: "ACTIVE".to_string(),
            created_by,
            created_at: now,
            updated_by: created_by,
            updated_at: now,
            deleted_by: None,
            deleted_at: None,
        }
    }

    /// Returns `true` for the built-in `"System Admin"` role.
    ///
    /// System Admin is a protected role — it cannot be deleted or renamed
    /// through the application.
    pub fn is_protected(&self) -> bool {
        self.name == "System Admin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_role_with_creator() {
        let role = Role::create("Tester".into(), Some(1));
        assert_eq!(role.name, "Tester");
        assert_eq!(role.created_by, Some(1));
        assert_eq!(role.updated_by, Some(1));
    }

    #[test]
    fn create_role_without_creator() {
        let role = Role::create("Seeded Role".into(), None);
        assert!(role.created_by.is_none());
        assert!(role.updated_by.is_none());
    }

    #[test]
    fn system_admin_is_protected() {
        let role = Role::create("System Admin".into(), None);
        assert!(role.is_protected());
    }

    #[test]
    fn other_roles_are_not_protected() {
        let role = Role::create("Tester".into(), None);
        assert!(!role.is_protected());
    }

    #[test]
    fn case_sensitive_protected_check() {
        // The check is exact — casing matters.
        let role = Role::create("system admin".into(), None);
        assert!(!role.is_protected());
    }
}
