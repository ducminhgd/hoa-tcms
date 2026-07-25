use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "test_plan_projects")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub plan_id: i64,
    #[sea_orm(primary_key)]
    pub project_id: i64,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::test_plans::Entity",
        from = "Column::PlanId",
        to = "super::test_plans::Column::Id"
    )]
    TestPlan,
    #[sea_orm(
        belongs_to = "super::projects::Entity",
        from = "Column::ProjectId",
        to = "super::projects::Column::Id"
    )]
    Projects,
}

impl ActiveModelBehavior for ActiveModel {}
