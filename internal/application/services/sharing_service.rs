//! `SharingService` — use cases for per-object sharing management.

use std::sync::Arc;

use crate::application::repositories::object_sharing_repository::{
    ObjectSharingRepository, ObjectSharingRow,
};
use crate::application::services::authorization::AuthorizationService;
use crate::application::services::errors::ServiceError;
use crate::domain::entities::object_sharing::ObjectSharing;

pub struct SharingService {
    sharing_repo: Box<dyn ObjectSharingRepository>,
    auth: Arc<AuthorizationService>,
}

impl SharingService {
    pub fn new(
        sharing_repo: Box<dyn ObjectSharingRepository>,
        auth: Arc<AuthorizationService>,
    ) -> Self {
        Self { sharing_repo, auth }
    }

    /// List sharing records for a resource.
    pub async fn list(
        &self,
        user_id: i64,
        resource_type: &str,
        resource_id: i64,
    ) -> Result<Vec<ObjectSharingRow>, ServiceError> {
        if !self.auth.check_permission(user_id, "share:read").await? {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }
        self.sharing_repo
            .list_by_resource(resource_type, resource_id)
            .await
            .map_err(ServiceError::from)
    }

    /// Share an object with a user.
    #[allow(clippy::too_many_arguments)]
    pub async fn share(
        &self,
        current_user_id: i64,
        user_id: i64,
        resource_type: &str,
        resource_id: i64,
        role: &str,
    ) -> Result<ObjectSharing, ServiceError> {
        if !self
            .auth
            .check_permission(current_user_id, "share:create")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }
        if !ObjectSharing::validate_resource_type(resource_type) {
            return Err(ServiceError::Validation("invalid resource type".into()));
        }
        if !ObjectSharing::validate_role(role) {
            return Err(ServiceError::Validation("invalid sharing role".into()));
        }

        // Check for existing share.
        if self
            .sharing_repo
            .find_by_user_and_resource(user_id, resource_type, resource_id)
            .await
            .map_err(ServiceError::from)?
            .is_some()
        {
            return Err(ServiceError::Conflict(
                "this user already has a sharing record for this resource".into(),
            ));
        }

        let sharing = ObjectSharing::create(
            user_id,
            resource_type.to_string(),
            resource_id,
            role.to_string(),
            current_user_id,
        );

        self.sharing_repo
            .create(&sharing)
            .await
            .map_err(ServiceError::from)
    }

    /// Update sharing role.
    pub async fn update_role(
        &self,
        current_user_id: i64,
        share_id: i64,
        role: &str,
    ) -> Result<(), ServiceError> {
        if !self
            .auth
            .check_permission(current_user_id, "share:create")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }
        if !ObjectSharing::validate_role(role) {
            return Err(ServiceError::Validation("invalid sharing role".into()));
        }
        self.sharing_repo
            .update_role(share_id, role)
            .await
            .map_err(ServiceError::from)
    }

    /// Remove sharing (hard delete).
    pub async fn remove(&self, current_user_id: i64, share_id: i64) -> Result<(), ServiceError> {
        if !self
            .auth
            .check_permission(current_user_id, "share:delete")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }
        self.sharing_repo
            .delete(share_id)
            .await
            .map_err(ServiceError::from)
    }

    /// Get effective role for a user on a resource.
    pub async fn get_effective_role(
        &self,
        user_id: i64,
        resource_type: &str,
        resource_id: i64,
    ) -> Result<Option<String>, ServiceError> {
        self.sharing_repo
            .get_effective_role(user_id, resource_type, resource_id)
            .await
            .map_err(ServiceError::from)
    }
}
