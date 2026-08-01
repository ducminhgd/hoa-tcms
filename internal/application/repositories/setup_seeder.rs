//! `SetupSeeder` trait — seeds global reference data at application startup.
//!
//! Unlike [`MetadataSeeder`] which seeds per-project metadata, this trait
//! handles **global** reference data: permissions, system groups, built-in
//! roles, and role-permission mappings.
//!
//! The seeder runs once at server startup, BEFORE any HTTP requests are
//! accepted. It is idempotent — safe to call repeatedly.

use async_trait::async_trait;

use crate::application::repositories::RepositoryResult;

/// Seeds global reference data at application startup.
///
/// The seeder is called synchronously during server startup. A failure
/// logs an error but does not prevent the server from starting — missing
/// reference data degrades authorization (all permission checks fail)
/// rather than completely halting the system.
#[async_trait]
pub trait SetupSeeder: Send + Sync {
    /// Seed all global reference data.
    ///
    /// Inserts any missing permissions, groups, roles, and role-permission
    /// associations. All operations use idempotent upserts so the seeder
    /// can be re-run safely (e.g. after adding new permissions to the YAML).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if any seed operation fails.
    async fn seed(&self) -> RepositoryResult<()>;
}
