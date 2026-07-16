//! `AdminBypassRepository` trait — checks for system administrator status.
//!
//! The "System Admin" role grants unrestricted access to all features. This
//! repository provides a query to determine whether a user holds that role,
//! either directly or through group membership.

use async_trait::async_trait;

use crate::application::repositories::RepositoryResult;

/// Determines whether a user is a system administrator.
///
/// System administrators bypass all permission checks — they have implicit
/// access to every feature regardless of individual permission assignments.
///
/// The check includes both:
/// 1. Direct assignment of the "System Admin" role to the user.
/// 2. Indirect assignment through membership in a group that holds the
///    "System Admin" role.
#[async_trait]
pub trait AdminBypassRepository: Send + Sync {
    /// Returns `true` if the user has the "System Admin" role.
    ///
    /// The check covers both direct role-to-user assignments and role
    /// inheritance through group membership.
    ///
    /// Returns `Ok(false)` if the user does not exist, has no roles, or
    /// is not a system administrator.
    async fn is_system_admin(&self, user_id: i64) -> RepositoryResult<bool>;
}
