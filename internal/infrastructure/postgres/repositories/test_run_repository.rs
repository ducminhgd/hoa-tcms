//! `SqlTestRunRepository` — PostgreSQL implementation of [`TestRunRepository`].

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, RelationTrait, Set, TransactionTrait,
};

use crate::application::repositories::test_run_repository::{
    TestRunListItem, TestRunRepository, TestRunStatistics,
};
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::domain::entities::test_run::TestRun;
use crate::infrastructure::db::entities::{
    test_case_results, test_executions, test_run_cases, test_runs,
};

pub struct SqlTestRunRepository {
    db: DatabaseConnection,
}

impl SqlTestRunRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn model_to_entity(model: test_runs::Model) -> RepositoryResult<TestRun> {
    Ok(TestRun {
        id: model.id,
        summary: model.summary,
        report_to_user_id: model.report_to_user_id,
        default_tester_id: model.default_tester_id,
        project_id: model.project_id.unwrap_or(0),
        plan_id: model.plan_id,
        version: model.version,
        notes: model.notes,
        planned_start: model.planned_start.map(|d| d.date_naive()),
        planned_stop: model.planned_stop.map(|d| d.date_naive()),
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

fn validate_sort(sort: &str) -> RepositoryResult<(test_runs::Column, sea_orm::Order)> {
    let (col, order) = if let Some(stripped) = sort.strip_prefix('-') {
        (stripped, sea_orm::Order::Desc)
    } else {
        (sort, sea_orm::Order::Asc)
    };
    let c = match col {
        "id" => test_runs::Column::Id,
        "summary" => test_runs::Column::Summary,
        "updated_at" => test_runs::Column::UpdatedAt,
        "created_at" => test_runs::Column::CreatedAt,
        _ => {
            return Err(RepositoryError::Database(format!(
                "invalid sort value: {}",
                sort
            )));
        }
    };
    Ok((c, order))
}

#[async_trait]
impl TestRunRepository for SqlTestRunRepository {
    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<TestRun>> {
        test_runs::Entity::find_by_id(id)
            .filter(test_runs::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn list_by_project(
        &self,
        project_id: i64,
        page: u32,
        limit: u32,
        plan_id: Option<i64>,
        search: Option<&str>,
        sort: &str,
    ) -> RepositoryResult<(Vec<TestRunListItem>, u64)> {
        let (sort_col, sort_order) = validate_sort(sort)?;

        let mut select = test_runs::Entity::find()
            .filter(test_runs::Column::ProjectId.eq(project_id))
            .filter(test_runs::Column::DeletedAt.is_null());

        if let Some(pid) = plan_id {
            select = select.filter(test_runs::Column::PlanId.eq(pid));
        }
        if let Some(s) = search {
            let trimmed = s.trim();
            if !trimmed.is_empty() && trimmed.len() <= 255 {
                let escaped = escape_like(trimmed);
                select = select.filter(test_runs::Column::Summary.like(format!("%{}%", escaped)));
            }
        }

        let total = select
            .clone()
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let offset = ((page.saturating_sub(1)) * limit) as u64;
        let rows = select
            .order_by(sort_col, sort_order)
            .offset(offset)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut items = Vec::with_capacity(rows.len());
        for row in &rows {
            let case_count: i64 = test_run_cases::Entity::find()
                .filter(test_run_cases::Column::RunId.eq(row.id))
                .count(&self.db)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?
                as i64;
            items.push(TestRunListItem {
                id: row.id,
                summary: row.summary.clone(),
                report_to_user_id: row.report_to_user_id,
                default_tester_id: row.default_tester_id,
                project_id: row.project_id.unwrap_or(0),
                plan_id: row.plan_id,
                version: row.version.clone(),
                planned_start: row.planned_start.map(|d| d.date_naive()),
                planned_stop: row.planned_stop.map(|d| d.date_naive()),
                case_count,
                created_by: row.created_by,
                created_at: DateTime::<Utc>::from(row.created_at),
                updated_at: DateTime::<Utc>::from(row.updated_at),
            });
        }
        Ok((items, total))
    }

    async fn find_by_summary(
        &self,
        project_id: i64,
        summary: &str,
    ) -> RepositoryResult<Option<TestRun>> {
        use sea_orm::sea_query::Expr;
        test_runs::Entity::find()
            .filter(test_runs::Column::ProjectId.eq(project_id))
            .filter(test_runs::Column::DeletedAt.is_null())
            .filter(Expr::cust_with_values(
                "LOWER(summary) = LOWER($1)",
                [summary],
            ))
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn get_case_ids(&self, run_id: i64) -> RepositoryResult<Vec<i64>> {
        test_run_cases::Entity::find()
            .filter(test_run_cases::Column::RunId.eq(run_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))
            .map(|rows| rows.into_iter().map(|r| r.test_case_id).collect())
    }

    async fn get_execution_ids(&self, run_id: i64) -> RepositoryResult<Vec<i64>> {
        test_executions::Entity::find()
            .select_only()
            .column(test_executions::Column::Id)
            .filter(test_executions::Column::TestRunId.eq(run_id))
            .filter(test_executions::Column::DeletedAt.is_null())
            .into_tuple()
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))
    }

    async fn compute_statistics(&self, run_id: i64) -> RepositoryResult<TestRunStatistics> {
        // Count results from all non-deleted executions under this run.
        // SELECT result, COUNT(*) FROM test_case_results tcr
        // JOIN test_executions te ON tcr.execution_id = te.id
        // WHERE te.test_run_id = $1 AND te.deleted_at IS NULL AND tcr.deleted_at IS NULL
        // GROUP BY result

        let rows: Vec<(String, i64)> = test_case_results::Entity::find()
            .select_only()
            .column(test_case_results::Column::Result)
            .column_as(test_case_results::Column::Id.count(), "cnt")
            .join(
                sea_orm::JoinType::InnerJoin,
                test_case_results::Relation::TestExecution.def(),
            )
            .filter(test_executions::Column::TestRunId.eq(run_id))
            .filter(test_executions::Column::DeletedAt.is_null())
            .filter(test_case_results::Column::DeletedAt.is_null())
            .group_by(test_case_results::Column::Result)
            .into_tuple()
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut stats = TestRunStatistics {
            total: 0,
            not_tested: 0,
            in_progress: 0,
            pass: 0,
            fail: 0,
            warning: 0,
            ignore: 0,
        };
        for (result, count) in &rows {
            stats.total += count;
            match result.to_uppercase().as_str() {
                "NOT_TESTED" => stats.not_tested = *count,
                "IN_PROGRESS" => stats.in_progress = *count,
                "PASS" => stats.pass = *count,
                "FAIL" => stats.fail = *count,
                "WARNING" => stats.warning = *count,
                "IGNORE" => stats.ignore = *count,
                _ => {}
            }
        }
        Ok(stats)
    }

    async fn create(&self, run: &TestRun, case_ids: &[i64]) -> RepositoryResult<TestRun> {
        let run = run.clone();
        let cids = case_ids.to_vec();

        let result = self
            .db
            .transaction::<_, TestRun, RepositoryError>(|txn| {
                let run = run.clone();
                let cids = cids.clone();
                Box::pin(async move {
                    let inserted = test_runs::ActiveModel {
                        summary: Set(run.summary.clone()),
                        report_to_user_id: Set(run.report_to_user_id),
                        default_tester_id: Set(run.default_tester_id),
                        project_id: Set(Some(run.project_id)),
                        plan_id: Set(run.plan_id),
                        version: Set(run.version.clone()),
                        notes: Set(run.notes.clone()),
                        planned_start: Set(run.planned_start.map(|d| {
                            d.and_hms_opt(0, 0, 0)
                                .unwrap()
                                .and_local_timezone(Utc)
                                .earliest()
                                .unwrap()
                                .fixed_offset()
                        })),
                        planned_stop: Set(run.planned_stop.map(|d| {
                            d.and_hms_opt(0, 0, 0)
                                .unwrap()
                                .and_local_timezone(Utc)
                                .earliest()
                                .unwrap()
                                .fixed_offset()
                        })),
                        created_by: Set(run.created_by),
                        created_at: Set(run.created_at.fixed_offset()),
                        updated_by: Set(run.updated_by),
                        updated_at: Set(run.updated_at.fixed_offset()),
                        ..Default::default()
                    }
                    .insert(txn)
                    .await
                    .map_err(|e| {
                        let msg = e.to_string();
                        if msg.contains("duplicate key") {
                            RepositoryError::Duplicate("test run summary already exists".into())
                        } else {
                            RepositoryError::Database(msg)
                        }
                    })?;

                    for &cid in &cids {
                        test_run_cases::ActiveModel {
                            run_id: Set(inserted.id),
                            test_case_id: Set(cid),
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

    async fn update(&self, run: &TestRun, case_ids: Option<&[i64]>) -> RepositoryResult<TestRun> {
        let run = run.clone();
        let cids = case_ids.map(|v| v.to_vec());

        let result = self
            .db
            .transaction::<_, TestRun, RepositoryError>(|txn| {
                let run = run.clone();
                let cids = cids.clone();
                Box::pin(async move {
                    if let Some(ref new_ids) = cids {
                        test_run_cases::Entity::delete_many()
                            .filter(test_run_cases::Column::RunId.eq(run.id))
                            .exec(txn)
                            .await
                            .map_err(|e| RepositoryError::Database(e.to_string()))?;

                        for &cid in new_ids {
                            test_run_cases::ActiveModel {
                                run_id: Set(run.id),
                                test_case_id: Set(cid),
                                ..Default::default()
                            }
                            .insert(txn)
                            .await
                            .map_err(|e| RepositoryError::Database(e.to_string()))?;
                        }
                    }

                    let inserted = test_runs::ActiveModel {
                        id: Set(run.id),
                        summary: Set(run.summary.clone()),
                        report_to_user_id: Set(run.report_to_user_id),
                        default_tester_id: Set(run.default_tester_id),
                        project_id: Set(Some(run.project_id)),
                        plan_id: Set(run.plan_id),
                        version: Set(run.version.clone()),
                        notes: Set(run.notes.clone()),
                        planned_start: Set(run.planned_start.map(|d| {
                            d.and_hms_opt(0, 0, 0)
                                .unwrap()
                                .and_local_timezone(Utc)
                                .earliest()
                                .unwrap()
                                .fixed_offset()
                        })),
                        planned_stop: Set(run.planned_stop.map(|d| {
                            d.and_hms_opt(0, 0, 0)
                                .unwrap()
                                .and_local_timezone(Utc)
                                .earliest()
                                .unwrap()
                                .fixed_offset()
                        })),
                        updated_by: Set(run.updated_by),
                        updated_at: Set(run.updated_at.fixed_offset()),
                        ..Default::default()
                    }
                    .update(txn)
                    .await
                    .map_err(|e| {
                        let msg = e.to_string();
                        if msg.contains("duplicate key") {
                            RepositoryError::Duplicate("test run summary already exists".into())
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
        let result = test_runs::Entity::update_many()
            .filter(test_runs::Column::Id.eq(id))
            .filter(test_runs::Column::DeletedAt.is_null())
            .set(test_runs::ActiveModel {
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
    fn check<T: TestRunRepository>() {}
    check::<SqlTestRunRepository>();
}
