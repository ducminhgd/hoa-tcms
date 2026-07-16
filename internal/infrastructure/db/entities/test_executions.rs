use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "test_executions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub test_run_id: i64,
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
