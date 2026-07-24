//! `SqlTestExecutionRepository` — PostgreSQL implementation of
//! [`TestExecutionRepository`].
//!
//! Maps the `test_executions` table to the [`TestExecution`] domain entity
//! using SeaORM.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};

use crate::application::repositories::test_execution_repository::{
    TestExecutionRepository, TesterInfoRow,
};
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::domain::entities::test_execution::TestExecution;
use crate::infrastructure::db::entities::{
    execution_testers, project_members, test_executions, test_runs, users,
};

/// PostgreSQL-backed [`TestExecutionRepository`].
pub struct SqlTestExecutionRepository {
    db: DatabaseConnection,
}

impl SqlTestExecutionRepository {
    /// Create a new repository bound to the given database connection.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

// ---------------------------------------------------------------------------
// Helper: SeaORM Model → domain Entity mapping
// ---------------------------------------------------------------------------

fn model_to_entity(model: test_executions::Model) -> RepositoryResult<TestExecution> {
    Ok(TestExecution {
        id: model.id,
        name: model.name,
        test_run_id: model.test_run_id,
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
impl TestExecutionRepository for SqlTestExecutionRepository {
    async fn create(
        &self,
        execution: &TestExecution,
        tester_ids: &[i64],
    ) -> RepositoryResult<TestExecution> {
        let tester_ids = tester_ids.to_vec();
        let execution = execution.clone();

        self.db
            .transaction::<_, TestExecution, RepositoryError>(|txn| {
                let execution = execution.clone();
                let tester_ids = tester_ids.clone();
                Box::pin(async move {
                    // 1. Insert execution.
                    let inserted = test_executions::ActiveModel {
                        name: Set(execution.name.clone()),
                        test_run_id: Set(execution.test_run_id),
                        created_by: Set(execution.created_by),
                        created_at: Set(execution.created_at.fixed_offset()),
                        updated_by: Set(execution.updated_by),
                        updated_at: Set(execution.updated_at.fixed_offset()),
                        ..Default::default()
                    }
                    .insert(txn)
                    .await
                    .map_err(|e| RepositoryError::Database(e.to_string()))?;

                    let execution_id = inserted.id;

                    // 2. Insert testers.
                    for &tester_id in &tester_ids {
                        execution_testers::ActiveModel {
                            execution_id: Set(execution_id),
                            user_id: Set(tester_id),
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
            .map_err(|e| RepositoryError::Database(e.to_string()))
    }

    async fn find_by_id(&self, id: i64) -> RepositoryResult<Option<TestExecution>> {
        test_executions::Entity::find_by_id(id)
            .filter(test_executions::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .map(model_to_entity)
            .transpose()
    }

    async fn update(&self, execution: &TestExecution) -> RepositoryResult<TestExecution> {
        // Read the current row to verify existence and preserve soft-delete state.
        let existing = test_executions::Entity::find_by_id(execution.id)
            .filter(test_executions::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .ok_or(RepositoryError::NotFound)?;

        test_executions::ActiveModel {
            id: Set(execution.id),
            name: Set(execution.name.clone()),
            test_run_id: Set(execution.test_run_id),
            updated_by: Set(execution.updated_by),
            // Preserve audit timestamps.
            created_by: Set(existing.created_by),
            created_at: Set(existing.created_at),
            // updated_at is handled by the trigger_set_updated_at trigger
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

    async fn update_with_testers(
        &self,
        execution: &TestExecution,
        tester_ids: &[i64],
    ) -> RepositoryResult<TestExecution> {
        let execution = execution.clone();
        let tester_ids = tester_ids.to_vec();

        self.db
            .transaction::<_, TestExecution, RepositoryError>(|txn| {
                let execution = execution.clone();
                let tester_ids = tester_ids.clone();
                Box::pin(async move {
                    // Read the current row to verify existence.
                    let existing = test_executions::Entity::find_by_id(execution.id)
                        .filter(test_executions::Column::DeletedAt.is_null())
                        .one(txn)
                        .await
                        .map_err(|e| RepositoryError::Database(e.to_string()))?
                        .ok_or(RepositoryError::NotFound)?;

                    // 1. Update execution row.
                    let updated = test_executions::ActiveModel {
                        id: Set(execution.id),
                        name: Set(execution.name.clone()),
                        test_run_id: Set(execution.test_run_id),
                        updated_by: Set(execution.updated_by),
                        created_by: Set(existing.created_by),
                        created_at: Set(existing.created_at),
                        deleted_at: Set(existing.deleted_at),
                        deleted_by: Set(existing.deleted_by),
                        ..Default::default()
                    }
                    .update(txn)
                    .await
                    .map_err(|e| RepositoryError::Database(e.to_string()))?;

                    // 2. Delete all existing testers.
                    execution_testers::Entity::delete_many()
                        .filter(execution_testers::Column::ExecutionId.eq(execution.id))
                        .exec(txn)
                        .await
                        .map_err(|e| RepositoryError::Database(e.to_string()))?;

                    // 3. Insert new testers.
                    for &tester_id in &tester_ids {
                        execution_testers::ActiveModel {
                            execution_id: Set(execution.id),
                            user_id: Set(tester_id),
                            ..Default::default()
                        }
                        .insert(txn)
                        .await
                        .map_err(|e| RepositoryError::Database(e.to_string()))?;
                    }

                    model_to_entity(updated)
                })
            })
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))
    }

    async fn soft_delete(&self, id: i64, deleted_by: i64) -> RepositoryResult<()> {
        let result = test_executions::Entity::update_many()
            .filter(test_executions::Column::Id.eq(id))
            .filter(test_executions::Column::DeletedAt.is_null())
            .set(test_executions::ActiveModel {
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

    async fn list(
        &self,
        page: u32,
        limit: u32,
        test_run_id: Option<i64>,
    ) -> RepositoryResult<(Vec<TestExecution>, u64)> {
        let page = page.max(1);
        let limit = limit.clamp(1, 100) as u64;
        let offset = (page as u64 - 1) * limit;

        let mut select = test_executions::Entity::find()
            .filter(test_executions::Column::DeletedAt.is_null())
            .order_by_desc(test_executions::Column::CreatedAt);

        if let Some(run_id) = test_run_id {
            select = select.filter(test_executions::Column::TestRunId.eq(run_id));
        }

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

    async fn list_by_user(
        &self,
        user_id: i64,
        page: u32,
        limit: u32,
        test_run_id: Option<i64>,
    ) -> RepositoryResult<(Vec<TestExecution>, u64)> {
        let page = page.max(1);
        let limit = limit.clamp(1, 100) as u64;
        let offset = (page as u64 - 1) * limit;

        // Collect accessible execution IDs.
        let mut accessible_ids: HashSet<i64> = HashSet::new();

        // 1. Created by user.
        for e in test_executions::Entity::find()
            .filter(test_executions::Column::DeletedAt.is_null())
            .filter(test_executions::Column::CreatedBy.eq(user_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
        {
            accessible_ids.insert(e.id);
        }

        // 2. Tester on execution.
        for et in execution_testers::Entity::find()
            .filter(execution_testers::Column::UserId.eq(user_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
        {
            accessible_ids.insert(et.execution_id);
        }

        // 3. Project member via test run's project.
        let project_ids: Vec<i64> = project_members::Entity::find()
            .filter(project_members::Column::UserId.eq(user_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            .into_iter()
            .map(|pm| pm.project_id)
            .collect();

        if !project_ids.is_empty() {
            // Get non-deleted run IDs for those projects.
            let run_ids: Vec<i64> = test_runs::Entity::find()
                .filter(test_runs::Column::DeletedAt.is_null())
                .filter(test_runs::Column::ProjectId.is_in(project_ids))
                .all(&self.db)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?
                .into_iter()
                .map(|tr| tr.id)
                .collect();

            if !run_ids.is_empty() {
                for e in test_executions::Entity::find()
                    .filter(test_executions::Column::DeletedAt.is_null())
                    .filter(test_executions::Column::TestRunId.is_in(run_ids))
                    .all(&self.db)
                    .await
                    .map_err(|e| RepositoryError::Database(e.to_string()))?
                {
                    accessible_ids.insert(e.id);
                }
            }
        }

        if accessible_ids.is_empty() {
            return Ok((vec![], 0));
        }

        let ids: Vec<i64> = accessible_ids.into_iter().collect();

        // Paginated query filtered by accessible IDs.
        let mut select = test_executions::Entity::find()
            .filter(test_executions::Column::DeletedAt.is_null())
            .filter(test_executions::Column::Id.is_in(ids))
            .order_by_desc(test_executions::Column::CreatedAt);

        if let Some(run_id) = test_run_id {
            select = select.filter(test_executions::Column::TestRunId.eq(run_id));
        }

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

    async fn user_can_access(&self, execution_id: i64, user_id: i64) -> RepositoryResult<bool> {
        // Access is granted if:
        // 1. User is the creator of the execution, OR
        // 2. User is a tester on the execution, OR
        // 3. User is a member of the project that owns the test run

        // Check 1 & 2: fetch the execution and check creator/tester membership.
        let execution = test_executions::Entity::find_by_id(execution_id)
            .filter(test_executions::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let execution = match execution {
            Some(e) => e,
            None => return Ok(false),
        };

        // Check 1: creator.
        if execution.created_by == user_id {
            return Ok(true);
        }

        // Check 2: tester.
        let is_tester = execution_testers::Entity::find()
            .filter(execution_testers::Column::ExecutionId.eq(execution_id))
            .filter(execution_testers::Column::UserId.eq(user_id))
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?
            > 0;

        if is_tester {
            return Ok(true);
        }

        // Check 3: member of the project that owns the test run (only non-deleted runs).
        let run = test_runs::Entity::find_by_id(execution.test_run_id)
            .filter(test_runs::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        if let Some(run) = run
            && let Some(project_id) = run.project_id
        {
            let is_member = project_members::Entity::find()
                .filter(project_members::Column::ProjectId.eq(project_id))
                .filter(project_members::Column::UserId.eq(user_id))
                .count(&self.db)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?
                > 0;

            if is_member {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn get_tester_ids(&self, execution_id: i64) -> RepositoryResult<Vec<i64>> {
        let testers = execution_testers::Entity::find()
            .filter(execution_testers::Column::ExecutionId.eq(execution_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(testers.into_iter().map(|t| t.user_id).collect())
    }

    async fn set_testers(&self, execution_id: i64, tester_ids: &[i64]) -> RepositoryResult<()> {
        let tester_ids = tester_ids.to_vec();

        self.db
            .transaction::<_, (), RepositoryError>(|txn| {
                let tester_ids = tester_ids.clone();
                Box::pin(async move {
                    // Delete all existing testers.
                    execution_testers::Entity::delete_many()
                        .filter(execution_testers::Column::ExecutionId.eq(execution_id))
                        .exec(txn)
                        .await
                        .map_err(|e| RepositoryError::Database(e.to_string()))?;

                    // Insert new testers.
                    for &tester_id in &tester_ids {
                        execution_testers::ActiveModel {
                            execution_id: Set(execution_id),
                            user_id: Set(tester_id),
                            ..Default::default()
                        }
                        .insert(txn)
                        .await
                        .map_err(|e| RepositoryError::Database(e.to_string()))?;
                    }

                    Ok(())
                })
            })
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))
    }

    async fn get_tester_counts(
        &self,
        execution_ids: &[i64],
    ) -> RepositoryResult<HashMap<i64, u64>> {
        if execution_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = execution_testers::Entity::find()
            .filter(execution_testers::Column::ExecutionId.is_in(execution_ids.to_vec()))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut counts: HashMap<i64, u64> = HashMap::new();
        for row in rows {
            *counts.entry(row.execution_id).or_default() += 1;
        }

        Ok(counts)
    }

    async fn get_tester_info(&self, execution_id: i64) -> RepositoryResult<Vec<TesterInfoRow>> {
        let testers = execution_testers::Entity::find()
            .filter(execution_testers::Column::ExecutionId.eq(execution_id))
            .all(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut result = Vec::new();
        for t in testers {
            if let Some(user) = users::Entity::find_by_id(t.user_id)
                .one(&self.db)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?
            {
                result.push(TesterInfoRow {
                    user_id: user.id,
                    username: user.username,
                    fullname: user.fullname,
                });
            }
        }

        Ok(result)
    }

    async fn test_run_exists(&self, run_id: i64) -> RepositoryResult<bool> {
        let count = test_runs::Entity::find_by_id(run_id)
            .filter(test_runs::Column::DeletedAt.is_null())
            .count(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(count > 0)
    }
}

// ---------------------------------------------------------------------------
// Compile-time check: SqlTestExecutionRepository satisfies TestExecutionRepository
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: TestExecutionRepository>() {}
    check::<SqlTestExecutionRepository>();
}
