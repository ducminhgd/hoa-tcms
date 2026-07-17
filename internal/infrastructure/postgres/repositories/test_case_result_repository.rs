//! `SqlTestCaseResultRepository` — PostgreSQL implementation of
//! [`TestCaseResultRepository`].
//!
//! Maps the `test_case_results` table to the [`TestCaseResult`] domain entity,
//! and reads from `test_cases` and `test_run_cases` for import operations.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};

use crate::application::repositories::test_case_result_repository::{
    TestCaseImportData, TestCaseResultRepository,
};
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::domain::entities::test_case_result::TestCaseResult;
use crate::domain::value_objects::test_result_status::TestResultStatus;
use crate::infrastructure::db::entities::{test_case_results, test_cases, test_run_cases};

/// PostgreSQL-backed [`TestCaseResultRepository`].
pub struct SqlTestCaseResultRepository {
    db: DatabaseConnection,
}

impl SqlTestCaseResultRepository {
    /// Create a new repository bound to the given database connection.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

// ---------------------------------------------------------------------------
// Helper: SeaORM Model → domain Entity mapping
// ---------------------------------------------------------------------------

fn model_to_entity(model: test_case_results::Model) -> RepositoryResult<TestCaseResult> {
    let result = TestResultStatus::parse(&model.result).ok_or_else(|| {
        RepositoryError::Database(format!(
            "invalid result status '{}' for test case result {}",
            model.result, model.id
        ))
    })?;

    Ok(TestCaseResult {
        id: model.id,
        execution_id: model.execution_id,
        test_case_id: model.test_case_id,
        summary: model.summary,
        description: model.description,
        priority: model.priority,
        result,
        logs: model.logs,
        tested_by: model.tested_by,
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
impl TestCaseResultRepository for SqlTestCaseResultRepository {
    async fn create(&self, result: &TestCaseResult) -> RepositoryResult<TestCaseResult> {
        test_case_results::ActiveModel {
            execution_id: Set(result.execution_id),
            test_case_id: Set(result.test_case_id),
            summary: Set(result.summary.clone()),
            description: Set(result.description.clone()),
            priority: Set(result.priority.clone()),
            result: Set(result.result.to_string()),
            logs: Set(result.logs.clone()),
            tested_by: Set(result.tested_by),
            created_by: Set(result.created_by),
            created_at: Set(result.created_at.fixed_offset()),
            updated_by: Set(result.updated_by),
            updated_at: Set(result.updated_at.fixed_offset()),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))
        .and_then(model_to_entity)
    }

    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<TestCaseResult>> {
        test_case_results::Entity::find_by_id(id)
            .filter(test_case_results::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn update(&self, result: &TestCaseResult) -> RepositoryResult<TestCaseResult> {
        // Read the current row to verify existence and preserve soft-delete state.
        let existing = test_case_results::Entity::find_by_id(result.id)
            .filter(test_case_results::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .ok_or(RepositoryError::NotFound)?;

        test_case_results::ActiveModel {
            id: Set(result.id),
            execution_id: Set(result.execution_id),
            test_case_id: Set(result.test_case_id),
            summary: Set(result.summary.clone()),
            description: Set(result.description.clone()),
            priority: Set(result.priority.clone()),
            result: Set(result.result.to_string()),
            logs: Set(result.logs.clone()),
            tested_by: Set(result.tested_by),
            updated_by: Set(result.updated_by),
            // Preserve audit timestamps.
            created_by: Set(existing.created_by),
            created_at: Set(existing.created_at),
            // Preserve soft-delete state.
            deleted_at: Set(existing.deleted_at),
            deleted_by: Set(existing.deleted_by),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))
        .and_then(model_to_entity)
    }

    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()> {
        let result = test_case_results::Entity::update_many()
            .filter(test_case_results::Column::Id.eq(id))
            .filter(test_case_results::Column::DeletedAt.is_null())
            .set(test_case_results::ActiveModel {
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

    async fn list_by_execution(
        &self,
        execution_id: i64,
        page: u32,
        limit: u32,
    ) -> RepositoryResult<(Vec<TestCaseResult>, u64)> {
        let page = page.max(1);
        let limit = limit.clamp(1, 100) as u64;
        let offset = (page as u64 - 1) * limit;

        let select = test_case_results::Entity::find()
            .filter(test_case_results::Column::ExecutionId.eq(execution_id))
            .filter(test_case_results::Column::DeletedAt.is_null())
            .order_by_desc(test_case_results::Column::CreatedAt);

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

    async fn find_by_execution_and_test_case(
        &self,
        execution_id: i64,
        test_case_id: i64,
    ) -> RepositoryResult<Option<TestCaseResult>> {
        test_case_results::Entity::find()
            .filter(test_case_results::Column::ExecutionId.eq(execution_id))
            .filter(test_case_results::Column::TestCaseId.eq(test_case_id))
            .filter(test_case_results::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn test_case_in_run(&self, run_id: i64, test_case_id: i64) -> RepositoryResult<bool> {
        let count = test_run_cases::Entity::find()
            .filter(test_run_cases::Column::RunId.eq(run_id))
            .filter(test_run_cases::Column::TestCaseId.eq(test_case_id))
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(count > 0)
    }

    async fn get_test_case_for_import(
        &self,
        test_case_id: i64,
    ) -> RepositoryResult<Option<TestCaseImportData>> {
        let tc = test_cases::Entity::find_by_id(test_case_id)
            .filter(test_cases::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        match tc {
            Some(tc) => Ok(Some(TestCaseImportData {
                test_case_id: tc.id,
                summary: tc.summary,
                description: tc.description,
                priority: tc.priority,
            })),
            None => Ok(None),
        }
    }

    async fn list_test_cases_in_run(
        &self,
        run_id: i64,
        page: u32,
        limit: u32,
    ) -> RepositoryResult<(Vec<TestCaseImportData>, u64)> {
        let page = page.max(1);
        let limit = limit.clamp(1, 100) as u64;
        let offset = (page as u64 - 1) * limit;

        // Count total test cases in the run.
        let count = test_run_cases::Entity::find()
            .filter(test_run_cases::Column::RunId.eq(run_id))
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        // Fetch paginated test cases via the junction table join.
        let rows = test_run_cases::Entity::find()
            .filter(test_run_cases::Column::RunId.eq(run_id))
            .offset(offset)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        // Collect all case IDs and fetch the test cases in a single query.
        let case_ids: Vec<i64> = rows.iter().map(|r| r.test_case_id).collect();

        // Build a HashMap for O(1) lookup.
        let tc_map: std::collections::HashMap<i64, test_cases::Model> = if case_ids.is_empty() {
            std::collections::HashMap::new()
        } else {
            test_cases::Entity::find()
                .filter(test_cases::Column::Id.is_in(case_ids.clone()))
                .filter(test_cases::Column::DeletedAt.is_null())
                .all(&self.db)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?
                .into_iter()
                .map(|tc| (tc.id, tc))
                .collect()
        };

        let mut result = Vec::new();
        for row in rows {
            if let Some(tc) = tc_map.get(&row.test_case_id) {
                result.push(TestCaseImportData {
                    test_case_id: tc.id,
                    summary: tc.summary.clone(),
                    description: tc.description.clone(),
                    priority: tc.priority.clone(),
                });
            }
        }

        Ok((result, count))
    }

    async fn batch_import_cases(
        &self,
        execution_id: i64,
        user_id: i64,
        case_ids: &[i64],
    ) -> RepositoryResult<(u32, u32)> {
        // Fetch all test case snapshots in one query.
        let snapshots: std::collections::HashMap<i64, test_cases::Model> =
            test_cases::Entity::find()
                .filter(test_cases::Column::Id.is_in(case_ids.to_vec()))
                .filter(test_cases::Column::DeletedAt.is_null())
                .all(&self.db)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?
                .into_iter()
                .map(|tc| (tc.id, tc))
                .collect();

        // Fetch all existing results for this execution + cases.
        let existing_results: std::collections::HashMap<i64, test_case_results::Model> =
            test_case_results::Entity::find()
                .filter(test_case_results::Column::ExecutionId.eq(execution_id))
                .filter(test_case_results::Column::TestCaseId.is_in(case_ids.to_vec()))
                .filter(test_case_results::Column::DeletedAt.is_null())
                .all(&self.db)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?
                .into_iter()
                .map(|m| (m.test_case_id, m))
                .collect();

        let now = Utc::now().fixed_offset();
        let case_ids = case_ids.to_vec();

        // Perform the batch import in a single transaction.
        self.db
            .transaction::<_, (u32, u32), RepositoryError>(|txn| {
                let case_ids = case_ids.clone();
                Box::pin(async move {
                    let mut imported = 0u32;
                    let mut refreshed = 0u32;

                    for &case_id in &case_ids {
                        let snapshot = match snapshots.get(&case_id) {
                            Some(s) => s,
                            None => continue, // skip non-existent or soft-deleted cases
                        };

                        if let Some(existing) = existing_results.get(&case_id) {
                            // Case already imported — refresh snapshot fields,
                            // preserving result, logs, and tested_by.
                            let am = test_case_results::ActiveModel {
                                id: Set(existing.id),
                                execution_id: Set(existing.execution_id),
                                test_case_id: Set(existing.test_case_id),
                                summary: Set(snapshot.summary.clone()),
                                description: Set(snapshot.description.clone()),
                                priority: Set(snapshot.priority.clone()),
                                result: Set(existing.result.clone()),
                                logs: Set(existing.logs.clone()),
                                tested_by: Set(existing.tested_by),
                                updated_by: Set(user_id),
                                created_by: Set(existing.created_by),
                                created_at: Set(existing.created_at),
                                // Preserve soft-delete state.
                                deleted_at: Set(existing.deleted_at),
                                deleted_by: Set(existing.deleted_by),
                                ..Default::default()
                            };
                            am.update(txn)
                                .await
                                .map_err(|e| RepositoryError::Database(e.to_string()))?;
                            refreshed += 1;
                        } else {
                            // New import.
                            let am = test_case_results::ActiveModel {
                                execution_id: Set(execution_id),
                                test_case_id: Set(case_id),
                                summary: Set(snapshot.summary.clone()),
                                description: Set(snapshot.description.clone()),
                                priority: Set(snapshot.priority.clone()),
                                result: Set("NOT_TESTED".to_string()),
                                logs: Set(None),
                                tested_by: Set(None),
                                created_by: Set(user_id),
                                created_at: Set(now),
                                updated_by: Set(user_id),
                                updated_at: Set(now),
                                ..Default::default()
                            };
                            am.insert(txn)
                                .await
                                .map_err(|e| RepositoryError::Database(e.to_string()))?;
                            imported += 1;
                        }
                    }

                    Ok((imported, refreshed))
                })
            })
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Compile-time check
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: TestCaseResultRepository>() {}
    check::<SqlTestCaseResultRepository>();
}
