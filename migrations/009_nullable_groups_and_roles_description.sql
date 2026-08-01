-- ==========================================================================
-- Migration: 009_nullable_groups_and_roles_description
-- Description: Make groups.created_by and groups.updated_by nullable,
--              and add a description column to the roles table.
--
-- The seeder creates system groups and roles before any user exists,
-- so the audit columns must accept NULL. The roles.description column
-- stores a human-readable summary of each role's purpose.
-- ==========================================================================

BEGIN;

-- ============================================================================
-- ALTER TABLE: groups — make created_by and updated_by nullable
-- ============================================================================

ALTER TABLE groups ALTER COLUMN created_by DROP NOT NULL;
ALTER TABLE groups ALTER COLUMN updated_by DROP NOT NULL;

COMMENT ON COLUMN groups.created_by IS $$Nullable because system groups are seeded with no creator user at application startup.$$;
COMMENT ON COLUMN groups.updated_by IS $$Nullable because system groups are seeded with no updater user at application startup.$$;

-- ============================================================================
-- ALTER TABLE: roles — add description column
-- ============================================================================

ALTER TABLE roles ADD COLUMN description TEXT;

COMMENT ON COLUMN roles.description IS 'Human-readable summary of the role''s purpose. Populated by the setup seeder.';

COMMIT;
