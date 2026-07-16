//! `PermissionRepository` trait — persistence contract for the [`Permission`]
//! entity.
//!
//! The permissions table is a **read-only reference table** seeded by
//! migrations and never modified through the application API. This repository
//! therefore exposes only read operations.

use async_trait::async_trait;

use crate::application::repositories::RepositoryResult;
use crate::domain::entities::permission::Permission;

/// Repository interface for [`Permission`] reads.
///
/// All methods return [`RepositoryResult`] and are `Send + Sync` so they
/// can be called from async handlers behind `Arc<dyn PermissionRepository>`.
#[async_trait]
pub trait PermissionRepository: Send + Sync {
    /// Return a paginated list of all permissions together with the total count.
    async fn find_all(&self, page: u32, limit: u32) -> RepositoryResult<(Vec<Permission>, u64)>;

    /// Look up a permission by its unique machine-readable code (e.g.
    /// `"user:create"`).
    ///
    /// Returns `None` when no permission exists with the given `code`.
    async fn find_by_code(&self, code: &str) -> RepositoryResult<Option<Permission>>;
}
