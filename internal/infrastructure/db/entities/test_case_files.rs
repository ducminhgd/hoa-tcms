//! SeaORM entity for the `test_case_files` table.
//!
//! Maps the database schema to Rust types for use with SeaORM queries.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `test_case_files` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "test_case_files")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub test_case_id: i64,
    pub file_name: String,
    pub file_path: String,
    pub file_size: i64,
    pub mime_type: String,
    pub uploaded_by: i64,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
    pub deleted_by: Option<i64>,
    pub deleted_at: Option<chrono::DateTime<chrono::FixedOffset>>,
}

/// SeaORM relation definitions for `test_case_files`.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::test_cases::Entity",
        from = "Column::TestCaseId",
        to = "super::test_cases::Column::Id"
    )]
    TestCase,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UploadedBy",
        to = "super::users::Column::Id"
    )]
    Uploader,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::DeletedBy",
        to = "super::users::Column::Id"
    )]
    Deleter,
}

impl ActiveModelBehavior for ActiveModel {}
