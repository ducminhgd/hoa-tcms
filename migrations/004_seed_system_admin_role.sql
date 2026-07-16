-- ==========================================================================
-- Migration: 004_seed_system_admin_role
-- Description: Seed the System Admin role and assign all permissions
--
-- The System Admin role is a built-in role with unrestricted access to
-- every permission in the system. This migration inserts the role (if it
-- does not already exist) and grants every seeded permission to it via
-- the role_permissions junction table.
--
-- Both steps are idempotent and safe to re-run.
-- ==========================================================================

BEGIN;

-- ============================================================================
-- Step 1: Insert the System Admin role
-- ============================================================================
-- created_by and updated_by are NULL because no user exists at migration time
-- to reference (migration 003 made these columns nullable).
-- is_system is TRUE so the application can identify this role as protected
-- without relying on exact name matching.

INSERT INTO roles (name, is_system, created_by, updated_by)
VALUES ('System Admin', TRUE, NULL, NULL)
ON CONFLICT (name) DO NOTHING;

-- ============================================================================
-- Step 2: Assign all permissions to the System Admin role
-- ============================================================================
-- Uses a subquery to resolve the role ID and selects every row from the
-- permissions table. ON CONFLICT DO NOTHING makes this safe to re-run.

INSERT INTO role_permissions (role_id, permission_id)
SELECT
    (SELECT id FROM roles WHERE name = 'System Admin'),
    id
FROM permissions
ON CONFLICT (role_id, permission_id) DO NOTHING;

COMMIT;

-- ============================================================================
-- DOWN MIGRATION (ROLLBACK)
-- ============================================================================
-- BEGIN;
--
-- DELETE FROM role_permissions
-- WHERE role_id = (SELECT id FROM roles WHERE name = 'System Admin');
--
-- DELETE FROM roles WHERE name = 'System Admin';
--
-- COMMIT;
