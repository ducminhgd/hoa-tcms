//! `SqlTestCaseFileRepository` — PostgreSQL implementation of
//! [`TestCaseFileRepository`].
//!
//! Maps the `test_case_files` table to the [`TestCaseFile`] domain entity
//! using SeaORM.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::application::repositories::test_case_file_repository::{
    TestCaseFileListItem, TestCaseFileRepository, TestCaseInfo,
};
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::domain::entities::test_case_file::TestCaseFile;
use crate::infrastructure::db::entities::{test_case_files, test_cases};

/// PostgreSQL-backed [`TestCaseFileRepository`].
pub struct SqlTestCaseFileRepository {
    db: DatabaseConnection,
}

impl SqlTestCaseFileRepository {
    /// Create a new repository bound to the given database connection.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

// ---------------------------------------------------------------------------
// Helpers: SeaORM Model <-> domain Entity mapping
// ---------------------------------------------------------------------------

fn model_to_entity(model: test_case_files::Model) -> RepositoryResult<TestCaseFile> {
    Ok(TestCaseFile {
        id: model.id,
        test_case_id: model.test_case_id,
        file_name: model.file_name,
        file_path: model.file_path,
        file_size: model.file_size,
        mime_type: model.mime_type,
        uploaded_by: model.uploaded_by,
        created_at: DateTime::<Utc>::from(model.created_at),
        updated_at: DateTime::<Utc>::from(model.updated_at),
        deleted_by: model.deleted_by,
        deleted_at: model.deleted_at.map(DateTime::<Utc>::from),
    })
}

fn model_to_list_item(model: test_case_files::Model) -> RepositoryResult<TestCaseFileListItem> {
    Ok(TestCaseFileListItem {
        id: model.id,
        file_name: model.file_name,
        file_size: model.file_size,
        mime_type: model.mime_type,
        uploaded_by: model.uploaded_by,
        created_at: DateTime::<Utc>::from(model.created_at),
        updated_at: DateTime::<Utc>::from(model.updated_at),
    })
}

/// Escape a string for safe use in `ILIKE` patterns.
///
/// Escape order: `\` -> `\\`, then `%` -> `\%`, then `_` -> `\_`.
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Validate and parse a `sort` parameter into a (column, order) tuple.
///
/// Acceptable values: `file_name`, `-file_name`, `file_size`, `-file_size`,
/// `created_at`, `-created_at`.
fn parse_sort(sort: &str) -> RepositoryResult<(test_case_files::Column, sea_orm::Order)> {
    let (column, order) = if let Some(stripped) = sort.strip_prefix('-') {
        (stripped, sea_orm::Order::Desc)
    } else {
        (sort, sea_orm::Order::Asc)
    };

    let col = match column {
        "file_name" => test_case_files::Column::FileName,
        "file_size" => test_case_files::Column::FileSize,
        "created_at" => test_case_files::Column::CreatedAt,
        _ => {
            return Err(RepositoryError::Database(format!(
                "invalid sort value: '{}'. Accepted: file_name, -file_name, file_size, -file_size, created_at, -created_at",
                sort
            )));
        }
    };

    Ok((col, order))
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl TestCaseFileRepository for SqlTestCaseFileRepository {
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<TestCaseFile>> {
        test_case_files::Entity::find_by_id(id)
            .filter(test_case_files::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn find_by_test_case(
        &self,
        test_case_id: i64,
        search: Option<&str>,
        sort: &str,
    ) -> RepositoryResult<Vec<TestCaseFileListItem>> {
        // Parse and validate sort parameter.
        let (sort_col, sort_order) = parse_sort(sort)?;

        let mut select = test_case_files::Entity::find()
            .filter(test_case_files::Column::TestCaseId.eq(test_case_id))
            .filter(test_case_files::Column::DeletedAt.is_null())
            .order_by(sort_col, sort_order);

        if let Some(search_term) = search {
            let trimmed = search_term.trim();
            if !trimmed.is_empty() && trimmed.len() <= 255 {
                let escaped = escape_like(trimmed);
                select =
                    select.filter(test_case_files::Column::FileName.like(format!("%{}%", escaped)));
            }
        }

        let rows = select
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        rows.into_iter()
            .map(model_to_list_item)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn save(&self, file: &TestCaseFile) -> RepositoryResult<TestCaseFile> {
        let inserted = test_case_files::ActiveModel {
            test_case_id: Set(file.test_case_id),
            file_name: Set(file.file_name.clone()),
            file_path: Set(file.file_path.clone()),
            file_size: Set(file.file_size),
            mime_type: Set(file.mime_type.clone()),
            uploaded_by: Set(file.uploaded_by),
            created_at: Set(file.created_at.fixed_offset()),
            updated_at: Set(file.updated_at.fixed_offset()),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        model_to_entity(inserted)
    }

    async fn get_test_case_info(
        &self,
        test_case_id: i64,
    ) -> RepositoryResult<Option<TestCaseInfo>> {
        test_cases::Entity::find_by_id(test_case_id)
            .filter(test_cases::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(|m| TestCaseInfo {
                created_by: m.created_by,
                project_id: m.project_id,
            })
            .map(Ok)
            .transpose()
    }

    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()> {
        let result = test_case_files::Entity::update_many()
            .filter(test_case_files::Column::Id.eq(id))
            .filter(test_case_files::Column::DeletedAt.is_null())
            .set(test_case_files::ActiveModel {
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
}

// ---------------------------------------------------------------------------
// Compile-time check
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: TestCaseFileRepository>() {}
    check::<SqlTestCaseFileRepository>();
}
