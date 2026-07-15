# Tasks: Test Execution -- Selective Import

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `TestCaseResult` entity and `ResultStatus` enum --
       `requirements.md#US-1`, `design.md#Components`
  - Fields: `id`, `execution_id`, `test_case_id`, `summary`, `description`, `priority`,
    `status`, `result_logs`, `created_by`, `created_at`, `updated_by`, `updated_at`
  - Factory method `TestCaseResult::create_snapshot(execution_id, test_case_id, summary,
    description, priority, created_by)`:
    - Sets `status` to `ResultStatus::NotTested`
    - Sets `result_logs` to `None`
    - Validates `summary` is non-empty after trim, max 500 chars
    - `description` is accepted as-is (nullable, max length validated at DB layer)
    - `priority` is accepted as-is (nullable string, max 50 chars)
    - Sets `created_by` and `updated_by` to `created_by`
  - `ResultStatus` enum with variants: `NotTested`, `InProgress`, `Pass`, `Fail`,
    `Warning`, `Ignore`
  - `ResultStatus::as_str()` returns the DB string representation
  - `ResultStatus::from_str(s)` parses from DB string
  - No framework imports; pure language struct + impl

- [ ] 2. Define domain value objects and exceptions -- `design.md#Components`,
       `design.md#Error Handling`
  - `ImportOutcome` value object:
    - `imported: Vec<TestCaseResult>`
    - `skipped: Vec<SkippedTestCase>`
    - Method `total_imported() -> usize`
    - Method `total_skipped() -> usize`
  - `SkippedTestCase` value object:
    - `test_case_id: i64`
    - `reason: String` (always `"already_imported"` in this feature)
  - `TestCaseSnapshot` value object (used to carry data read from test_cases at import
    time):
    - `test_case_id: i64`
    - `summary: String`
    - `description: Option<String>`
    - `priority: Option<String>` (resolved priority name, nullable)
  - Domain exceptions:
    - `TestExecutionImportPermissionDenied` (reason: missing system permission or wrong
      project role)
    - `TestCaseNotInTestRunError` (carries `Vec<InvalidTestCaseDetail>` listing each
      invalid ID and the reason)
    - `InvalidTestCaseDetail` (test_case_id: i64, reason: enum -- NotFound, SoftDeleted,
      NotInTestRun)
    - `ImportValidationError` (carries field-level details for invalid request body)

---

## Layer 2 -- Application

- [ ] 3. Define `TestCaseResultRepository` interface (port) -- `design.md#Components`,
       `design.md#Sequence`
  - Methods:
    - `find_existing_test_case_ids(execution_id, test_case_ids) -> Vec<i64>` -- returns
      the subset of `test_case_ids` that already exist in `test_case_results` for this
      execution. Used to determine which IDs to skip.
    - `save_batch(results: Vec<TestCaseResult>) -> Vec<TestCaseResult>` -- bulk inserts
      rows and returns them with generated `id`, `created_at`, `updated_at`.
  - Both methods accept a transaction context for transactional composition
  - `save_batch` uses a single INSERT with multiple VALUE rows for efficiency
  - Returns domain entities (not raw DB rows)

- [ ] 4. Define command/query/response DTOs -- `design.md#API Contract`
  - `ImportTestCasesCommand`:
    - `test_case_ids: Vec<i64>` (non-empty, validated at handler layer)
  - `ImportTestCasesResponse`:
    - `imported: Vec<ImportedResultItem>`
    - `skipped: Vec<SkippedResultItem>`
  - `ImportedResultItem`:
    - `id: i64`, `execution_id: i64`, `test_case_id: i64`, `summary: String`,
      `description: Option<String>`, `priority: Option<String>`, `status: String`,
      `created_by: i64`, `created_at: DateTime<Utc>`, `updated_by: i64`,
      `updated_at: DateTime<Utc>`
    - Note: `result_logs` is omitted from the response (always null on import)
  - `SkippedResultItem`:
    - `test_case_id: i64`
    - `reason: String` (always `"already_imported"`)

- [ ] 5. Implement `TestExecutionImportService` -- `requirements.md#US-1` through `US-4`,
       `design.md#Sequence`
  - Single method: `import_test_cases(project_id, test_run_id, execution_id,
    test_case_ids, current_user_id) -> ImportOutcome`
  - Step-by-step logic:
    1. Check `test_execution:import` system permission via `AuthorizationService`.
       System Admin bypasses.
    2. Check the user is a Contributor, Editor, or Owner of the project via
       `ProjectMemberRepository`. Viewer or non-member -> `403`.
    3. Begin a database transaction.
    4. Within transaction:
       a. Validate project exists and is not soft-deleted (via `ProjectRepository`).
          Not found -> `404`.
       b. Validate test run exists, not soft-deleted, belongs to project (via
          `TestRunRepository`). Not found -> `404`.
       c. Validate execution exists, not soft-deleted, belongs to project, and its
          `test_run_id` matches the path param (via `TestExecutionRepository`).
          Not found or mismatch -> `404`.
       d. Deduplicate `test_case_ids` (remove duplicates from the request array --
          subsequent occurrences of the same ID are treated as already-imported skips
          after the first occurrence is processed).
       e. Validate all test case IDs are members of the test run. Query the
          test-run-to-test-case join table for the given `test_run_id` and
          `test_case_ids`. Also verify each test case is not soft-deleted. Collect
          any invalid IDs with reasons (not found, soft-deleted, not in test run).
          If any invalid -> roll back and return `422` with `TEST_CASE_NOT_IN_TEST_RUN`.
       f. Query `TestCaseResultRepository::find_existing_test_case_ids` to identify
          already-imported test cases in this execution. These go into `skipped`.
       g. For the remaining (new) test case IDs, fetch snapshot data:
          call `TestCaseRepository::find_snapshots_by_ids(remaining_ids)` to get
          summary, description, and resolved priority name for each test case.
       h. For each snapshot, construct a `TestCaseResult` entity via
          `create_snapshot(...)`.
       i. Call `TestCaseResultRepository::save_batch(results)` to bulk-insert.
       j. Build `ImportOutcome` from the saved results (imported) and the
          already-existing IDs (skipped).
    5. Commit transaction.
    6. Return `ImportOutcome`.
  - All methods check system permission via `AuthorizationService`
  - All methods check project membership/role via `ProjectMemberRepository`
  - System Admin bypasses all permission and membership checks

- [ ] 6. Extend `TestCaseRepository` interface (port) -- `design.md#Components`
  - Add method: `find_snapshots_by_ids(ids: Vec<i64>) -> Vec<TestCaseSnapshot>`
  - Query:
    ```sql
    SELECT tc.id, tc.summary, tc.description, tp.name AS priority
    FROM test_cases tc
    LEFT JOIN test_priorities tp
      ON tc.priority_id = tp.id AND tp.deleted_at IS NULL
    WHERE tc.id = ANY($1) AND tc.deleted_at IS NULL
    ```
  - Returns only non-deleted test cases
  - Priority name is null if `priority_id` is null or the referenced priority is
    soft-deleted
  - This is a read-only operation -- the repository interface change is additive
    and backwards-compatible

- [ ] 7. Write unit tests for `TestExecutionImportService` -- `requirements.md#US-1`
       through `US-4`
  - Table-driven tests with mock repositories and services
  - Happy path:
    - Import a single test case (verify snapshot fields copied, status NOT_TESTED)
    - Import multiple test cases in one request (verify batch insert)
    - Import with priority (verify priority name resolved in snapshot)
    - Import with null priority (verify snapshot priority is null)
    - Import with null description (verify snapshot description is null)
  - Idempotency:
    - Import a test case, then import the same test case again (verify skipped, not
      duplicated)
    - Import request with all already-imported IDs (verify 200 with empty imported,
      all in skipped)
    - Import request with mix of new and already-imported IDs (verify partial import)
    - Duplicate IDs within the same request array (verify first occurrence imported
      or skipped, subsequent occurrences skipped)
  - Validation:
    - Empty `test_case_ids` array (verify error)
    - Test case not in test run (verify 422 with `TEST_CASE_NOT_IN_TEST_RUN`)
    - Test case soft-deleted (verify 422 with `TEST_CASE_NOT_IN_TEST_RUN`)
    - Test case does not exist (verify 422 with `TEST_CASE_NOT_IN_TEST_RUN`)
    - Mix of valid and invalid IDs (verify all invalid listed in error details)
    - Execution not linked to the given test run (execution.test_run_id != path
      test_run_id) (verify 404)
    - Execution soft-deleted (verify 404)
    - Test run not found or soft-deleted (verify 404)
    - Project not found or soft-deleted (verify 404)
  - Authorization:
    - User lacks `test_execution:import` permission (verify 403)
    - User is a Viewer of the project (verify 403)
    - User is a Contributor (verify success)
    - User is an Editor (verify success)
    - User is an Owner (verify success)
    - System Admin bypasses all gates (verify success)
    - User is not a project member (verify 403)
  - Snapshot semantics:
    - Original test case summary/description/priority changed after import -- imported
      result retains old values (verify no automatic sync)
  - Transaction rollback:
    - If validation fails at step (e) (invalid test case IDs), no rows inserted
    - If DB unique violation on `(execution_id, test_case_id)` due to race, the
      conflicting test case is treated as skipped (not an error)

---

## Layer 3 -- Adapters (HTTP)

- [ ] 8. Implement `TestExecutionImportHandler` -- `design.md#API Contract`
  - Single handler method: `import`
  - Deserialize request body into `ImportTestCasesCommand`
  - Validate `test_case_ids`:
    - Must be present (not null, not missing)
    - Must be a JSON array
    - Must be non-empty
    - Each element must be a positive integer (reject non-integer values,
      negative numbers, zero)
  - Validate project exists and is not soft-deleted before import operation
  - Call `TestExecutionImportService::import_test_cases(...)`
  - Serialize response:
    - `200 OK` with `data.imported` and `data.skipped` arrays
    - Map `ImportOutcome` to `ImportTestCasesResponse` DTO
    - Each imported item includes all fields except `result_logs`
  - Map domain/service errors to HTTP status codes and standard error body format:
    - `TestExecutionImportPermissionDenied` -> `403 Forbidden` (generic message)
    - `TestCaseNotInTestRunError` -> `422 Unprocessable Entity` with
      `TEST_CASE_NOT_IN_TEST_RUN` and details for each invalid ID
    - `ImportValidationError` -> `422 Unprocessable Entity` with field-level details
    - Project not found -> `404 Not Found`
    - Test run not found / wrong project / not linked -> `404 Not Found`
    - Execution not found / wrong project / soft-deleted -> `404 Not Found`

- [ ] 9. Register import route in HTTP router -- `design.md#Route Registration`
  - `POST /api/v1/projects/{projectId}/test-runs/{testRunId}/executions/{executionId}/import`
    -> `import_test_cases`
  - Route requires session auth middleware
  - Route is registered **after** execution CRUD routes (the import endpoint depends on
    the execution resource being registered first)
  - Ensure the route path with four path segments is correctly parsed by the router
    framework

- [ ] 10. Write integration tests for import HTTP handler -- `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back)
  - Test `200 OK` with `imported` array containing the newly created results
  - Test `200 OK` with `skipped` array for already-imported test cases
  - Test `200 OK` with mix of `imported` and `skipped`
  - Test `200 OK` when all IDs are already imported (empty imported, all in skipped)
  - Test that imported result has correct snapshot values (summary, description,
    priority resolved from test case at import time)
  - Test that imported result has `status: "NOT_TESTED"`
  - Test that `result_logs` is not present in the response
  - Test that audit fields (`created_by`, `created_at`, `updated_by`, `updated_at`)
    are present and correct
  - Test `422` with `VALIDATION_ERROR` when `test_case_ids` is missing
  - Test `422` with `VALIDATION_ERROR` when `test_case_ids` is empty array
  - Test `422` with `VALIDATION_ERROR` when `test_case_ids` contains non-integer
    values (string, float, null)
  - Test `422` with `VALIDATION_ERROR` when `test_case_ids` contains negative numbers
    or zero
  - Test `422` with `TEST_CASE_NOT_IN_TEST_RUN` when a test case ID is not linked to
    the test run (verify all invalid IDs listed in error details with specific reasons)
  - Test `422` with `TEST_CASE_NOT_IN_TEST_RUN` when a test case is soft-deleted
  - Test `422` with `TEST_CASE_NOT_IN_TEST_RUN` when a test case does not exist
  - Test `404 Not Found` when project does not exist
  - Test `404 Not Found` when test run does not exist or is soft-deleted
  - Test `404 Not Found` when test run belongs to a different project
  - Test `404 Not Found` when execution does not exist or is soft-deleted
  - Test `404 Not Found` when execution is not linked to the given test run
    (execution.test_run_id != path test_run_id)
  - Test `404 Not Found` when execution belongs to a different project
  - Test `403 Forbidden` when user lacks `test_execution:import` permission
  - Test `403 Forbidden` when user is a Viewer of the project
  - Test `403 Forbidden` when user is not a project member
  - Test that Contributor can import (verify 200)
  - Test that Editor can import (verify 200)
  - Test that Owner can import (verify 200)
  - Test that System Admin can import regardless of membership
  - Test `401 Unauthorized` when session is missing or invalid
  - Test that duplicate IDs in the request array are handled correctly (first import,
    subsequent skip)
  - Test that snapshot data is frozen (update original test case after import, verify
    imported result unchanged)
  - Test batch insert with many test case IDs (e.g., 50) to verify bulk INSERT
    performance and correctness
  - Test that unrecognised fields in the request body return `422` (strict mode)

---

## Layer 4 -- Infrastructure

- [ ] 11. Create `TEST_CASE_RESULTS` database migration -- `design.md#Data Model`
  - Table definition with all columns, PK, FKs, constraints
  - `CHECK` constraint on `status`:
    `status IN ('NOT_TESTED', 'IN_PROGRESS', 'PASS', 'FAIL', 'WARNING', 'IGNORE')`
  - `DEFAULT 'NOT_TESTED'` on `status` column
  - `NOT NULL` on `summary`, `execution_id`, `test_case_id`, `status`, `created_by`,
    `updated_by`
  - Unique index `uq_test_case_results_execution_test_case` on
    `(execution_id, test_case_id)`
  - Foreign key indexes on `execution_id`, `test_case_id`, `created_by`, `updated_by`
  - Composite index `idx_test_case_results_execution_status` on
    `(execution_id, status)`
  - `BEFORE UPDATE` trigger `trg_test_case_results_updated_at` that sets
    `NEW.updated_at = NOW()`
  - All FKs use `ON DELETE RESTRICT`
  - Rollback migration: `DROP TRIGGER IF EXISTS`, `DROP FUNCTION IF EXISTS`,
    `DROP TABLE IF EXISTS`
  - Migration must run **after** the `TEST_EXECUTIONS` and `TEST_CASES` migrations
    (FK dependencies)

- [ ] 12. Add `test_execution:import` permission code to the permissions seed migration --
       `design.md#New Permission Code`
  - Add 1 new row to the permissions seed data with
    `INSERT ... ON CONFLICT (code) DO NOTHING`:
    `('test_execution:import', 'Import Test Cases into Execution')`
  - Let the database auto-assign IDs (do NOT hardcode numeric IDs)
  - If permissions are seeded in a single migration file, append to that file
  - Verify idempotency: `ON CONFLICT (code) DO NOTHING` so the migration is safe to
    re-run

- [ ] 13. Implement `SqlTestCaseResultRepository` -- `design.md#Components`
  - All methods from `TestCaseResultRepository` interface
  - `find_existing_test_case_ids(execution_id, test_case_ids)`:
    ```sql
    SELECT test_case_id
    FROM test_case_results
    WHERE execution_id = $1 AND test_case_id = ANY($2)
    ```
    Returns the list of test case IDs that already have results for this execution.
  - `save_batch(results)`:
    - Builds a multi-row INSERT with one VALUE row per `TestCaseResult`.
    - Uses `RETURNING id, execution_id, test_case_id, summary, description,
      priority, status, created_by, created_at, updated_by, updated_at` to get back
      generated IDs and timestamps.
    - Maps the returned rows back to `TestCaseResult` entities.
    - If a PostgreSQL unique violation (error 23505) occurs on
      `uq_test_case_results_execution_test_case`, it means a concurrent request just
      imported the same test case. In this case, catch the error at the service layer
      and treat it as a skip (not an error) -- include it in the `skipped` array.
  - Use parameterized queries exclusively -- never string interpolation
  - Write unit tests with a test database (one test transaction per case):
    - Insert a batch of results and verify all returned with generated IDs and
      timestamps
    - Insert a single result and verify
    - `find_existing_test_case_ids` returns correct subset
    - `find_existing_test_case_ids` returns empty when no results exist
    - `find_existing_test_case_ids` returns empty for a different execution
    - Unique constraint prevents duplicate `(execution_id, test_case_id)` (verify
      DB error on direct insert attempt)
    - Batch insert with 50 rows (verify all inserted)
    - Batch insert with 1 row (verify single insert)
    - `BEFORE UPDATE` trigger sets `updated_at` correctly on update
    - Snapshot values (summary, description, priority) are correctly stored and
      retrieved

---

## Cross-Cutting Tasks

- [ ] 14. Wire authorization for test execution import -- `design.md#Components`
  - Register the `test_execution:import` permission code in the permission registry
  - Extend the project-scope authorization guard for the import endpoint:
    - `test_execution:import` -> Contributor, Editor, or Owner (Viewer excluded)
  - System Admin bypasses all role checks
  - Write unit tests:
    - Verify Viewer gets `403`
    - Verify Contributor gets accepted
    - Verify Editor gets accepted
    - Verify Owner gets accepted
    - Verify non-member gets `403`
    - Verify System Admin bypasses

- [ ] 15. Wire dependency injection for import components -- `design.md#Components`
  - Register `SqlTestCaseResultRepository` as the implementation of
    `TestCaseResultRepository`
  - Register `TestExecutionImportService` with its dependencies:
    - `TestCaseResultRepository`
    - `TestCaseRepository`
    - `TestRunRepository`
    - `TestExecutionRepository`
    - `ProjectRepository`
    - `AuthorizationService`
    - `ProjectMemberRepository`
  - Register `TestExecutionImportHandler` with `TestExecutionImportService`
  - If using a DI container, ensure all lifetimes/scopes are correct (e.g.,
    repositories scoped to request, service transient/singleton)
  - If using manual wiring in `main`, add the wiring code in the correct order

- [ ] 16. Manual QA checklist for test execution import --
       `requirements.md#US-1` through `US-4`
  - [ ] Import a single test case (verify snapshot: summary, description, priority
        resolved; status is NOT_TESTED)
  - [ ] Import multiple test cases in one request (verify all imported)
  - [ ] Import a test case with priority (verify priority name is snapshotted, not ID)
  - [ ] Import a test case with null priority (verify priority is null in result)
  - [ ] Import a test case with null description (verify description is null in result)
  - [ ] Import a test case, then import the same test case again (verify skipped, not
        duplicated)
  - [ ] Import with all already-imported IDs (verify 200 with empty imported, all
        skipped)
  - [ ] Import with mix of new and already-imported (verify partial import)
  - [ ] Attempt import with empty `test_case_ids` (verify 422)
  - [ ] Attempt import with non-integer `test_case_ids` (verify 422)
  - [ ] Attempt import with a test case ID not linked to the test run (verify 422 with
        `TEST_CASE_NOT_IN_TEST_RUN`)
  - [ ] Attempt import with a soft-deleted test case (verify 422)
  - [ ] Attempt import with a non-existent test case (verify 422)
  - [ ] Attempt import with an execution not linked to the given test run (verify 404)
  - [ ] Attempt import with a non-existent execution (verify 404)
  - [ ] Attempt import with a non-existent test run (verify 404)
  - [ ] Attempt import with a non-existent project (verify 404)
  - [ ] Attempt import as Viewer (verify 403)
  - [ ] Attempt import without `test_execution:import` permission (verify 403)
  - [ ] Attempt import as non-project-member (verify 403)
  - [ ] Import as System Admin on a project the admin is not a member of (verify 200)
  - [ ] Update the original test case summary after import (verify imported result
        retains old snapshot)
  - [ ] Update the original test case priority after import (verify imported result
        retains old snapshot)
  - [ ] Soft-delete a test case after it was imported (verify the result row remains
        with its snapshot)
  - [ ] Verify rate limit headers present on the response
  - [ ] Verify `X-Request-ID` header present on the response
  - [ ] Verify duplicate IDs in the same request array are handled (first processes,
        subsequent skip)
  - [ ] Verify unrecognised fields in request body are rejected (strict mode)

- [ ] 17. Verify rate limit configuration covers import endpoint --
       `requirements.md#Security Considerations`
  - Ensure the import endpoint is in the 30 req/min rate limit group (state-changing
    endpoints)
  - If rate limiting is per-endpoint, add the configuration for
    `POST .../executions/{eid}/import`
  - Verify `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` headers
  - Verify `Retry-After` header on `429` responses

- [ ] 18. Verify migration dependency order -- `design.md#Data Model`
  - `TEST_CASE_RESULTS` migration must run after:
    - `TEST_EXECUTIONS` (FK `execution_id`)
    - `TEST_CASES` (FK `test_case_id`)
    - `USERS` (FKs `created_by`, `updated_by`)
  - `TEST_EXECUTIONS` must have a `test_run_id` column referencing the test runs table
  - The test-run-to-test-case join table must exist (for validation)
  - `TEST_PRIORITIES` must exist (for resolving priority names at snapshot time)
