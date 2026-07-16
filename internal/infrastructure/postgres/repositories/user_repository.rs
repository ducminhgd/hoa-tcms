//! `SqlUserRepository` — PostgreSQL implementation of [`UserRepository`].
//!
//! Maps the `users` table to the [`User`] domain entity using SeaORM.
//!
//! **Note**: The `users` table does NOT have the `trigger_set_updated_at`
//! trigger (see migration `001_initial_schema.sql` for the chicken-and-egg
//! rationale), so all UPDATE queries set `updated_at` explicitly.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};

use crate::application::repositories::user_repository::UserRepository;
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::domain::entities::user::User;
use crate::domain::value_objects::user_status::UserStatus;
use crate::infrastructure::db::entities::users;

/// PostgreSQL-backed [`UserRepository`].
pub struct SqlUserRepository {
    db: DatabaseConnection,
}

impl SqlUserRepository {
    /// Create a new repository bound to the given database connection.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

// ---------------------------------------------------------------------------
// SeaORM Model -> domain Entity mapping
// ---------------------------------------------------------------------------

fn model_to_entity(model: users::Model) -> RepositoryResult<User> {
    let status: UserStatus = model.status.parse().map_err(|e| {
        RepositoryError::Database(format!(
            "invalid user status '{}' for user {}: {}",
            model.status, model.id, e
        ))
    })?;

    Ok(User {
        id: model.id,
        username: model.username,
        email: model.email,
        password_hash: model.password_hash,
        fullname: model.fullname,
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
impl UserRepository for SqlUserRepository {
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<User>> {
        users::Entity::find_by_id(id)
            .filter(users::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn find_by_username(&self, username: &str) -> RepositoryResult<Option<User>> {
        users::Entity::find()
            .filter(Expr::cust_with_values(
                "LOWER(username) = LOWER($1)",
                [username.to_string()],
            ))
            .filter(users::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn find_by_email(&self, email: &str) -> RepositoryResult<Option<User>> {
        users::Entity::find()
            .filter(Expr::cust_with_values(
                "LOWER(email) = LOWER($1)",
                [email.to_string()],
            ))
            .filter(users::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn create(&self, user: &User) -> RepositoryResult<User> {
        users::ActiveModel {
            username: Set(user.username.clone()),
            email: Set(user.email.clone()),
            password_hash: Set(user.password_hash.clone()),
            fullname: Set(user.fullname.clone()),
            status: Set(user.status.to_string()),
            created_by: Set(user.created_by),
            updated_by: Set(user.updated_by),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))
        .and_then(|m| model_to_entity(m))
    }

    async fn update(&self, user: &User) -> RepositoryResult<User> {
        // Read the current row to (a) verify it exists and is not soft-deleted,
        // and (b) preserve the soft-delete state so a concurrent soft_delete
        // cannot be accidentally reverted.
        let existing = users::Entity::find_by_id(user.id)
            .filter(users::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .ok_or(RepositoryError::NotFound)?;

        users::ActiveModel {
            id: Set(user.id),
            username: Set(user.username.clone()),
            email: Set(user.email.clone()),
            password_hash: Set(user.password_hash.clone()),
            fullname: Set(user.fullname.clone()),
            status: Set(user.status.to_string()),
            updated_by: Set(user.updated_by),
            updated_at: Set(Utc::now().naive_utc()),
            // Preserve soft-delete state from the existing row so a concurrent
            // soft_delete cannot be reverted.
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
        // Use a filtered update to atomically check that the record is not
        // already soft-deleted — avoids a TOCTOU race between find and update.
        let result = users::Entity::update_many()
            .filter(users::Column::Id.eq(id))
            .filter(users::Column::DeletedAt.is_null())
            .set(users::ActiveModel {
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

    async fn list_paginated(&self, page: u32, limit: u32) -> RepositoryResult<(Vec<User>, u64)> {
        let paginator = users::Entity::find()
            .filter(users::Column::DeletedAt.is_null())
            .order_by_asc(users::Column::Id)
            .paginate(&self.db, limit as u64);

        let page_zero_based = page.saturating_sub(1) as u64;

        let users: Vec<User> = paginator
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

        Ok((users, count))
    }

    async fn update_password_hash(&self, id: i64, password_hash: &str) -> RepositoryResult<()> {
        // Use a filtered update so we atomically check the user exists and is
        // not soft-deleted.
        let result = users::Entity::update_many()
            .filter(users::Column::Id.eq(id))
            .filter(users::Column::DeletedAt.is_null())
            .set(users::ActiveModel {
                password_hash: Set(password_hash.to_owned()),
                updated_at: Set(Utc::now().naive_utc()),
                ..Default::default()
            })
            .exec(&self.db)
            .await
            .map_err(|e| match &e {
                DbErr::RecordNotFound(_) => RepositoryError::NotFound,
                _ => RepositoryError::Database(e.to_string()),
            })?;

        if result.rows_affected == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Compile-time check: SqlUserRepository satisfies UserRepository
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: UserRepository>() {}
    check::<SqlUserRepository>();
}
