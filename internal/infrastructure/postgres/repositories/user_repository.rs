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

fn model_to_entity(model: users::Model) -> User {
    User {
        id: model.id,
        username: model.username,
        email: model.email,
        password_hash: model.password_hash,
        fullname: model.fullname,
        status: model.status.parse::<UserStatus>().unwrap_or_else(|_| {
            tracing::warn!("invalid user status in database, defaulting to Active");
            UserStatus::Active
        }),
        created_by: model.created_by,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(model.created_at, Utc),
        updated_by: model.updated_by,
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(model.updated_at, Utc),
        deleted_by: model.deleted_by,
        deleted_at: model
            .deleted_at
            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
    }
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
            .map_err(|e| RepositoryError::Database(e.to_string()))
            .map(|opt| opt.map(model_to_entity))
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
            .map_err(|e| RepositoryError::Database(e.to_string()))
            .map(|opt| opt.map(model_to_entity))
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
            .map_err(|e| RepositoryError::Database(e.to_string()))
            .map(|opt| opt.map(model_to_entity))
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
        .map(model_to_entity)
        .map_err(|e| RepositoryError::Database(e.to_string()))
    }

    async fn update(&self, user: &User) -> RepositoryResult<User> {
        // Verify the record exists and is not soft-deleted.
        let _existing = self
            .find_by_id(user.id)
            .await?
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
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map(model_to_entity)
        .map_err(|e| RepositoryError::Database(e.to_string()))
    }

    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()> {
        // Verify the record exists and is not already soft-deleted.
        let _existing = users::Entity::find_by_id(id)
            .filter(users::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .ok_or(RepositoryError::NotFound)?;

        users::ActiveModel {
            id: Set(id),
            deleted_at: Set(Some(Utc::now().naive_utc())),
            deleted_by: Set(Some(deleted_by)),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn list_paginated(&self, page: u32, limit: u32) -> RepositoryResult<(Vec<User>, u64)> {
        let paginator = users::Entity::find()
            .filter(users::Column::DeletedAt.is_null())
            .order_by_asc(users::Column::Id)
            .paginate(&self.db, limit as u64);

        let page_zero_based = page.saturating_sub(1) as u64;

        let users = paginator
            .fetch_page(page_zero_based)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(model_to_entity)
            .collect();

        let count = paginator
            .num_items()
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok((users, count))
    }

    async fn update_password_hash(&self, id: i64, password_hash: &str) -> RepositoryResult<()> {
        // Verify the user exists and is not soft-deleted.
        let _existing = self
            .find_by_id(id)
            .await?
            .ok_or(RepositoryError::NotFound)?;

        users::ActiveModel {
            id: Set(id),
            password_hash: Set(password_hash.to_owned()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map(|_| ())
        .map_err(|e| match &e {
            DbErr::RecordNotFound(_) => RepositoryError::NotFound,
            _ => RepositoryError::Database(e.to_string()),
        })
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
