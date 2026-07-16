//! PostgreSQL implementation of [`GroupRoleRepository`].
//!
//! Manages the `group_roles` junction table that links roles to groups.

use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set, SqlErr,
};

use crate::application::repositories::group_role_repository::GroupRoleRepository;
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::infrastructure::db::entities::{group_roles, roles};

/// PostgreSQL-backed [`GroupRoleRepository`].
pub struct SqlGroupRoleRepository {
    db: DatabaseConnection,
}

impl SqlGroupRoleRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl GroupRoleRepository for SqlGroupRoleRepository {
    async fn add(&self, group_id: i64, role_id: i64) -> RepositoryResult<()> {
        group_roles::ActiveModel {
            group_id: Set(group_id),
            role_id: Set(role_id),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map(|_| ())
        .map_err(|e| match e.sql_err() {
            Some(SqlErr::ForeignKeyConstraintViolation(_)) => RepositoryError::NotFound,
            Some(SqlErr::UniqueConstraintViolation(_)) => {
                RepositoryError::Duplicate("Role is already assigned to this group".to_string())
            }
            _ => RepositoryError::Database(e.to_string()),
        })
    }

    async fn remove(&self, group_id: i64, role_id: i64) -> RepositoryResult<()> {
        group_roles::Entity::delete_many()
            .filter(group_roles::Column::GroupId.eq(group_id))
            .filter(group_roles::Column::RoleId.eq(role_id))
            .exec(&self.db)
            .await
            .map(|_| ())
            .map_err(|e| RepositoryError::Database(e.to_string()))
    }

    async fn find_by_group(&self, group_id: i64) -> RepositoryResult<Vec<(i64, String)>> {
        let role_ids: Vec<i64> = group_roles::Entity::find()
            .filter(group_roles::Column::GroupId.eq(group_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|gr| gr.role_id)
            .collect();

        if role_ids.is_empty() {
            return Ok(Vec::new());
        }

        let found = roles::Entity::find()
            .filter(roles::Column::Id.is_in(role_ids))
            .filter(roles::Column::DeletedAt.is_null())
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(found.into_iter().map(|r| (r.id, r.name)).collect())
    }

    async fn count_by_group(&self, group_id: i64) -> RepositoryResult<u64> {
        let role_ids: Vec<i64> = group_roles::Entity::find()
            .filter(group_roles::Column::GroupId.eq(group_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|gr| gr.role_id)
            .collect();

        if role_ids.is_empty() {
            return Ok(0);
        }

        let count = roles::Entity::find()
            .filter(roles::Column::Id.is_in(role_ids))
            .filter(roles::Column::DeletedAt.is_null())
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(count)
    }

    async fn validate_roles_exist(&self, role_ids: &[i64]) -> RepositoryResult<Vec<i64>> {
        let found = roles::Entity::find()
            .filter(roles::Column::Id.is_in(role_ids.to_vec()))
            .filter(roles::Column::DeletedAt.is_null())
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(found.into_iter().map(|r| r.id).collect())
    }
}

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: GroupRoleRepository>() {}
    check::<SqlGroupRoleRepository>();
}
