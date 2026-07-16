//! `SqlGroupRepository` — PostgreSQL implementation of [`GroupRepository`].
//!
//! Maps the `groups` table to the [`Group`] domain entity using SeaORM.
//!
//! **Note**: The `groups` table has the `trigger_set_updated_at` trigger, so
//! UPDATE queries do NOT set `updated_at` / `updated_by` explicitly — the
//! trigger handles that. However, we still pass `updated_by` in the ActiveModel
//! so the trigger can fall back to it when the session parameter
//! `app.current_user_id` is not set.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};

use crate::application::repositories::group_repository::GroupRepository;
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::domain::entities::group::Group;
use crate::domain::value_objects::group_status::GroupStatus;
use crate::infrastructure::db::entities::groups;

/// PostgreSQL-backed [`GroupRepository`].
pub struct SqlGroupRepository {
    db: DatabaseConnection,
}

impl SqlGroupRepository {
    /// Create a new repository bound to the given database connection.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

// ---------------------------------------------------------------------------
// SeaORM Model -> domain Entity mapping
// ---------------------------------------------------------------------------

fn model_to_entity(model: groups::Model) -> RepositoryResult<Group> {
    let status: GroupStatus = model.status.parse().map_err(|e| {
        RepositoryError::Database(format!(
            "invalid group status '{}' for group {}: {}",
            model.status, model.id, e
        ))
    })?;

    Ok(Group {
        id: model.id,
        name: model.name,
        description: model.description,
        status,
        created_by: model.created_by,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(model.created_at, Utc),
        updated_by: model.updated_by,
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(model.updated_at, Utc),
        deleted_by: model.deleted_by,
        deleted_at: model
            .deleted_at
            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
    })
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl GroupRepository for SqlGroupRepository {
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<Group>> {
        groups::Entity::find_by_id(id)
            .filter(groups::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn find_by_name(&self, name: &str) -> RepositoryResult<Option<Group>> {
        groups::Entity::find()
            .filter(Expr::cust_with_values(
                "LOWER(name) = LOWER($1)",
                [name.to_string()],
            ))
            .filter(groups::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn save(&self, group: &Group) -> RepositoryResult<Group> {
        groups::ActiveModel {
            name: Set(group.name.clone()),
            description: Set(group.description.clone()),
            status: Set(group.status.to_string()),
            created_by: Set(group.created_by),
            updated_by: Set(group.updated_by),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))
        .and_then(|m| model_to_entity(m))
    }

    async fn update(&self, group: &Group) -> RepositoryResult<Group> {
        // Read the current row to verify it exists, is not soft-deleted, and
        // preserve the soft-delete state so a concurrent soft_delete cannot
        // be accidentally reverted.
        let existing = groups::Entity::find_by_id(group.id)
            .filter(groups::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .ok_or(RepositoryError::NotFound)?;

        groups::ActiveModel {
            id: Set(group.id),
            name: Set(group.name.clone()),
            description: Set(group.description.clone()),
            status: Set(group.status.to_string()),
            updated_by: Set(group.updated_by),
            // Preserve soft-delete state from the existing row.
            deleted_at: Set(existing.deleted_at),
            deleted_by: Set(existing.deleted_by),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))
        .and_then(|m| model_to_entity(m))
    }

    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()> {
        // Use a filtered update to atomically verify the record is not already
        // soft-deleted — avoids a TOCTOU race between find and update.
        let result = groups::Entity::update_many()
            .filter(groups::Column::Id.eq(id))
            .filter(groups::Column::DeletedAt.is_null())
            .set(groups::ActiveModel {
                deleted_at: Set(Some(Utc::now().naive_utc())),
                deleted_by: Set(Some(deleted_by)),
                ..Default::default()
            })
            .exec(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        if result.rows_affected == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn find_all(
        &self,
        page: u32,
        limit: u32,
        search: Option<&str>,
        status_filter: Option<GroupStatus>,
    ) -> RepositoryResult<(Vec<Group>, u64)> {
        // Build dynamic filter conditions.
        let mut condition = Condition::all().add(groups::Column::DeletedAt.is_null());

        if let Some(s) = search {
            condition = condition.add(groups::Column::Name.contains(s));
        }
        if let Some(ref s) = status_filter {
            condition = condition.add(groups::Column::Status.eq(s.to_string()));
        }

        let paginator = groups::Entity::find()
            .filter(condition)
            .order_by_asc(groups::Column::Id)
            .paginate(&self.db, limit as u64);

        let page_zero_based = page.saturating_sub(1) as u64;

        let groups: Vec<Group> = paginator
            .fetch_page(page_zero_based)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(model_to_entity)
            .collect::<Result<Vec<_>, _>>()?;

        let count = paginator
            .num_items()
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok((groups, count))
    }
}

// ---------------------------------------------------------------------------
// Compile-time check: SqlGroupRepository satisfies GroupRepository
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: GroupRepository>() {}
    check::<SqlGroupRepository>();
}
