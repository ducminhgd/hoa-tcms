//! `PermissionResolver` trait — resolves effective permissions for a user.
//!
//! Permissions are granted to roles, and roles are assigned to users either
//! directly or through group membership. This trait encapsulates the logic
//! of aggregating all permissions a user inherits through both paths.

use std::collections::HashSet;

use async_trait::async_trait;

use crate::application::services::errors::ServiceError;

/// Resolves the full set of permissions available to a user.
///
/// The resolver considers two sources of role assignments:
/// 1. **Direct** — roles assigned directly to the user.
/// 2. **Group-inherited** — roles assigned to any group the user belongs to.
///
/// The union of both sources forms the user's effective permissions.
#[async_trait]
pub trait PermissionResolver: Send + Sync {
    /// Resolve all permission IDs that the user has access to.
    ///
    /// Combines permissions from direct role-to-user assignments and
    /// permissions inherited through group membership.
    ///
    /// Returns an empty `HashSet` if the user has no permissions (or does
    /// not exist). The returned set is deduplicated — a permission granted
    /// through both direct and indirect paths appears only once.
    ///
    /// # Errors
    ///
    /// Returns `Err(ServiceError::Database)` if the resolution could not
    /// be completed (e.g. database error).
    async fn resolve_effective_permissions(
        &self,
        user_id: i64,
    ) -> Result<HashSet<i64>, ServiceError>;

    /// Check whether a user has a specific permission identified by its
    /// string code.
    ///
    /// This is a convenience method that resolves the permission code to
    /// its numeric identifier internally and checks membership in the
    /// user's effective permission set.
    ///
    /// # Errors
    ///
    /// Returns `Err(ServiceError::Database)` if the resolution or check
    /// could not be completed.
    async fn has_permission_by_code(&self, user_id: i64, code: &str) -> Result<bool, ServiceError>;
}
