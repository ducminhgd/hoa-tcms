-- Create test_case_files table for file attachments on test cases.
-- Safe to re-run (uses IF NOT EXISTS).

BEGIN;

CREATE TABLE IF NOT EXISTS test_case_files (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    test_case_id BIGINT NOT NULL REFERENCES test_cases(id) ON DELETE RESTRICT,
    file_name VARCHAR(255) NOT NULL,
    file_path VARCHAR(500) NOT NULL,
    file_size BIGINT NOT NULL,
    mime_type VARCHAR(127) NOT NULL,
    uploaded_by BIGINT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_by BIGINT REFERENCES users(id) ON DELETE RESTRICT,
    deleted_at TIMESTAMPTZ,

    CONSTRAINT chk_test_case_files_file_name CHECK (char_length(TRIM(file_name)) > 0),
    CONSTRAINT chk_test_case_files_file_path CHECK (char_length(TRIM(file_path)) > 0),
    CONSTRAINT chk_test_case_files_file_size CHECK (file_size > 0),
    CONSTRAINT chk_test_case_files_mime_type CHECK (char_length(TRIM(mime_type)) > 0)
);

CREATE INDEX IF NOT EXISTS idx_test_case_files_test_case_id ON test_case_files (test_case_id);
CREATE INDEX IF NOT EXISTS idx_test_case_files_uploaded_by ON test_case_files (uploaded_by);
CREATE INDEX IF NOT EXISTS idx_test_case_files_deleted_by ON test_case_files (deleted_by);
CREATE INDEX IF NOT EXISTS idx_test_case_files_active ON test_case_files (test_case_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_test_case_files_name_search ON test_case_files (test_case_id, file_name) WHERE deleted_at IS NULL;

-- BEFORE UPDATE trigger for updated_at
CREATE OR REPLACE FUNCTION trg_test_case_files_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_test_case_files_updated_at ON test_case_files;
CREATE TRIGGER trg_test_case_files_updated_at
  BEFORE UPDATE ON test_case_files
  FOR EACH ROW
  EXECUTE FUNCTION trg_test_case_files_updated_at();

COMMIT;
