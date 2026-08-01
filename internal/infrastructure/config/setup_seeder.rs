//! `ConfigFileSetupSeeder` — seeds global reference data from a YAML config.
//!
//! Loads [`SetupConfig`] from a YAML file (default: `config/default-setup.yaml`)
//! and upserts permissions, system groups, built-in roles, and role-permission
//! associations into the database.
//!
//! All operations are idempotent — the seeder checks for existing rows before
//! inserting and skips already-present data. Safe to call on every server start.
//!
//! Uses raw SQL via `sqlx` to avoid chrono type-mapping issues with SeaORM's
//! `DateTime` → `TIMESTAMP` vs PostgreSQL's `TIMESTAMPTZ`.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::HashSet;

use crate::application::repositories::setup_seeder::SetupSeeder;
use crate::application::repositories::{RepositoryError, RepositoryResult};

// ---------------------------------------------------------------------------
// YAML configuration structures
// ---------------------------------------------------------------------------

/// Root of `config/default-setup.yaml`.
#[derive(Debug, Clone, Deserialize)]
pub struct SetupConfig {
    #[serde(default)]
    pub permissions: Vec<PermissionEntry>,
    #[serde(default)]
    pub groups: Vec<GroupEntry>,
    #[serde(default)]
    pub roles: Vec<RoleEntry>,
}

/// A single permission to seed.
#[derive(Debug, Clone, Deserialize)]
pub struct PermissionEntry {
    pub code: String,
    pub name: String,
}

/// A system group to seed.
#[derive(Debug, Clone, Deserialize)]
pub struct GroupEntry {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Role names to assign to this group via group_roles.
    #[serde(default)]
    pub roles: Vec<String>,
}

/// A built-in role with its permission assignments.
///
/// Permission entries support two glob-like patterns:
/// - `"*"` — every seeded permission.
/// - `"resource:*"` — all permissions whose code starts with `resource:`.
#[derive(Debug, Clone, Deserialize)]
pub struct RoleEntry {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_system: bool,
    pub permissions: Vec<String>,
}

// ---------------------------------------------------------------------------
// Seeder
// ---------------------------------------------------------------------------

/// Seeds global reference data from a YAML configuration file.
pub struct ConfigFileSetupSeeder {
    pool: PgPool,
    config: SetupConfig,
}

impl ConfigFileSetupSeeder {
    /// Create a new seeder with an already-parsed configuration.
    /// Borrows the underlying [`PgPool`] from the SeaORM connection.
    pub fn new(db: &DatabaseConnection, config: SetupConfig) -> Self {
        // Clone the underlying sqlx::PgPool to run raw SQL and avoid
        // chrono type-mapping issues with SeaORM's DateTime type.
        let pool = db.get_postgres_connection_pool().clone();
        Self { pool, config }
    }

    /// Load and parse the setup configuration from a YAML file.
    pub fn load_config(path: &str) -> Result<SetupConfig, String> {
        let contents =
            std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        serde_yaml::from_str::<SetupConfig>(&contents)
            .map_err(|e| format!("invalid YAML in {path}: {e}"))
    }

    /// Return an empty configuration (used as a fallback when the config file
    /// cannot be loaded).
    pub fn empty_config() -> SetupConfig {
        SetupConfig {
            permissions: Vec::new(),
            groups: Vec::new(),
            roles: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Permission seeding
    // -----------------------------------------------------------------------

    async fn seed_permissions(&self) -> RepositoryResult<HashSet<String>> {
        let mut seeded_codes = HashSet::new();

        for entry in &self.config.permissions {
            sqlx::query(
                "INSERT INTO permissions (name, code) VALUES ($1, $2) ON CONFLICT (code) DO NOTHING",
            )
            .bind(&entry.name)
            .bind(&entry.code)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

            seeded_codes.insert(entry.code.clone());
        }

        Ok(seeded_codes)
    }

    // -----------------------------------------------------------------------
    // Group seeding
    // -----------------------------------------------------------------------

    async fn seed_groups(&self) -> RepositoryResult<()> {
        for entry in &self.config.groups {
            sqlx::query(
                r#"INSERT INTO groups (name, description, status, created_by, updated_by)
                   VALUES ($1, $2, 'ACTIVE', NULL, NULL)
                   ON CONFLICT (name) DO NOTHING"#,
            )
            .bind(&entry.name)
            .bind(&entry.description)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Group-role assignments
    // -----------------------------------------------------------------------

    async fn seed_group_roles(&self) -> RepositoryResult<()> {
        for entry in &self.config.groups {
            if entry.roles.is_empty() {
                continue;
            }

            let (group_id,): (i64,) =
                sqlx::query_as("SELECT id FROM groups WHERE name = $1 AND deleted_at IS NULL")
                    .bind(&entry.name)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|e| RepositoryError::Database(e.to_string()))?;

            for role_name in &entry.roles {
                let (role_id,): (i64,) =
                    sqlx::query_as("SELECT id FROM roles WHERE name = $1 AND deleted_at IS NULL")
                        .bind(role_name)
                        .fetch_one(&self.pool)
                        .await
                        .map_err(|e| RepositoryError::Database(e.to_string()))?;

                sqlx::query(
                    "INSERT INTO group_roles (group_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                )
                .bind(group_id)
                .bind(role_id)
                .execute(&self.pool)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?;
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Role seeding
    // -----------------------------------------------------------------------

    async fn seed_roles(&self, seeded_codes: &HashSet<String>) -> RepositoryResult<()> {
        for entry in &self.config.roles {
            // Upsert the role. The ON CONFLICT DO UPDATE fires the
            // trigger_set_updated_at trigger. Migration 010 fixes
            // the trigger so COALESCE(TEXT, BIGINT) works correctly.
            sqlx::query(
                r#"INSERT INTO roles (name, description, status, is_system, created_by, updated_by)
                   VALUES ($1, $2, 'ACTIVE', $3, NULL, NULL)
                   ON CONFLICT (name) DO UPDATE SET description = EXCLUDED.description"#,
            )
            .bind(&entry.name)
            .bind(&entry.description)
            .bind(entry.is_system)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

            // Get the role ID.
            let role_id: (i64,) =
                sqlx::query_as("SELECT id FROM roles WHERE name = $1 AND deleted_at IS NULL")
                    .bind(&entry.name)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|e| RepositoryError::Database(e.to_string()))?;
            let role_id = role_id.0;

            // Resolve permission IDs from the patterns.
            let desired_perm_ids = self
                .resolve_permission_ids(seeded_codes, &entry.permissions)
                .await?;

            if desired_perm_ids.is_empty() {
                continue;
            }

            // Diff: only insert missing role_permission rows.
            let existing: HashSet<i64> =
                sqlx::query_as("SELECT permission_id FROM role_permissions WHERE role_id = $1")
                    .bind(role_id)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| RepositoryError::Database(e.to_string()))?
                    .into_iter()
                    .map(|(pid,): (i64,)| pid)
                    .collect();

            for perm_id in &desired_perm_ids {
                if !existing.contains(perm_id) {
                    sqlx::query(
                        "INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)",
                    )
                    .bind(role_id)
                    .bind(*perm_id)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| RepositoryError::Database(e.to_string()))?;
                }
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Glob expansion
    // -----------------------------------------------------------------------

    /// Expand a list of permission patterns into concrete permission IDs.
    async fn resolve_permission_ids(
        &self,
        seeded_codes: &HashSet<String>,
        patterns: &[String],
    ) -> RepositoryResult<Vec<i64>> {
        let mut expanded_codes = Vec::new();

        for pattern in patterns {
            if pattern == "*" {
                expanded_codes.extend(seeded_codes.iter().cloned());
            } else if let Some(prefix) = pattern.strip_suffix('*') {
                expanded_codes.extend(
                    seeded_codes
                        .iter()
                        .filter(|c| c.starts_with(prefix))
                        .cloned(),
                );
            } else {
                expanded_codes.push(pattern.clone());
            }
        }

        if expanded_codes.is_empty() {
            return Ok(Vec::new());
        }

        // Resolve codes to IDs via raw SQL.
        let codes: Vec<&str> = expanded_codes.iter().map(|s| s.as_str()).collect();
        let placeholders: Vec<String> = codes
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect();

        let query_str = format!(
            "SELECT id FROM permissions WHERE code IN ({})",
            placeholders.join(", ")
        );

        let mut query = sqlx::query_as(&query_str);
        for code in &codes {
            query = query.bind(code);
        }

        let rows: Vec<(i64,)> = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl SetupSeeder for ConfigFileSetupSeeder {
    async fn seed(&self) -> RepositoryResult<()> {
        // Phase 1 — Permissions.
        tracing::info!("seeding permissions...");
        let seeded_codes = self.seed_permissions().await?;
        tracing::info!(count = seeded_codes.len(), "permissions seeded");

        // Phase 2 — Groups.
        tracing::info!("seeding groups...");
        self.seed_groups().await?;
        tracing::info!("groups seeded");

        // Phase 3 — Roles and role-permission assignments.
        tracing::info!("seeding roles...");
        self.seed_roles(&seeded_codes).await?;
        tracing::info!("roles seeded");

        // Phase 4 — Group-role assignments.
        tracing::info!("seeding group-role assignments...");
        self.seed_group_roles().await?;
        tracing::info!("group-role assignments seeded");

        tracing::info!(
            permissions = self.config.permissions.len(),
            groups = self.config.groups.len(),
            roles = self.config.roles.len(),
            "seeded global reference data",
        );

        Ok(())
    }
}
