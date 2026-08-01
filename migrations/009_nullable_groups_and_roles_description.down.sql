-- ==========================================================================
-- DOWN MIGRATION (ROLLBACK) for 009_nullable_groups_and_roles_description
-- ==========================================================================
-- IMPORTANT: Ensure no rows in groups have NULL created_by or updated_by
-- before running this rollback. The NOT NULL constraint restore will fail
-- if any group was seeded by the setup seeder without a creator.
--
-- The roles.description column is dropped unconditionally — any data in it
-- will be lost.
-- ==========================================================================

BEGIN;

-- Restore NOT NULL on groups audit columns.
-- (Only safe if no seeder-created groups exist with NULL audit columns.)
ALTER TABLE groups ALTER COLUMN created_by SET NOT NULL;
ALTER TABLE groups ALTER COLUMN updated_by SET NOT NULL;

-- Remove the description column added to roles.
ALTER TABLE roles DROP COLUMN IF EXISTS description;

COMMIT;
