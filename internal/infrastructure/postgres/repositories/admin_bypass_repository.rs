//! PostgreSQL implementation of [`AdminBypassRepository`].
//!
//! Checks whether a user holds the "System Admin" role, either directly
//! or through group membership.

use async_trait::async_trait;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};

use crate::application::repositories::admin_bypass_repository::AdminBypassRepository;
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::infrastructure::db::entities::{
    group_roles, groups, roles, user_groups, user_roles, users,
};

/// PostgreSQL-backed [`AdminBypassRepository`].
pub struct SqlAdminBypassRepository {
    db: DatabaseConnection,
}

impl SqlAdminBypassRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AdminBypassRepository for SqlAdminBypassRepository {
    async fn is_system_admin(&self, user_id: i64) -> RepositoryResult<bool> {
        // Verify user exists and is not soft-deleted.
        let user_exists = users::Entity::find_by_id(user_id)
            .filter(users::Column::DeletedAt.is_null())
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            > 0;

        if !user_exists {
            return Ok(false);
        }

        // Check direct role assignment.
        let direct_role_ids: Vec<i64> = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(user_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|ur| ur.role_id)
            .collect();

        let has_direct = !direct_role_ids.is_empty()
            && roles::Entity::find()
                .filter(roles::Column::Id.is_in(direct_role_ids))
                .filter(roles::Column::Name.eq("System Admin"))
                .filter(roles::Column::DeletedAt.is_null())
                .count(&self.db)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?
                > 0;

        if has_direct {
            return Ok(true);
        }

        // Check group-inherited role assignment.
        let group_ids: Vec<i64> = user_groups::Entity::find()
            .filter(user_groups::Column::UserId.eq(user_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|ug| ug.group_id)
            .collect();

        // Exclude soft-deleted groups.
        let active_group_ids: Vec<i64> = groups::Entity::find()
            .filter(groups::Column::Id.is_in(group_ids))
            .filter(groups::Column::DeletedAt.is_null())
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|g| g.id)
            .collect();

        if active_group_ids.is_empty() {
            return Ok(false);
        }

        let group_role_ids: Vec<i64> = group_roles::Entity::find()
            .filter(group_roles::Column::GroupId.is_in(active_group_ids))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|gr| gr.role_id)
            .collect();

        if group_role_ids.is_empty() {
            return Ok(false);
        }

        let has_group_inherited = roles::Entity::find()
            .filter(roles::Column::Id.is_in(group_role_ids))
            .filter(roles::Column::Name.eq("System Admin"))
            .filter(roles::Column::DeletedAt.is_null())
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            > 0;

        Ok(has_group_inherited)
    }
}

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: AdminBypassRepository>() {}
    check::<SqlAdminBypassRepository>();
}
