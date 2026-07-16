//! `SqlProjectMemberRepository` — PostgreSQL implementation of [`ProjectMemberRepository`].
//!
//! Maps the `project_members` junction table using SeaORM.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};

use crate::application::repositories::project_member_repository::{
    ProjectMemberRepository, ProjectMemberRow,
};
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::infrastructure::db::entities::{project_members, users};

/// PostgreSQL-backed [`ProjectMemberRepository`].
pub struct SqlProjectMemberRepository {
    db: DatabaseConnection,
}

impl SqlProjectMemberRepository {
    /// Create a new repository bound to the given database connection.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

fn dt_to_utc(dt: chrono::DateTime<chrono::FixedOffset>) -> DateTime<Utc> {
    DateTime::<Utc>::from(dt)
}

#[async_trait]
impl ProjectMemberRepository for SqlProjectMemberRepository {
    async fn add_member(&self, project_id: i64, user_id: i64, role: &str) -> RepositoryResult<()> {
        project_members::ActiveModel {
            project_id: Set(project_id),
            user_id: Set(user_id),
            role: Set(role.to_string()),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("duplicate key") || msg.contains("violates unique constraint") {
                RepositoryError::Duplicate(format!(
                    "user {} is already a member of project {}",
                    user_id, project_id
                ))
            } else {
                RepositoryError::Database(msg)
            }
        })?;

        Ok(())
    }

    async fn remove_member(&self, project_id: i64, user_id: i64) -> RepositoryResult<()> {
        project_members::Entity::delete_many()
            .filter(project_members::Column::ProjectId.eq(project_id))
            .filter(project_members::Column::UserId.eq(user_id))
            .exec(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn change_role(&self, project_id: i64, user_id: i64, role: &str) -> RepositoryResult<()> {
        let result = project_members::Entity::update_many()
            .filter(project_members::Column::ProjectId.eq(project_id))
            .filter(project_members::Column::UserId.eq(user_id))
            .set(project_members::ActiveModel {
                role: Set(role.to_string()),
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

    async fn list_by_project(&self, project_id: i64) -> RepositoryResult<Vec<ProjectMemberRow>> {
        let rows = project_members::Entity::find()
            .filter(project_members::Column::ProjectId.eq(project_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut members = Vec::new();
        for m in rows {
            if let Some(user) = users::Entity::find_by_id(m.user_id)
                .filter(users::Column::DeletedAt.is_null())
                .filter(users::Column::Status.eq("ACTIVE"))
                .one(&self.db)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?
            {
                members.push(ProjectMemberRow {
                    project_id: m.project_id,
                    user_id: m.user_id,
                    username: user.username,
                    fullname: user.fullname,
                    role: m.role,
                    created_at: dt_to_utc(m.created_at),
                });
            }
        }

        Ok(members)
    }

    async fn get_member(
        &self,
        project_id: i64,
        user_id: i64,
    ) -> RepositoryResult<Option<ProjectMemberRow>> {
        let membership = project_members::Entity::find()
            .filter(project_members::Column::ProjectId.eq(project_id))
            .filter(project_members::Column::UserId.eq(user_id))
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        match membership {
            Some(m) => {
                if let Some(user) = users::Entity::find_by_id(m.user_id)
                    .filter(users::Column::DeletedAt.is_null())
                    .one(&self.db)
                    .await
                    .map_err(|e| RepositoryError::Database(e.to_string()))?
                {
                    Ok(Some(ProjectMemberRow {
                        project_id: m.project_id,
                        user_id: m.user_id,
                        username: user.username,
                        fullname: user.fullname,
                        role: m.role,
                        created_at: dt_to_utc(m.created_at),
                    }))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn count_by_role(&self, project_id: i64, role: &str) -> RepositoryResult<u64> {
        project_members::Entity::find()
            .filter(project_members::Column::ProjectId.eq(project_id))
            .filter(project_members::Column::Role.eq(role.to_string()))
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))
    }

    async fn count_all(&self, project_id: i64) -> RepositoryResult<u64> {
        project_members::Entity::find()
            .filter(project_members::Column::ProjectId.eq(project_id))
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Compile-time check
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: ProjectMemberRepository>() {}
    check::<SqlProjectMemberRepository>();
}
