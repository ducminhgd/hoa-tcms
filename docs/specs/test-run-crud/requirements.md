# Feature: Test Run CRUD

## Overview

Test Runs represent a planned execution cycle within a project. Each test run belongs to a
single project and groups a set of test cases for execution. A test run captures metadata
such as who the run reports to, the default tester assigned, an optional version string
(e.g., "v2.3.1"), planning dates, and free-form notes.

Test runs can optionally be linked to a test plan. When linked, the plan must belong to the
same project. Test runs are independently manageable -- creating, updating, and deleting a
test run does not affect other test runs or the linked plan.

The feature supports full CRUD operations with role-based ownership rules: Contributors can
create test runs and manage their own, while Owners and Editors can manage all test runs in
the project.

---

## User Stories

### US-1: Create Test Run

As a project member with the `test_run:create` system permission and a project role of
Contributor, Editor, or Owner, I want to create a new test run within a project, so that I
can define the test execution scope with its summary, assigned personnel, version, planning
dates, and optional plan linkage.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_run:create` permission (or a System Admin) sends a valid
  `POST /api/v1/projects/{projectId}/test-runs` request with `summary` and `report_to`
  and optional fields (`plan_id`, `default_tester`, `version`, `notes`, `planned_start_date`,
  `planned_end_date`), THE SYSTEM SHALL insert the test run into `TEST_RUNS` within a
  database transaction (to prevent TOCTOU races on the duplicate-summary check and FK
  validation), scope it to the given `project_id`, set `created_by` and `updated_by` to the
  authenticated user's ID, and return `201 Created` with the test run representation and a
  `Location` header pointing to the new resource.

- IF the user does not hold the `test_run:create` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).

- IF the user holds `test_run:create` but is a Viewer of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (same generic message).

- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.

- IF `summary` is empty, contains only whitespace, or exceeds 500 characters, THE SYSTEM
  SHALL return `422 Unprocessable Entity` with field-level validation details. Leading and
  trailing whitespace is trimmed before length validation and storage.

- IF a test run with the same `summary` already exists in the same project (case-insensitive
  comparison, excluding soft-deleted test runs), THE SYSTEM SHALL return `409 Conflict`
  with error code `DUPLICATE_TEST_RUN_SUMMARY`.

- IF a test run with the same `summary` exists but is soft-deleted, THE SYSTEM SHALL allow
  creation of the new test run (the soft-deleted record does not prevent re-creation).

- IF `plan_id` is provided and non-null: THE SYSTEM SHALL validate that a non-deleted test
  plan with that ID exists AND its `project_id` matches the test run's `project_id`. If the
  plan does not exist, is soft-deleted, or belongs to a different project, THE SYSTEM SHALL
  return `422 Unprocessable Entity` with error code `INVALID_PLAN` and a message indicating
  the plan is invalid for this project.

- IF `report_to` is provided: THE SYSTEM SHALL validate that an active user (not soft-deleted)
  with that ID exists. If the user does not exist or is soft-deleted, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with error code `INVALID_USER` and a field-level detail for
  `report_to`.

- IF `default_tester` is provided and non-null: THE SYSTEM SHALL validate that an active user
  (not soft-deleted) with that ID exists. If the user does not exist or is soft-deleted, THE
  SYSTEM SHALL return `422 Unprocessable Entity` with error code `INVALID_USER` and a
  field-level detail for `default_tester`.

- IF `version` is provided and exceeds 100 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity`.

- IF `notes` is provided and exceeds 10000 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity`. An empty string `""` is stored as-is (distinct from `null`).
  Sending `"notes": null` explicitly clears the field (sets it to `NULL` in the DB). Omitting
  `notes` from the request body defaults to `null`.

- IF `planned_start_date` or `planned_end_date` is provided: THE SYSTEM SHALL accept an ISO
  8601 date string (`YYYY-MM-DD`). If the date format is invalid, THE SYSTEM SHALL return
  `422 Unprocessable Entity`.

- IF both `planned_start_date` and `planned_end_date` are provided and `planned_end_date` is
  before `planned_start_date`, THE SYSTEM SHALL return `422 Unprocessable Entity` with
  error code `INVALID_DATE_RANGE`.

- IF the request body includes unrecognised fields, THE SYSTEM SHALL return
  `422 Unprocessable Entity` (strict mode -- rejects typos and unknown fields for
  defence-in-depth).

### US-2: List Test Runs

As a project member, I want to view a paginated list of test runs in the current project,
with the ability to filter by plan and search on summary, so that I can browse and locate
test runs efficiently.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_run:read_list` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/test-runs`, THE SYSTEM SHALL return a paginated
  list of non-deleted test runs scoped to that project, ordered by `id` descending
  (newest first).

- IF the user does not hold the `test_run:read_list` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.

- IF the user holds `test_run:read_list` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.

- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.

- THE SYSTEM SHALL support pagination via query parameters `page` (default 1, minimum 1,
  maximum 1000) and `limit` (default 25, minimum 1, maximum 100). Requests with
  `page > 1000` return `422`.

- THE SYSTEM SHALL include pagination metadata (`total`, `page`, `limit`) in the response.

- THE SYSTEM SHALL support an optional `plan_id` query parameter that filters test runs to
  only those linked to the given plan. If the plan does not exist, is soft-deleted, or
  belongs to a different project, the result set is empty (not an error).

- THE SYSTEM SHALL support an optional `search` query parameter that performs a
  case-insensitive substring match against `summary`. The `search` value must not exceed
  255 characters; longer values return `422`. The characters `%` and `_` and `\` in the
  search string are escaped (treated as literals, not ILIKE wildcards or escape characters).

- THE SYSTEM SHALL support an optional `sort` query parameter with values
  `-id` (default, newest first / descending), `id` (ascending, oldest first),
  `summary`, `-summary` (descending), `created_at`, `-created_at` (descending).
  Invalid sort values return `422`.

- THE SYSTEM SHALL include resolved `report_to_username`, `report_to_fullname`,
  `default_tester_username`, and `default_tester_fullname` in each list item (resolved via
  JOIN on the `users` table).

- WHILE a test run is soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL exclude it
  from the list.

- IF the user is a System Admin, THE SYSTEM SHALL return all non-deleted test runs for the
  project regardless of the admin's project membership (admin bypass).

### US-3: View Test Run Detail

As a project member, I want to view the full details of a single test run, including
resolved user names for report-to and default tester, so that I can review the complete
test run information before editing or executing it.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_run:read` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/test-runs/{id}`, THE SYSTEM SHALL return the test
  run fields (id, summary, project_id, plan_id, report_to with resolved username/fullname,
  default_tester with resolved username/fullname, version, notes, planned_start_date,
  planned_end_date, and audit timestamps -- created_at, created_by, updated_at, updated_by),
  excluding soft-deleted test runs.

- IF the user does not hold the `test_run:read` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.

- IF the user holds `test_run:read` but is not a member of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.

- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.

- IF the test run does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`
  (same message to avoid information leakage about soft-deleted records).

- THE SYSTEM SHALL resolve `report_to_username` and `report_to_fullname` by joining `users`
  on `report_to`. If the user is soft-deleted, the names SHALL still be returned (audit
  integrity).

- THE SYSTEM SHALL resolve `default_tester_username` and `default_tester_fullname` by
  joining `users` on `default_tester`. If `default_tester` is `NULL`, both name fields SHALL
  be `null`. If the user is soft-deleted, the names SHALL still be returned.

- THE SYSTEM SHALL NOT return `deleted_at` or `deleted_by` in the response body (these are
  internal fields).

### US-4: Update Test Run

As a project member, I want to update a test run's fields, so that I can keep the test run
metadata accurate. As a Contributor I can only update test runs I created. As an Owner or
Editor I can update any test run in the project.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_run:update` permission (or a System Admin) sends
  `PATCH /api/v1/projects/{projectId}/test-runs/{id}` with one or more updatable fields
  (`summary`, `plan_id`, `report_to`, `default_tester`, `version`, `notes`,
  `planned_start_date`, `planned_end_date`), THE SYSTEM SHALL apply the changes within a
  database transaction (to prevent TOCTOU races on the duplicate-summary check and FK
  validation), set `updated_by` to the authenticated user's ID, set `updated_at = NOW()`,
  and return `200 OK` with the updated test run representation.

- IF the user does not hold the `test_run:update` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.

- IF the user holds `test_run:update` but is a Viewer of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.

- IF the user holds `test_run:update` and is a Contributor: THE SYSTEM SHALL verify that
  the test run's `created_by` matches the authenticated user's ID. If it does not match,
  THE SYSTEM SHALL return `403 Forbidden` with a message indicating that Contributors can
  only update their own test runs.

- IF the user holds `test_run:update` and is an Owner or Editor: THE SYSTEM SHALL allow
  updating any test run in the project regardless of `created_by`.

- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.

- IF the test run does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`
  (per FR-54c: UPDATE on soft-deleted rows is rejected).

- IF the updated `summary` conflicts with another non-deleted test run in the same project
  (case-insensitive comparison), THE SYSTEM SHALL return `409 Conflict` with
  `DUPLICATE_TEST_RUN_SUMMARY`.

- IF `plan_id` is provided: THE SYSTEM SHALL validate it against the same project. Sending
  `null` explicitly clears the plan reference. Omitted preserves the current value. Same
  validation rules as US-1 (must exist, non-deleted, same project).

- IF `report_to` is provided: THE SYSTEM SHALL validate the user exists and is not
  soft-deleted. Same validation rules as US-1.

- IF `default_tester` is provided: THE SYSTEM SHALL validate if non-null. Sending `null`
  explicitly clears the default tester reference. Omitted preserves the current value.

- IF no updatable fields are provided (empty body), THE SYSTEM SHALL return
  `422 Unprocessable Entity`.

- IF the request body includes unrecognised fields, THE SYSTEM SHALL reject the request with
  `422 Unprocessable Entity` (strict mode).

- THE SYSTEM SHALL NOT update `id`, `project_id`, `created_at`, `created_by`, `deleted_at`,
  or `deleted_by`.

- THE SYSTEM SHALL NOT allow changing the `project_id` of a test run (test runs are
  permanently scoped to their project).

- IF `summary` is provided: it must contain at least one non-whitespace character and must
  not exceed 500 characters after trimming leading/trailing whitespace. Whitespace-only
  values return `422`.

- IF `version` is provided: must not exceed 100 characters. Values exceeding 100 characters
  return `422`.

- IF `notes` is provided: omitting the field preserves the current value. Sending
  `"notes": null` explicitly clears it (sets to `NULL`). Sending `"notes": ""` stores an
  empty string. Values exceeding 10000 characters return `422`.

- IF `planned_start_date` or `planned_end_date` is provided and the resulting pair violates
  `planned_end_date >= planned_start_date` (when both are non-null), THE SYSTEM SHALL return
  `422 Unprocessable Entity` with error code `INVALID_DATE_RANGE`.

### US-5: Delete Test Run (Soft-delete)

As a project member, I want to soft-delete a test run so that it is hidden from all views
while preserving its data. As a Contributor I can only delete test runs I created. As an
Owner or Editor I can delete any test run in the project.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_run:delete` permission (or a System Admin) sends
  `DELETE /api/v1/projects/{projectId}/test-runs/{id}`, THE SYSTEM SHALL set
  `deleted_at = NOW()` and `deleted_by = <current_user_id>` and return `204 No Content`.

- IF the user does not hold the `test_run:delete` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.

- IF the user holds `test_run:delete` but is a Viewer of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.

- IF the user holds `test_run:delete` and is a Contributor: THE SYSTEM SHALL verify that
  the test run's `created_by` matches the authenticated user's ID. If it does not match,
  THE SYSTEM SHALL return `403 Forbidden` with a message indicating that Contributors can
  only delete their own test runs.

- IF the user holds `test_run:delete` and is an Owner or Editor: THE SYSTEM SHALL allow
  deleting any test run in the project regardless of `created_by`.

- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.

- IF the test run does not exist or is already soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.

- THE SYSTEM SHALL NOT check for referential integrity constraints before soft-deleting a
  test run. Test runs are parent entities for test run cases and test executions; those
  child entities remain and are hidden via query filtering (joining through the test run
  and filtering on `test_run.deleted_at IS NULL`).

- THE SYSTEM SHALL NOT hard-delete any record.

- After soft-delete, the test run is excluded from list, detail, and select endpoints.

### US-6: Select Test Runs (Dropdown)

As a project member with `test_run:select` permission, I want to retrieve a compact list
of all non-deleted test runs in the project (without pagination) for use in dropdown and
selection UI components, so that I can link test runs to other entities efficiently.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_run:select` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/test-runs/select`, THE SYSTEM SHALL return a flat
  array of all non-deleted test runs in the project, each containing only `id` and
  `summary`, ordered by `summary` ascending (case-insensitive).

- IF the user does not hold the `test_run:select` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.

- IF the user holds `test_run:select` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.

- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.

- THE SYSTEM SHALL NOT paginate this endpoint -- it is designed for dropdown population and
  returns all results.

- THE SYSTEM SHALL exclude soft-deleted test runs from the results.

- THE SYSTEM SHALL cap the result at 1000 test runs; if a project exceeds this limit, the
  response is truncated and a warning is logged.

---

## Security Considerations

### Authentication
All endpoints require a valid authenticated session. Requests without a valid session cookie
return `401 Unauthorized` with error code `NOT_AUTHENTICATED`. This applies to all six
test run endpoints and is enforced by `AuthMiddleware` before any handler logic executes.

### Input Sanitization (XSS Prevention)
All user-supplied text fields (`summary`, `version`, `notes`) must be sanitized on input to
strip disallowed HTML tags and malicious script content, and output-encoded at the
presentation layer to prevent Cross-Site Scripting (XSS). This implements the project-wide
policy defined in PRD 5.3 (XSS prevention). Defence-in-depth requires both input
sanitisation and output encoding.

### CSRF Protection
All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must be protected against
Cross-Site Request Forgery (CSRF). Session cookies must carry the `SameSite=Lax` attribute
(or stricter). Implementations must verify `Content-Type` headers and/or include anti-CSRF
tokens for defense in depth.

### Authorization Consistency
The system permission check is the first authorization gate; the project membership and role
check is the second gate. For Contributors, an ownership check (`created_by` matches the
authenticated user) is a third gate on update and delete operations. System Admin implicitly
holds all permissions and bypasses all gates. A `403 Forbidden` response must use a generic
message without revealing which gate was triggered, to prevent information leakage about
project existence or membership. The Contributor ownership check is the one exception: it
returns a distinct message ("Contributors can only update/delete their own test runs")
because the user already passed the project membership gate.

Authorization checks (system permissions and project membership) are performed against live
data on every request -- not cached in the session. If a user's role or permissions are
changed, the new authorization takes effect on their next request.

### Rate Limiting
All endpoints apply rate limiting configured at the infrastructure/middleware layer:

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../test-runs` (list) | 60 requests | per minute |
| `GET .../test-runs/{id}` (detail) | 60 requests | per minute |
| `GET .../test-runs/select` (dropdown) | 120 requests | per minute |
| `POST .../test-runs` (create) | 30 requests | per minute |
| `PATCH .../test-runs/{id}` (update) | 30 requests | per minute |
| `DELETE .../test-runs/{id}` (delete) | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### FK Validation (Same-Project Constraint)
The `plan_id` FK validation is a security boundary: it prevents cross-project data leakage
where a malicious client could provide a `plan_id` from project B when creating a test run
in project A. The validation must verify both that the referenced plan exists AND that its
`project_id` matches the test run's `project_id`. This check is performed within the same
database transaction as the insert/update to prevent TOCTOU races.

The `report_to` and `default_tester` FK validation verifies the referenced user exists and
is not soft-deleted. These FKs cross the project boundary intentionally (users are global,
not project-scoped), so no cross-project check applies. Only existence and active-status
checks are needed.

---

## Out of Scope

- **Bulk create/update/delete test runs** (only single-resource operations in Phase 1)
- **Test run cases** (adding/removing test cases to a test run -- separate feature
  `test-run-cases`)
- **Test run statistics** (aggregation of pass/fail/warning counts -- separate feature
  `test-run-statistics`)
- **Test run executions** (linked test executions on detail view -- separate feature
  `test-run-executions`)
- **Test run history / versioning** (full audit log of field changes -- deferred to a
  cross-cutting audit feature in a future phase)
- **Test run restore** (clearing `deleted_at` / `deleted_by` -- deferred to a future phase)
- **Hard delete** (not implemented for any entity in Phase 1)
- **Test run copy / clone** (duplicate an existing test run -- separate feature)
- **Bulk import / export** (CSV, Excel -- separate feature)
- **UI views** (pages, forms, list views -- covered in `ui-*` specs)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission codes `test_run:create`, `test_run:read`,
  `test_run:read_list`, `test_run:update`, `test_run:delete`, `test_run:select` must be
  seeded in the `PERMISSIONS` table and assignable to roles.
- **IAM Users** -- `users.id` FK reference for `report_to`, `default_tester`, `created_by`,
  `updated_by`, `deleted_by` audit columns. User name resolution (username, fullname) via
  JOIN on detail and list endpoints.
- **Project CRUD** -- The `PROJECTS` table must exist; test runs reference `projects(id)`
  via `project_id` FK.
- **Project Members** -- Project membership and roles (Owner, Editor, Contributor, Viewer)
  are used for authorization scope checks.
- **Auth RBAC** -- System permission checks for `test_run:*` codes.
- **Auth Project Scope** -- Project membership scope check on all endpoints.
- **Test Plan CRUD** -- Test runs can optionally reference `test_plans(id)` via `plan_id`
  FK. FK validation ensures the plan belongs to the same project. Ordering dependency:
  `test-plan-crud` migration must run before `test-run-crud` migration.
