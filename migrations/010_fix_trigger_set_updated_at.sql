-- ==========================================================================
-- Migration: 010_fix_trigger_set_updated_at
-- Description: Fix type mismatch in trigger_set_updated_at() where
--              COALESCE(TEXT, BIGINT) fails because current_setting()
--              returns TEXT but NEW.updated_by is BIGINT.
--
-- The fix moves the ::BIGINT cast inside the COALESCE so the
-- NULLIF(current_setting(...), '') result is cast before the
-- coalesce, making both arguments BIGINT.
-- ==========================================================================

BEGIN;

CREATE OR REPLACE FUNCTION trigger_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    NEW.updated_by = COALESCE(
        NULLIF(current_setting('app.current_user_id', TRUE), '')::BIGINT,
        NEW.updated_by
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMIT;
