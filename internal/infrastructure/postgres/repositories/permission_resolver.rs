//! PostgreSQL implementation of [`PermissionResolver`].
//!
//! Resolves the full set of permissions available to a user through both
//! direct role-to-user assignments and group-inherited roles.

use std::collections::HashSet;

use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, TransactionTrait,
};

use crate::application::repositories::RepositoryError;
use crate::application::services::errors::ServiceError;
use crate::application::services::permission_resolver::PermissionResolver;
use crate::infrastructure::db::entities::{
    group_roles, groups, permissions, role_permissions, roles, user_groups, user_roles, users,
};

/// PostgreSQL-backed [`PermissionResolver`].
pub struct SqlPermissionResolver {
    db: DatabaseConnection,
}

impl SqlPermissionResolver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PermissionResolver for SqlPermissionResolver {
    async fn resolve_effective_permissions(
        &self,
        user_id: i64,
    ) -> Result<HashSet<i64>, ServiceError> {
        // Wrap the entire resolution in a transaction so that concurrent
        // role/group/permission modifications cannot interleave with our
        // queries and produce an inconsistent permission set. A read-only
        // transaction handle is sufficient — no commit is needed.
        let txn = self
            .db
            .begin()
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        // Verify user exists and is not soft-deleted.
        let user_exists = users::Entity::find_by_id(user_id)
            .filter(users::Column::DeletedAt.is_null())
            .count(&txn)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            > 0;

        if !user_exists {
            return Ok(HashSet::new());
        }

        // Collect direct role IDs from user_roles.
        let direct_role_ids: Vec<i64> = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(user_id))
            .all(&txn)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|ur| ur.role_id)
            .collect();

        // Collect group-inherited role IDs from user_groups → group_roles.
        let group_ids: Vec<i64> = user_groups::Entity::find()
            .filter(user_groups::Column::UserId.eq(user_id))
            .all(&txn)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|ug| ug.group_id)
            .collect();

        let inherited_role_ids: Vec<i64> = if group_ids.is_empty() {
            Vec::new()
        } else {
            // Exclude soft-deleted groups.
            let active_group_ids: Vec<i64> = groups::Entity::find()
                .filter(groups::Column::Id.is_in(group_ids))
                .filter(groups::Column::DeletedAt.is_null())
                .all(&txn)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?
                .into_iter()
                .map(|g| g.id)
                .collect();

            if active_group_ids.is_empty() {
                Vec::new()
            } else {
                group_roles::Entity::find()
                    .filter(group_roles::Column::GroupId.is_in(active_group_ids))
                    .all(&txn)
                    .await
                    .map_err(|e| RepositoryError::Database(e.to_string()))?
                    .into_iter()
                    .map(|gr| gr.role_id)
                    .collect()
            }
        };

        // Combine all unique role IDs.
        let mut all_role_ids: Vec<i64> = direct_role_ids;
        all_role_ids.extend(inherited_role_ids);
        all_role_ids.sort_unstable();
        all_role_ids.dedup();

        if all_role_ids.is_empty() {
            return Ok(HashSet::new());
        }

        // Exclude soft-deleted roles.
        let active_role_ids: Vec<i64> = roles::Entity::find()
            .filter(roles::Column::Id.is_in(all_role_ids.clone()))
            .filter(roles::Column::DeletedAt.is_null())
            .all(&txn)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|r| r.id)
            .collect();

        if active_role_ids.is_empty() {
            return Ok(HashSet::new());
        }

        // Resolve permissions from role_permissions.
        let perm_ids = role_permissions::Entity::find()
            .filter(role_permissions::Column::RoleId.is_in(active_role_ids))
            .all(&txn)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|rp| rp.permission_id)
            .collect();

        // Read-only transaction: no commit needed; dropping the handle
        // ends the snapshot.
        Ok(perm_ids)
    }

    async fn has_permission_by_code(&self, user_id: i64, code: &str) -> Result<bool, ServiceError> {
        // Look up the permission code to get its ID first.
        let perm = permissions::Entity::find()
            .filter(permissions::Column::Code.eq(code))
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let perm_id = match perm {
            Some(p) => p.id,
            None => return Ok(false),
        };

        // Resolve effective permissions and check membership.
        let effective = self.resolve_effective_permissions(user_id).await?;
        Ok(effective.contains(&perm_id))
    }
}

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: PermissionResolver>() {}
    check::<SqlPermissionResolver>();
}
