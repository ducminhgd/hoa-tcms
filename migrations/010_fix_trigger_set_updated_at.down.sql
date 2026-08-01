-- ==========================================================================
-- DOWN MIGRATION (ROLLBACK) for 010_fix_trigger_set_updated_at
-- ==========================================================================
-- Reverts the trigger to the original (buggy) version where ::BIGINT
-- is outside COALESCE. This is provided for completeness — the original
-- version works as long as app.current_user_id is always set to a
-- numeric string before any UPDATE that fires this trigger.

BEGIN;

CREATE OR REPLACE FUNCTION trigger_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    NEW.updated_by = COALESCE(
        NULLIF(current_setting('app.current_user_id', TRUE), ''),
        NEW.updated_by
    )::BIGINT;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMIT;
