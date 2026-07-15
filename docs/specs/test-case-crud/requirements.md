# Feature: Test Case CRUD

## Overview

Test Cases are the core entity of the test case management system. Each test case
belongs to a project and captures a single test scenario with its metadata. Test
cases are scoped to a single project and have no cross-project visibility.

Test cases can reference a category and a priority for organisation and triage
purposes. Both category and priority are optional (nullable FK), but when
provided they must belong to the same project as the test case.

The feature supports full CRUD operations with role-based ownership rules:
Contributors can create test cases and manage their own, while Owners and Editors
can manage all test cases in the project.

---

## User Stories

### US-1: Create Test Case

As a project member with the `test_case:create` system permission and a project role of
Contributor, Editor, or Owner, I want to create a new test case within a project, so that
I can document test scenarios with their summary, category, priority, automation status,
and detailed instructions.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case:create` permission (or a System Admin) sends a valid
  `POST /api/v1/projects/{projectId}/test-cases` request with `summary` and optional fields
  (`category_id`, `priority_id`, `automated`, `description`, `notes`), THE SYSTEM SHALL
  insert the test case into `TEST_CASES` within a database transaction (to prevent TOCTOU
  races on the duplicate-summary check and FK validation), scope it to the given
  `project_id`, set `created_by` and `updated_by` to the authenticated user's ID, and
  return `201 Created` with the test case representation and a `Location` header pointing
  to the new resource.
- IF the user does not hold the `test_case:create` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `test_case:create` but is a Viewer of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (same generic message).
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF `summary` is empty, contains only whitespace, or exceeds 500 characters, THE SYSTEM
  SHALL return `422 Unprocessable Entity` with field-level validation details. Leading and
  trailing whitespace is trimmed before length validation and storage.
- IF a test case with the same `summary` already exists in the same project (case-insensitive
  comparison, excluding soft-deleted test cases), THE SYSTEM SHALL return `409 Conflict`
  with error code `DUPLICATE_TEST_CASE_SUMMARY`.
- IF a test case with the same `summary` exists but is soft-deleted, THE SYSTEM SHALL allow
  creation of the new test case (the soft-deleted record does not prevent re-creation of
  a test case with the same summary).
- IF `category_id` is provided and non-null: THE SYSTEM SHALL validate that a non-deleted
  category with that ID exists AND its `project_id` matches the test case's `project_id`.
  If the category does not exist, is soft-deleted, or belongs to a different project, THE
  SYSTEM SHALL return `422 Unprocessable Entity` with error code `INVALID_CATEGORY` and
  a message indicating the category is invalid for this project.
- IF `priority_id` is provided and non-null: THE SYSTEM SHALL validate that a non-deleted
  priority with that ID exists AND its `project_id` matches the test case's `project_id`.
  If the priority does not exist, is soft-deleted, or belongs to a different project, THE
  SYSTEM SHALL return `422 Unprocessable Entity` with error code `INVALID_PRIORITY` and
  a message indicating the priority is invalid for this project.
- IF `automated` is provided, THE SYSTEM SHALL accept only boolean `true` or `false`. Any
  other value (string, number, null) returns `422 Unprocessable Entity`.
- IF `automated` is omitted, THE SYSTEM SHALL default it to `false`.
- IF `description` is provided and exceeds 10000 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity`. An empty string `""` is stored as-is (distinct from `null`).
  Sending `"description": null` explicitly clears the field (sets it to `NULL` in the DB).
  Omitting `description` from the request body defaults to `null`.
- IF `notes` is provided and exceeds 5000 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity`. An empty string `""` is stored as-is (distinct from `null`).
  Sending `"notes": null` explicitly clears the field (sets it to `NULL` in the DB).
  Omitting `notes` from the request body defaults to `null`.
- IF the request body includes unrecognised fields, THE SYSTEM SHALL return
  `422 Unprocessable Entity` (strict mode -- rejects typos and unknown fields for
  defence-in-depth).

### US-2: List Test Cases

As a project member, I want to view a paginated list of test cases in the current
project, with the ability to filter by category, priority, and search on summary and
description, so that I can browse and locate test cases efficiently.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case:read_list` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/test-cases`, THE SYSTEM SHALL return a paginated
  list of non-deleted test cases scoped to that project, ordered by `id` descending
  (newest first).
- IF the user does not hold the `test_case:read_list` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_case:read_list` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL support pagination via query parameters `page` (default 1, minimum 1,
  maximum 1000) and `limit` (default 25, minimum 1, maximum 100). Requests with
  `page > 1000` return `422`.
- THE SYSTEM SHALL include pagination metadata (`total`, `page`, `limit`) in the response.
- THE SYSTEM SHALL support an optional `category_id` query parameter that filters test
  cases to only those with the given category. If the category does not exist, is
  soft-deleted, or belongs to a different project, the result set is empty (not an error).
- THE SYSTEM SHALL support an optional `priority_id` query parameter that filters test
  cases to only those with the given priority. If the priority does not exist, is
  soft-deleted, or belongs to a different project, the result set is empty (not an error).
- THE SYSTEM SHALL support an optional `search` query parameter that performs a
  case-insensitive substring match against `summary` and `description`. The `search` value
  must not exceed 255 characters; longer values return `422`. The characters `%` and `_`
  and `\` in the search string are escaped (treated as literals, not ILIKE wildcards
  or escape characters).
- THE SYSTEM SHALL support an optional `automated` query parameter (boolean `true` or
  `false`) that filters test cases by their automation status. Invalid values return `422`.
- THE SYSTEM SHALL support an optional `sort` query parameter with values
  `-id` (default, newest first / descending), `id` (ascending, oldest first),
  `summary`, `-summary` (descending), `created_at`, `-created_at` (descending).
  Invalid sort values return `422`.
- WHILE a test case is soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL exclude it
  from the list.
- IF the user is a System Admin, THE SYSTEM SHALL return all non-deleted test cases
  for the project regardless of the admin's project membership (admin bypass).

### US-3: View Test Case Detail

As a project member, I want to view the full details of a single test case, including
the names of its associated category and priority, so that I can review the complete
test case information before editing or executing it.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case:read` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/test-cases/{id}`, THE SYSTEM SHALL return the test
  case fields (id, summary, description, notes, automated, project_id, category_id,
  category_name, priority_id, priority_name, and audit timestamps -- created_at,
  created_by, updated_at, updated_by), excluding soft-deleted test cases.
- IF the user does not hold the `test_case:read` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_case:read` but is not a member of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the test case does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`
  (same message to avoid information leakage about soft-deleted records).
- THE SYSTEM SHALL resolve `category_name` by joining `TEST_CATEGORIES` on `category_id`.
  If `category_id` is `NULL` or the category is soft-deleted, `category_name` SHALL be
  `null`.
- THE SYSTEM SHALL resolve `priority_name` by joining `TEST_PRIORITIES` on `priority_id`.
  If `priority_id` is `NULL` or the priority is soft-deleted, `priority_name` SHALL be
  `null`.
- THE SYSTEM SHALL NOT return `deleted_at` or `deleted_by` in the response body (these are
  internal fields).

### US-4: Update Test Case

As a project member, I want to update a test case's fields, so that I can keep the test
case documentation accurate. As a Contributor I can only update test cases I created.
As an Owner or Editor I can update any test case in the project.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case:update` permission (or a System Admin) sends
  `PATCH /api/v1/projects/{projectId}/test-cases/{id}` with one or more updatable fields
  (`summary`, `category_id`, `priority_id`, `automated`, `description`, `notes`), THE
  SYSTEM SHALL apply the changes within a database transaction (to prevent TOCTOU races on
  the duplicate-summary check and FK validation), set `updated_by` to the authenticated
  user's ID, set `updated_at = NOW()`, and return `200 OK` with the updated test case
  representation.
- IF the user does not hold the `test_case:update` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_case:update` but is a Viewer of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_case:update` and is a Contributor: THE SYSTEM SHALL verify that
  the test case's `created_by` matches the authenticated user's ID. If it does not match,
  THE SYSTEM SHALL return `403 Forbidden` with a message indicating that Contributors can
  only update their own test cases.
- IF the user holds `test_case:update` and is an Owner or Editor: THE SYSTEM SHALL allow
  updating any test case in the project regardless of `created_by`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the test case does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`
  (per FR-54c: UPDATE on soft-deleted rows is rejected).
- IF the updated `summary` conflicts with another non-deleted test case in the same project
  (case-insensitive comparison), THE SYSTEM SHALL return `409 Conflict` with
  `DUPLICATE_TEST_CASE_SUMMARY`.
- IF `category_id` is provided: THE SYSTEM SHALL validate it against the same project.
  Sending `null` explicitly clears the category reference. Omitted preserves the current
  value. Same validation rules as US-1 (must exist, non-deleted, same project).
- IF `priority_id` is provided: THE SYSTEM SHALL validate it against the same project.
  Sending `null` explicitly clears the priority reference. Omitted preserves the current
  value. Same validation rules as US-1 (must exist, non-deleted, same project).
- IF no updatable fields are provided (empty body), THE SYSTEM SHALL return
  `422 Unprocessable Entity`.
- IF the request body includes unrecognised fields, THE SYSTEM SHALL reject the request with
  `422 Unprocessable Entity` (strict mode -- rejects typos and unknown fields for
  defence-in-depth).
- THE SYSTEM SHALL NOT update `id`, `project_id`, `created_at`, `created_by`, `deleted_at`,
  or `deleted_by`.
- THE SYSTEM SHALL NOT allow changing the `project_id` of a test case (test cases are
  permanently scoped to their project).
- IF `summary` is provided: it must contain at least one non-whitespace character and must
  not exceed 500 characters after trimming leading/trailing whitespace. Whitespace-only
  values return `422`.
- IF `description` is provided: omitting the field preserves the current value. Sending
  `"description": null` explicitly clears it (sets to `NULL`). Sending `"description": ""`
  stores an empty string. Values exceeding 10000 characters return `422`.
- IF `notes` is provided: omitting the field preserves the current value. Sending
  `"notes": null` explicitly clears it (sets to `NULL`). Sending `"notes": ""` stores an
  empty string. Values exceeding 5000 characters return `422`.
- IF `automated` is provided: must be a boolean (`true` or `false`). Any other value
  returns `422`.

### US-5: Delete Test Case (Soft-delete)

As a project member, I want to soft-delete a test case so that it is hidden from all views
while preserving its data. As a Contributor I can only delete test cases I created. As an
Owner or Editor I can delete any test case in the project.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case:delete` permission (or a System Admin) sends
  `DELETE /api/v1/projects/{projectId}/test-cases/{id}`, THE SYSTEM SHALL set
  `deleted_at = NOW()` and `deleted_by = <current_user_id>` and return `204 No Content`.
- IF the user does not hold the `test_case:delete` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_case:delete` but is a Viewer of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_case:delete` and is a Contributor: THE SYSTEM SHALL verify that
  the test case's `created_by` matches the authenticated user's ID. If it does not match,
  THE SYSTEM SHALL return `403 Forbidden` with a message indicating that Contributors can
  only delete their own test cases.
- IF the user holds `test_case:delete` and is an Owner or Editor: THE SYSTEM SHALL allow
  deleting any test case in the project regardless of `created_by`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the test case does not exist or is already soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL NOT check for referential integrity constraints before soft-deleting a
  test case (test cases are leaf entities -- they are not referenced by other tables).
- THE SYSTEM SHALL NOT hard-delete any record.
- After soft-delete, the test case is excluded from list, detail, and select endpoints.

### US-6: Select Test Cases (Dropdown)

As a project member with `test_case:select` permission, I want to retrieve a compact list
of all non-deleted test cases in the project (without pagination) for use in dropdown and
selection UI components, so that I can link test cases to other entities efficiently.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case:select` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/test-cases/select`, THE SYSTEM SHALL return a flat
  array of all non-deleted test cases in the project, each containing only `id` and
  `summary`, ordered by `summary` ascending (case-insensitive).
- IF the user does not hold the `test_case:select` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_case:select` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL NOT paginate this endpoint -- it is designed for dropdown population and
  returns all results.
- THE SYSTEM SHALL exclude soft-deleted test cases from the results.
- THE SYSTEM SHALL cap the result at 1000 test cases; if a project exceeds this limit, the
  response is truncated and a warning is logged. (The expected operational range is under
  500 test cases per project, but large test suites may approach 1000.)

---

## Security Considerations

### Authentication
All endpoints require a valid authenticated session. Requests without a valid session cookie
return `401 Unauthorized` with error code `NOT_AUTHENTICATED`. This applies to all six
category endpoints and is enforced by `AuthMiddleware` before any handler logic executes.

### Input Sanitization (XSS Prevention)
All user-supplied text fields (`summary`, `description`, `notes`) must be sanitized on
input to strip disallowed HTML tags and malicious script content, and output-encoded at the
presentation layer to prevent Cross-Site Scripting (XSS). This implements the project-wide
policy defined in PRD 5.3 (XSS prevention). Defence-in-depth requires both input
sanitisation and output encoding.

### CSRF Protection
All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must be protected against
Cross-Site Request Forgery (CSRF). Session cookies must carry the `SameSite=Lax` attribute
(or stricter). Implementations must verify `Content-Type` headers and/or include
anti-CSRF tokens for defense in depth.

### Authorization Consistency
The system permission check is the first authorization gate; the project membership and
role check is the second gate. For Contributors, an ownership check (`created_by` matches
the authenticated user) is a third gate on update and delete operations. System Admin
implicitly holds all permissions and bypasses all gates. A `403 Forbidden` response must
use a generic message without revealing which gate was triggered, to prevent information
leakage about project existence or membership. The Contributor ownership check is the one
exception: it returns a distinct message ("Contributors can only update/delete their own
test cases") because the user already passed the project membership gate and the
information does not leak project state.

Authorization checks (system permissions and project membership) are performed against
live data on every request -- not cached in the session. If a user's role or permissions
are changed, the new authorization takes effect on their next request. There is no cached
authorization state that would allow stale-role access.

### Rate Limiting
All endpoints apply rate limiting configured at the infrastructure/middleware layer:

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../test-cases` (list) | 60 requests | per minute |
| `GET .../test-cases/{id}` (detail) | 60 requests | per minute |
| `GET .../test-cases/select` (dropdown) | 120 requests | per minute |
| `POST .../test-cases` (create) | 30 requests | per minute |
| `PATCH .../test-cases/{id}` (update) | 30 requests | per minute |
| `DELETE .../test-cases/{id}` (delete) | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### FK Validation (Same-Project Constraint)
The `category_id` and `priority_id` FK validation is a security boundary: it prevents
cross-project data leakage where a malicious client could provide a `category_id` from
project B when creating a test case in project A. The validation must verify both that
the referenced row exists AND that its `project_id` matches the test case's `project_id`.
This check is performed within the same database transaction as the insert/update to
prevent TOCTOU races.

---

## Out of Scope

- **Bulk create/update/delete test cases** (only single-resource operations in Phase 1)
- **Test case execution tracking** (test runs, results, status history -- separate feature)
- **Test steps** (ordered step-by-step instructions within a test case -- separate feature)
- **Test case attachments** (file uploads, screenshots -- separate feature)
- **Test case history / versioning** (full audit log of field changes -- deferred to a
  cross-cutting audit feature in a future phase)
- **Test case restore** (clearing `deleted_at` / `deleted_by` -- deferred to a future phase;
  data is preserved but no restore endpoint exists in Phase 1)
- **Hard delete** (not implemented for any entity in Phase 1)
- **Test case copy / clone** (duplicate an existing test case -- separate feature)
- **Test case linking / traces** (linking test cases to requirements, defects -- separate
  feature)
- **Bulk import / export** (CSV, Excel -- separate feature)
- **UI views** (pages, forms, list views -- covered in `ui-*` specs)
- **Test case tags / labels** (free-form tagging beyond category and priority -- separate
  feature)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission codes `test_case:create`, `test_case:read`,
  `test_case:read_list`, `test_case:update`, `test_case:delete`, `test_case:select` must
  be seeded in the `PERMISSIONS` table and assignable to roles.
- **IAM Users** -- `users.id` FK reference for `created_by`, `updated_by`, `deleted_by`
  audit columns.
- **Project CRUD** -- The `PROJECTS` table must exist; test cases reference
  `projects(id)` via `project_id` FK.
- **Project Members** -- Project membership and roles (Owner, Editor, Contributor,
  Viewer) are used for authorization scope checks. The Contributor role has ownership-based
  restrictions for update and delete.
- **Auth RBAC** -- System permission checks for `test_case:*` codes.
- **Auth Project Scope** -- Project membership scope check on all endpoints.
- **Metadata Categories** -- Test cases reference `TEST_CATEGORIES` via an optional
  `category_id` FK. Category name is resolved via JOIN on detail view. FK validation
  ensures the category belongs to the same project.
- **Metadata Priorities** -- Test cases reference `TEST_PRIORITIES` via an optional
  `priority_id` FK. Priority name is resolved via JOIN on detail view. FK validation
  ensures the priority belongs to the same project.
- **UI List Views** -- The test cases list view follows the standard list-view pattern
  (checkbox column, action icons, ID/Summary navigation, Add New button, filter controls).
