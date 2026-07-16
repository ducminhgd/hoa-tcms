-- ==========================================================================
-- Migration: 005_add_is_system_to_roles
-- Description: Add is_system boolean column to the roles table
--
-- The is_system flag replaces the fragile case-sensitive name check for
-- identifying protected roles (e.g. "System Admin"). System-protected roles
-- cannot be deleted or have their permissions modified through the app API.
-- ==========================================================================

BEGIN;

-- ============================================================================
-- ALTER TABLE: roles — add is_system column
-- ============================================================================

ALTER TABLE roles ADD COLUMN is_system BOOLEAN NOT NULL DEFAULT FALSE;

-- Mark the existing System Admin role as a system role.
UPDATE roles SET is_system = TRUE WHERE name = 'System Admin';

COMMENT ON COLUMN roles.is_system IS 'System-protected roles cannot be deleted or have their permissions modified through the application API. Set TRUE for built-in roles seeded by migrations.';

COMMIT;

-- ============================================================================
-- DOWN MIGRATION (ROLLBACK)
-- ============================================================================
-- BEGIN;
--
-- ALTER TABLE roles DROP COLUMN is_system;
--
-- COMMIT;
