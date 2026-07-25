-- Rollback migration 008: drop test_case_files table.
-- Safe to re-run (uses IF EXISTS).

BEGIN;

DROP TRIGGER IF EXISTS trg_test_case_files_updated_at ON test_case_files;
DROP FUNCTION IF EXISTS trg_test_case_files_updated_at();
DROP TABLE IF EXISTS test_case_files;

COMMIT;
