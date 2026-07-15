# Tasks: Test Run Cases

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `RunTestCase` entity -- `requirements.md#US-1`, `design.md#Components`
  - Fields: `run_id`, `test_case_id`, `added_by`, `added_at`
  - Constructor validates all IDs are positive; `added_at` defaults to current time
  - Factory method `RunTestCase::new(run_id, test_case_id, added_by)` produces the entity
    with `added_at = Utc::now()`
  - No framework imports; pure Rust struct + impl

- [ ] 2. Define domain exceptions for test run cases -- `design.md#Error Handling`
  - `TestCaseAlreadyInRunError` (carries conflicting test case IDs)
  - `TestCaseNotInRunError` (carries run_id and test_case_id)
  - `InvalidTestCaseError` (carries invalid test case IDs -- not found, deleted, or wrong
    project)
  - `RunNotFoundError` (test run does not exist or is soft-deleted)
  - `RunCasePermissionDenied` (Contributor ownership restriction)

---

## Layer 2 -- Application

- [ ] 3. Define `RunTestCaseRepository` interface (port) -- `design.md#Components`,
      `design.md#Data Model`
  - Methods:
    - `find_by_run(run_id, page, limit) -> Result<(Vec<RunTestCaseWithDetail>, u64)>` --
      paginated list with JOINed test case, category, and priority fields
    - `exists(run_id, test_case_id) -> Result<bool>` -- duplicate check
    - `insert_batch(entries: Vec<RunTestCase>) -> Result<()>` -- bulk insert
    - `delete(run_id, test_case_id) -> Result<bool>` -- returns whether a row was deleted
    - `test_run_exists_and_belongs(project_id, run_id) -> Result<i64>` -- returns
      `created_by` for ownership check; errors if not found or wrong project
    - `validate_test_case_ids(ids: &[i64], project_id: i64) -> Result<Vec<(i64, String)>>`
      -- returns valid (id, summary) pairs; missing IDs collected for error reporting
  - All methods accept a transaction handle for compositional use
  - Define `RunTestCaseWithDetail` struct: `test_case_id`, `summary`, `category_id`,
    `category_name`, `priority_id`, `priority_name`, `automated`, `added_by`, `added_at`

- [ ] 4. Implement `RunTestCaseService` -- `requirements.md#US-1` through `US-3`,
      `design.md#Sequence`
  - `list_cases(run_id, page, limit, current_user_id)`: validates `test_run:read`
    permission, checks project membership, calls `find_by_run`
  - `add_cases(project_id, run_id, test_case_ids, current_user_id)`: validates
    `test_run:update` permission, checks project membership (Contributor/Editor/Owner),
    checks Contributor ownership, acquires row lock, validates all IDs, checks for
    duplicates, inserts in batch, returns summary
  - `remove_case(project_id, run_id, test_case_id, current_user_id)`: validates
    `test_run:update` permission, checks project membership, checks Contributor ownership,
    deletes the junction row, returns `404` if not found
  - All methods check the required system permission via `AuthorizationService`

- [ ] 5. Define command/query/response DTOs -- `design.md#API Contract`
  - `AddCasesCommand` (test_case_ids: Vec<i64>)
  - `ListCasesQuery` (page: u32, limit: u32)
  - `AddedCaseEntry` (test_case_id: i64, summary: String)
  - `AddCasesResponse` (added: Vec<AddedCaseEntry>)
  - `RunCaseItem` (test_case_id, summary, category_id, category_name, priority_id,
    priority_name, automated, added_by, added_at)
  - `ListCasesResponse` (data: Vec<RunCaseItem>, meta: PaginationMeta)

- [ ] 6. Write unit tests for `RunTestCaseService` -- `requirements.md#US-1` through
      `US-4`
  - Table-driven tests covering:
    - Happy path for list (returns paginated results)
    - Happy path for add batch (single and multiple test cases)
    - Happy path for remove (case is removed, returns success)
    - Duplicate test case in add batch returns `409`
    - Invalid test case IDs (non-existent, soft-deleted, wrong project) returns `422`
    - Empty `test_case_ids` array returns `422`
    - Permission denial: missing `test_run:read` returns `403`
    - Permission denial: missing `test_run:update` returns `403`
    - Permission denial: Viewer role on POST/DELETE returns `403`
    - Contributor ownership check: non-owner Contributor returns `403`
    - Test run not found or soft-deleted returns `404`
    - Removing a test case not in the run returns `404`
    - System Admin bypasses all permission gates
    - All-or-nothing: if one test case in the batch is invalid, none are inserted

---

## Layer 3 -- Adapters (HTTP)

- [ ] 7. Implement `RunTestCaseHandler` -- `design.md#API Contract`, `design.md#Components`
  - Three handler methods: `list_cases`, `add_cases`, `remove_case`
  - Deserialize request bodies (JSON) and query params into DTOs
  - Validate path parameters (project_id, run_id, test_case_id are positive integers)
  - Call `RunTestCaseService` methods
  - Serialize responses with proper status codes (`200`, `201`, `204`, `403`, `404`,
    `409`, `422`)
  - Set `Location` header on `201 Created` is not applicable (cases are a sub-resource
    of the run; no singular GET endpoint for a case-in-run)

- [ ] 8. Register test run case routes in HTTP router -- `design.md#Route Registration`
  - `GET    /api/v1/projects/{projectId}/test-runs/{runId}/cases`          -> `list_cases`
  - `POST   /api/v1/projects/{projectId}/test-runs/{runId}/cases`          -> `add_cases`
  - `DELETE /api/v1/projects/{projectId}/test-runs/{runId}/cases/{testCaseId}` -> `remove_case`
  - All routes require session auth middleware
  - Route order: `/{testCaseId}` is a dynamic segment; no static segment collision

- [ ] 9. Write integration tests for HTTP handlers -- `design.md#API Contract`
  - Full request/response cycle with a test database
  - Test `200 OK` with paginated test case list including resolved category/priority names
  - Test `201 Created` with batch add response including summaries
  - Test `204 No Content` on successful remove
  - Test `409 Conflict` when adding a test case already in the run
  - Test `422 Unprocessable Entity` on empty `test_case_ids` array
  - Test `422 Unprocessable Entity` on invalid test case IDs (non-existent, wrong project)
  - Test `403 Forbidden` on missing permission
  - Test `403 Forbidden` on Contributor ownership restriction
  - Test `404 Not Found` on non-existent/soft-deleted test run
  - Test `404 Not Found` on removing a test case not in the run
  - Test pagination query parameters (page, limit)
  - Verify all-or-nothing behavior: no partial inserts when one ID in the batch is invalid

---

## Layer 4 -- Infrastructure

> **Note:** The `TEST_RUNS` table migration is owned by the `test-run-crud` feature.
> This feature only references the table; the migration is defined in
> `specs/test-run-crud/tasks.md`.

- [ ] 10. Create `TEST_RUN_TEST_CASES` database migration -- `design.md#Data Model`
  - Table definition with `run_id`, `test_case_id`, `added_by`, `added_at`
  - Composite primary key on `(run_id, test_case_id)`
  - Foreign keys with `ON DELETE RESTRICT` on `run_id` (to `test_runs`),
    `test_case_id` (to `test_cases`), `added_by` (to `users`)
  - Indexes: `idx_trtc_run_id`, `idx_trtc_test_case_id`, `idx_trtc_run_added` (covering)
  - Rollback migration: `DROP TABLE IF EXISTS test_run_test_cases`

- [ ] 11. Implement `SqlRunTestCaseRepository` -- `design.md#Components`
  - All methods from `RunTestCaseRepository` interface
  - `find_by_run`: JOIN query with `test_cases` (filter `deleted_at IS NULL`),
    LEFT JOIN `test_categories`, LEFT JOIN `test_priorities`; paginated with
    `ORDER BY added_at ASC`
  - `insert_batch`: multi-row `INSERT INTO test_run_test_cases (run_id, test_case_id,
    added_by) VALUES ...` using parameterized queries
  - `delete`: `DELETE FROM test_run_test_cases WHERE run_id = $1 AND test_case_id = $2`
    returning affected row count
  - `test_run_exists_and_belongs`: `SELECT id, created_by FROM test_runs WHERE id = $1
    AND project_id = $2 AND deleted_at IS NULL`
  - `validate_test_case_ids`: `SELECT id, summary FROM test_cases WHERE id = ANY($1)
    AND project_id = $2 AND deleted_at IS NULL`; compute missing IDs from the diff
  - Write unit tests with a test transaction:
    - Insert batch then list should return inserted cases
    - Insert duplicate should raise DB unique violation
    - Delete existing row should return true
    - Delete non-existent row should return false
    - Validate returns only matching cases for the project

---

## Verification and Cleanup

- [ ] 12. End-to-end verification and security hardening --
       `requirements.md#US-1` through `US-4`
  - Manual or automated walkthrough of all user stories:
    - Add test cases to a run, verify they appear in the list with resolved
      category/priority names
    - Remove a test case, verify it no longer appears
    - Verify batch add: multiple test cases are all added atomically
    - Verify duplicate rejection: adding an already-linked case returns `409`
    - Verify cross-project rejection: adding a case from a different project returns
      `422`
    - Verify soft-deleted test cases are excluded from list and rejected on add
    - Verify permission enforcement (each endpoint with insufficient permission or role)
    - Verify Contributor ownership: Contributor can modify own runs, cannot modify
      others' runs
    - Verify System Admin bypasses all gates
    - Verify pagination: page and limit query parameters work correctly
    - Verify concurrent additions are serialized (no phantom duplicates under load)
  - Run all unit and integration tests with `cargo test`
  - Verify the `SELECT ... FOR UPDATE` lock prevents concurrent-add race by writing a
    test that fires two concurrent batch adds and confirms no duplicate key violation
    reaches the client as a `500`

---

## Security and Hardening

> Tasks 13-14 apply security measures that match the project-wide patterns established
> by `project-crud` and `project-members`.

- [ ] 13. **Implement CSRF protection** -- requirements.md#security-considerations
  - Ensure session cookie carries `SameSite=Lax` (or stricter)
  - Ensure `POST` and `DELETE` endpoints reject requests without
    `Content-Type: application/json` (block simple form-based CSRF)
  - Integration test: verify `POST .../cases` without JSON content type is rejected

- [ ] 14. **Add authorization integration tests** -- design.md#security-requirements
  - Test that `GET .../cases` returns `403` for authenticated non-member with
    `test_run:read` permission
  - Test that `POST .../cases` returns `403` for project member with Viewer role
  - Test that `DELETE .../cases/{testCaseId}` returns `403` for project Editor on a
    run owned by someone else... wait, Editors can modify any run. Verify Editor CAN
    delete.
  - Test that `POST .../cases` returns `403` for project Contributor on a run owned
    by another user
  - Test that System Admin can list, add, and remove cases on any run regardless of
    membership or ownership
