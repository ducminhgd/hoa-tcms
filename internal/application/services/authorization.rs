//! `AuthorizationService` — high-level permission check for use cases.
//!
//! This is a concrete service (not a trait) that combines the permission
//! resolver and the admin bypass check into a single `check_permission`
//! method used by application use cases.

use crate::application::repositories::admin_bypass_repository::AdminBypassRepository;
use crate::application::services::errors::ServiceError;
use crate::application::services::permission_resolver::PermissionResolver;

/// Unified permission check combining admin bypass with resolved permissions.
///
/// Use cases call `check_permission` instead of interacting with the resolver
/// or bypass repository directly. This ensures every permission check
/// consistently honours the "System Admin" role without duplicating logic.
pub struct AuthorizationService {
    /// Resolves effective permissions for a user.
    permission_resolver: Box<dyn PermissionResolver>,

    /// Checks whether a user is a system administrator.
    admin_bypass_repo: Box<dyn AdminBypassRepository>,
}

impl AuthorizationService {
    /// Create a new `AuthorizationService`.
    pub fn new(
        permission_resolver: Box<dyn PermissionResolver>,
        admin_bypass_repo: Box<dyn AdminBypassRepository>,
    ) -> Self {
        Self {
            permission_resolver,
            admin_bypass_repo,
        }
    }

    /// Check whether a user has a specific permission.
    ///
    /// Returns `true` if:
    /// 1. The user is a system administrator (admin bypass), **or**
    /// 2. The user has the permission identified by `code` (either directly
    ///    or through group membership).
    ///
    /// Returns `false` if the user does not exist, has no matching
    /// permission, and is not a system administrator.
    ///
    /// # Errors
    ///
    /// Returns `Err(ServiceError::Database)` if the underlying resolver or
    /// repository encounters a failure (e.g. database error).
    /// Returns `Err(ServiceError::PermissionDenied)` if the caller lacks
    /// the necessary privilege.
    pub async fn check_permission(&self, user_id: i64, code: &str) -> Result<bool, ServiceError> {
        // System administrators bypass all permission checks.
        if self
            .admin_bypass_repo
            .is_system_admin(user_id)
            .await
            .map_err(ServiceError::from)?
        {
            return Ok(true);
        }

        // Check the specific permission.
        self.permission_resolver
            .has_permission_by_code(user_id, code)
            .await
    }
}
