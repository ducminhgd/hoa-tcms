use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "test_case_results")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub execution_id: i64,
    pub test_case_id: i64,
    pub summary: String,
    pub description: Option<String>,
    pub priority: String,
    pub result: String,
    pub logs: Option<String>,
    pub tested_by: Option<i64>,
    pub created_by: i64,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub updated_by: i64,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
    pub deleted_by: Option<i64>,
    pub deleted_at: Option<chrono::DateTime<chrono::FixedOffset>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::test_executions::Entity",
        from = "Column::ExecutionId",
        to = "super::test_executions::Column::Id"
    )]
    TestExecution,
}

impl ActiveModelBehavior for ActiveModel {}
