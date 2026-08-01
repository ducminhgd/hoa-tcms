//! `Group` entity — a named collection of users with shared role assignments.
//!
//! Maps to the `groups` table. Groups provide a convenient way to manage
//! permissions for a set of users as a single unit.

use chrono::{DateTime, Utc};

use crate::domain::value_objects::group_status::GroupStatus;

/// A named group of users.
///
/// Groups serve as the primary unit of access control — roles are assigned
/// to groups, and users inherit those roles through group membership.
#[derive(Debug, Clone)]
pub struct Group {
    /// Auto-increment primary key (assigned by the database).
    pub id: i64,

    /// Unique human-readable name.
    pub name: String,

    /// Optional free-text description.
    pub description: Option<String>,

    /// Current lifecycle status.
    pub status: GroupStatus,

    /// ID of the user who created this group.
    /// `None` for system-seeded groups.
    pub created_by: Option<i64>,

    /// Timestamp of creation.
    pub created_at: DateTime<Utc>,

    /// ID of the user who last updated this group.
    /// `None` for system-seeded groups.
    pub updated_by: Option<i64>,

    /// Timestamp of the last update.
    pub updated_at: DateTime<Utc>,

    /// ID of the user who soft-deleted this group, if any.
    pub deleted_by: Option<i64>,

    /// Timestamp of soft-deletion, if any.
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Group {
    /// Create a new active group.
    ///
    /// The `id` field is set to `0` — the repository assigns the real value
    /// when persisting to the database.
    ///
    /// # Arguments
    ///
    /// * `name` — Unique group name.
    /// * `description` — Optional description.
    /// * `created_by` — ID of the creating user, or `None` for system-seeded groups.
    pub fn create(name: String, description: Option<String>, created_by: Option<i64>) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            name,
            description,
            status: GroupStatus::Active,
            created_by,
            created_at: now,
            updated_by: created_by,
            updated_at: now,
            deleted_by: None,
            deleted_at: None,
        }
    }

    /// Returns `true` when the group status is `Active`.
    pub fn is_active(&self) -> bool {
        self.status == GroupStatus::Active
    }

    /// Returns `true` when the group has been soft-deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_active_group() {
        let group = Group::create("Testers".into(), Some("Quality team".into()), Some(1));
        assert_eq!(group.name, "Testers");
        assert_eq!(group.description.as_deref(), Some("Quality team"));
        assert_eq!(group.created_by, Some(1));
        assert_eq!(group.updated_by, Some(1));
        assert!(group.is_active());
        assert!(!group.is_deleted());
    }

    #[test]
    fn create_group_without_description() {
        let group = Group::create("Developers".into(), None, Some(2));
        assert!(group.description.is_none());
        assert!(group.is_active());
    }

    #[test]
    fn create_group_without_creator() {
        let group = Group::create("Seeded Group".into(), None, None);
        assert!(group.created_by.is_none());
        assert!(group.updated_by.is_none());
    }

    #[test]
    fn is_deleted_returns_false_for_new_group() {
        let group = Group::create("Testers".into(), None, Some(1));
        assert!(!group.is_deleted());
    }

    #[test]
    fn deleted_group_reports_deleted() {
        let mut group = Group::create("Testers".into(), None, Some(1));
        group.deleted_at = Some(Utc::now());
        group.deleted_by = Some(1);
        assert!(group.is_deleted());
        // Note: soft-delete is independent of status — a deleted group
        // may still be Active. Only is_deleted() reflects soft-deletion.
        assert!(group.is_active());
    }
}
