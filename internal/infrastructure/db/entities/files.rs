use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "files")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub filename: String,
    pub storage_path: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub owner_type: String,
    pub owner_id: i64,
    pub created_by: i64,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub updated_by: i64,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
    pub deleted_by: Option<i64>,
    pub deleted_at: Option<chrono::DateTime<chrono::FixedOffset>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
