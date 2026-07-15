# Feature: Test Case Result Update

## Overview

Test Case Result Update allows testers to independently update the status, logs, and attached
files of each Test Case Result within a Test Execution. Results progress through a defined
status lifecycle (NOT_TESTED -> IN_PROGRESS -> PASS/FAIL/WARNING/IGNORE) with the ability to
re-open completed results for re-testing. A full audit trail captures every status change.
Permission to update a result is granted to project Owners, Editors, Contributors (own
results), and testers explicitly assigned to the execution.

---

## User Stories

### US-1: Update Test Case Result Status

As a tester assigned to a Test Execution, I want to update a Test Case Result's status to
IN_PROGRESS, PASS, FAIL, WARNING, or IGNORE, so that I can record the outcome of each test
case execution step.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:update` permission sends `PATCH
  /api/v1/projects/{pid}/test-runs/{rid}/executions/{eid}/results/{resultId}` with a valid
  `status` field, THE SYSTEM SHALL validate the status transition, update the result,
  record the user who performed the change, and return `200 OK` with the updated result
  representation.
- IF the status transition is NOT_TESTED -> IN_PROGRESS, THE SYSTEM SHALL accept it and
  set `tested_by` to the current user's ID on this first transition away from NOT_TESTED.
- IF the status transition is IN_PROGRESS -> PASS, FAIL, WARNING, or IGNORE, THE SYSTEM
  SHALL accept it.
- IF the status transition is PASS, FAIL, WARNING, or IGNORE -> IN_PROGRESS (re-open),
  THE SYSTEM SHALL accept it. The `tested_by` field is NOT modified on re-open; it retains
  the original tester's ID.
- IF the status transition is INVALID (e.g., NOT_TESTED -> PASS directly, or WARNING ->
  FAIL without passing through IN_PROGRESS), THE SYSTEM SHALL return `422 Unprocessable
  Entity` with error code `INVALID_STATUS_TRANSITION` and details describing the valid
  transitions from the current status.
- IF `status` is set to its current value (no-op), THE SYSTEM SHALL still return `200 OK`
  and record the update for audit consistency.
- IF the user lacks the `test_execution:update` system permission, THE SYSTEM SHALL return
  `403 Forbidden`.
- IF the user holds the system permission but is not a project member (Owner/Editor/
  Contributor) of the test run's project AND is not an assigned tester on the execution,
  THE SYSTEM SHALL return `403 Forbidden` (same generic message).
- IF the user is a Contributor on the project but is not the creator of the result AND is
  not an assigned tester, THE SYSTEM SHALL return `403 Forbidden` (ownership restriction).
- IF the Test Case Result is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.

### US-2: Record Test Logs During Execution

As a tester executing test cases, I want to write or update execution logs for a Test Case
Result, so that I can document observed behavior, reproduction steps, error messages, or
test evidence as free-form text.

**Acceptance Criteria (EARS)**

- WHEN a user with permission sends `PATCH .../results/{resultId}` with a `result_logs`
  field, THE SYSTEM SHALL store the text and return `200 OK` with the updated result
  representation.
- IF `result_logs` exceeds 10000 characters, THE SYSTEM SHALL return `422 Unprocessable
  Entity` with field-level validation details.
- IF `result_logs` is provided as `null`, THE SYSTEM SHALL clear the logs (set to NULL).
- IF `result_logs` is omitted from the request body, THE SYSTEM SHALL preserve the
  current value.
- THE SYSTEM SHALL sanitize `result_logs` on input (strip disallowed HTML tags) as a
  defence-in-depth XSS measure before storage.
- WHEN `result_logs` is updated alongside a status change in the same request, THE SYSTEM
  SHALL apply both changes atomically within a single database transaction.

### US-3: Update Status and Logs Together

As a tester completing a test execution run, I want to set the final status (PASS/FAIL/
WARNING/IGNORE) and record my execution observations in a single request, so that my
workflow is efficient and the update is atomic.

**Acceptance Criteria (EARS)**

- WHEN a user sends `PATCH .../results/{resultId}` with both `status` and `result_logs`,
  THE SYSTEM SHALL apply both fields atomically in a single transaction and return `200 OK`
  with the full updated result.
- IF either field fails validation (invalid transition for `status`, or `result_logs` too
  long), THE SYSTEM SHALL reject the entire request with `422 Unprocessable Entity` and
  roll back any partial changes.
- IF the request body contains only unrecognised fields (neither `status` nor
  `result_logs`), THE SYSTEM SHALL return `422 Unprocessable Entity`.
- THE SYSTEM SHALL reject unrecognised fields in the request body (strict mode -- rejects
  typos and unknown fields for defence-in-depth).

### US-4: View Result Update History

As a QA lead reviewing an execution, I want to see an audit trail of who changed each
result's status and when, so that I can trace the testing progress, identify bottlenecks,
and maintain accountability for result decisions.

**Acceptance Criteria (EARS)**

- WHEN any Test Case Result is updated via `PATCH .../results/{resultId}`, THE SYSTEM
  SHALL set `updated_by` to the authenticated user's ID and `updated_at` to the current
  timestamp.
- WHEN the result transitions from NOT_TESTED to IN_PROGRESS for the first time, THE
  SYSTEM SHALL set `tested_by` to the authenticated user's ID (this field is NOT modified
  on subsequent updates).
- THE SYSTEM SHALL log every status change as an audit event (at minimum, the combination
  of `updated_by` + `updated_at` + `old_status` -> `new_status` should be recoverable
  from the database record and is returned in the API response).
- THE SYSTEM SHALL include `updated_by` in the PATCH response, allowing the UI to display
  "last updated by X at Y" on the result detail view.
- The audit trail is satisfied by the existing audit columns (`created_by`, `created_at`,
  `updated_by`, `updated_at`) on the `TEST_CASE_RESULTS` table and the `status` change
  embodied in the `updated_at` timestamp and `result` column value.

---

## Security Considerations

### Input Sanitization (XSS Prevention)
The `result_logs` field is free-form text and must be sanitized on input to strip
disallowed HTML tags and malicious script content (implementing PRD 5.3 XSS prevention).
Output-encoding is applied at the presentation layer. Defence-in-depth requires both input
sanitisation and output encoding.

### CSRF Protection
The `PATCH` endpoint is state-changing and must be protected against CSRF. Session cookies
must carry `SameSite=Lax` (or stricter). The handler must verify `Content-Type:
application/json` to block simple form-based CSRF attacks.

### Authorization Layering
Three authorization gates apply:

1. System permission check (`test_execution:update`)
2. Project membership check: Owner, Editor, or Contributor on the project that owns the
   test run (when the test run has a `project_id`; skipped if the run is project-less)
3. Fine-grained access:
   - Owners and Editors of the project can update any result in any execution under the
     project's test runs
   - Contributors can update results they created, or results on executions where they are
     assigned as a tester
   - Assigned testers on the execution (via `execution_testers` junction table) can update
     any result in that execution, even if their project role is Viewer or they are not a
     project member
   - System Admin bypasses all checks

The `403 Forbidden` response must use a generic message for missing system permission or
project role. The Contributor ownership restriction returns a distinct message ("You can
only update your own Test Case Results").

### Rate Limiting
The result update endpoint allows 60 req/min (state-changing, but high churn during active
test execution).

### Concurrent Updates
Per FR-45, concurrent updates by multiple testers use last-write-wins semantics. No
optimistic locking (e.g., no ETag-based conditional PUT) for MVP. The database's row-level
locking ensures atomic writes.

---

## Out of Scope

- **File attachments on Test Case Results** (upload, download, delete -- covered in
  `test-case-result-files` spec)
- **Test Execution CRUD** (create, read, update, delete executions -- covered in
  `test-execution-crud` spec)
- **Importing test cases into an execution** (covered in `test-execution-import` spec)
- **Re-import (snapshot refresh)** while preserving results, logs, files (covered in
  `test-execution-reimport` spec)
- **Setting `IGNORE` with a mandatory reason** (the `IGNORE` status is accepted; requiring
  a reason is a UI-level enforcement, not a backend constraint)
- **Bulk result updates** (updating multiple results in one request -- deferred to a
  future phase)
- **Soft-delete of Test Case Results** (individual result soft-delete is not supported;
  results are removed when their parent execution is soft-deleted, per cascade-hide
  semantics FR-53)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; the endpoint requires an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission code `test_execution:update` must be seeded in the
  `PERMISSIONS` table and assignable to roles.
- **Test Execution CRUD** -- Test Executions must exist and the `TEST_EXECUTIONS` and
  `EXECUTION_TESTERS` tables must be populated before results can be updated.
- **Test Run CRUD** -- Test Runs must exist and the `TEST_RUNS` table must be populated
  (the route is nested under a test run).
- **Project Members** -- Project membership lookups via `PROJECT_MEMBERS` are required
  for authorization when the test run is project-scoped.
- **Test Execution Import** -- Test Case Results are created during the import step
  (`test-execution-import` spec). This spec only handles updates to already-imported
  results.
