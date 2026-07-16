-- ==========================================================================
-- Migration: 001_initial_schema
-- Description: Initial database schema for HOA TCMS
--
-- Creates all 14 entity tables, 9 junction tables, indexes, and a
-- trigger function for automatic updated_at/updated_by maintenance.
-- ==========================================================================

BEGIN;

-- ============================================================================
-- TRIGGER FUNCTION: trigger_set_updated_at
-- ============================================================================
-- Automatically sets updated_at and updated_by on row update.
-- The application MUST execute:
--   SET LOCAL app.current_user_id = <id>;
-- within each transaction that performs writes. If the session parameter
-- is not set, updated_by falls back to the existing row value.

CREATE OR REPLACE FUNCTION trigger_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    NEW.updated_by = COALESCE(
        NULLIF(current_setting('app.current_user_id', true), ''),
        NEW.updated_by
    )::BIGINT;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE: users
-- ============================================================================

CREATE TABLE users (
    id              BIGSERIAL       PRIMARY KEY,
    username        VARCHAR(100)    NOT NULL,
    email           VARCHAR(255)    NOT NULL,
    password_hash   VARCHAR(255)    NOT NULL,
    fullname        VARCHAR(255)    NOT NULL,
    status          VARCHAR(20)     NOT NULL DEFAULT 'ACTIVE',
    created_by      BIGINT,                         -- nullable because CLI bootstrap admin has no creator; FK omitted intentionally (self-referencing cycle)
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by      BIGINT          REFERENCES users(id),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by      BIGINT          REFERENCES users(id),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT uq_users_username UNIQUE (username),
    CONSTRAINT uq_users_email UNIQUE (email),
    CONSTRAINT chk_users_status CHECK (status IN ('ACTIVE', 'INACTIVE'))
);

COMMENT ON COLUMN users.created_by IS 'Nullable: bootstrap admin is created via CLI with no creator. FK omitted to avoid self-referencing cycle.';
COMMENT ON COLUMN users.updated_by IS 'Nullable because the bootstrap admin row is inserted with updated_by = NULL (no existing user to reference). The application sets updated_by explicitly for all subsequent user operations.';

-- NOTE: The trigger_set_updated_at trigger does NOT apply to `users` due to
-- the chicken-and-egg problem with the self-referencing FK. The application
-- sets updated_at and updated_by explicitly for user operations. The bootstrap
-- admin is created with both created_by and updated_by as NULL.

-- ============================================================================
-- TABLE: groups
-- ============================================================================

CREATE TABLE groups (
    id              BIGSERIAL       PRIMARY KEY,
    name            VARCHAR(255)    NOT NULL,
    description     TEXT,
    status          VARCHAR(20)     NOT NULL DEFAULT 'ACTIVE',
    created_by      BIGINT          NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by      BIGINT          NOT NULL REFERENCES users(id),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by      BIGINT          REFERENCES users(id),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT uq_groups_name UNIQUE (name),
    CONSTRAINT chk_groups_status CHECK (status IN ('ACTIVE', 'INACTIVE'))
);

CREATE TRIGGER trg_groups_set_updated_at
    BEFORE UPDATE ON groups
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_updated_at();

-- ============================================================================
-- TABLE: roles
-- ============================================================================

CREATE TABLE roles (
    id              BIGSERIAL       PRIMARY KEY,
    name            VARCHAR(255)    NOT NULL,
    status          VARCHAR(20)     NOT NULL DEFAULT 'ACTIVE',
    created_by      BIGINT          NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by      BIGINT          NOT NULL REFERENCES users(id),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by      BIGINT          REFERENCES users(id),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT uq_roles_name UNIQUE (name),
    CONSTRAINT chk_roles_status CHECK (status IN ('ACTIVE', 'INACTIVE'))
);

CREATE TRIGGER trg_roles_set_updated_at
    BEFORE UPDATE ON roles
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_updated_at();

-- ============================================================================
-- TABLE: permissions  (read-only reference table)
-- ============================================================================
-- Seeded via migration, never modified through application UI.
-- Only created_at is present — no audit pairs, no soft-delete.

CREATE TABLE permissions (
    id              BIGSERIAL       PRIMARY KEY,
    name            VARCHAR(255)    NOT NULL,
    code            VARCHAR(100)    NOT NULL,
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_permissions_code UNIQUE (code)
);

COMMENT ON TABLE permissions IS 'Read-only reference table (seeded, never modified via application UI). Only created_at is present — no audit pairs, no soft-delete.';

-- ============================================================================
-- TABLE: projects
-- ============================================================================

CREATE TABLE projects (
    id              BIGSERIAL       PRIMARY KEY,
    name            VARCHAR(255)    NOT NULL,
    description     TEXT,
    status          VARCHAR(20)     NOT NULL DEFAULT 'ACTIVE',
    created_by      BIGINT          NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by      BIGINT          NOT NULL REFERENCES users(id),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by      BIGINT          REFERENCES users(id),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT chk_projects_status CHECK (status IN ('ACTIVE', 'INACTIVE'))
);

CREATE TRIGGER trg_projects_set_updated_at
    BEFORE UPDATE ON projects
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_updated_at();

-- ============================================================================
-- TABLE: test_categories
-- ============================================================================

CREATE TABLE test_categories (
    id              BIGSERIAL       PRIMARY KEY,
    project_id      BIGINT          NOT NULL REFERENCES projects(id),
    name            VARCHAR(255)    NOT NULL,
    description     TEXT,
    created_by      BIGINT          NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by      BIGINT          NOT NULL REFERENCES users(id),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by      BIGINT          REFERENCES users(id),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT uq_test_categories_project_name UNIQUE (project_id, name)
);

CREATE TRIGGER trg_test_categories_set_updated_at
    BEFORE UPDATE ON test_categories
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_updated_at();

-- ============================================================================
-- TABLE: test_case_templates
-- ============================================================================

CREATE TABLE test_case_templates (
    id                  BIGSERIAL       PRIMARY KEY,
    project_id          BIGINT          NOT NULL REFERENCES projects(id),
    name                VARCHAR(255)    NOT NULL,
    template_content    TEXT,
    created_by          BIGINT          NOT NULL REFERENCES users(id),
    created_at          TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by          BIGINT          NOT NULL REFERENCES users(id),
    updated_at          TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by          BIGINT          REFERENCES users(id),
    deleted_at          TIMESTAMPTZ,

    CONSTRAINT uq_test_case_templates_project_name UNIQUE (project_id, name)
);

CREATE TRIGGER trg_test_case_templates_set_updated_at
    BEFORE UPDATE ON test_case_templates
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_updated_at();

-- ============================================================================
-- TABLE: test_cases
-- ============================================================================

CREATE TABLE test_cases (
    id              BIGSERIAL       PRIMARY KEY,
    summary         VARCHAR(500)    NOT NULL,
    project_id      BIGINT          REFERENCES projects(id),   -- nullable because test cases can be orphaned (no project)
    priority        VARCHAR(20)     NOT NULL DEFAULT 'MEDIUM',
    is_automated    BOOLEAN         NOT NULL DEFAULT FALSE,
    description     TEXT,
    notes           TEXT,
    script          VARCHAR(255),                               -- reserved for Phase 2 automation scripts
    arguments       TEXT,                                       -- reserved for Phase 2 automation arguments
    created_by      BIGINT          NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by      BIGINT          NOT NULL REFERENCES users(id),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by      BIGINT          REFERENCES users(id),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT chk_test_cases_priority CHECK (priority IN ('HIGHEST', 'HIGH', 'MEDIUM', 'LOW', 'LOWEST'))
);

COMMENT ON COLUMN test_cases.project_id IS 'Nullable because test cases can be orphaned (no project). Orphaned test cases are visible only to their creator and explicitly shared users.';

CREATE TRIGGER trg_test_cases_set_updated_at
    BEFORE UPDATE ON test_cases
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_updated_at();

-- ============================================================================
-- TABLE: test_plans
-- ============================================================================

CREATE TABLE test_plans (
    id              BIGSERIAL       PRIMARY KEY,
    name            VARCHAR(255)    NOT NULL,
    version         VARCHAR(50)     NOT NULL DEFAULT 'UNSPECIFIED',
    types           JSONB           NOT NULL DEFAULT '[]'::jsonb,
    description     TEXT,
    status          VARCHAR(20)     NOT NULL DEFAULT 'TO_DO',
    created_by      BIGINT          NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by      BIGINT          NOT NULL REFERENCES users(id),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by      BIGINT          REFERENCES users(id),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT chk_test_plans_status CHECK (status IN ('TO_DO', 'IN_PROGRESS', 'DONE', 'CANCEL'))
);

CREATE TRIGGER trg_test_plans_set_updated_at
    BEFORE UPDATE ON test_plans
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_updated_at();

-- ============================================================================
-- TABLE: test_runs
-- ============================================================================

CREATE TABLE test_runs (
    id                  BIGSERIAL       PRIMARY KEY,
    summary             VARCHAR(500)    NOT NULL,
    report_to_user_id   BIGINT          REFERENCES users(id),      -- reserved for Phase 3 notifications
    default_tester_id   BIGINT          NOT NULL REFERENCES users(id),
    project_id          BIGINT          REFERENCES projects(id),   -- nullable: run can be cross-project
    plan_id             BIGINT          REFERENCES test_plans(id), -- nullable: ad-hoc run without a plan
    version             VARCHAR(50),
    notes               TEXT,
    planned_start       TIMESTAMPTZ,
    planned_stop        TIMESTAMPTZ,
    created_by          BIGINT          NOT NULL REFERENCES users(id),
    created_at          TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by          BIGINT          NOT NULL REFERENCES users(id),
    updated_at          TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by          BIGINT          REFERENCES users(id),
    deleted_at          TIMESTAMPTZ,

    CONSTRAINT chk_test_runs_planned CHECK (
        planned_stop IS NULL OR planned_start IS NULL OR planned_stop > planned_start
    )
);

COMMENT ON COLUMN test_runs.project_id IS 'Nullable because a run can be cross-project.';
COMMENT ON COLUMN test_runs.plan_id IS 'Nullable because a run can be ad-hoc without a plan.';

CREATE TRIGGER trg_test_runs_set_updated_at
    BEFORE UPDATE ON test_runs
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_updated_at();

-- ============================================================================
-- TABLE: test_executions
-- ============================================================================

CREATE TABLE test_executions (
    id              BIGSERIAL       PRIMARY KEY,
    name            VARCHAR(255)    NOT NULL,
    test_run_id     BIGINT          NOT NULL REFERENCES test_runs(id),
    created_by      BIGINT          NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by      BIGINT          NOT NULL REFERENCES users(id),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by      BIGINT          REFERENCES users(id),
    deleted_at      TIMESTAMPTZ
);

CREATE TRIGGER trg_test_executions_set_updated_at
    BEFORE UPDATE ON test_executions
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_updated_at();

-- ============================================================================
-- TABLE: test_case_results
-- ============================================================================
-- summary, description, priority are snapshots cloned from the test_case
-- at import time. Re-import updates these snapshot fields but preserves
-- result, logs, and attached files.
-- tested_by is set once when the result first transitions away from
-- NOT_TESTED, and is never modified thereafter.

CREATE TABLE test_case_results (
    id              BIGSERIAL       PRIMARY KEY,
    execution_id    BIGINT          NOT NULL REFERENCES test_executions(id),
    test_case_id    BIGINT          NOT NULL REFERENCES test_cases(id),
    summary         VARCHAR(500)    NOT NULL,      -- snapshot from test case at import time
    description     TEXT,                           -- snapshot from test case at import time
    priority        VARCHAR(20)     NOT NULL DEFAULT 'MEDIUM',  -- snapshot from test case at import time
    result          VARCHAR(20)     NOT NULL DEFAULT 'NOT_TESTED',
    logs            TEXT,
    tested_by       BIGINT          REFERENCES users(id),  -- set once on first transition away from NOT_TESTED
    created_by      BIGINT          NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by      BIGINT          NOT NULL REFERENCES users(id),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by      BIGINT          REFERENCES users(id),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT chk_test_case_results_priority CHECK (priority IN ('HIGHEST', 'HIGH', 'MEDIUM', 'LOW', 'LOWEST')),
    CONSTRAINT chk_test_case_results_result CHECK (result IN ('NOT_TESTED', 'IN_PROGRESS', 'PASS', 'FAIL', 'WARNING', 'IGNORE'))
);

COMMENT ON COLUMN test_case_results.summary IS 'Snapshot cloned from the test_case at import time. Re-import updates this field but preserves result, logs, and attached files.';
COMMENT ON COLUMN test_case_results.description IS 'Snapshot cloned from the test_case at import time. Re-import updates this field but preserves result, logs, and attached files.';
COMMENT ON COLUMN test_case_results.priority IS 'Snapshot cloned from the test_case at import time. Re-import updates this field but preserves result, logs, and attached files.';
COMMENT ON COLUMN test_case_results.tested_by IS 'Set once when the result first transitions away from NOT_TESTED, and is never modified thereafter.';

CREATE TRIGGER trg_test_case_results_set_updated_at
    BEFORE UPDATE ON test_case_results
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_updated_at();

-- ============================================================================
-- TABLE: object_sharing
-- ============================================================================
-- Hard delete — no deleted_at / deleted_by columns.
-- resource_type + resource_id form a polymorphic reference to any
-- shareable object type.

CREATE TABLE object_sharing (
    id              BIGSERIAL       PRIMARY KEY,
    user_id         BIGINT          NOT NULL REFERENCES users(id),
    resource_type   VARCHAR(50)     NOT NULL,
    resource_id     BIGINT          NOT NULL,
    role            VARCHAR(20)     NOT NULL,
    created_by      BIGINT          NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by      BIGINT          NOT NULL REFERENCES users(id),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_object_sharing UNIQUE (user_id, resource_type, resource_id),
    CONSTRAINT chk_object_sharing_resource_type CHECK (
        resource_type IN ('project', 'test_plan', 'test_case', 'test_run', 'test_execution')
    ),
    CONSTRAINT chk_object_sharing_role CHECK (
        role IN ('Editor', 'Contributor', 'Viewer')
    )
);

COMMENT ON TABLE object_sharing IS 'Hard delete — no deleted_at/deleted_by columns. resource_type + resource_id form a polymorphic reference to any shareable object type.';

CREATE TRIGGER trg_object_sharing_set_updated_at
    BEFORE UPDATE ON object_sharing
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_updated_at();

-- ============================================================================
-- TABLE: files
-- ============================================================================
-- owner_type + owner_id form a polymorphic reference to the owning entity.

CREATE TABLE files (
    id              BIGSERIAL       PRIMARY KEY,
    filename        VARCHAR(255)    NOT NULL,
    storage_path    VARCHAR(500)    NOT NULL,
    mime_type       VARCHAR(127)    NOT NULL,
    size_bytes      BIGINT          NOT NULL,
    owner_type      VARCHAR(50)     NOT NULL,
    owner_id        BIGINT          NOT NULL,
    created_by      BIGINT          NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by      BIGINT          NOT NULL REFERENCES users(id),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by      BIGINT          REFERENCES users(id),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT chk_files_size_bytes CHECK (size_bytes >= 0),
    CONSTRAINT chk_files_owner_type CHECK (
        owner_type IN ('test_case', 'test_case_result')
    )
);

COMMENT ON TABLE files IS 'owner_type + owner_id form a polymorphic reference to the owning entity.';

CREATE TRIGGER trg_files_set_updated_at
    BEFORE UPDATE ON files
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_updated_at();

-- ============================================================================
-- JUNCTION TABLES
-- ============================================================================
-- All junction tables use a composite primary key on both FK columns,
-- only created_at for timestamping, and hard delete (no soft-delete columns).

-- --------------------------------------------------------------------------
-- TABLE: user_groups
-- --------------------------------------------------------------------------

CREATE TABLE user_groups (
    user_id     BIGINT          NOT NULL REFERENCES users(id),
    group_id    BIGINT          NOT NULL REFERENCES groups(id),
    created_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    PRIMARY KEY (user_id, group_id)
);

-- --------------------------------------------------------------------------
-- TABLE: user_roles
-- --------------------------------------------------------------------------

CREATE TABLE user_roles (
    user_id     BIGINT          NOT NULL REFERENCES users(id),
    role_id     BIGINT          NOT NULL REFERENCES roles(id),
    created_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    PRIMARY KEY (user_id, role_id)
);

-- --------------------------------------------------------------------------
-- TABLE: group_roles
-- --------------------------------------------------------------------------

CREATE TABLE group_roles (
    group_id    BIGINT          NOT NULL REFERENCES groups(id),
    role_id     BIGINT          NOT NULL REFERENCES roles(id),
    created_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    PRIMARY KEY (group_id, role_id)
);

-- --------------------------------------------------------------------------
-- TABLE: role_permissions
-- --------------------------------------------------------------------------

CREATE TABLE role_permissions (
    role_id         BIGINT          NOT NULL REFERENCES roles(id),
    permission_id   BIGINT          NOT NULL REFERENCES permissions(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    PRIMARY KEY (role_id, permission_id)
);

-- --------------------------------------------------------------------------
-- TABLE: project_members
-- --------------------------------------------------------------------------

CREATE TABLE project_members (
    project_id  BIGINT          NOT NULL REFERENCES projects(id),
    user_id     BIGINT          NOT NULL REFERENCES users(id),
    role        VARCHAR(20)     NOT NULL,
    created_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    PRIMARY KEY (project_id, user_id),

    CONSTRAINT chk_project_members_role CHECK (
        role IN ('Owner', 'Editor', 'Contributor', 'Viewer')
    )
);

-- --------------------------------------------------------------------------
-- TABLE: test_case_categories
-- --------------------------------------------------------------------------

CREATE TABLE test_case_categories (
    test_case_id    BIGINT          NOT NULL REFERENCES test_cases(id),
    category_id     BIGINT          NOT NULL REFERENCES test_categories(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    PRIMARY KEY (test_case_id, category_id)
);

-- --------------------------------------------------------------------------
-- TABLE: test_plan_projects
-- --------------------------------------------------------------------------

CREATE TABLE test_plan_projects (
    plan_id     BIGINT          NOT NULL REFERENCES test_plans(id),
    project_id  BIGINT          NOT NULL REFERENCES projects(id),
    created_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    PRIMARY KEY (plan_id, project_id)
);

-- --------------------------------------------------------------------------
-- TABLE: test_run_cases
-- --------------------------------------------------------------------------

CREATE TABLE test_run_cases (
    run_id          BIGINT          NOT NULL REFERENCES test_runs(id),
    test_case_id    BIGINT          NOT NULL REFERENCES test_cases(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    PRIMARY KEY (run_id, test_case_id)
);

-- --------------------------------------------------------------------------
-- TABLE: execution_testers
-- --------------------------------------------------------------------------

CREATE TABLE execution_testers (
    execution_id    BIGINT          NOT NULL REFERENCES test_executions(id),
    user_id         BIGINT          NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    PRIMARY KEY (execution_id, user_id)
);

-- ============================================================================
-- INDEXES
-- ============================================================================
-- Every foreign key column is indexed. Entity tables with soft-delete get a
-- composite index on (deleted_at, id) for efficient active-row filtering.

-- Users
CREATE INDEX idx_users_deleted_at_id ON users(deleted_at, id);

-- Groups
CREATE INDEX idx_groups_created_by ON groups(created_by);
CREATE INDEX idx_groups_updated_by ON groups(updated_by);
CREATE INDEX idx_groups_deleted_at_id ON groups(deleted_at, id);

-- Roles
CREATE INDEX idx_roles_created_by ON roles(created_by);
CREATE INDEX idx_roles_updated_by ON roles(updated_by);
CREATE INDEX idx_roles_deleted_at_id ON roles(deleted_at, id);

-- Projects
CREATE INDEX idx_projects_created_by ON projects(created_by);
CREATE INDEX idx_projects_updated_by ON projects(updated_by);
CREATE INDEX idx_projects_deleted_at_id ON projects(deleted_at, id);

-- Test Categories
CREATE INDEX idx_test_categories_project_id ON test_categories(project_id);
CREATE INDEX idx_test_categories_created_by ON test_categories(created_by);
CREATE INDEX idx_test_categories_updated_by ON test_categories(updated_by);
CREATE INDEX idx_test_categories_deleted_at_id ON test_categories(deleted_at, id);

-- Test Case Templates
CREATE INDEX idx_test_case_templates_project_id ON test_case_templates(project_id);
CREATE INDEX idx_test_case_templates_created_by ON test_case_templates(created_by);
CREATE INDEX idx_test_case_templates_updated_by ON test_case_templates(updated_by);
CREATE INDEX idx_test_case_templates_deleted_at_id ON test_case_templates(deleted_at, id);

-- Test Cases
CREATE INDEX idx_test_cases_project_id ON test_cases(project_id);
CREATE INDEX idx_test_cases_created_by ON test_cases(created_by);
CREATE INDEX idx_test_cases_updated_by ON test_cases(updated_by);
CREATE INDEX idx_test_cases_deleted_at_id ON test_cases(deleted_at, id);

-- Test Plans
CREATE INDEX idx_test_plans_created_by ON test_plans(created_by);
CREATE INDEX idx_test_plans_updated_by ON test_plans(updated_by);
CREATE INDEX idx_test_plans_deleted_at_id ON test_plans(deleted_at, id);

-- Test Runs
CREATE INDEX idx_test_runs_report_to_user_id ON test_runs(report_to_user_id);
CREATE INDEX idx_test_runs_default_tester_id ON test_runs(default_tester_id);
CREATE INDEX idx_test_runs_project_id ON test_runs(project_id);
CREATE INDEX idx_test_runs_plan_id ON test_runs(plan_id);
CREATE INDEX idx_test_runs_created_by ON test_runs(created_by);
CREATE INDEX idx_test_runs_updated_by ON test_runs(updated_by);
CREATE INDEX idx_test_runs_deleted_at_id ON test_runs(deleted_at, id);

-- Test Executions
CREATE INDEX idx_test_executions_test_run_id ON test_executions(test_run_id);
CREATE INDEX idx_test_executions_created_by ON test_executions(created_by);
CREATE INDEX idx_test_executions_updated_by ON test_executions(updated_by);
CREATE INDEX idx_test_executions_deleted_at_id ON test_executions(deleted_at, id);

-- Test Case Results
CREATE INDEX idx_test_case_results_execution_id ON test_case_results(execution_id);
CREATE INDEX idx_test_case_results_test_case_id ON test_case_results(test_case_id);
CREATE INDEX idx_test_case_results_tested_by ON test_case_results(tested_by);
CREATE INDEX idx_test_case_results_created_by ON test_case_results(created_by);
CREATE INDEX idx_test_case_results_updated_by ON test_case_results(updated_by);
CREATE INDEX idx_test_case_results_result ON test_case_results(result);
CREATE INDEX idx_test_case_results_deleted_at_id ON test_case_results(deleted_at, id);

-- Object Sharing
CREATE INDEX idx_object_sharing_user_id ON object_sharing(user_id);
CREATE INDEX idx_object_sharing_resource ON object_sharing(resource_type, resource_id);
CREATE INDEX idx_object_sharing_created_by ON object_sharing(created_by);
CREATE INDEX idx_object_sharing_updated_by ON object_sharing(updated_by);

-- Files
CREATE INDEX idx_files_owner ON files(owner_type, owner_id);
CREATE INDEX idx_files_created_by ON files(created_by);
CREATE INDEX idx_files_updated_by ON files(updated_by);
CREATE INDEX idx_files_deleted_at_id ON files(deleted_at, id);

-- Junction table indexes (second FK column of each composite PK)
CREATE INDEX idx_user_groups_group_id ON user_groups(group_id);
CREATE INDEX idx_user_roles_role_id ON user_roles(role_id);
CREATE INDEX idx_group_roles_role_id ON group_roles(role_id);
CREATE INDEX idx_role_permissions_permission_id ON role_permissions(permission_id);
CREATE INDEX idx_project_members_user_id ON project_members(user_id);
CREATE INDEX idx_test_case_categories_category_id ON test_case_categories(category_id);
CREATE INDEX idx_test_plan_projects_project_id ON test_plan_projects(project_id);
CREATE INDEX idx_test_run_cases_test_case_id ON test_run_cases(test_case_id);
CREATE INDEX idx_execution_testers_user_id ON execution_testers(user_id);

-- ============================================================================
COMMIT;

-- ============================================================================
-- DOWN MIGRATION (ROLLBACK)
-- ============================================================================
-- BEGIN;
--
-- -- Drop triggers (order does not matter)
-- DROP TRIGGER IF EXISTS trg_groups_set_updated_at ON groups;
-- DROP TRIGGER IF EXISTS trg_roles_set_updated_at ON roles;
-- DROP TRIGGER IF EXISTS trg_projects_set_updated_at ON projects;
-- DROP TRIGGER IF EXISTS trg_test_categories_set_updated_at ON test_categories;
-- DROP TRIGGER IF EXISTS trg_test_case_templates_set_updated_at ON test_case_templates;
-- DROP TRIGGER IF EXISTS trg_test_cases_set_updated_at ON test_cases;
-- DROP TRIGGER IF EXISTS trg_test_plans_set_updated_at ON test_plans;
-- DROP TRIGGER IF EXISTS trg_test_runs_set_updated_at ON test_runs;
-- DROP TRIGGER IF EXISTS trg_test_executions_set_updated_at ON test_executions;
-- DROP TRIGGER IF EXISTS trg_test_case_results_set_updated_at ON test_case_results;
-- DROP TRIGGER IF EXISTS trg_object_sharing_set_updated_at ON object_sharing;
-- DROP TRIGGER IF EXISTS trg_files_set_updated_at ON files;
--
-- -- Drop trigger function
-- DROP FUNCTION IF EXISTS trigger_set_updated_at;
--
-- -- Drop junction tables first (depend on entity tables)
-- DROP TABLE IF EXISTS execution_testers CASCADE;
-- DROP TABLE IF EXISTS test_run_cases CASCADE;
-- DROP TABLE IF EXISTS test_plan_projects CASCADE;
-- DROP TABLE IF EXISTS test_case_categories CASCADE;
-- DROP TABLE IF EXISTS project_members CASCADE;
-- DROP TABLE IF EXISTS role_permissions CASCADE;
-- DROP TABLE IF EXISTS group_roles CASCADE;
-- DROP TABLE IF EXISTS user_roles CASCADE;
-- DROP TABLE IF EXISTS user_groups CASCADE;
--
-- -- Drop entity tables in reverse creation order
-- DROP TABLE IF EXISTS files CASCADE;
-- DROP TABLE IF EXISTS object_sharing CASCADE;
-- DROP TABLE IF EXISTS test_case_results CASCADE;
-- DROP TABLE IF EXISTS test_executions CASCADE;
-- DROP TABLE IF EXISTS test_runs CASCADE;
-- DROP TABLE IF EXISTS test_plans CASCADE;
-- DROP TABLE IF EXISTS test_cases CASCADE;
-- DROP TABLE IF EXISTS test_case_templates CASCADE;
-- DROP TABLE IF EXISTS test_categories CASCADE;
-- DROP TABLE IF EXISTS projects CASCADE;
-- DROP TABLE IF EXISTS permissions CASCADE;
-- DROP TABLE IF EXISTS roles CASCADE;
-- DROP TABLE IF EXISTS groups CASCADE;
-- DROP TABLE IF EXISTS users CASCADE;
--
-- COMMIT;
