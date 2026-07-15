# Tasks: Test Case Result Update

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `ResultStatus` enum with transition validation --
       `requirements.md#US-1`, `design.md#Status Transition State Machine`
  - Variants: `NotTested`, `InProgress`, `Pass`, `Fail`, `Warning`, `Ignore`
  - Display string mapping: NOT_TESTED, IN_PROGRESS, PASS, FAIL, WARNING, IGNORE
  - Method `transition(from: &ResultStatus, to: &ResultStatus) -> Result<ResultStatus, StatusTransitionError>`
    validates the transition against the state machine rules
  - `StatusTransitionError` carries the current status and a list of valid target statuses
  - No-no transitions: NOT_TESTED -> any terminal state; terminal -> terminal; any -> NOT_TESTED
  - Re-open transitions (PASS/FAIL/WARNING/IGNORE -> IN_PROGRESS) are allowed
  - Same-status transitions (no-op) are allowed
  - No framework imports; pure domain code

- [ ] 2. Implement `TestCaseResult` entity -- `requirements.md#US-1`, `design.md#Components`
  - Fields: `id`, `execution_id`, `test_case_id`, `summary`, `description`, `priority`,
    `status` (ResultStatus), `logs`, `tested_by`, audit columns
  - Method `apply_update(cmd: &UpdateResultCommand, current_user_id: i64) -> Result<(), DomainError>`
    validates the status transition (if `status` is provided), sets `status`, sanitizes and
    sets `logs`, sets `tested_by` if this is the first transition from NOT_TESTED (tested_by
    is NULL and new status is IN_PROGRESS), sets `updated_by` to `current_user_id`
  - Domain validation only; no ORM or framework imports
  - Unit-testable without a database (test state machine rules in isolation)

- [ ] 3. Define domain exceptions for test case results -- `design.md#Error Handling`
  - `StatusTransitionError` (current_status: ResultStatus, valid_targets: Vec<ResultStatus>)
  - `ResultNotFoundError`
  - `ResultPermissionDenied`
  - `ResultValidationError` (carries field-level details)

---

## Layer 2 -- Application

- [ ] 4. Define `TestCaseResultRepository` interface (port) -- `design.md#Components`
  - Methods:
    - `find_by_id(result_id: i64) -> Result<Option<TestCaseResult>>`
    - `find_with_chain(project_id, run_id, execution_id, result_id) -> Result<Option<TestCaseResult>>`
      (validates the full parent chain: result -> exec -> run -> project; JOINs across four
      tables; returns None if any link is broken or any entity is soft-deleted)
    - `update(result: &TestCaseResult, current_user_id: i64) -> Result<()>` (sets `updated_by`
      and delegates to DB)
    - `touch_execution(execution_id: i64) -> Result<()>` (updates parent execution's
      `updated_at` to reflect activity)
  - All methods operate on domain entities (not DTOs)
  - `find_with_chain` is the primary lookup for authorization (confirms result belongs to
    the specified project/run/execution hierarchy)

- [ ] 5. Define `ExecutionTesterRepository` interface (port) -- `design.md#Components`
  - Method: `is_assigned_tester(execution_id: i64, user_id: i64) -> Result<bool>`
  - Checks `execution_testers` junction table for the given execution_id and user_id
  - Used in fine-grained authorization: assigned testers can update any result in their
    execution regardless of project role

- [ ] 6. Define command/response DTOs -- `design.md#API Contract`
  - `UpdateResultCommand` (status: Option<String>, result_logs: Option<Option<String>>)
    - The double Option for `result_logs` distinguishes between "not provided" (None) and
      "explicitly set to null" (Some(None))
  - `ResultDetailResponse` (id, execution_id, test_case_id, summary, description, priority,
    status, result_logs, tested_by, created_by, created_at, updated_by, updated_at)
  - Domain entity -> DTO mapping happens at the handler/adapter boundary

- [ ] 7. Implement `TestCaseResultService` -- `requirements.md#US-1` through `US-4`,
       `design.md#Sequence`
  - Single method: `update_result(project_id, run_id, execution_id, result_id, cmd, current_user_id)`
  - Loads the result via `find_with_chain` to validate the full parent hierarchy
  - Checks `test_execution:update` system permission via `AuthorizationService`
  - Resolves fine-grained authorization:
    1. System Admin -> allowed
    2. Project Owner or Editor -> allowed
    3. Project Contributor + owns result (`created_by == current_user_id`) -> allowed
    4. Project Contributor + assigned tester -> allowed
    5. Assigned tester (any project role, including Viewer) -> allowed
    6. Otherwise -> `403 Forbidden`
  - If `status` is provided, validates the transition via `ResultStatus::transition()`
  - Calls `result.apply_update(cmd, current_user_id)` to apply domain changes
  - Begins transaction, calls `update()` + `touch_execution()`, commits
  - Maps result to `ResultDetailResponse`

- [ ] 8. Write unit tests for `TestCaseResultService` -- `requirements.md#US-1` through `US-4`
  - Table-driven tests covering:
    - Happy path: NOT_TESTED -> IN_PROGRESS (sets `tested_by`)
    - Happy path: IN_PROGRESS -> PASS (does not overwrite `tested_by`)
    - Happy path: re-open PASS -> IN_PROGRESS (preserves `tested_by`)
    - Happy path: update `result_logs` without changing status
    - Happy path: update both `status` and `result_logs` atomically
    - Invalid transition from NOT_TESTED directly to PASS
    - Invalid transition from PASS directly to FAIL
    - Missing system permission -> `403`
    - Contributor updating another contributor's result (not assigned tester) -> `403`
    - Assigned tester (Viewer role) updating a result -> allowed
    - System Admin bypassing all checks
    - Result not found -> `404`
    - Empty body (no fields provided) -> `422`
    - `result_logs` exceeding 10000 chars -> `422`
    - No-op status (same status as current) -> allowed, still records `updated_by`

---

## Layer 3 -- Adapters (HTTP)

- [ ] 9. Implement `TestCaseResultHandler` -- `design.md#API Contract`, `design.md#Components`
  - Single handler method: `update_result`
  - Validates path parameters (project_id, run_id, execution_id, result_id) are present and
    positive integers
  - Deserializes request body into `UpdateResultCommand` with strict mode (reject unknown
    fields)
  - Validates at least one of `status` or `result_logs` is provided
  - Validates `status` value is a recognised variant
  - Validates `result_logs` length does not exceed 10000 characters
  - Validates the project, run, execution, and result exist and the parent chain is intact
    (called before the service as a preliminary check, with the service doing a second
    authoritative check within its transaction for TOCTOU safety)
  - Calls `TestCaseResultService::update_result()`
  - On success: serializes the `ResultDetailResponse` and returns `200 OK`
  - On error: maps domain errors to HTTP status codes per the error handling table

- [ ] 10. Register result update route in HTTP router -- `design.md#Route Registration`
  - `PATCH /api/v1/projects/{projectId}/test-runs/{runId}/executions/{executionId}/results/{resultId}`
    -> `update_result`
  - Route requires session auth middleware
  - Route registered after test-runs and test-executions routes exist
  - Route order: this is a deeply nested path; no path ordering conflicts expected with
    sibling routes

- [ ] 11. Write integration tests for the result update HTTP handler --
       `design.md#API Contract`
  - Full request/response cycle with a test database
  - Test `200 OK` on valid status transition with response body matching the expected shape
  - Test `200 OK` on re-open (PASS -> IN_PROGRESS) and verify `tested_by` is unchanged
  - Test `tested_by` is set on first NOT_TESTED -> IN_PROGRESS transition
  - Test `tested_by` is NOT overwritten on subsequent status updates
  - Test `422` on invalid transition with `INVALID_STATUS_TRANSITION` code and details
    listing valid targets
  - Test `422` on empty body (no fields)
  - Test `422` on `result_logs` exceeding 10000 characters
  - Test `422` on unrecognised `status` value (e.g., "SKIPPED")
  - Test `422` on unrecognised fields in body (strict mode rejection)
  - Test `403` for user without `test_execution:update` permission
  - Test `403` for project Contributor updating another contributor's result (not assigned
    tester) -- distinct message
  - Test `200` for assigned tester (Viewer role on project) updating any result
  - Test `200` for System Admin updating any result
  - Test `404` for non-existent project
  - Test `404` for broken parent chain (result belongs to different execution)
  - Test `404` for soft-deleted result
  - Test that parent execution's `updated_at` is touched on every result update
  - Test concurrent updates (two testers updating the same result) -- last write wins,
    no error

---

## Layer 4 -- Infrastructure

- [ ] 12. Implement `SqlTestCaseResultRepository` -- `design.md#Components`
  - All methods from `TestCaseResultRepository` interface
  - `find_by_id`: SELECT with `WHERE id = $1 AND deleted_at IS NULL`
  - `find_with_chain`: JOIN across test_case_results, test_executions, test_runs to validate
    the full parent chain
    ```sql
    SELECT r.*
    FROM test_case_results r
    JOIN test_executions e ON r.execution_id = e.id AND e.deleted_at IS NULL
    JOIN test_runs t ON e.run_id = t.id AND t.deleted_at IS NULL
    WHERE r.id = $1
      AND r.deleted_at IS NULL
      AND e.id = $2
      AND t.id = $3
      AND (t.project_id = $4 OR (t.project_id IS NULL AND $4 IS NULL))
    ```
  - `update`: UPDATE test_case_results SET result = $2, logs = $3, tested_by = COALESCE($4,
    tested_by), updated_by = $5 WHERE id = $1 AND deleted_at IS NULL
    - `tested_by` uses COALESCE to ignore the new value if already set (only applies on first
      transition)
    - `updated_at` is auto-set by the BEFORE UPDATE trigger
  - `touch_execution`: UPDATE test_executions SET updated_at = NOW() WHERE id = $1
  - All queries use parameterized statements exclusively
  - Map DB rows to domain `TestCaseResult` entities

- [ ] 13. Implement `SqlExecutionTesterRepository` -- `design.md#Components`
  - `is_assigned_tester(execution_id, user_id)`: SELECT 1 FROM execution_testers WHERE
    execution_id = $1 AND user_id = $2
  - Returns true if any row is found
  - No soft-delete on junction tables (hard delete)

---

## Verification & Cleanup

- [ ] 14. End-to-end verification -- `requirements.md#US-1` through `US-4`
  - Walkthrough of all four user stories with a real database
  - Verify NOT_TESTED -> IN_PROGRESS -> PASS lifecycle
  - Verify re-open (PASS -> IN_PROGRESS -> FAIL)
  - Verify `tested_by` is set once and never overwritten
  - Verify `updated_by` changes on every update
  - Verify all invalid transitions are rejected with appropriate error details
  - Verify `result_logs` can be updated independently of status
  - Verify permission enforcement for each authorization level
  - Verify parent execution `updated_at` is touched
  - Verify soft-deleted results are not updatable
  - Verify audit trail: `updated_by` + `updated_at` + `result` column tell the full story

- [ ] 15. Update `specs/README.md` -- `specs/README.md`
  - Mark `test-case-result-update` as having completed specs (requirements.md, design.md,
    tasks.md)

---

## Security & Hardening

- [ ] 16. **Implement XSS input sanitization** -- requirements.md#security-considerations
  - Strip disallowed HTML tags from `result_logs` field before storage, at the HTTP handler
    boundary
  - Integration test: submit `<script>alert('xss')</script>` in `result_logs`, verify the
    stored value has HTML tags stripped

- [ ] 17. **Implement CSRF protection** -- requirements.md#security-considerations
  - Ensure session cookie carries `SameSite=Lax` (or stricter)
  - Ensure `PATCH` endpoint rejects requests without `Content-Type: application/json`
    (to block simple form-based CSRF)
  - Integration test: verify `PATCH` without JSON content type is rejected

- [ ] 18. **Add authorization integration tests** -- design.md#security-requirements
  - Test that a user with `test_execution:update` permission but no project membership
    or tester assignment gets `403` for a project-scoped run
  - Test that a project Viewer (not assigned tester) gets `403`
  - Test that a project Viewer assigned as a tester gets `200` (can update results)
  - Test that a non-member assigned as a tester gets `200` (tester assignment overrides
    lack of project membership)
  - Test that a Contributor updating a result they did not create (and are not assigned
    tester) gets a distinct `403` message
  - Test that System Admin bypasses all authorization checks
