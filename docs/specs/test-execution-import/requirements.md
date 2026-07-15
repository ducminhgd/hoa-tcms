# Feature: Test Execution -- Selective Import

## Overview

After creating a Test Execution linked to a Test Run, the user needs to select which
Test Cases from that Test Run to include in the execution session. The import makes a
**snapshot** of each selected Test Case (copying its `summary`, `description`, and
`priority` name at the time of import) into the execution's result set. This decouples
execution results from later edits to the original test case -- the execution always
reflects what was tested, not what the test case says today.

Each imported Test Case becomes a row in `TEST_CASE_RESULTS` with an initial status of
`NOT_TESTED`. The import is idempotent: importing a test case that is already present in
the execution is silently skipped (no error, no duplicate).

The endpoint is a single `POST` action nested under the project, test run, and execution
resource path. The caller must have the `test_execution:import` system permission and be a
Contributor, Editor, or Owner of the project.

---

## User Stories

### US-1: Import Selected Test Cases into Execution

As a project member with the `test_execution:import` system permission and a project role
of Contributor, Editor, or Owner, I want to select a subset of Test Cases from the linked
Test Run and import them into the Execution as a snapshot, so that I can track per-test-case
execution results independently from future changes to the original Test Cases.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:import` permission (or a System Admin) sends a valid
  `POST /api/v1/projects/{projectId}/test-runs/{testRunId}/executions/{executionId}/import`
  request with `test_case_ids` (a non-empty array of test case IDs to import), THE SYSTEM
  SHALL, within a single database transaction:
  1. Verify the project exists and is not soft-deleted.
  2. Verify the test run exists, is not soft-deleted, belongs to the project, and is linked
     to the given execution.
  3. Verify the execution exists, is not soft-deleted, and belongs to the project.
  4. For each `test_case_id` in the request:
     a. Verify the test case exists, is not soft-deleted, and is a member of the linked
        test run (i.e., a row exists in the test-run-to-test-case join table).
     b. If the test case is not already present in `TEST_CASE_RESULTS` for this execution:
        read its current `summary`, `description`, and resolve its `priority` name from the
        `test_priorities` table. Insert a row into `TEST_CASE_RESULTS` with these snapshot
        values, `execution_id` set to the execution ID, `test_case_id` set to the test case
        ID, and `status` set to `NOT_TESTED`.
     c. If the test case is already present in `TEST_CASE_RESULTS` for this execution
        (identified by the unique pair `(execution_id, test_case_id)`), skip it silently
        (no error, no duplicate row).
  5. Return `200 OK` with an import summary listing which test cases were newly imported
     and which were skipped (already present).
- IF the user does not hold the `test_execution:import` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `test_execution:import` but is a Viewer of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden` (same generic message).
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the test run does not exist, is soft-deleted, belongs to a different project, or is
  not linked to the given execution, THE SYSTEM SHALL return `404 Not Found`.
- IF the execution does not exist, is soft-deleted, or belongs to a different project, THE
  SYSTEM SHALL return `404 Not Found`.
- IF `test_case_ids` is empty, missing, or contains non-integer values, THE SYSTEM SHALL
  return `422 Unprocessable Entity` with field-level validation details.
- IF any `test_case_id` does not exist, is soft-deleted, or is not a member of the linked
  test run, THE SYSTEM SHALL return `422 Unprocessable Entity` with error code
  `TEST_CASE_NOT_IN_TEST_RUN`, listing the specific invalid test case IDs and the reason
  for each rejection.

### US-2: Snapshot Semantics on Import

As a tester, I want the imported Test Case results to capture a snapshot of the test case's
`summary`, `description`, and `priority` at the time of import, so that later edits to the
original Test Case do not retroactively change the execution results I have already recorded.

**Acceptance Criteria (EARS)**

- WHEN a test case is imported, THE SYSTEM SHALL copy its current `summary` (trimmed) into
  `TEST_CASE_RESULTS.summary`.
- WHEN a test case is imported, THE SYSTEM SHALL copy its current `description` into
  `TEST_CASE_RESULTS.description`. If the test case's description is `NULL`, the snapshot
  description SHALL be `NULL`.
- WHEN a test case is imported, THE SYSTEM SHALL resolve the test case's `priority_id` to
  its priority name via a join on `test_priorities` at the time of import. If `priority_id`
  is `NULL` or the referenced priority is soft-deleted, the snapshot priority SHALL be
  `NULL`.
- IF the original Test Case's `summary`, `description`, or `priority_id` is later updated,
  the previously imported `TEST_CASE_RESULTS` rows SHALL NOT be affected (they retain the
  snapshot values as of the import time).
- IF a test case is soft-deleted after being imported into an execution, the existing
  `TEST_CASE_RESULTS` rows SHALL remain intact (the execution holds an independent copy).

### US-3: Idempotent Import (Skip Already-Imported)

As a project member, I want the import endpoint to be idempotent -- if I accidentally
include a test case that has already been imported into this execution, the system should
silently skip it rather than returning an error or creating a duplicate, so that the
client can safely submit the full desired set without tracking which ones were already
imported.

**Acceptance Criteria (EARS)**

- WHEN a `test_case_id` in the request already exists in `TEST_CASE_RESULTS` for the given
  execution (matching `(execution_id, test_case_id)` pair), THE SYSTEM SHALL skip it
  silently (no error, no duplicate insert).
- THE SYSTEM SHALL include skipped test case IDs in the response body under a `skipped`
  array so the client knows which test cases were already present and did not need to be
  re-imported. Each entry SHALL include the `test_case_id` and the string `"already_imported"`.
- IF the request contains only already-imported test case IDs, THE SYSTEM SHALL still return
  `200 OK` with an empty `imported` array and all IDs in the `skipped` array. This is a
  valid no-op request, not an error.
- THE SYSTEM SHALL prevent duplicate inserts at the database level with a `UNIQUE` constraint
  on `(execution_id, test_case_id)` as a defence-in-depth measure catching any race
  condition that bypasses the application-level check.

### US-4: Import Audit Trail

As a project member, I want to know who imported which test cases and when, so that I can
trace the origin of each test case result in the execution.

**Acceptance Criteria (EARS)**

- WHEN a test case is imported, THE SYSTEM SHALL set `created_by` and `updated_by` on the
  new `TEST_CASE_RESULTS` row to the authenticated user's ID.
- WHEN a test case is imported, THE SYSTEM SHALL set `created_at` and `updated_at` on the
  new `TEST_CASE_RESULTS` row to the current timestamp (via `DEFAULT NOW()` and the
  `BEFORE UPDATE` trigger).
- THE SYSTEM SHALL NOT update the audit fields on skipped (already-imported) test cases --
  skipped rows are untouched.
- The response body for newly imported test case results SHALL include `created_by`,
  `created_at`, `updated_by`, and `updated_at` so the client can display import provenance.

---

## Security Considerations

### Authentication

All endpoints require a valid authenticated session. Requests without a valid session
cookie return `401 Unauthorized` with error code `NOT_AUTHENTICATED`. This is enforced by
`AuthMiddleware` before any handler logic executes.

### Authorization Consistency

The system permission check (`test_execution:import`) is the first authorization gate;
the project membership and role check (Contributor, Editor, or Owner -- Viewer excluded)
is the second gate. System Admin implicitly holds all permissions and bypasses all gates.

A `403 Forbidden` response must use a generic message without revealing which gate was
triggered, to prevent information leakage about project existence or membership.

Authorization checks are performed against live data on every request -- not cached in the
session. If a user's role or permissions are changed, the new authorization takes effect on
their next request.

### CSRF Protection

The `POST` endpoint must be protected against CSRF. Session cookies must carry
`SameSite=Lax` (or stricter). The handler must verify the `Content-Type: application/json`
header to block simple form-based CSRF attacks.

### Input Validation

- `test_case_ids` must be a non-empty JSON array of positive integers.
- Duplicate IDs within the request array are accepted (the idempotency handling means
  the first occurrence imports or skips; subsequent occurrences are treated as
  already-imported skips).
- Each `test_case_id` is validated against the test run's actual members (must exist in
  the test-run-to-test-case join table).
- IDs that do not exist, are soft-deleted, or are not linked to the test run are
  rejected with specific detail about which ID failed and why.

### Same-Project Constraint

Every entity in the import chain must belong to the same project:
- The execution must belong to the project.
- The test run must belong to the project and be linked to the execution.
- Each test case must belong to the test run (which indirectly scopes it to the project
  since the test run is project-scoped).

Cross-project data injection is blocked at every validation step.

### Rate Limiting

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `POST .../executions/{eid}/import` | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

---

## Out of Scope

- **Re-import / refresh** (re-import updates snapshot fields while preserving result
  status, logs, and files -- separate feature `test-execution-reimport`).
- **Test case result update** (changing status, logs, attaching files to individual
  results -- separate feature `test-case-result-update`).
- **Test case result file attachments** (separate feature `test-case-result-files`).
- **Bulk delete of imported results** (removing results from an execution -- separate
  feature or part of execution management).
- **Import all test cases from test run** (bulk import without selection -- the `test_case_ids`
  array supports both individual selection and "select all" by providing all IDs, so a
  dedicated endpoint is not needed).
- **Test Execution CRUD** (creating, listing, updating, deleting executions -- separate
  feature `test-execution-crud`).
- **Test Run Cases** (adding/removing test cases to/from a test run -- separate feature
  `test-run-cases`).
- **UI views** (pages, forms, selection UI -- covered in `ui-*` specs).

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission code `test_execution:import` must be seeded in the
  `PERMISSIONS` table and assignable to roles.
- **IAM Users** -- `users.id` FK reference for `created_by` and `updated_by` audit columns.
- **Project CRUD** -- The `PROJECTS` table must exist; all entities are project-scoped.
- **Project Members** -- Project membership and roles (Contributor, Editor, Owner) are used
  for authorization scope checks.
- **Auth RBAC** -- System permission check for `test_execution:import`.
- **Auth Project Scope** -- Project membership scope check on the endpoint.
- **Test Case CRUD** -- The `TEST_CASES` table must exist; test case data (summary,
  description, priority_id) is read at import time for snapshotting.
- **Metadata Priorities** -- Priority names are resolved from `TEST_PRIORITIES` at import
  time for the snapshot.
- **Test Run Management** -- The test run table and test-run-to-test-case join table must
  exist. The test run must be linked to the execution (via a `test_run_id` FK on the
  `TEST_EXECUTIONS` table).
- **Test Execution CRUD** -- The `TEST_EXECUTIONS` table must exist with a `test_run_id` FK
  linking each execution to its parent test run.
