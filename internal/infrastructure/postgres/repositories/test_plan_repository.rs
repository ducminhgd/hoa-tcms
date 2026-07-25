//! `SqlTestPlanRepository` — PostgreSQL implementation of [`TestPlanRepository`].
//!
//! Maps `test_plans` + `test_plan_projects` to the [`TestPlan`] domain entity
//! using SeaORM. Multi-project membership filtering is done with simple
//! multi-step queries for correctness and maintainability.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};

use crate::application::repositories::test_plan_repository::{
    TestPlanFilters, TestPlanListItem, TestPlanRepository,
};
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::domain::entities::test_plan::{TestPlan, TestPlanSelectItem};
use crate::domain::value_objects::test_plan_status::TestPlanStatus;
use crate::infrastructure::db::entities::{
    project_members, projects, test_plan_projects, test_plans,
};

pub struct SqlTestPlanRepository {
    db: DatabaseConnection,
}

impl SqlTestPlanRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn model_to_entity(model: test_plans::Model) -> RepositoryResult<TestPlan> {
    let status = TestPlanStatus::parse(&model.status).unwrap_or(TestPlanStatus::ToDo);
    Ok(TestPlan {
        id: model.id,
        name: model.name,
        version: model.version,
        types: model.types,
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

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn validate_sort(sort: &str) -> RepositoryResult<(test_plans::Column, sea_orm::Order)> {
    let (col, order) = if let Some(stripped) = sort.strip_prefix('-') {
        (stripped, sea_orm::Order::Desc)
    } else {
        (sort, sea_orm::Order::Asc)
    };
    let c = match col {
        "updated_at" => test_plans::Column::UpdatedAt,
        "name" => test_plans::Column::Name,
        "status" => test_plans::Column::Status,
        _ => {
            return Err(RepositoryError::Database(format!(
                "invalid sort value: {}",
                sort
            )));
        }
    };
    Ok((c, order))
}

fn types_contains(types: &serde_json::Value, target: &str) -> bool {
    types.as_array().is_some_and(|arr| {
        arr.iter()
            .any(|v| v.as_str().is_some_and(|s| s.eq_ignore_ascii_case(target)))
    })
}

/// Get plan IDs accessible by a user (via project membership).
async fn accessible_plan_ids(
    db: &DatabaseConnection,
    user_id: i64,
) -> Result<Vec<i64>, sea_orm::DbErr> {
    // Step 1: get project_ids where user is a member.
    let member_project_ids: Vec<i64> = project_members::Entity::find()
        .select_only()
        .column(project_members::Column::ProjectId)
        .filter(project_members::Column::UserId.eq(user_id))
        .into_tuple()
        .all(db)
        .await?;

    if member_project_ids.is_empty() {
        return Ok(vec![]);
    }

    // Step 2: get plan_ids from test_plan_projects for those projects.
    let plan_ids: Vec<i64> = test_plan_projects::Entity::find()
        .select_only()
        .column(test_plan_projects::Column::PlanId)
        .filter(test_plan_projects::Column::ProjectId.is_in(member_project_ids))
        .distinct()
        .into_tuple()
        .all(db)
        .await?;

    Ok(plan_ids)
}

#[async_trait]
impl TestPlanRepository for SqlTestPlanRepository {
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<TestPlan>> {
        test_plans::Entity::find_by_id(id)
            .filter(test_plans::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn find_by_name(&self, name: &str) -> RepositoryResult<Option<TestPlan>> {
        use sea_orm::sea_query::Expr;

        test_plans::Entity::find()
            .filter(test_plans::Column::DeletedAt.is_null())
            .filter(Expr::cust_with_values("LOWER(name) = LOWER($1)", [name]))
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn find_accessible(
        &self,
        user_id: i64,
        is_admin: bool,
        page: u32,
        limit: u32,
        filters: &TestPlanFilters,
    ) -> RepositoryResult<(Vec<TestPlanListItem>, u64)> {
        let (sort_col, sort_order) = validate_sort(&filters.sort)?;

        // Get accessible plan IDs.
        let plan_ids: Option<Vec<i64>> = if is_admin {
            None
        } else {
            let ids = accessible_plan_ids(&self.db, user_id)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?;
            if ids.is_empty() {
                return Ok((vec![], 0));
            }
            Some(ids)
        };

        let mut select = test_plans::Entity::find().filter(test_plans::Column::DeletedAt.is_null());

        if let Some(ref ids) = plan_ids {
            select = select.filter(test_plans::Column::Id.is_in(ids.clone()));
        }

        // Apply filters.
        if let Some(ref status) = filters.status {
            select = select.filter(test_plans::Column::Status.eq(status));
        }
        if let Some(ref project_id) = filters.project_id {
            let linked_ids: Vec<i64> = test_plan_projects::Entity::find()
                .select_only()
                .column(test_plan_projects::Column::PlanId)
                .filter(test_plan_projects::Column::ProjectId.eq(*project_id))
                .into_tuple()
                .all(&self.db)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?;
            if linked_ids.is_empty() {
                return Ok((vec![], 0));
            }
            select = select.filter(test_plans::Column::Id.is_in(linked_ids));
        }
        if let Some(ref search) = filters.search {
            let trimmed = search.trim();
            if !trimmed.is_empty() && trimmed.len() <= 255 {
                let escaped = escape_like(trimmed);
                select = select.filter(test_plans::Column::Name.like(format!("%{}%", &escaped)));
            }
        }

        // Count (before plan_type in-memory filter).
        let total = select
            .clone()
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        // Paginate.
        let offset = ((page.saturating_sub(1)) * limit) as u64;
        let rows = select
            .order_by(sort_col, sort_order)
            .offset(offset)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        // Collect items with project_ids.
        let mut items: Vec<TestPlanListItem> = Vec::with_capacity(rows.len());
        for row in &rows {
            let pids = self.get_project_ids(row.id).await.unwrap_or_default();
            items.push(TestPlanListItem {
                id: row.id,
                name: row.name.clone(),
                version: row.version.clone(),
                types: row.types.clone(),
                status: row.status.clone(),
                project_ids: pids,
                created_by: row.created_by,
                created_at: DateTime::<Utc>::from(row.created_at),
                updated_at: DateTime::<Utc>::from(row.updated_at),
            });
        }

        // Apply plan_type filter in-memory.
        let items = if let Some(ref pt) = filters.plan_type {
            items
                .into_iter()
                .filter(|item| types_contains(&item.types, pt))
                .collect()
        } else {
            items
        };

        Ok((items, total))
    }

    async fn find_selectable(
        &self,
        user_id: i64,
        is_admin: bool,
    ) -> RepositoryResult<Vec<TestPlanSelectItem>> {
        let mut select = test_plans::Entity::find()
            .select_only()
            .column(test_plans::Column::Id)
            .column(test_plans::Column::Name)
            .filter(test_plans::Column::DeletedAt.is_null());

        if !is_admin {
            let ids = accessible_plan_ids(&self.db, user_id)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?;
            if ids.is_empty() {
                return Ok(vec![]);
            }
            select = select.filter(test_plans::Column::Id.is_in(ids));
        }

        // Order by name case-insensitive, limit 1000.
        let rows = select
            .order_by_asc(sea_orm::sea_query::Expr::cust("LOWER(test_plans.name)"))
            .limit(1000)
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut items = Vec::with_capacity(rows.len());
        for row in &rows {
            items.push(TestPlanSelectItem {
                id: row.id,
                name: row.name.clone(),
            });
        }
        Ok(items)
    }

    async fn get_project_ids(&self, plan_id: i64) -> RepositoryResult<Vec<i64>> {
        let rows = test_plan_projects::Entity::find()
            .filter(test_plan_projects::Column::PlanId.eq(plan_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.project_id).collect())
    }

    async fn validate_project_ids(&self, project_ids: &[i64]) -> RepositoryResult<Vec<i64>> {
        let rows = projects::Entity::find()
            .filter(projects::Column::Id.is_in(project_ids.iter().copied()))
            .filter(projects::Column::DeletedAt.is_null())
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.id).collect())
    }

    async fn create(&self, plan: &TestPlan, project_ids: &[i64]) -> RepositoryResult<TestPlan> {
        let plan = plan.clone();
        let pids = project_ids.to_vec();

        let result = self
            .db
            .transaction::<_, TestPlan, RepositoryError>(|txn| {
                let plan = plan.clone();
                let pids = pids.clone();
                Box::pin(async move {
                    let inserted = test_plans::ActiveModel {
                        name: Set(plan.name.clone()),
                        version: Set(plan.version.clone()),
                        types: Set(plan.types.clone()),
                        description: Set(plan.description.clone()),
                        status: Set(plan.status.as_str().to_string()),
                        created_by: Set(plan.created_by),
                        created_at: Set(plan.created_at.fixed_offset()),
                        updated_by: Set(plan.updated_by),
                        updated_at: Set(plan.updated_at.fixed_offset()),
                        ..Default::default()
                    }
                    .insert(txn)
                    .await
                    .map_err(|e| {
                        let msg = e.to_string();
                        if msg.contains("uq_test_plans_name") || msg.contains("duplicate key") {
                            RepositoryError::Duplicate("test plan name already exists".into())
                        } else {
                            RepositoryError::Database(msg)
                        }
                    })?;

                    for &pid in &pids {
                        test_plan_projects::ActiveModel {
                            plan_id: Set(inserted.id),
                            project_id: Set(pid),
                            ..Default::default()
                        }
                        .insert(txn)
                        .await
                        .map_err(|e| RepositoryError::Database(e.to_string()))?;
                    }

                    model_to_entity(inserted)
                })
            })
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(result)
    }

    async fn update(
        &self,
        plan: &TestPlan,
        project_ids: Option<&[i64]>,
    ) -> RepositoryResult<TestPlan> {
        let plan = plan.clone();
        let pids = project_ids.map(|v| v.to_vec());

        let result = self
            .db
            .transaction::<_, TestPlan, RepositoryError>(|txn| {
                let plan = plan.clone();
                let pids = pids.clone();
                Box::pin(async move {
                    if let Some(ref new_pids) = pids {
                        test_plan_projects::Entity::delete_many()
                            .filter(test_plan_projects::Column::PlanId.eq(plan.id))
                            .exec(txn)
                            .await
                            .map_err(|e| RepositoryError::Database(e.to_string()))?;

                        for &pid in new_pids {
                            test_plan_projects::ActiveModel {
                                plan_id: Set(plan.id),
                                project_id: Set(pid),
                                ..Default::default()
                            }
                            .insert(txn)
                            .await
                            .map_err(|e| RepositoryError::Database(e.to_string()))?;
                        }
                    }

                    let inserted = test_plans::ActiveModel {
                        id: Set(plan.id),
                        name: Set(plan.name.clone()),
                        version: Set(plan.version.clone()),
                        types: Set(plan.types.clone()),
                        description: Set(plan.description.clone()),
                        status: Set(plan.status.as_str().to_string()),
                        updated_by: Set(plan.updated_by),
                        updated_at: Set(plan.updated_at.fixed_offset()),
                        ..Default::default()
                    }
                    .update(txn)
                    .await
                    .map_err(|e| {
                        let msg = e.to_string();
                        if msg.contains("uq_test_plans_name") || msg.contains("duplicate key") {
                            RepositoryError::Duplicate("test plan name already exists".into())
                        } else {
                            RepositoryError::Database(msg)
                        }
                    })?;

                    model_to_entity(inserted)
                })
            })
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(result)
    }

    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()> {
        let result = test_plans::Entity::update_many()
            .filter(test_plans::Column::Id.eq(id))
            .filter(test_plans::Column::DeletedAt.is_null())
            .set(test_plans::ActiveModel {
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

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: TestPlanRepository>() {}
    check::<SqlTestPlanRepository>();
}
