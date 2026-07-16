//! `Permission` entity — a single action that a user may or may not be
//! allowed to perform.
//!
//! Maps to the `permissions` table. This is a **read-only reference table**
//! seeded by migrations — never modified through the application API.

use chrono::{DateTime, Utc};

/// A granular permission representing a single action on a resource.
///
/// Permission codes follow the format `{resource}:{action}` (e.g.
/// `"user:create"`, `"project:read_list"`). Permissions are granted to
/// roles via the `role_permissions` junction table.
///
/// The permissions table is **read-only** — it is populated by the
/// `002_seed_permissions` migration and never modified through the
/// application UI.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Permission {
    /// Auto-increment primary key (assigned by the database).
    pub id: i64,

    /// Human-readable display name (e.g. `"Create User"`).
    pub name: String,

    /// Machine-readable permission code (e.g. `"user:create"`).
    pub code: String,

    /// Timestamp of creation (assigned by the database `DEFAULT NOW()`).
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_equality() {
        let p1 = Permission {
            id: 1,
            name: "Create User".into(),
            code: "user:create".into(),
            created_at: Utc::now(),
        };
        let p2 = Permission {
            id: 1,
            name: "Create User".into(),
            code: "user:create".into(),
            created_at: p1.created_at,
        };
        let p3 = Permission {
            id: 2,
            name: "Delete User".into(),
            code: "user:delete".into(),
            created_at: Utc::now(),
        };
        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn permission_hash() {
        use std::collections::HashSet;
        let p = Permission {
            id: 1,
            name: "Create User".into(),
            code: "user:create".into(),
            created_at: Utc::now(),
        };
        let mut set = HashSet::new();
        set.insert(p.clone());
        assert!(set.contains(&p));
    }
}
