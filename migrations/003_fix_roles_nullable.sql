-- ==========================================================================
-- Migration: 003_fix_roles_nullable
-- Description: Make roles.created_by and roles.updated_by nullable
--
-- The System Admin role needs to be seeded with created_by = NULL and
-- updated_by = NULL because no user exists at migration time to reference.
-- This migration removes the NOT NULL constraint on both columns to allow
-- that, matching the same pattern already used on users.created_by and
-- users.updated_by.
-- ==========================================================================

BEGIN;

-- ============================================================================
-- ALTER TABLE: roles — make created_by and updated_by nullable
-- ============================================================================

ALTER TABLE roles ALTER COLUMN created_by DROP NOT NULL;
ALTER TABLE roles ALTER COLUMN updated_by DROP NOT NULL;

COMMENT ON COLUMN roles.created_by IS 'Nullable because the System Admin role is seeded via migration with no creator. The application sets created_by explicitly for all subsequent role operations.';
COMMENT ON COLUMN roles.updated_by IS 'Nullable because the System Admin role is seeded via migration with no updater. The trigger_set_updated_at function or the application sets updated_by explicitly for all subsequent role operations.';

COMMIT;

-- ============================================================================
-- DOWN MIGRATION (ROLLBACK)
-- ============================================================================
-- IMPORTANT: Run the 004_seed_system_admin_role rollback BEFORE this rollback.
-- The System Admin role has NULL in created_by and updated_by, which would
-- violate the NOT NULL constraint being restored below.
--
-- BEGIN;
--
-- ALTER TABLE roles ALTER COLUMN created_by SET NOT NULL;
-- ALTER TABLE roles ALTER COLUMN updated_by SET NOT NULL;
--
-- COMMIT;
