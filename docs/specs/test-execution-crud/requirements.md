# Feature: Test Execution CRUD

## Overview

Test Executions represent a discrete testing cycle linked to a specific Test Run. Each
execution captures who performed the testing (one or more testers) and inherits its test
cases from the linked Test Run at creation time via a snapshot import (see
`test-execution-import`). Test executions are always scoped under a Test Run, which is
itself scoped under a Project.

The feature supports full CRUD operations with role-based ownership rules: Contributors
can create executions and manage their own, while Owners and Editors can manage all
executions in the project.

---

## User Stories

### US-1: Create Test Execution

As a project member with the `test_execution:create` system permission and a project role
of Contributor, Editor, or Owner, I want to create a new test execution linked to a Test
Run with one or more testers assigned, so that I can track a testing cycle and record
results against the test cases imported from that Test Run.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:create` permission (or a System Admin) sends a valid
  `POST /api/v1/projects/{projectId}/test-runs/{runId}/executions` request with `name`
  and `tester_ids`, THE SYSTEM SHALL insert the execution into `TEST_EXECUTIONS` within a
  database transaction, link the selected testers via the `TEST_EXECUTION_TESTERS`
  junction table, import test cases from the linked Test Run as a snapshot (delegated to
  the test case import flow described in `test-execution-import`), set `created_by` and
  `updated_by` to the authenticated user's ID, and return `201 Created` with the
  execution representation and a `Location` header pointing to the new resource.
- IF the user does not hold the `test_execution:create` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `test_execution:create` but is a Viewer of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden` (same generic message).
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the Test Run does not exist, is soft-deleted, or belongs to a different project,
  THE SYSTEM SHALL return `404 Not Found`.
- IF `name` is empty, contains only whitespace, or exceeds 500 characters, THE SYSTEM
  SHALL return `422 Unprocessable Entity` with field-level validation details. Leading
  and trailing whitespace is trimmed before length validation and storage.
- IF an execution with the same `name` already exists in the same Test Run
  (case-insensitive comparison, excluding soft-deleted executions), THE SYSTEM SHALL
  return `409 Conflict` with error code `DUPLICATE_EXECUTION_NAME`.
- IF an execution with the same `name` exists but is soft-deleted, THE SYSTEM SHALL
  allow creation of the new execution.
- IF `tester_ids` is empty or missing, THE SYSTEM SHALL return
  `422 Unprocessable Entity` (at least one tester is required).
- IF any ID in `tester_ids` does not exist, is not ACTIVE, or is not a member of the
  project, THE SYSTEM SHALL return `422 Unprocessable Entity` with error code
  `INVALID_TESTER` and field-level details identifying the invalid user ID(s).
- IF the request body includes unrecognised fields, THE SYSTEM SHALL return
  `422 Unprocessable Entity` (strict mode -- rejects typos and unknown fields for
  defence-in-depth).

### US-2: List Test Executions

As a project member, I want to view a paginated list of test executions within a Test
Run, with the ability to search by name, so that I can browse and locate execution
cycles efficiently.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:read_list` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/test-runs/{runId}/executions`, THE SYSTEM SHALL
  return a paginated list of non-deleted test executions scoped to that Test Run, ordered
  by `id` descending (newest first).
- IF the user does not hold the `test_execution:read_list` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_execution:read_list` but is not a member of the project AND is
  not a System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the Test Run does not exist, is soft-deleted, or belongs to a different project,
  THE SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL support pagination via query parameters `page` (default 1, minimum 1,
  maximum 1000) and `limit` (default 25, minimum 1, maximum 100). Requests with
  `page > 1000` return `422`.
- THE SYSTEM SHALL include pagination metadata (`total`, `page`, `limit`) in the
  response.
- THE SYSTEM SHALL support an optional `search` query parameter that performs a
  case-insensitive substring match against `name`. The `search` value must not exceed
  255 characters; longer values return `422`. The characters `%`, `_`, and `\` in the
  search string are escaped (treated as literals, not ILIKE wildcards or escape
  characters).
- THE SYSTEM SHALL support an optional `sort` query parameter with values
  `-id` (default, newest first / descending), `id` (ascending, oldest first), `name`,
  `-name` (descending), `created_at`, `-created_at` (descending). Invalid sort values
  return `422`.
- WHILE an execution is soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL
  exclude it from the list.
- THE SYSTEM SHALL include the list of assigned testers (user IDs and usernames) in each
  list item for the UI to display tester names without a separate detail request.
- IF the user is a System Admin, THE SYSTEM SHALL return all non-deleted executions for
  the Test Run regardless of the admin's project membership (admin bypass).

### US-3: View Test Execution Detail

As a project member, I want to view the full details of a single test execution,
including the list of assigned testers, so that I can review the execution metadata
before viewing or recording results.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:read` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}`, THE SYSTEM
  SHALL return the execution fields (id, name, test_run_id, testers list, and audit
  timestamps), excluding soft-deleted executions.
- IF the user does not hold the `test_execution:read` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_execution:read` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the Test Run does not exist, is soft-deleted, or belongs to a different project,
  THE SYSTEM SHALL return `404 Not Found`.
- IF the execution does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found` (same message to avoid information leakage about soft-deleted records).
- THE SYSTEM SHALL resolve the testers list by joining `TEST_EXECUTION_TESTERS` with
  `USERS`, returning each tester's `user_id` and `username`.
- THE SYSTEM SHALL NOT return `deleted_at` or `deleted_by` in the response body.

### US-4: Update Test Execution

As a project member, I want to update a test execution's name and assigned testers, so
that I can keep the execution metadata accurate. As a Contributor I can only update
executions I created. As an Owner or Editor I can update any execution in the project.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:update` permission (or a System Admin) sends
  `PATCH /api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}` with one or
  more updatable fields (`name`, `tester_ids`), THE SYSTEM SHALL apply the changes within
  a database transaction, reconcile the tester assignments in the junction table, set
  `updated_by` to the authenticated user's ID, set `updated_at = NOW()`, and return
  `200 OK` with the updated execution representation.
- IF the user does not hold the `test_execution:update` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_execution:update` but is a Viewer of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_execution:update` and is a Contributor: THE SYSTEM SHALL
  verify that the execution's `created_by` matches the authenticated user's ID. If it
  does not match, THE SYSTEM SHALL return `403 Forbidden` with a message indicating that
  Contributors can only update their own executions.
- IF the user holds `test_execution:update` and is an Owner or Editor: THE SYSTEM SHALL
  allow updating any execution in the project regardless of `created_by`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the Test Run does not exist, is soft-deleted, or belongs to a different project,
  THE SYSTEM SHALL return `404 Not Found`.
- IF the execution does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF `name` is provided: it must contain at least one non-whitespace character and must
  not exceed 500 characters after trimming. Whitespace-only values return `422`.
- IF the updated `name` conflicts with another non-deleted execution in the same Test
  Run (case-insensitive comparison), THE SYSTEM SHALL return `409 Conflict` with
  `DUPLICATE_EXECUTION_NAME`.
- IF `tester_ids` is provided and empty: THE SYSTEM SHALL return
  `422 Unprocessable Entity` (at least one tester is required). Omitting `tester_ids`
  preserves the current set of testers.
- IF any ID in the new `tester_ids` is invalid (user does not exist, is not ACTIVE, or
  is not a member of the project), THE SYSTEM SHALL return `422 Unprocessable Entity`
  with error code `INVALID_TESTER`.
- IF no updatable fields are provided (empty body), THE SYSTEM SHALL return
  `422 Unprocessable Entity`.
- IF the request body includes unrecognised fields, THE SYSTEM SHALL reject the request
  with `422 Unprocessable Entity` (strict mode).
- THE SYSTEM SHALL NOT update `id`, `test_run_id`, `created_at`, `created_by`,
  `deleted_at`, or `deleted_by`.
- THE SYSTEM SHALL NOT allow changing the `test_run_id` of an execution (executions are
  permanently scoped to their Test Run).

### US-5: Delete Test Execution (Soft-delete)

As a project member, I want to soft-delete a test execution so that it is hidden from
all views while preserving its data and the linked test case results. As a Contributor I
can only delete executions I created. As an Owner or Editor I can delete any execution
in the project.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:delete` permission (or a System Admin) sends
  `DELETE /api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}`, THE SYSTEM
  SHALL set `deleted_at = NOW()` and `deleted_by = <current_user_id>` and return
  `204 No Content`.
- IF the user does not hold the `test_execution:delete` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_execution:delete` but is a Viewer of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_execution:delete` and is a Contributor: THE SYSTEM SHALL
  verify that the execution's `created_by` matches the authenticated user's ID. If it
  does not match, THE SYSTEM SHALL return `403 Forbidden` with a message indicating that
  Contributors can only delete their own executions.
- IF the user holds `test_execution:delete` and is an Owner or Editor: THE SYSTEM SHALL
  allow deleting any execution in the project regardless of `created_by`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the Test Run does not exist, is soft-deleted, or belongs to a different project,
  THE SYSTEM SHALL return `404 Not Found`.
- IF the execution does not exist or is already soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL NOT check for referential integrity constraints before soft-deleting
  an execution. Test case results that reference this execution are preserved (they
  remain with their original `execution_id`; they are hidden because queries filter on
  `WHERE execution.deleted_at IS NULL`).
- THE SYSTEM SHALL NOT hard-delete any record.
- After soft-delete, the execution is excluded from list, detail, and select endpoints.

### US-6: Select Test Executions (Dropdown)

As a project member with `test_execution:select` permission, I want to retrieve a
compact list of all non-deleted test executions within a Test Run (without pagination)
for use in dropdown and selection UI components, so that I can link executions to other
entities efficiently.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:select` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/test-runs/{runId}/executions/select`, THE SYSTEM
  SHALL return a flat array of all non-deleted executions in the Test Run, each
  containing only `id` and `name`, ordered by `name` ascending (case-insensitive).
- IF the user does not hold the `test_execution:select` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_execution:select` but is not a member of the project AND is
  not a System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the Test Run does not exist, is soft-deleted, or belongs to a different project,
  THE SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL NOT paginate this endpoint -- it is designed for dropdown population
  and returns all results.
- THE SYSTEM SHALL exclude soft-deleted executions from the results.
- THE SYSTEM SHALL cap the result at 500 executions; if a Test Run exceeds this limit,
  the response is truncated and a warning is logged, and an `X-Result-Truncated: true`
  response header is set.

---

## Security Considerations

### Authentication
All endpoints require a valid authenticated session. Requests without a valid session
cookie return `401 Unauthorized` with error code `NOT_AUTHENTICATED`. This applies to
all six execution endpoints and is enforced by `AuthMiddleware` before any handler logic
executes.

### Input Sanitization (XSS Prevention)
All user-supplied text fields (`name`) must be sanitized on input to strip disallowed
HTML tags and malicious script content, and output-encoded at the presentation layer to
prevent Cross-Site Scripting (XSS). This implements the project-wide policy defined in
PRD 5.3 (XSS prevention). Defence-in-depth requires both input sanitisation and output
encoding.

### CSRF Protection
All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must be protected against
Cross-Site Request Forgery (CSRF). Session cookies must carry the `SameSite=Lax`
attribute (or stricter). Implementations must verify `Content-Type` headers and/or
include anti-CSRF tokens for defense in depth.

### Authorization Consistency
The system permission check is the first authorization gate; the project membership and
role check is the second gate. For Contributors, an ownership check (`created_by` matches
the authenticated user) is a third gate on update and delete operations. System Admin
implicitly holds all permissions and bypasses all gates. A `403 Forbidden` response must
use a generic message without revealing which gate was triggered, to prevent information
leakage about project existence or membership. The Contributor ownership check is the one
exception: it returns a distinct message ("Contributors can only update/delete their own
executions") because the user already passed the project membership gate.

Authorization checks (system permissions and project membership) are performed against
live data on every request -- not cached in the session. If a user's role or permissions
are changed, the new authorization takes effect on their next request.

### Rate Limiting
All endpoints apply rate limiting configured at the infrastructure/middleware layer:

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../executions` (list) | 60 requests | per minute |
| `GET .../executions/{id}` (detail) | 60 requests | per minute |
| `GET .../executions/select` (dropdown) | 120 requests | per minute |
| `POST .../executions` (create) | 30 requests | per minute |
| `PATCH .../executions/{id}` (update) | 30 requests | per minute |
| `DELETE .../executions/{id}` (delete) | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### FK Validation (Test Run Existence and Scope)
The `test_run_id` is mandatory and must reference a non-deleted Test Run that belongs to
the same project. This validation is performed within the same database transaction as
the insert to prevent TOCTOU races. The Test Run's project scope is verified before
proceeding with any execution operation.

### Tester Validation (Project Membership)
All tester IDs provided in `tester_ids` must be validated: the user must exist, be
ACTIVE, and be a member of the project. This prevents assigning testers from outside
the project. The validation uses `SELECT ... FOR UPDATE` on the project membership rows
within the transaction to prevent TOCTOU races.

---

## Out of Scope

- **Test case import logic** (selecting which test cases to import, snapshot creation --
  covered in `test-execution-import`)
- **Test case re-import** (refreshing snapshot fields -- covered in `test-execution-reimport`)
- **Test case result update** (updating individual test case results within an execution
  -- covered in `test-case-result-update`)
- **Test case result file attachments** (covered in `test-case-result-files`)
- **Execution statistics** (aggregation of pass/fail counts -- deferred to a separate
  feature in a future phase)
- **Bulk create/update/delete** (only single-resource operations in Phase 1)
- **Execution history / versioning** (full audit log of field changes -- deferred to a
  cross-cutting audit feature in a future phase)
- **Execution restore** (clearing `deleted_at` / `deleted_by` -- deferred to a future
  phase; data is preserved but no restore endpoint exists in Phase 1)
- **Hard delete** (not implemented for any entity in Phase 1)
- **Execution copy / clone** (duplicate an existing execution -- separate feature)
- **UI views** (pages, forms, list views -- covered in `ui-*` specs)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission codes `test_execution:create`,
  `test_execution:read`, `test_execution:read_list`, `test_execution:update`,
  `test_execution:delete`, `test_execution:select` must be seeded in the `PERMISSIONS`
  table and assignable to roles.
- **IAM Users** -- `users.id` FK reference for `created_by`, `updated_by`, `deleted_by`
  audit columns. Tester validation checks user existence and ACTIVE status.
- **Project CRUD** -- The `PROJECTS` table must exist; project scope is validated for
  every execution operation.
- **Project Members** -- Project membership and roles (Owner, Editor, Contributor,
  Viewer) are used for authorization scope checks. Testers must be project members.
- **Auth RBAC** -- System permission checks for `test_execution:*` codes.
- **Auth Project Scope** -- Project membership scope check on all endpoints.
- **Test Run CRUD** -- The `TEST_RUNS` table must exist; every execution references a
  mandatory `test_run_id` FK.
- **Test Case CRUD** -- `TEST_CASES` table must exist; test cases are imported from the
  linked Test Run into the execution at creation time.
- **Test Run Cases** -- Test cases linked to a Test Run are the source for the execution
  snapshot import at creation time.
- **Test Execution Import** -- The import logic that snapshots test cases from the Test
  Run into the execution is invoked during execution creation. The CRUD spec calls the
  import flow but does not define it.
- **UI List Views** -- The executions list view follows the standard list-view pattern
  (checkbox column, action icons, ID/Name navigation, Add New button, filter controls).
