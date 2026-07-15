# Feature: Test Plan CRUD

## Overview

Test Plans are multi-project entities that group test cases into a cohesive testing effort.
Unlike test cases, a test plan can span multiple projects, making it suitable for
cross-project testing initiatives (e.g., integration testing across services owned by
different projects, or a regression test run covering multiple modules).

Each test plan has a name, version string, a mandatory reference to a plan type (from one of
its associated projects), a description, and a lifecycle status (TODO, IN_PROGRESS, DONE,
CANCEL) with validated transitions. Multi-project membership is managed through the
`TEST_PLAN_PROJECTS` junction table.

The feature supports full CRUD operations with role-based authorization across multiple
projects. A user must be an Owner or Editor in at least one of the test plan's projects to
create, update, or delete; any project membership (across all linked projects) suffices for
read access.

---

## User Stories

### US-1: Create Test Plan

As a project Owner or Editor with `test_plan:create` system permission, I want to create a
new test plan associated with one or more projects, so that I can plan and organise testing
efforts that span multiple project boundaries.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_plan:create` permission (or a System Admin) sends a valid
  `POST /api/v1/test-plans` request with `name`, `project_ids` (at least one), `plan_type_id`,
  and optional fields (`version`, `description`), THE SYSTEM SHALL insert the test plan into
  `TEST_PLANS` and the project associations into `TEST_PLAN_PROJECTS` within a database
  transaction, set `status` to `TODO`, set `created_by` and `updated_by` to the authenticated
  user's ID, and return `201 Created` with the test plan representation and a `Location`
  header pointing to the new resource.
- IF the user does not hold the `test_plan:create` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `test_plan:create` but is NOT an Owner or Editor in at least one of the
  specified projects AND is not a System Admin, THE SYSTEM SHALL return `403 Forbidden`
  (same generic message).
- IF `project_ids` is empty or missing, THE SYSTEM SHALL return `422 Unprocessable Entity`.
- IF `project_ids` contains a project ID that does not exist or is soft-deleted, THE SYSTEM
  SHALL return `422 Unprocessable Entity` with error code `INVALID_PROJECT`.
- IF `project_ids` contains duplicate project IDs, THE SYSTEM SHALL deduplicate them and
  proceed (the duplicate is ignored, not an error).
- IF `name` is empty, contains only whitespace, or exceeds 255 characters, THE SYSTEM SHALL
  return `422 Unprocessable Entity` with field-level validation details. Leading and trailing
  whitespace is trimmed before length validation and storage.
- IF a non-deleted test plan with the same `name` already exists (case-insensitive comparison,
  across all projects), THE SYSTEM SHALL return `409 Conflict` with error code
  `DUPLICATE_TEST_PLAN_NAME`.
- IF `plan_type_id` is missing or null, THE SYSTEM SHALL return `422 Unprocessable Entity`.
- IF `plan_type_id` is provided: THE SYSTEM SHALL validate that a non-deleted plan type with
  that ID exists AND its `project_id` is among the test plan's associated projects. If the
  plan type does not exist, is soft-deleted, or belongs to a project not in the test plan's
  project list, THE SYSTEM SHALL return `422 Unprocessable Entity` with error code
  `INVALID_PLAN_TYPE`.
- IF `version` is provided and exceeds 50 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity`. Leading and trailing whitespace is trimmed. Empty string `""`
  is stored as-is. Omitting `version` defaults to `"1.0"`.
- IF `description` is provided and exceeds 10000 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity`. An empty string `""` is stored as-is (distinct from `null`).
  Sending `"description": null` explicitly clears the field. Omitting `description` defaults
  to `null`.
- IF the request body includes unrecognised fields, THE SYSTEM SHALL return
  `422 Unprocessable Entity` (strict mode -- rejects typos and unknown fields for
  defence-in-depth).
- THE SYSTEM SHALL set the initial status to `TODO` regardless of what the client sends
  (the `status` field is not accepted on create).

### US-2: List Test Plans

As an authenticated user, I want to view a paginated list of test plans that I have access
to (across all my projects), with filtering by status and plan type, and search on name and
description, so that I can browse and locate test plans efficiently.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_plan:read_list` permission (or a System Admin) sends
  `GET /api/v1/test-plans`, THE SYSTEM SHALL return a paginated list of non-deleted test
  plans where the user is a member of at least one associated project, ordered by `updated_at`
  descending (most recently updated first).
- IF the user does not hold the `test_plan:read_list` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- THE SYSTEM SHALL support pagination via query parameters `page` (default 1, minimum 1,
  maximum 1000) and `limit` (default 25, minimum 1, maximum 100). Requests with
  `page > 1000` return `422`.
- THE SYSTEM SHALL include pagination metadata (`total`, `page`, `limit`) in the response.
- THE SYSTEM SHALL support an optional `status` query parameter that filters test plans by
  their lifecycle status. Accepted values: `TODO`, `IN_PROGRESS`, `DONE`, `CANCEL`.
- THE SYSTEM SHALL support an optional `plan_type_id` query parameter that filters test
  plans to only those with the given plan type. If the plan type does not exist or is
  soft-deleted, the result set is empty (not an error).
- THE SYSTEM SHALL support an optional `project_id` query parameter that filters test plans
  to only those linked to the given project AND where the user is a member of that project.
  If the project does not exist or is soft-deleted, the result set is empty (not an error).
- THE SYSTEM SHALL support an optional `search` query parameter that performs a
  case-insensitive substring match against `name` and `description`. The `search` value must
  not exceed 255 characters; longer values return `422`. The characters `%` and `_` and `\`
  in the search string are escaped (treated as literals, not ILIKE wildcards or escape
  characters).
- THE SYSTEM SHALL support an optional `sort` query parameter with values
  `-updated_at` (default, most recently updated first), `updated_at` (oldest first),
  `name`, `-name` (descending), `status`, `-status` (descending).
  Invalid sort values return `422`.
- WHILE a test plan is soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL exclude it
  from the list.
- IF the user is a System Admin, THE SYSTEM SHALL return all non-deleted test plans
  regardless of project membership (admin bypass).

### US-3: View Test Plan Detail

As an authenticated user who is a member of at least one project linked to the test plan,
I want to view the full details of a single test plan, including its linked projects and
plan type name, so that I can review the complete test plan information before editing or
executing it.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_plan:read` permission (or a System Admin) sends
  `GET /api/v1/test-plans/{id}`, THE SYSTEM SHALL return the test plan fields (id, name,
  version, plan_type_id, plan_type_name, description, status, project_ids, and audit
  timestamps -- created_at, created_by, updated_at, updated_by), excluding soft-deleted test
  plans.
- IF the user does not hold the `test_plan:read` system permission AND is not a System Admin,
  THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_plan:read` but is NOT a member of any project linked to the test
  plan AND is not a System Admin, THE SYSTEM SHALL return `404 Not Found` (same message as
  non-existent to avoid leaking the existence of test plans the user cannot access).
- IF the test plan does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL resolve `plan_type_name` by joining `TEST_PLAN_TYPES` on `plan_type_id`.
  If the plan type is soft-deleted, `plan_type_name` SHALL still be resolved (plan types
  are not soft-deletable in Phase 1, but the join is defensive).
- THE SYSTEM SHALL include the list of linked project IDs in the response as a `project_ids`
  array.
- THE SYSTEM SHALL NOT return `deleted_at` or `deleted_by` in the response body (these are
  internal fields).

### US-4: Update Test Plan

As a project Owner or Editor in at least one of the test plan's linked projects, I want to
update a test plan's fields (including its status and project associations), so that I can
keep the test plan accurate and move it through its lifecycle.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_plan:update` permission (or a System Admin) sends
  `PATCH /api/v1/test-plans/{id}` with one or more updatable fields (`name`, `version`,
  `plan_type_id`, `description`, `project_ids`), THE SYSTEM SHALL apply the changes
  within a database transaction, set `updated_by` to the authenticated user's ID, set
  `updated_at = NOW()`, and return `200 OK` with the updated test plan representation.
- IF the user does not hold the `test_plan:update` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_plan:update`, IS a member of at least one linked project, but is
  NOT an Owner or Editor in any of those linked projects AND is not a System Admin, THE SYSTEM
  SHALL return `403 Forbidden`.
- IF the test plan does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`
  (per FR: UPDATE on soft-deleted rows is rejected).
- IF the user holds `test_plan:update` but is NOT a member of ANY linked project AND is not a
  System Admin, THE SYSTEM SHALL return `404 Not Found` (same as non-existent -- no information
  leakage).
- IF the updated `name` conflicts with another non-deleted test plan (case-insensitive
  comparison, across all projects), THE SYSTEM SHALL return `409 Conflict` with
  `DUPLICATE_TEST_PLAN_NAME`.
- IF `plan_type_id` is provided: THE SYSTEM SHALL validate that the plan type exists, is
  non-deleted, and belongs to a project that is currently linked (or being added in the same
  request) to the test plan. If the plan type is invalid, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with error code `INVALID_PLAN_TYPE`.
- IF `project_ids` is provided: THE SYSTEM SHALL replace the entire set of project
  associations (sync-replace). The new set must be non-empty. Each project ID must exist and
  be non-deleted. The user must be an Owner or Editor in at least one project in the new set
  (or be a System Admin).
- IF `project_ids` is provided and the plan type's project is no longer in the new project
  set, THE SYSTEM SHALL return `422 Unprocessable Entity` with error code
  `PLAN_TYPE_PROJECT_MISMATCH` (the plan type must belong to at least one of the test
  plan's linked projects).
- IF `status` is provided in the request body, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with error code `FIELD_NOT_UPDATABLE` and a detail message
  indicating that status changes must use the dedicated
  `POST /api/v1/test-plans/{id}/transition-status` endpoint (see test-plan-status spec).
- IF no updatable fields are provided (empty body), THE SYSTEM SHALL return
  `422 Unprocessable Entity`.
- IF the request body includes unrecognised fields, THE SYSTEM SHALL reject the request with
  `422 Unprocessable Entity` (strict mode -- rejects typos and unknown fields for
  defence-in-depth).
- THE SYSTEM SHALL NOT update `id`, `created_at`, `created_by`, `deleted_at`, or `deleted_by`.
- IF `name` is provided: it must contain at least one non-whitespace character and must not
  exceed 255 characters after trimming leading/trailing whitespace. Whitespace-only values
  return `422`.
- IF `version` is provided: it must not exceed 50 characters. Leading and trailing whitespace
  is trimmed. Empty string `""` is stored as-is. `null` explicitly clears it (sets to
  `NULL`).
- IF `description` is provided: omitting the field preserves the current value. Sending
  `"description": null` explicitly clears it (sets to `NULL`). Sending `"description": ""`
  stores an empty string. Values exceeding 10000 characters return `422`.
- IF `project_ids` is provided with an empty array, THE SYSTEM SHALL return
  `422 Unprocessable Entity` (a test plan must be linked to at least one project).

### US-5: Delete Test Plan (Soft-delete)

As a project Owner or Editor in at least one of the test plan's linked projects, I want to
soft-delete a test plan so that it is hidden from all views while preserving its data and
project associations.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_plan:delete` permission (or a System Admin) sends
  `DELETE /api/v1/test-plans/{id}`, THE SYSTEM SHALL set `deleted_at = NOW()` and
  `deleted_by = <current_user_id>` and return `204 No Content`.
- IF the user does not hold the `test_plan:delete` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_plan:delete`, IS a member of at least one linked project, but is
  NOT an Owner or Editor in any of those linked projects AND is not a System Admin, THE SYSTEM
  SHALL return `403 Forbidden`.
- IF the user holds `test_plan:delete` but is NOT a member of ANY linked project AND is not a
  System Admin, THE SYSTEM SHALL return `404 Not Found` (same as non-existent -- no information
  leakage).
- IF the test plan does not exist or is already soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL NOT check for referential integrity constraints before soft-deleting a
  test plan (test plans may be linked to test cases in the future, so a future feature may
  need to add a referential check). In Phase 1, test plans are leaf entities.
- THE SYSTEM SHALL NOT hard-delete any record.
- After soft-delete, the test plan is excluded from list, detail, and select endpoints.
  The project associations in `TEST_PLAN_PROJECTS` are NOT deleted (they are preserved
  for audit trail integrity).

### US-6: Select Test Plans (Dropdown)

As an authenticated user with `test_plan:select` permission, I want to retrieve a compact
list of all non-deleted test plans I have access to (without pagination) for use in dropdown
and selection UI components, so that I can link test plans to other entities efficiently.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_plan:select` permission (or a System Admin) sends
  `GET /api/v1/test-plans/select`, THE SYSTEM SHALL return a flat array of all non-deleted
  test plans where the user is a member of at least one associated project, each containing
  only `id` and `name`, ordered by `name` ascending (case-insensitive).
- IF the user does not hold the `test_plan:select` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- THE SYSTEM SHALL NOT paginate this endpoint -- it is designed for dropdown population and
  returns all results.
- THE SYSTEM SHALL exclude soft-deleted test plans from the results.
- THE SYSTEM SHALL cap the result at 1000 test plans; if the result set exceeds this limit,
  the response is truncated and a warning is logged. An `X-Result-Truncated: true` response
  header is set.
- IF the user is a System Admin, THE SYSTEM SHALL return all non-deleted test plans
  regardless of membership (admin bypass).

---

## Security Considerations

### Authentication
All endpoints require a valid authenticated session. Requests without a valid session cookie
return `401 Unauthorized` with error code `NOT_AUTHENTICATED`. This applies to all six
endpoints and is enforced by `AuthMiddleware` before any handler logic executes.

### Input Sanitization (XSS Prevention)
All user-supplied text fields (`name`, `version`, `description`) must be sanitized on input
to strip disallowed HTML tags and malicious script content, and output-encoded at the
presentation layer to prevent Cross-Site Scripting (XSS). This implements the project-wide
policy defined in PRD 5.3 (XSS prevention). Defence-in-depth requires both input
sanitisation and output encoding.

### CSRF Protection
All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must be protected against
Cross-Site Request Forgery (CSRF). Session cookies must carry the `SameSite=Lax` attribute
(or stricter). Implementations must verify `Content-Type` headers and/or include anti-CSRF
tokens for defense in depth.

### Authorization Consistency

Authorization is a two-gate process (three for detail endpoints):

1. **System permission check** -- the user must hold the appropriate `test_plan:*` permission.
2. **Project membership and role check** -- for mutations (create/update/delete), the user
   must be an Owner or Editor in at least one of the test plan's linked projects. For reads
   (list/detail/select), the user must be a member of at least one linked project.
3. **Visibility gate (detail only)** -- if the user is not a member of any linked project,
   the response is `404 Not Found` (indistinguishable from non-existent) to prevent
   information leakage.

System Admin implicitly holds all permissions and bypasses all gates.

Authorization checks (system permissions and project membership) are performed against live
data on every request -- not cached in the session. If a user's role or permissions are
changed, the new authorization takes effect on their next request.

### Multi-Project Authorization

The multi-project nature of test plans introduces a unique authorization pattern:

- **Create:** The user must be an Owner or Editor in at least one of the projects specified
  in `project_ids`.
- **Update/Delete:** The user must be an Owner or Editor in at least one of the test plan's
  CURRENT linked projects (not the new set being applied, for update -- both the current and
  new sets must satisfy this requirement).
- **Read:** The user must be a member (any role) of at least one linked project.
- **List/Select:** Only test plans where the user is a member of at least one linked project
  are returned.

When `project_ids` is updated, the user must be Owner/Editor in at least one project in the
NEW set. This prevents a user from adding a project they are not an Owner/Editor of and then
locking themselves out.

### Status Transition Enforcement

Status transitions are validated server-side against a state machine. The client may not
bypass transition rules. Invalid transitions return `422` with `INVALID_STATUS_TRANSITION`.
See design.md for the full state diagram.

### Rate Limiting
All endpoints apply rate limiting configured at the infrastructure/middleware layer:

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../test-plans` (list) | 60 requests | per minute |
| `GET .../test-plans/{id}` (detail) | 60 requests | per minute |
| `GET .../test-plans/select` (dropdown) | 120 requests | per minute |
| `POST .../test-plans` (create) | 30 requests | per minute |
| `PATCH .../test-plans/{id}` (update) | 30 requests | per minute |
| `DELETE .../test-plans/{id}` (delete) | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### Plan Type Cross-Project Validation

The `plan_type_id` FK validation must verify that the plan type belongs to a project that
is (or will be) linked to the test plan. This prevents cross-project injection where a
malicious client could assign a plan type from an unrelated project. The validation is
performed within the same database transaction as the insert/update to prevent TOCTOU races.

---

## Out of Scope

- **Test plan execution / test runs** (assigning test cases, tracking execution results --
  separate feature)
- **Test plan cloning** (duplicate an existing test plan -- separate feature)
- **Test plan history / versioning** (full audit log of field changes -- deferred to a
  cross-cutting audit feature in a future phase)
- **Test plan restore** (clearing `deleted_at` / `deleted_by` -- deferred to a future phase;
  data is preserved but no restore endpoint exists in Phase 1)
- **Hard delete** (not implemented for any entity in Phase 1)
- **Bulk create/update/delete test plans** (only single-resource operations in Phase 1)
- **Test case assignment** (linking test cases to test plans -- separate feature)
- **Test plan scheduling** (start/end dates, milestones -- separate feature)
- **UI views** (pages, forms, list views -- covered in `ui-*` specs)
- **Structured audit log** (the `created_by`/`updated_by` columns provide basic attribution;
  a full audit log is deferred to a cross-cutting audit feature)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission codes `test_plan:create`, `test_plan:read`,
  `test_plan:read_list`, `test_plan:update`, `test_plan:delete`, `test_plan:select` must
  be seeded in the `PERMISSIONS` table and assignable to roles.
- **IAM Users** -- `users.id` FK reference for `created_by`, `updated_by`, `deleted_by`
  audit columns.
- **Project CRUD** -- The `PROJECTS` table must exist; test plans reference projects
  through the `TEST_PLAN_PROJECTS` junction table via `project_id` FK.
- **Project Members** -- Project membership and roles (Owner, Editor, Contributor, Viewer)
  are used for authorization scope checks across all linked projects.
- **Auth RBAC** -- System permission checks for `test_plan:*` codes.
- **Metadata Plan Types** -- Test plans reference `TEST_PLAN_TYPES` via a mandatory
  `plan_type_id` FK. The plan type must belong to one of the test plan's linked projects.
- **UI List Views** -- The test plans list view follows the standard list-view pattern
  (checkbox column, action icons, ID/Name navigation, Add New button, filter controls).
