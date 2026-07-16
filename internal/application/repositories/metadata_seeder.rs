//! `MetadataSeeder` trait — seeds per-project metadata from configuration.
//!
//! When a project is created, default metadata (categories, templates,
//! priorities, plan types) is copied from a YAML configuration file into
//! the project's per-project metadata tables. This trait abstracts the
//! seeding mechanism so the application layer can invoke it without
//! knowing the config file format.

use async_trait::async_trait;

use crate::application::repositories::RepositoryResult;

/// Seeds default metadata for a newly created project.
///
/// Called within the create-project transaction so that seed failures
/// roll back the entire project creation.
#[async_trait]
pub trait MetadataSeeder: Send + Sync {
    /// Seed default metadata for the given project.
    ///
    /// Inserts records into `test_categories`, `test_case_templates`,
    /// and other per-project metadata tables based on the YAML
    /// configuration loaded at startup.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the seed operations fail.
    async fn seed(&self, project_id: i64, created_by: i64) -> RepositoryResult<()>;
}
