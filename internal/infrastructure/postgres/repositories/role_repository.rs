//! `SqlRoleRepository` — PostgreSQL implementation of [`RoleRepository`].
//!
//! Maps the `roles` table to the [`Role`] domain entity using SeaORM, and the
//! `role_permissions` junction table for permission assignment.
//!
//! **Note**: The `roles` table has the `trigger_set_updated_at` trigger, so
//! UPDATE queries do NOT set `updated_at` explicitly — the trigger handles
//! it. We still pass `updated_by` in the ActiveModel so the trigger can fall
//! back to it when the session parameter is not set.

use async_trait::async_trait;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};

use crate::application::repositories::role_repository::RoleRepository;
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::domain::entities::permission::Permission;
use crate::domain::entities::role::Role;
use crate::domain::value_objects::role_status::RoleStatus;
use crate::infrastructure::db::entities::{permissions, role_permissions, roles};

/// PostgreSQL-backed [`RoleRepository`].
pub struct SqlRoleRepository {
    db: DatabaseConnection,
}

impl SqlRoleRepository {
    /// Create a new repository bound to the given database connection.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

// ---------------------------------------------------------------------------
// SeaORM Model -> domain Entity mapping
// ---------------------------------------------------------------------------

fn model_to_role(model: roles::Model) -> RepositoryResult<Role> {
    let status: RoleStatus = model.status.parse().map_err(|e| {
        RepositoryError::Database(format!(
            "invalid role status '{}' for role {}: {}",
            model.status, model.id, e
        ))
    })?;

    Ok(Role {
        id: model.id,
        name: model.name,
        status,
        is_system: model.is_system,
        description: model.description,
        created_by: model.created_by,
        created_at: model.created_at,
        updated_by: model.updated_by,
        updated_at: model.updated_at,
        deleted_by: model.deleted_by,
        deleted_at: model.deleted_at,
    })
}

fn permission_model_to_entity(model: permissions::Model) -> Permission {
    Permission {
        id: model.id,
        name: model.name,
        code: model.code,
        created_at: model.created_at,
    }
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl RoleRepository for SqlRoleRepository {
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<Role>> {
        roles::Entity::find_by_id(id)
            .filter(roles::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_role)
            .transpose()
    }

    async fn find_by_name(&self, name: &str) -> RepositoryResult<Option<Role>> {
        roles::Entity::find()
            .filter(Expr::cust_with_values(
                "LOWER(name) = LOWER($1)",
                [name.to_string()],
            ))
            .filter(roles::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_role)
            .transpose()
    }

    async fn save(&self, role: &Role) -> RepositoryResult<Role> {
        roles::ActiveModel {
            name: Set(role.name.clone()),
            status: Set(role.status.to_string()),
            is_system: Set(role.is_system),
            description: Set(role.description.clone()),
            created_by: Set(role.created_by),
            updated_by: Set(role.updated_by),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))
        .and_then(model_to_role)
    }

    async fn update(&self, role: &Role) -> RepositoryResult<Role> {
        // Read the current row to (a) verify it exists and is not soft-deleted,
        // and (b) preserve the soft-delete state so a concurrent soft_delete
        // cannot be accidentally reverted.
        let existing = roles::Entity::find_by_id(role.id)
            .filter(roles::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .ok_or(RepositoryError::NotFound)?;

        roles::ActiveModel {
            id: Set(role.id),
            name: Set(role.name.clone()),
            status: Set(role.status.to_string()),
            updated_by: Set(role.updated_by),
            // Preserve soft-delete state and is_system flag from the existing row.
            deleted_at: Set(existing.deleted_at),
            deleted_by: Set(existing.deleted_by),
            is_system: Set(existing.is_system),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))
        .and_then(model_to_role)
    }

    async fn find_all(&self, page: u32, limit: u32) -> RepositoryResult<(Vec<Role>, u64)> {
        let paginator = roles::Entity::find()
            .filter(roles::Column::DeletedAt.is_null())
            .order_by_asc(roles::Column::Id)
            .paginate(&self.db, limit as u64);

        let page_zero_based = page.saturating_sub(1) as u64;

        let roles: Vec<Role> = paginator
            .fetch_page(page_zero_based)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(model_to_role)
            .collect::<Result<Vec<_>, _>>()?;

        let count = paginator
            .num_items()
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok((roles, count))
    }

    async fn set_permissions(&self, role_id: i64, permission_ids: &[i64]) -> RepositoryResult<()> {
        // Verify the role exists, is not soft-deleted, and is not protected.
        let role = self
            .find_by_id(role_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;

        if role.is_protected() {
            return Err(RepositoryError::Forbidden(
                "cannot modify permissions of the System Admin role".to_string(),
            ));
        }

        // Run DELETE + INSERT inside a transaction so the operation is atomic.
        let txn = self
            .db
            .begin()
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        if let Err(e) = role_permissions::Entity::delete_many()
            .filter(role_permissions::Column::RoleId.eq(role_id))
            .exec(&txn)
            .await
        {
            let _ = txn.rollback().await;
            return Err(RepositoryError::Database(e.to_string()));
        }

        if !permission_ids.is_empty() {
            let models: Vec<role_permissions::ActiveModel> = permission_ids
                .iter()
                .map(|perm_id| role_permissions::ActiveModel {
                    role_id: Set(role_id),
                    permission_id: Set(*perm_id),
                    ..Default::default()
                })
                .collect();

            if let Err(e) = role_permissions::Entity::insert_many(models)
                .exec(&txn)
                .await
            {
                let _ = txn.rollback().await;
                return Err(RepositoryError::Database(e.to_string()));
            }
        }

        txn.commit()
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_permissions(&self, role_id: i64) -> RepositoryResult<Vec<Permission>> {
        // Fetch the permission_ids from the junction table.
        let perm_ids: Vec<i64> = role_permissions::Entity::find()
            .filter(role_permissions::Column::RoleId.eq(role_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|jp| jp.permission_id)
            .collect();

        if perm_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Resolve the actual permission rows.
        let models = permissions::Entity::find()
            .filter(permissions::Column::Id.is_in(perm_ids))
            .order_by_asc(permissions::Column::Code)
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(models.into_iter().map(permission_model_to_entity).collect())
    }
}

// ---------------------------------------------------------------------------
// Compile-time check: SqlRoleRepository satisfies RoleRepository
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: RoleRepository>() {}
    check::<SqlRoleRepository>();
}
