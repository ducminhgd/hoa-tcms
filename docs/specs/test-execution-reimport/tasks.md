# Tasks: Test Execution Re-import

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 2 -- Application

- [ ] 1. Define DTOs for re-import operations -- `design.md#Components`
  - `ReimportSingleResult`:
    - `data: TestCaseResultDetail` (id, execution_id, test_case_id, summary, description,
      priority_id, priority_name, status, result_logs, created_by, created_at, updated_by,
      updated_at)
    - `meta: { changed: bool }`
  - `ReimportBulkResult`:
    - `data: { imported_count: u64, skipped_count: u64, failed_count: u64, failures: Vec<ReimportFailure> }`
    - `meta: { total_results: u64 }`
  - `ReimportFailure`:
    - `result_id: i64`
    - `reason: String`
  - `SourceTestCase` (lightweight read model):
    - `id: i64`, `summary: String`, `description: Option<String>`, `priority_id: Option<i64>`

- [ ] 2. Extend `TestCaseResultRepository` interface with re-import methods --
      `design.md#Components`
  - `find_by_execution_id(execution_id: i64) -> Option<TestCaseResult>` -- returns the
    single result linked to an execution, including `status`
  - `find_all_by_execution_id(execution_id: i64) -> Vec<TestCaseResult>` -- returns all
    non-deleted results for an execution, ordered by `id`
  - `update_snapshot(result_id: i64, summary: &str, description: Option<&str>,
    priority_id: Option<i64>, updated_by: i64)` -- updates the three snapshot columns and
    `updated_by` (does NOT touch status, result_logs, or file associations)
  - `find_result_with_priority_name(result_id: i64) -> Option<TestCaseResultDetail>` --
    re-fetches a result with resolved `priority_name` via LEFT JOIN on `test_priorities`
  - `batch_find_source_test_cases(test_case_ids: &[i64]) -> Vec<SourceTestCase>` --
    fetches current summary, description, priority_id from `test_cases` for a set of IDs.
    Includes soft-deleted rows.
  - All methods accept a transaction context

- [ ] 3. Implement `ExecutionReimportService` -- `requirements.md#US-1` through `US-3`,
      `design.md#Sequence`
  - `reimport_single(project_id, run_id, execution_id, current_user_id)`:
    - Checks `test_execution:reimport` system permission via `AuthorizationService`
    - Checks user is Contributor, Editor, or Owner of the project (Viewer rejected)
    - If Contributor: checks ownership of the Test Execution (if ownership rule applies;
      exact check TBD by `test-execution-crud`). Owners and Editors skip.
    - Begins database transaction
    - Within transaction:
      a. Fetches the test case result for this execution via
         `TestCaseResultRepository::find_by_execution_id(execution_id)`.
         If None -> `404 RESULT_NOT_FOUND` (rollback)
      b. Checks result status. If != `NOT_TESTED` ->
         `409 RESULT_ALREADY_STARTED` (rollback)
      c. Fetches source Test Case via `batch_find_source_test_cases` (single-item batch
         for reuse). If hard-deleted (not found) -> `500 INTERNAL_ERROR` (rollback)
      d. Compares `summary`, `description`, `priority_id` between result and source:
         - If all match -> no-op; set `changed = false`
         - If any differs -> calls `update_snapshot(...)` with new values; set
           `changed = true`
      e. Re-fetches the result with resolved `priority_name` via
         `find_result_with_priority_name(result_id)`
    - Commits transaction
    - Returns `ReimportSingleResult` with `meta.changed`
  - `reimport_all(project_id, run_id, execution_id, current_user_id)`:
    - Same authorization checks as `reimport_single`
    - Begins database transaction
    - Within transaction:
      a. Fetches all results via `find_all_by_execution_id(execution_id)`
      b. Collects unique `test_case_id` values; fetches source Test Cases in batch
      c. Initializes counters: `imported = 0`, `skipped = 0`, `failed = 0`,
         `failures = []`
      d. For each result:
         - If status != `NOT_TESTED`: `skipped++`, continue
         - If source Test Case not found in batch: `failed++`,
           `failures.push({result_id, reason: "Source test case not found"})`, continue
         - Compares snapshot fields with source values
         - If any differs: calls `update_snapshot(...)`
         - `imported++` (counted regardless of whether a write occurred)
      e. Computes `total_results = imported + skipped + failed`
    - Commits transaction
    - Returns `ReimportBulkResult`
  - System Admin bypasses all permission, membership, and ownership checks

- [ ] 4. Write unit tests for `ExecutionReimportService` --
      `requirements.md#US-1` through `US-3`
  - Table-driven tests with mock `TestCaseResultRepository`, mock `AuthorizationService`,
    mock `ProjectMemberRepository`
  - Happy path: single re-import updates all three snapshot fields
  - Happy path: single re-import with changed summary only (description and priority_id
    unchanged from source)
  - Happy path: single re-import with changed description only
  - Happy path: single re-import with priority_id changed from null to value
  - Happy path: single re-import with priority_id changed from value to null
  - No-op: single re-import where all fields already match -> `meta.changed = false`,
    no UPDATE executed
  - No-op: single re-import where summary and description match but priority_id differs
    (change detected, UPDATE executed)
  - Denied: result status is PASS -> `409 RESULT_ALREADY_STARTED`
  - Denied: result status is FAIL -> `409 RESULT_ALREADY_STARTED`
  - Denied: result status is IN_PROGRESS -> `409 RESULT_ALREADY_STARTED`
  - Denied: result status is WARNING -> `409 RESULT_ALREADY_STARTED`
  - Denied: result status is IGNORE -> `409 RESULT_ALREADY_STARTED`
  - Allowed: result status is NOT_TESTED -> re-import proceeds
  - Edge case: source Test Case soft-deleted -> still serves as source (values read)
  - Edge case: source Test Case hard-deleted (not in batch) -> single mode `500`,
    bulk mode counted as failure
  - Bulk mode: 5 NOT_TESTED + 3 PASS results -> imported=5, skipped=3, failed=0
  - Bulk mode: all results are PASS -> imported=0, skipped=N, failed=0
  - Bulk mode: zero results in execution -> imported=0, skipped=0, failed=0, total_results=0
  - Bulk mode: one result hard-deleted source -> imported=4, skipped=3, failed=1,
    failures contains one entry
  - Bulk mode: summary is the only field that changed for each row -> all imported
  - Bulk mode: no fields changed for any row -> all imported (counted as imported
    without writes)
  - Permission denial: missing `test_execution:reimport`
  - Permission denial: Viewer role rejected
  - Permission denial: not a project member
  - Permission denial: Contributor fails ownership check (if applicable)
  - Acceptance: Contributor passes ownership check (if applicable)
  - Acceptance: Owner/Editor can re-import any execution
  - System Admin bypasses all checks
  - Transaction rollback: DB error during single re-import
  - Transaction rollback: DB error during bulk re-import (all changes rolled back)

---

## Layer 3 -- Adapters (HTTP)

- [ ] 5. Implement `ReimportHandler` -- `design.md#API Contract`, `design.md#Components`
  - Single HTTP handler method attached to `POST /.../reimport`
  - Deserializes path parameters (`projectId`, `runId`, `executionId`) as integers
  - Parses optional `scope` query parameter: defaults to `"single"`, accepts `"single"`
    or `"all"`. Any other value -> `422 Unprocessable Entity`
  - Validates project exists and is not soft-deleted (via project repository or shared
    guard)
  - Validates Test Run exists, belongs to project, and is not soft-deleted (via Test Run
    repository)
  - Validates Test Execution exists, belongs to Test Run, and is not soft-deleted (via
    Test Execution repository)
  - Any of these validations fail -> `404 Not Found` (same generic message for all)
  - Based on `scope`:
    - `"single"` -> calls `ExecutionReimportService::reimport_single(...)`
    - `"all"` -> calls `ExecutionReimportService::reimport_all(...)`
  - Serializes response:
    - Single mode: `200 OK` with `ReimportSingleResult`
    - Bulk mode: `200 OK` with `ReimportBulkResult`
  - Maps service/domain errors to HTTP status codes:
    - Permission/role/ownership denial -> `403 Forbidden`
    - Execution/result not found -> `404 Not Found`
    - Result already started (409) -> `409 Conflict` with `RESULT_ALREADY_STARTED`
    - Validation error (invalid scope) -> `422 Unprocessable Entity`
    - Internal error -> `500 Internal Server Error`
  - Request body is rejected if present (endpoint expects no body); an unexpected body
    returns `422 Unprocessable Entity`

- [ ] 6. Register re-import route in HTTP router -- `design.md#Route Registration`
  - `POST /api/v1/projects/{projectId}/test-runs/{runId}/executions/{executionId}/reimport`
  - Requires session auth middleware
  - Route registration must come after the Test Execution CRUD routes (which define
    `/{executionId}`). The `/reimport` path is a static suffix on the execution resource
    and does not conflict with dynamic path segments.

- [ ] 7. Write integration tests for re-import HTTP handler -- `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back)
  - Test single re-import: snapshot fields updated, status and result_logs preserved
  - Test single re-import response includes resolved `priority_name`
  - Test single re-import with `meta.changed: true` when fields differ
  - Test single re-import with `meta.changed: false` when fields are identical
  - Test single re-import rejects non-NOT_TESTED status with `409 Conflict`
  - Test single re-import on non-existent execution -> `404 Not Found`
  - Test single re-import on execution in wrong project -> `404 Not Found`
  - Test single re-import on execution in wrong run -> `404 Not Found`
  - Test bulk re-import: correct counts (imported, skipped, failed)
  - Test bulk re-import with mixed statuses: NOT_TESTED refreshed, others skipped
  - Test bulk re-import with zero NOT_TESTED results: imported=0
  - Test bulk re-import with all NOT_TESTED results: all imported
  - Test bulk re-import with soft-deleted source Test Case: values still read and applied
  - Test that `updated_at` changes when snapshot fields are modified
  - Test that `updated_at` does NOT change when fields are unchanged (no-op)
  - Test that `result_logs` is preserved after re-import
  - Test that `status` is preserved (remains NOT_TESTED) after successful re-import
  - Test that files attached to the result are preserved (no file deletions or changes)
  - Test `403 Forbidden` on missing `test_execution:reimport` permission
  - Test `403 Forbidden` on Viewer role
  - Test `403 Forbidden` on Contributor failing ownership check (if applicable)
  - Test that System Admin can re-import any execution
  - Test `422` on invalid `scope` parameter (e.g., `scope=invalid`, `scope=changed`)
  - Test `422` when request body is present (unexpected body)
  - Test `401 Unauthorized` when session cookie is missing
  - Test rate limit headers present on response

---

## Layer 4 -- Infrastructure

- [ ] 8. Extend `SqlTestCaseResultRepository` with re-import query methods --
      `design.md#Sequence`
  - `find_by_execution_id(execution_id)`:
    ```sql
    SELECT tcr.*
    FROM test_case_results tcr
    JOIN test_executions te ON tcr.execution_id = te.id
    WHERE te.id = $1 AND te.deleted_at IS NULL AND tcr.deleted_at IS NULL;
    ```
    Returns at most one row (each execution has at most one result in single-mode scenario).

  - `find_all_by_execution_id(execution_id)`:
    ```sql
    SELECT tcr.id, tcr.test_case_id, tcr.summary, tcr.description,
           tcr.priority_id, tcr.status
    FROM test_case_results tcr
    WHERE tcr.execution_id = $1 AND tcr.deleted_at IS NULL
    ORDER BY tcr.id;
    ```

  - `update_snapshot(result_id, summary, description, priority_id, updated_by)`:
    ```sql
    UPDATE test_case_results
    SET summary = $2,
        description = $3,
        priority_id = $4,
        updated_by = $5
    WHERE id = $1;
    ```
    Note: `updated_at` is set by the BEFORE UPDATE trigger. `status`, `result_logs`,
    `test_case_id`, `execution_id`, `created_by`, `created_at` are not touched.

  - `find_result_with_priority_name(result_id)`:
    ```sql
    SELECT tcr.*, tp.name AS priority_name
    FROM test_case_results tcr
    LEFT JOIN test_priorities tp
      ON tcr.priority_id = tp.id AND tp.deleted_at IS NULL
    WHERE tcr.id = $1;
    ```

  - `batch_find_source_test_cases(test_case_ids)`:
    ```sql
    SELECT id, summary, description, priority_id
    FROM test_cases
    WHERE id = ANY($1);
    ```
    No `deleted_at IS NULL` filter (soft-deleted Test Cases are valid sources).

  - Use parameterized queries exclusively
  - Write unit tests with a test database (one test transaction per case):
    - `update_snapshot` modifies summary, description, priority_id
    - `update_snapshot` sets priority_id to null
    - `update_snapshot` does not modify status or result_logs
    - `batch_find_source_test_cases` returns soft-deleted Test Cases
    - `batch_find_source_test_cases` returns empty for hard-deleted or never-existed IDs
    - `find_result_with_priority_name` resolves priority name correctly
    - `find_result_with_priority_name` returns `null` for priority_name when priority is
      soft-deleted
    - `find_by_execution_id` returns the single result for the execution
    - `find_all_by_execution_id` returns all results, ordered by id, excluding
      soft-deleted results

- [ ] 9. Add `test_execution:reimport` permission code to permissions seed migration --
      `design.md#New Permission Code`
  - Add 1 new row to the permissions seed data with
    `INSERT ... ON CONFLICT (code) DO NOTHING`:
    `('test_execution:reimport', 'Re-import Test Execution')`
  - Let the database auto-assign IDs
  - If permissions are seeded in a single migration file, append to that file
  - Verify idempotency: `ON CONFLICT (code) DO NOTHING`

---

## Cross-Cutting Tasks

- [ ] 10. Wire dependency injection for re-import components -- `design.md#Components`
  - Register `ExecutionReimportService` with its dependencies:
    (`TestCaseResultRepository`, `AuthorizationService`, `ProjectMemberRepository`,
    `TestExecutionRepository`)
  - Register `ReimportHandler` with `ExecutionReimportService`
  - If using a DI container, ensure all lifetimes/scopes are correct
  - If using manual wiring in `main`, add the wiring code in the correct order
  - Verify that `SqlTestCaseResultRepository` already extends `TestCaseResultRepository`;
    if not, update the existing registration to include the new methods

- [ ] 11. Verify rate limit configuration covers the re-import endpoint --
      `requirements.md#Security Considerations`
  - Single mode (`scope=single` or omitted): 30 req/min
  - Bulk mode (`scope=all`): 10 req/min
  - Configure rate limiting at the endpoint level; the mode (single vs bulk) can be
    distinguished by the presence of the `scope=all` query parameter
  - If rate limiting cannot distinguish by query parameter, apply the stricter limit
    (10 req/min) to the entire endpoint
  - Verify `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` headers
  - Verify `Retry-After` header on `429` responses

- [ ] 12. Write API documentation for re-import endpoint -- `design.md#API Contract`
  - OpenAPI 3.x spec for `POST /reimport`
  - Document: path, method, query parameters, all possible response codes and bodies
  - Include examples for both single and bulk mode responses
  - Document the `RESULT_ALREADY_STARTED` error condition and the scope=all behaviour
  - Document the permission requirement (`test_execution:reimport`)

- [ ] 13. Manual QA checklist for test execution re-import --
      `requirements.md#US-1` through `US-3`
  - [ ] Single re-import: update a Test Case (change summary, description, priority)
    then re-import and verify snapshot fields match current Test Case
  - [ ] Single re-import: verify status remains "NOT_TESTED" after refresh
  - [ ] Single re-import: verify result_logs is preserved after refresh
  - [ ] Single re-import: verify attached files are preserved after refresh
  - [ ] Single re-import: verify `meta.changed: true` when fields were updated
  - [ ] Single re-import: verify `meta.changed: false` when fields were unchanged
  - [ ] Single re-import: attempt on PASS result -> verify 409 Conflict
  - [ ] Single re-import: attempt on FAIL result -> verify 409 Conflict
  - [ ] Single re-import: attempt on IN_PROGRESS result -> verify 409 Conflict
  - [ ] Single re-import: verify 404 on non-existent execution
  - [ ] Single re-import: verify 403 for Viewer
  - [ ] Single re-import: verify System Admin can re-import
  - [ ] Bulk re-import: create execution with 10 results, update 3 Test Cases, run
    bulk re-import -> verify 3 refreshed, 7 unchanged (but counted as imported)
  - [ ] Bulk re-import: set 3 results to PASS, keep 7 as NOT_TESTED -> verify
    imported=7, skipped=3, failed=0
  - [ ] Bulk re-import: verify response includes `total_results` that matches
    imported + skipped + failed
  - [ ] Bulk re-import: soft-delete a source Test Case, re-import -> verify values
    still read from soft-deleted row
  - [ ] Verify `updated_at` changes only when snapshot fields modified
  - [ ] Verify `updated_by` set to authenticated user on re-import
  - [ ] Verify rate limit headers present
  - [ ] Verify invalid `scope` parameter returns 422
