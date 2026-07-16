//! `SqlPermissionRepository` — PostgreSQL implementation of
//! [`PermissionRepository`].
//!
//! The `permissions` table is a **read-only reference table** seeded by
//! migrations and never modified through the application API, so this
//! repository exposes only read operations using SeaORM.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use crate::application::repositories::permission_repository::PermissionRepository;
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::domain::entities::permission::Permission;
use crate::infrastructure::db::entities::permissions;

/// PostgreSQL-backed [`PermissionRepository`].
pub struct SqlPermissionRepository {
    db: DatabaseConnection,
}

impl SqlPermissionRepository {
    /// Create a new repository bound to the given database connection.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

// ---------------------------------------------------------------------------
// SeaORM Model -> domain Entity mapping
// ---------------------------------------------------------------------------

fn model_to_entity(model: permissions::Model) -> Permission {
    Permission {
        id: model.id,
        name: model.name,
        code: model.code,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(model.created_at, Utc),
    }
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl PermissionRepository for SqlPermissionRepository {
    async fn find_all(&self, page: u32, limit: u32) -> RepositoryResult<(Vec<Permission>, u64)> {
        let paginator = permissions::Entity::find()
            .order_by_asc(permissions::Column::Code)
            .paginate(&self.db, limit as u64);

        let page_zero_based = page.saturating_sub(1) as u64;

        let permissions = paginator
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

        Ok((permissions, count))
    }

    async fn find_by_code(&self, code: &str) -> RepositoryResult<Option<Permission>> {
        permissions::Entity::find()
            .filter(permissions::Column::Code.eq(code))
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))
            .map(|opt| opt.map(model_to_entity))
    }
}

// ---------------------------------------------------------------------------
// Compile-time check: SqlPermissionRepository satisfies PermissionRepository
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: PermissionRepository>() {}
    check::<SqlPermissionRepository>();
}
