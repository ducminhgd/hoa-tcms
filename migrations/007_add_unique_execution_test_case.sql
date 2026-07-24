BEGIN;
ALTER TABLE test_case_results
  ADD CONSTRAINT uq_test_case_results_execution_case UNIQUE (execution_id, test_case_id);
COMMIT;
