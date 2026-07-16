//! PostgreSQL implementation of [`AdminBypassRepository`].
//!
//! Checks whether a user holds a system-protected role (is_system = TRUE),
//! either directly or through group membership. Uses a single SQL query
//! with JOINs instead of multiple sequential round-trips.

use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseConnection, FromQueryResult, Statement};

use crate::application::repositories::admin_bypass_repository::AdminBypassRepository;
use crate::application::repositories::{RepositoryError, RepositoryResult};

/// PostgreSQL-backed [`AdminBypassRepository`].
pub struct SqlAdminBypassRepository {
    db: DatabaseConnection,
}

impl SqlAdminBypassRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

/// Helper struct to deserialize the EXISTS result.
#[derive(Debug, FromQueryResult)]
struct IsSystemAdmin {
    is_system_admin: bool,
}

#[async_trait]
impl AdminBypassRepository for SqlAdminBypassRepository {
    async fn is_system_admin(&self, user_id: i64) -> RepositoryResult<bool> {
        // Single query that checks for a system role via both direct and
        // group-inherited paths. The is_system column on roles replaces the
        // fragile case-sensitive name check.
        //
        // Returns TRUE if the user exists, is not soft-deleted, and has a
        // system-protected role (is_system = TRUE) either:
        //   1. Through direct role assignment (user_roles → roles), OR
        //   2. Through group membership (user_groups → groups → group_roles → roles).
        let result: Option<IsSystemAdmin> =
            IsSystemAdmin::find_by_statement(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                r#"
                SELECT EXISTS(
                    SELECT 1
                    FROM users u
                    LEFT JOIN user_roles ur ON ur.user_id = u.id
                    LEFT JOIN roles r_direct
                        ON r_direct.id = ur.role_id
                        AND r_direct.is_system = TRUE
                        AND r_direct.deleted_at IS NULL
                    LEFT JOIN user_groups ug ON ug.user_id = u.id
                    LEFT JOIN groups g
                        ON g.id = ug.group_id
                        AND g.deleted_at IS NULL
                    LEFT JOIN group_roles gr ON gr.group_id = g.id
                    LEFT JOIN roles r_group
                        ON r_group.id = gr.role_id
                        AND r_group.is_system = TRUE
                        AND r_group.deleted_at IS NULL
                    WHERE u.id = $1
                        AND u.deleted_at IS NULL
                        AND (
                            r_direct.id IS NOT NULL
                            OR r_group.id IS NOT NULL
                        )
                ) AS is_system_admin
                "#,
                [user_id.into()],
            ))
            .one(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(result.map(|r| r.is_system_admin).unwrap_or(false))
    }
}

#[allow(dead_code)]
fn assert_impl() {
    fn check<T: AdminBypassRepository>() {}
    check::<SqlAdminBypassRepository>();
}
