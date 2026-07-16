//! `SqlProjectRepository` — PostgreSQL implementation of [`ProjectRepository`].
//!
//! Maps the `projects` table to the [`Project`] domain entity using SeaORM.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};

use crate::application::repositories::project_repository::ProjectRepository;
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::domain::entities::project::Project;
use crate::domain::value_objects::project_status::ProjectStatus;
use crate::infrastructure::config::metadata_seeder::MetadataConfig;
use crate::infrastructure::db::entities::{
    project_members, projects, test_case_templates, test_categories,
};

/// PostgreSQL-backed [`ProjectRepository`].
pub struct SqlProjectRepository {
    db: DatabaseConnection,
    metadata_config: MetadataConfig,
}

impl SqlProjectRepository {
    /// Create a new repository bound to the given database connection.
    pub fn new(db: DatabaseConnection, metadata_config: MetadataConfig) -> Self {
        Self {
            db,
            metadata_config,
        }
    }
}

// ---------------------------------------------------------------------------
// SeaORM Model → domain Entity mapping
// ---------------------------------------------------------------------------

fn model_to_entity(model: projects::Model) -> RepositoryResult<Project> {
    let status = ProjectStatus::from_str(&model.status).ok_or_else(|| {
        RepositoryError::Database(format!(
            "invalid project status '{}' for project {}",
            model.status, model.id
        ))
    })?;

    Ok(Project {
        id: model.id,
        name: model.name,
        description: model.description,
        status,
        created_by: model.created_by,
        created_at: DateTime::<Utc>::from(model.created_at),
        updated_by: model.updated_by,
        updated_at: DateTime::<Utc>::from(model.updated_at),
        deleted_by: model.deleted_by,
        deleted_at: model.deleted_at.map(DateTime::<Utc>::from),
    })
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl ProjectRepository for SqlProjectRepository {
    async fn create_project_transactional(
        &self,
        project: &Project,
        created_by: i64,
    ) -> RepositoryResult<Project> {
        let metadata = self.metadata_config.clone();

        self.db
            .transaction::<_, Project, RepositoryError>(|txn| {
                let project = project.clone();
                let metadata = metadata.clone();
                Box::pin(async move {
                    // 1. Insert project.
                    let inserted = projects::ActiveModel {
                        name: Set(project.name.clone()),
                        description: Set(project.description.clone()),
                        status: Set(project.status.to_string()),
                        created_by: Set(project.created_by),
                        updated_by: Set(project.updated_by),
                        ..Default::default()
                    }
                    .insert(txn)
                    .await
                    .map_err(|e| {
                        let msg = e.to_string();
                        if msg.contains("uq_projects_name_lower") || msg.contains("duplicate key") {
                            RepositoryError::Duplicate(format!(
                                "a project with name '{}' already exists",
                                project.name
                            ))
                        } else {
                            RepositoryError::Database(msg)
                        }
                    })?;

                    let project_id = inserted.id;

                    // 2. Add creator as Owner member.
                    project_members::ActiveModel {
                        project_id: Set(project_id),
                        user_id: Set(created_by),
                        role: Set("Owner".to_string()),
                        ..Default::default()
                    }
                    .insert(txn)
                    .await
                    .map_err(|e| RepositoryError::Database(e.to_string()))?;

                    // 3. Seed categories.
                    for cat in &metadata.categories {
                        test_categories::ActiveModel {
                            project_id: Set(project_id),
                            name: Set(cat.name.clone()),
                            description: Set(cat.description.clone()),
                            created_by: Set(created_by),
                            updated_by: Set(created_by),
                            ..Default::default()
                        }
                        .insert(txn)
                        .await
                        .map_err(|e| RepositoryError::Database(e.to_string()))?;
                    }

                    // 4. Seed templates.
                    for tmpl in &metadata.templates {
                        test_case_templates::ActiveModel {
                            project_id: Set(project_id),
                            name: Set(tmpl.name.clone()),
                            template_content: Set(tmpl.content.clone()),
                            created_by: Set(created_by),
                            updated_by: Set(created_by),
                            ..Default::default()
                        }
                        .insert(txn)
                        .await
                        .map_err(|e| RepositoryError::Database(e.to_string()))?;
                    }

                    Ok(model_to_entity(inserted)?)
                })
            })
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))
    }

    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<Project>> {
        projects::Entity::find_by_id(id)
            .filter(projects::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn find_by_name(&self, name: &str) -> RepositoryResult<Option<Project>> {
        projects::Entity::find()
            .filter(Expr::cust_with_values(
                "LOWER(name) = LOWER($1)",
                [name.to_string()],
            ))
            .filter(projects::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn insert(&self, project: &Project) -> RepositoryResult<Project> {
        projects::ActiveModel {
            name: Set(project.name.clone()),
            description: Set(project.description.clone()),
            status: Set(project.status.to_string()),
            created_by: Set(project.created_by),
            updated_by: Set(project.updated_by),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("uq_projects_name_lower") || msg.contains("duplicate key") {
                RepositoryError::Duplicate(format!(
                    "a project with name '{}' already exists",
                    project.name
                ))
            } else {
                RepositoryError::Database(msg)
            }
        })
        .and_then(model_to_entity)
    }

    async fn update(&self, project: &Project) -> RepositoryResult<Project> {
        // Read the current row to verify existence and preserve soft-delete state.
        let existing = projects::Entity::find_by_id(project.id)
            .filter(projects::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .ok_or(RepositoryError::NotFound)?;

        projects::ActiveModel {
            id: Set(project.id),
            name: Set(project.name.clone()),
            description: Set(project.description.clone()),
            status: Set(project.status.to_string()),
            updated_by: Set(project.updated_by),
            updated_at: Set(Utc::now().fixed_offset()),
            // Preserve audit timestamps.
            created_by: Set(existing.created_by),
            created_at: Set(existing.created_at),
            // Preserve soft-delete state.
            deleted_at: Set(existing.deleted_at),
            deleted_by: Set(existing.deleted_by),
        }
        .update(&self.db)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))
        .and_then(model_to_entity)
    }

    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()> {
        let result = projects::Entity::update_many()
            .filter(projects::Column::Id.eq(id))
            .filter(projects::Column::DeletedAt.is_null())
            .set(projects::ActiveModel {
                deleted_at: Set(Some(Utc::now().fixed_offset())),
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

    async fn list_by_user(
        &self,
        user_id: i64,
        page: u32,
        limit: u32,
        status_filter: Option<&str>,
    ) -> RepositoryResult<(Vec<Project>, u64)> {
        use crate::infrastructure::db::entities::project_members;

        let page = page.max(1);
        let limit = limit.clamp(1, 100) as u64;
        let offset = (page as u64 - 1) * limit;

        // Build base query joining through project_members.
        let mut select = projects::Entity::find()
            .join_rev(
                sea_orm::JoinType::InnerJoin,
                project_members::Entity::belongs_to(projects::Entity)
                    .from(project_members::Column::ProjectId)
                    .to(projects::Column::Id)
                    .into(),
            )
            .filter(project_members::Column::UserId.eq(user_id))
            .filter(projects::Column::DeletedAt.is_null())
            .order_by_desc(projects::Column::CreatedAt);

        if let Some(status) = status_filter {
            select = select.filter(projects::Column::Status.eq(status.to_uppercase()));
        }

        // Count total matching rows.
        let count = select
            .clone()
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        // Fetch page.
        let rows = select
            .offset(offset)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(model_to_entity)
            .collect::<Result<Vec<_>, _>>()?;

        Ok((rows, count))
    }

    async fn list_all(
        &self,
        page: u32,
        limit: u32,
        status_filter: Option<&str>,
    ) -> RepositoryResult<(Vec<Project>, u64)> {
        let page = page.max(1);
        let limit = limit.clamp(1, 100) as u64;
        let offset = (page as u64 - 1) * limit;

        let mut select = projects::Entity::find()
            .filter(projects::Column::DeletedAt.is_null())
            .order_by_desc(projects::Column::CreatedAt);

        if let Some(status) = status_filter {
            select = select.filter(projects::Column::Status.eq(status.to_uppercase()));
        }

        let count = select
            .clone()
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let rows = select
            .offset(offset)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(model_to_entity)
            .collect::<Result<Vec<_>, _>>()?;

        Ok((rows, count))
    }

    async fn member_count(&self, project_id: i64) -> RepositoryResult<u64> {
        use crate::infrastructure::db::entities::project_members;

        project_members::Entity::find()
            .filter(project_members::Column::ProjectId.eq(project_id))
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Compile-time check: SqlProjectRepository satisfies ProjectRepository
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: ProjectRepository>() {}
    check::<SqlProjectRepository>();
}
