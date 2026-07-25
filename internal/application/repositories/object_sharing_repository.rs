//! `ObjectSharingRepository` trait — persistence for per-object sharing records.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::application::repositories::RepositoryResult;
use crate::domain::entities::object_sharing::ObjectSharing;

/// A sharing record with resolved username.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ObjectSharingRow {
    pub id: i64,
    pub user_id: i64,
    pub username: String,
    pub fullname: String,
    pub resource_type: String,
    pub resource_id: i64,
    pub role: String,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait ObjectSharingRepository: Send + Sync {
    /// List all active sharing records for a resource.
    async fn list_by_resource(
        &self,
        resource_type: &str,
        resource_id: i64,
    ) -> RepositoryResult<Vec<ObjectSharingRow>>;

    /// Get sharing record for a specific user on a resource.
    async fn find_by_user_and_resource(
        &self,
        user_id: i64,
        resource_type: &str,
        resource_id: i64,
    ) -> RepositoryResult<Option<ObjectSharing>>;

    /// Get the effective role for a user on a resource (sharing overrides
    /// project membership).
    async fn get_effective_role(
        &self,
        user_id: i64,
        resource_type: &str,
        resource_id: i64,
    ) -> RepositoryResult<Option<String>>;

    /// Create a new sharing record.
    async fn create(&self, sharing: &ObjectSharing) -> RepositoryResult<ObjectSharing>;

    /// Update the role on an existing sharing record.
    async fn update_role(&self, id: i64, role: &str) -> RepositoryResult<()>;

    /// Delete a sharing record (hard delete).
    async fn delete(&self, id: i64) -> RepositoryResult<()>;
}
