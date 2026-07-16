//! PostgreSQL implementation of [`MemberRepository`].
//!
//! Manages the `user_groups` junction table that links users to groups.

use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set, SqlErr,
};

use crate::application::repositories::member_repository::MemberRepository;
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::infrastructure::db::entities::{user_groups, users};

/// PostgreSQL-backed [`MemberRepository`].
pub struct SqlMemberRepository {
    db: DatabaseConnection,
}

impl SqlMemberRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MemberRepository for SqlMemberRepository {
    async fn add(&self, group_id: i64, user_id: i64) -> RepositoryResult<()> {
        user_groups::ActiveModel {
            user_id: Set(user_id),
            group_id: Set(group_id),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map(|_| ())
        .map_err(|e| match e.sql_err() {
            Some(SqlErr::ForeignKeyConstraintViolation(_)) => RepositoryError::NotFound,
            Some(SqlErr::UniqueConstraintViolation(_)) => {
                RepositoryError::Duplicate("User is already a member of this group".to_string())
            }
            _ => RepositoryError::Database(e.to_string()),
        })
    }

    async fn remove(&self, group_id: i64, user_id: i64) -> RepositoryResult<()> {
        user_groups::Entity::delete_many()
            .filter(user_groups::Column::UserId.eq(user_id))
            .filter(user_groups::Column::GroupId.eq(group_id))
            .exec(&self.db)
            .await
            .map(|_| ())
            .map_err(|e| RepositoryError::Database(e.to_string()))
    }

    async fn find_by_group(&self, group_id: i64) -> RepositoryResult<Vec<(i64, String)>> {
        let user_ids: Vec<i64> = user_groups::Entity::find()
            .filter(user_groups::Column::GroupId.eq(group_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|ug| ug.user_id)
            .collect();

        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let members = users::Entity::find()
            .filter(users::Column::Id.is_in(user_ids))
            .filter(users::Column::DeletedAt.is_null())
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(members.into_iter().map(|u| (u.id, u.username)).collect())
    }

    async fn count_by_group(&self, group_id: i64) -> RepositoryResult<u64> {
        let user_ids: Vec<i64> = user_groups::Entity::find()
            .filter(user_groups::Column::GroupId.eq(group_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|ug| ug.user_id)
            .collect();

        if user_ids.is_empty() {
            return Ok(0);
        }

        let count = users::Entity::find()
            .filter(users::Column::Id.is_in(user_ids))
            .filter(users::Column::DeletedAt.is_null())
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(count)
    }

    async fn validate_users_exist(&self, user_ids: &[i64]) -> RepositoryResult<Vec<i64>> {
        let found = users::Entity::find()
            .filter(users::Column::Id.is_in(user_ids.to_vec()))
            .filter(users::Column::DeletedAt.is_null())
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(found.into_iter().map(|u| u.id).collect())
    }
}

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: MemberRepository>() {}
    check::<SqlMemberRepository>();
}
