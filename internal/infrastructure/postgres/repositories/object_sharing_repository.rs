//! `SqlObjectSharingRepository` — PostgreSQL implementation.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::application::repositories::object_sharing_repository::{
    ObjectSharingRepository, ObjectSharingRow,
};
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::domain::entities::object_sharing::ObjectSharing;
use crate::infrastructure::db::entities::{object_sharing, users};

pub struct SqlObjectSharingRepository {
    db: DatabaseConnection,
}

impl SqlObjectSharingRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ObjectSharingRepository for SqlObjectSharingRepository {
    async fn list_by_resource(
        &self,
        resource_type: &str,
        resource_id: i64,
    ) -> RepositoryResult<Vec<ObjectSharingRow>> {
        let shares = object_sharing::Entity::find()
            .filter(object_sharing::Column::ResourceType.eq(resource_type))
            .filter(object_sharing::Column::ResourceId.eq(resource_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut rows = Vec::with_capacity(shares.len());
        for s in shares {
            let user = users::Entity::find_by_id(s.user_id)
                .one(&self.db)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?;
            let username = user
                .as_ref()
                .map_or_else(|| "unknown".into(), |u| u.username.clone());
            let fullname = user
                .as_ref()
                .map_or_else(|| "Unknown".into(), |u| u.fullname.clone());
            let resource_type = s.resource_type.clone();
            let role = s.role.clone();
            rows.push(ObjectSharingRow {
                id: s.id,
                user_id: s.user_id,
                username,
                fullname,
                resource_type,
                resource_id: s.resource_id,
                role,
                created_by: s.created_by,
                created_at: DateTime::<Utc>::from(s.created_at),
            });
        }
        Ok(rows)
    }

    async fn find_by_user_and_resource(
        &self,
        user_id: i64,
        resource_type: &str,
        resource_id: i64,
    ) -> RepositoryResult<Option<ObjectSharing>> {
        let m = object_sharing::Entity::find()
            .filter(object_sharing::Column::UserId.eq(user_id))
            .filter(object_sharing::Column::ResourceType.eq(resource_type))
            .filter(object_sharing::Column::ResourceId.eq(resource_id))
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        Ok(m.map(|m| ObjectSharing {
            id: m.id,
            user_id: m.user_id,
            resource_type: m.resource_type,
            resource_id: m.resource_id,
            role: m.role,
            created_by: m.created_by,
            created_at: DateTime::<Utc>::from(m.created_at),
            updated_by: m.updated_by,
            updated_at: DateTime::<Utc>::from(m.updated_at),
        }))
    }

    async fn get_effective_role(
        &self,
        user_id: i64,
        resource_type: &str,
        resource_id: i64,
    ) -> RepositoryResult<Option<String>> {
        self.find_by_user_and_resource(user_id, resource_type, resource_id)
            .await
            .map(|opt| opt.map(|s| s.role))
    }

    async fn create(&self, sharing: &ObjectSharing) -> RepositoryResult<ObjectSharing> {
        let inserted = object_sharing::ActiveModel {
            user_id: Set(sharing.user_id),
            resource_type: Set(sharing.resource_type.clone()),
            resource_id: Set(sharing.resource_id),
            role: Set(sharing.role.clone()),
            created_by: Set(sharing.created_by),
            created_at: Set(sharing.created_at.fixed_offset()),
            updated_by: Set(sharing.updated_by),
            updated_at: Set(sharing.updated_at.fixed_offset()),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(ObjectSharing {
            id: inserted.id,
            user_id: inserted.user_id,
            resource_type: inserted.resource_type,
            resource_id: inserted.resource_id,
            role: inserted.role,
            created_by: inserted.created_by,
            created_at: DateTime::<Utc>::from(inserted.created_at),
            updated_by: inserted.updated_by,
            updated_at: DateTime::<Utc>::from(inserted.updated_at),
        })
    }

    async fn update_role(&self, id: i64, role: &str) -> RepositoryResult<()> {
        object_sharing::Entity::update_many()
            .filter(object_sharing::Column::Id.eq(id))
            .set(object_sharing::ActiveModel {
                role: Set(role.to_string()),
                ..Default::default()
            })
            .exec(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        Ok(())
    }

    async fn delete(&self, id: i64) -> RepositoryResult<()> {
        object_sharing::Entity::delete_by_id(id)
            .exec(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        Ok(())
    }
}

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: ObjectSharingRepository>() {}
    check::<SqlObjectSharingRepository>();
}
