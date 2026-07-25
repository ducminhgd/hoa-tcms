-- Seed Permissions
-- This migration populates the read-only `permissions` reference table.
-- Safe to re-run (uses ON CONFLICT DO NOTHING).
--
-- Permission code format: {resource}:{action}
-- Actions: create, read, read_list, update, select, delete

BEGIN;

INSERT INTO permissions (name, code)
VALUES
    -- User permissions
    ('Create User', 'user:create'),
    ('Read User', 'user:read'),
    ('List Users', 'user:read_list'),
    ('Update User', 'user:update'),
    ('Select User', 'user:select'),
    ('Delete User', 'user:delete'),

    -- Group permissions
    ('Create Group', 'group:create'),
    ('Read Group', 'group:read'),
    ('List Groups', 'group:read_list'),
    ('Update Group', 'group:update'),
    ('Select Group', 'group:select'),
    ('Delete Group', 'group:delete'),

    -- Role permissions
    ('Create Role', 'role:create'),
    ('Read Role', 'role:read'),
    ('List Roles', 'role:read_list'),
    ('Update Role', 'role:update'),
    ('Select Role', 'role:select'),
    ('Delete Role', 'role:delete'),

    -- Permission permissions (read-only)
    ('List Permissions', 'permission:read_list'),

    -- Project permissions
    ('Create Project', 'project:create'),
    ('Read Project', 'project:read'),
    ('List Projects', 'project:read_list'),
    ('Update Project', 'project:update'),
    ('Select Project', 'project:select'),
    ('Delete Project', 'project:delete'),

    -- Test Case permissions
    ('Create Test Case', 'test_case:create'),
    ('Read Test Case', 'test_case:read'),
    ('List Test Cases', 'test_case:read_list'),
    ('Update Test Case', 'test_case:update'),
    ('Select Test Case', 'test_case:select'),
    ('Delete Test Case', 'test_case:delete'),

    -- Test Plan permissions
    ('Create Test Plan', 'test_plan:create'),
    ('Read Test Plan', 'test_plan:read'),
    ('List Test Plans', 'test_plan:read_list'),
    ('Update Test Plan', 'test_plan:update'),
    ('Select Test Plan', 'test_plan:select'),
    ('Delete Test Plan', 'test_plan:delete'),

    -- Test Run permissions
    ('Create Test Run', 'test_run:create'),
    ('Read Test Run', 'test_run:read'),
    ('List Test Runs', 'test_run:read_list'),
    ('Update Test Run', 'test_run:update'),
    ('Select Test Run', 'test_run:select'),
    ('Delete Test Run', 'test_run:delete'),

    -- Test Execution permissions
    ('Create Test Execution', 'test_execution:create'),
    ('Read Test Execution', 'test_execution:read'),
    ('List Test Executions', 'test_execution:read_list'),
    ('Update Test Execution', 'test_execution:update'),
    ('Select Test Execution', 'test_execution:select'),
    ('Delete Test Execution', 'test_execution:delete'),

    -- Sharing permissions
    ('Share Object', 'share:create'),
    ('Remove Share', 'share:delete'),

    -- Test Case File permissions
    ('Upload Test Case File', 'test_case_file:upload'),
    ('Read Test Case File', 'test_case_file:read'),
    ('Delete Test Case File', 'test_case_file:delete'),

    -- Share permissions
    ('Create Share', 'share:create'),
    ('Read Share', 'share:read'),
    ('Delete Share', 'share:delete')
ON CONFLICT (code) DO NOTHING;

COMMIT;

-- Down migration
-- BEGIN;
-- DELETE FROM permissions WHERE code IN (
--     'user:create', 'user:read', 'user:read_list', 'user:update', 'user:select', 'user:delete',
--     'group:create', 'group:read', 'group:read_list', 'group:update', 'group:select', 'group:delete',
--     'role:create', 'role:read', 'role:read_list', 'role:update', 'role:select', 'role:delete',
--     'permission:read_list',
--     'project:create', 'project:read', 'project:read_list', 'project:update', 'project:select', 'project:delete',
--     'test_case:create', 'test_case:read', 'test_case:read_list', 'test_case:update', 'test_case:select', 'test_case:delete',
--     'test_plan:create', 'test_plan:read', 'test_plan:read_list', 'test_plan:update', 'test_plan:select', 'test_plan:delete',
--     'test_run:create', 'test_run:read', 'test_run:read_list', 'test_run:update', 'test_run:select', 'test_run:delete',
--     'test_execution:create', 'test_execution:read', 'test_execution:read_list', 'test_execution:update', 'test_execution:select', 'test_execution:delete',
--     'share:create', 'share:delete',
--     'test_case_file:upload', 'test_case_file:read', 'test_case_file:delete'
-- );
-- COMMIT;
