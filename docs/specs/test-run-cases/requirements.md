# Feature: Test Run Cases

## Overview

Test Run Cases manages the set of test cases assigned to a test run. Users with appropriate
permissions can list, batch-add, and remove test cases from a run. The junction table
`TEST_RUN_TEST_CASES` serves as the authoritative link between runs and cases, and is the
source of truth from which test executions import their case snapshots (see
`test-execution-import`). The feature supports a tabular UI where users select multiple
test cases from the project and add them in a single request.

---

## User Stories

### US-1: View Test Cases in a Test Run

As a project member, I want to see all test cases assigned to a test run, along with their
summary, category, priority, automation status, and audit trail, so that I can review what
the run covers.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_run:read` permission sends
  `GET /api/v1/projects/{projectId}/test-runs/{runId}/cases`, THE SYSTEM SHALL return a
  paginated list of test cases linked to the run, each with `test_case_id`, `summary`,
  `category_id`, `category_name`, `priority_id`, `priority_name`, `automated`, `added_by`,
  and `added_at`.
- IF the user lacks the `test_run:read` system permission, THE SYSTEM SHALL return
  `403 Forbidden`.
- IF the user is not a member of the project and is not a System Admin, THE SYSTEM SHALL
  return `403 Forbidden`.
- IF the test run does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the test run belongs to a different project, THE SYSTEM SHALL return `404 Not Found`
  (same message as non-existent run).
- THE SYSTEM SHALL support pagination via query parameters `page` (default 1, minimum 1)
  and `limit` (default 25, minimum 1, maximum 100).
- THE SYSTEM SHALL include pagination metadata (`total`, `page`, `limit`) in the response.
- THE SYSTEM SHALL resolve `category_name` and `priority_name` via LEFT JOIN; a null or
  soft-deleted category/priority yields `null` for the name field.

### US-2: Add Test Cases to a Test Run (Batch)

As a project member with write access, I want to add one or more test cases to a test run
in a single batch request, so that I can efficiently build the run's test suite from the
tabular selection interface.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_run:update` permission and appropriate project role sends
  `POST /api/v1/projects/{projectId}/test-runs/{runId}/cases` with a `test_case_ids`
  array, THE SYSTEM SHALL add all listed test cases to the run and return `201 Created`
  with a summary of added test case IDs.
- IF the user lacks the `test_run:update` system permission, THE SYSTEM SHALL return
  `403 Forbidden`.
- IF the user is a Viewer of the project, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user is a Contributor who does not own the test run (`created_by` mismatch),
  THE SYSTEM SHALL return `403 Forbidden` with a distinct message.
- IF the test run does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF any `test_case_id` does not exist, is soft-deleted, or belongs to a different project
  than the test run, THE SYSTEM SHALL return `422 Unprocessable Entity` with field-level
  error details indicating which IDs are invalid.
- IF any `test_case_id` is already in the run, THE SYSTEM SHALL return `409 Conflict`
  with the list of conflicting IDs.
- IF the `test_case_ids` array is empty or missing, THE SYSTEM SHALL return
  `422 Unprocessable Entity`.
- THE SYSTEM SHALL execute all insertions within a single database transaction (all-or-nothing).

### US-3: Remove a Test Case from a Test Run

As a project member with write access, I want to remove a single test case from a test run,
so that I can correct mistakes or adjust the run scope.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_run:update` permission and appropriate project role sends
  `DELETE /api/v1/projects/{projectId}/test-runs/{runId}/cases/{testCaseId}`, THE SYSTEM
  SHALL remove the test case from the run and return `204 No Content`.
- IF the user lacks the `test_run:update` system permission, THE SYSTEM SHALL return
  `403 Forbidden`.
- IF the user is a Viewer of the project, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user is a Contributor who does not own the test run (`created_by` mismatch),
  THE SYSTEM SHALL return `403 Forbidden` with a distinct message.
- IF the test run does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the test case is not currently in the run, THE SYSTEM SHALL return `404 Not Found`
  (same message as non-existent link).
- THE SYSTEM SHALL hard-delete the junction row (no soft-delete on junction tables).

### US-4: Data Integrity Enforcement on Case Management

As a test run manager, I want the system to prevent invalid additions that would corrupt
the run's data, so that I can trust the integrity of my test runs.

**Acceptance Criteria (EARS)**

- WHEN adding test cases, THE SYSTEM SHALL validate that every test case is active
  (`deleted_at IS NULL`) and belongs to the same project as the test run. Invalid cases
  are rejected at the validation gate before any writes occur.
- WHEN adding test cases, THE SYSTEM SHALL reject the entire batch if any test case is
  already in the run (duplicate prevention).
- WHEN removing a test case, THE SYSTEM SHALL verify the test case is currently linked
  to the run before attempting deletion.
- WHEN a Contributor attempts to modify a test run they did not create, THE SYSTEM SHALL
  reject the request with `403 Forbidden` and a distinct message indicating the ownership
  restriction.
- THE SYSTEM SHALL use `SELECT ... FOR UPDATE` on the test run row within the transaction
  to serialize concurrent modifications to the same run's case set.
- THE SYSTEM SHALL NOT cascade or propagate to test executions when a case is removed from
  a run (execution snapshots are independent once imported).

---

## Security Considerations

### Input Sanitization (XSS Prevention)
No user-supplied free-text fields are accepted by this feature (only IDs are submitted).
However, the list endpoint returns `summary` from the `TEST_CASES` table. The `summary`
field must be sanitized on input at the test-case-crud boundary and output-encoded at the
presentation layer. See PRD section 5.3 for the project-wide XSS prevention policy.

### CSRF Protection
All state-changing endpoints (`POST .../cases`, `DELETE .../cases/{testCaseId}`) must be
protected against Cross-Site Request Forgery (CSRF). Session cookies must carry the
`SameSite=Lax` attribute (or stricter). Requests with `Content-Type: application/json`
must verify the `Content-Type` header to block simple form-based CSRF attacks.

### Authorization Layering
Three independent authorization gates apply:

1. System permission check (`test_run:read` for GET, `test_run:update` for POST/DELETE)
2. Project membership and role check (Owner, Editor, or Contributor for mutations; any
   role for reads; Viewer excluded from all mutations)
3. Contributor ownership check (on POST and DELETE only): if the user's project role is
   Contributor, verify `test_run.created_by` matches the authenticated user ID

All three gates must pass (or the caller must be a System Admin, who bypasses all gates).
A `403 Forbidden` response must use a generic message that does not distinguish between
"missing system permission" and "wrong project role". The Contributor ownership check is
an exception: it returns a distinct message because the user has already passed the
project membership gate.

### Race Condition Protection
Concurrent additions to the same test run are serialized using `SELECT ... FOR UPDATE` on
the test run row. This prevents phantom duplicates where two requests both pass the
duplicate check before either inserts.

### Rate Limiting
All endpoints are protected by rate limiting:
- `GET .../cases`: 60 req/min (read)
- `POST .../cases`: 30 req/min (state-changing)
- `DELETE .../cases/{testCaseId}`: 30 req/min (state-changing)

---

## Out of Scope

- **Test Run CRUD** — creating, updating, deleting test runs (covered in `test-run-crud`
  spec).
- **Test Execution Import** — copying test cases from a run into an execution snapshot
  (covered in `test-execution-import` spec). This feature only defines the data that the
  import reads from.
- **Reordering test cases within a run** — the junction table does not capture ordering;
  cases are returned in the order they were added.
- **Bulk remove** — removal is single-case only. A bulk remove endpoint is deferred.
- **Run statistics** — aggregation of case result statuses (covered in
  `test-run-statistics` spec).
- **UI views** — the tabular interface, checkbox selection, and drag-and-drop (covered in
  `ui-*` specs).

---

## Dependencies

- **TEST_RUNS table** — must exist with `id`, `project_id`, `created_by`, `deleted_at`
  columns (from `test-run-crud` feature).
- **TEST_CASES table** — must exist with `id`, `project_id`, `summary`, `category_id`,
  `priority_id`, `automated`, `deleted_at` columns (from `test-case-crud` feature).
- **TEST_CATEGORIES table** — must exist with `id`, `name` columns (from
  `metadata-categories` feature).
- **TEST_PRIORITIES table** — must exist with `id`, `name` columns (from
  `metadata-priorities` feature).
- **USERS table** — must exist with `id` column (from `iam-users` feature) for
  `added_by` FK.
- **IAM Auth** — session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** — the permission codes `test_run:read` and `test_run:update` must
  be seeded in the `PERMISSIONS` table and assignable to roles. These codes are shared
  with the `test-run-crud` feature.
- **Project Members** — project membership and role resolution must be available to
  enforce the second authorization gate.
