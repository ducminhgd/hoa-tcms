use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "project_members")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub project_id: i64,
    #[sea_orm(primary_key)]
    pub user_id: i64,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
