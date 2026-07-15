# Feature: Metadata Categories

## Overview

Test Categories provide a way to organise test cases within a project. Each project has
its own set of categories, seeded from a YAML configuration file at project creation.
Project members with appropriate roles (Owner, Editor) can manage categories throughout
the project lifecycle. Categories are scoped to a single project and have no
cross-project visibility.

---

## User Stories

### US-1: Create Category

As a project Owner or Editor with the `category:create` system permission, I want to
create a new test category within my project, so that test cases can be organised by
thematic area (e.g., "Smoke Tests", "Regression", "Login", "Checkout").

**Acceptance Criteria (EARS)**

- WHEN a user with `category:create` permission (or a System Admin) sends a valid
  `POST /api/v1/projects/{projectId}/categories` request with `name` and optional
  `description`, THE SYSTEM SHALL insert the category into `TEST_CATEGORIES` within a
  database transaction (to prevent TOCTOU races on the duplicate-name check), scope it to
  the given `project_id`, set `created_by` and `updated_by` to the authenticated user's ID,
  and return `201 Created` with the category representation and a `Location` header pointing
  to the new resource.
- IF the user does not hold the `category:create` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `category:create` but is not an Owner or Editor of the project AND is not
  a System Admin, THE SYSTEM SHALL return `403 Forbidden` (same generic message).
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF `name` is empty, contains only whitespace, or exceeds 255 characters, THE SYSTEM SHALL
  return `422 Unprocessable Entity` with field-level validation details. Leading and trailing
  whitespace is trimmed before length validation and storage.
- IF `description` is provided and exceeds 2000 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity`. An empty string `""` is stored as-is (distinct from `null`).
  Sending `"description": null` explicitly clears the field (sets it to `NULL` in the DB).
  Omitting `description` from the request body defaults to `null`.
- IF a category with the same `name` already exists in the same project (case-insensitive
  comparison, excluding soft-deleted categories), THE SYSTEM SHALL return `409 Conflict`
  with error code `DUPLICATE_CATEGORY_NAME`.
- IF a category with the same `name` exists but is soft-deleted, THE SYSTEM SHALL allow
  creation of the new category (the soft-deleted record does not prevent re-creation of
  a category with the same name).
- IF the request body includes unrecognised fields, THE SYSTEM SHALL return
  `422 Unprocessable Entity` (strict mode — rejects typos and unknown fields for
  defence-in-depth).

### US-2: List Categories

As a project member, I want to view a paginated list of test categories in the current
project, sorted alphabetically by name, with the ability to filter and search, so that I
can browse available categories when organising test cases.

**Acceptance Criteria (EARS)**

- WHEN a user with `category:read_list` permission (or a System Admin) sends `GET /api/v1/projects/{projectId}/categories`,
  THE SYSTEM SHALL return a paginated list of non-deleted categories scoped to that project,
  ordered by `name` ascending.
- IF the user does not hold the `category:read_list` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `category:read_list` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL support pagination via query parameters `page` (default 1, minimum 1,
  maximum 1000) and `limit` (default 25, minimum 1, maximum 100). Requests with
  `page > 1000` return `422`.
- THE SYSTEM SHALL include pagination metadata (`total`, `page`, `limit`) in the response.
- THE SYSTEM SHALL support an optional `search` query parameter that performs a
  case-insensitive substring match against `name` and `description`. The `search` value
  must not exceed 255 characters; longer values return `422`. The characters `%` and `_`
  and `\` in the search string are escaped (treated as literals, not ILIKE wildcards
  or escape characters).
- THE SYSTEM SHALL support an optional `sort` query parameter with values `name` (default),
  `-name` (descending), `created_at`, `-created_at` (descending). Invalid sort values
  return `422`.
- WHILE a category is soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL exclude it
  from the list.
- IF the user is a System Admin, THE SYSTEM SHALL return all non-deleted categories
  for the project regardless of the admin's project membership (admin bypass).

### US-3: View Category Detail

As a project member, I want to view the full details of a single category, including its
audit timestamps, so that I can confirm its metadata before assigning it to test cases.

**Acceptance Criteria (EARS)**

- WHEN a user with `category:read` permission (or a System Admin) sends `GET /api/v1/projects/{projectId}/categories/{id}`,
  THE SYSTEM SHALL return the category fields (id, name, description, project_id, and audit
  timestamps — created_at, created_by, updated_at, updated_by), excluding soft-deleted
  categories.
- IF the user does not hold the `category:read` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `category:read` but is not a member of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the category does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`
  (same message to avoid information leakage about soft-deleted records).
- THE SYSTEM SHALL NOT return `deleted_at` or `deleted_by` in the response body (these are
  internal fields).

### US-4: Update Category

As a project Owner or Editor with `category:update` permission, I want to update a
category's name and/or description, so that I can keep the category taxonomy accurate
and useful.

**Acceptance Criteria (EARS)**

- WHEN a user with `category:update` permission (or a System Admin) sends
  `PATCH /api/v1/projects/{projectId}/categories/{id}` with one or both updatable fields
  (`name`, `description`), THE SYSTEM SHALL apply the changes within a database transaction
  (to prevent TOCTOU races on the duplicate-name check), set `updated_by` to the
  authenticated user's ID, set `updated_at = NOW()`, and return `200 OK` with the updated
  category representation. On success, `updated_by` is set to the caller and `updated_at`
  reflects the change.
- IF the user does not hold the `category:update` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `category:update` but is not an Owner or Editor of the project AND is
  not a System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the category does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`
  (per FR-54c: UPDATE on soft-deleted rows is rejected).
- IF the updated `name` conflicts with another non-deleted category in the same project
  (case-insensitive comparison), THE SYSTEM SHALL return `409 Conflict` with
  `DUPLICATE_CATEGORY_NAME`.
- IF no updatable fields are provided (empty body), THE SYSTEM SHALL return
  `422 Unprocessable Entity`.
- IF the request body includes unrecognised fields, THE SYSTEM SHALL reject the request with
  `422 Unprocessable Entity` (strict mode — rejects typos and unknown fields for
  defence-in-depth).
- THE SYSTEM SHALL NOT update `id`, `project_id`, `created_at`, `created_by`, `deleted_at`,
  or `deleted_by`.
- THE SYSTEM SHALL NOT allow changing the `project_id` of a category (categories are
  permanently scoped to their project).
- IF `name` is provided: it must contain at least one non-whitespace character and must not
  exceed 255 characters after trimming leading/trailing whitespace. Whitespace-only values
  return `422`.
- IF `description` is provided: omitting the field preserves the current value. Sending
  `"description": null` explicitly clears it (sets to `NULL`). Sending `"description": ""`
  stores an empty string. Values exceeding 2000 characters return `422`.

### US-5: Delete Category (Soft-delete)

As a project Owner or Editor with `category:delete` permission, I want to soft-delete a
category so that it is hidden from all views while preserving its data. The category must
not be deleted if any test cases reference it.

**Acceptance Criteria (EARS)**

- WHEN a user with `category:delete` permission (or a System Admin) sends
  `DELETE /api/v1/projects/{projectId}/categories/{id}`, THE SYSTEM SHALL set
  `deleted_at = NOW()` and `deleted_by = <current_user_id>` and return `204 No Content`.
- IF the user does not hold the `category:delete` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `category:delete` but is not an Owner or Editor of the project AND is
  not a System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the category does not exist or is already soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the category is still referenced by one or more non-deleted test cases, THE SYSTEM
  SHALL return `409 Conflict` with error code `CATEGORY_IN_USE`, a message indicating the
  category cannot be deleted while test cases reference it, and the count of referencing
  test cases in the error details.
- THE SYSTEM SHALL NOT hard-delete any record.
- After soft-delete, existing test case references (FKs) remain intact; the test cases
  still display the category name for audit/historical purposes. The category simply
  does not appear in selection dropdowns for new/edited test cases.

### US-6: Select Categories (Dropdown)

As a project member with `category:select` permission, I want to retrieve a compact list of
all non-deleted categories in the project (without pagination) for use in dropdown and
selection UI components, so that I can assign categories to test cases efficiently.

**Acceptance Criteria (EARS)**

- WHEN a user with `category:select` permission (or a System Admin) sends `GET /api/v1/projects/{projectId}/categories/select`,
  THE SYSTEM SHALL return a flat array of all non-deleted categories in the project, each
  containing only `id` and `name`, ordered by `name` ascending.
- IF the user does not hold the `category:select` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `category:select` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL NOT paginate this endpoint — it is designed for dropdown population and
  returns all results.
- THE SYSTEM SHALL exclude soft-deleted categories from the results.
- THE SYSTEM SHALL cap the result at 500 categories; if a project exceeds this limit, the
  response is truncated and a warning is logged. (The expected operational range is under
  200 categories per project.)

---

## Security Considerations

### Input Sanitization (XSS Prevention)
All user-supplied text fields (`name`, `description`) must be sanitized on input to strip
disallowed HTML tags and malicious script content, and output-encoded at the presentation
layer to prevent Cross-Site Scripting (XSS). This implements the project-wide policy
defined in PRD 5.3 (XSS prevention). Defence-in-depth requires both input sanitisation
and output encoding.

### CSRF Protection
All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must be protected against
Cross-Site Request Forgery (CSRF). Session cookies must carry the `SameSite=Lax` attribute
(or stricter). Implementations must verify `Content-Type` headers and/or include
anti-CSRF tokens for defense in depth.

### Authorization Consistency
The system permission check is the first authorization gate; the project membership and
role check is the second gate. Both must be enforced server-side for every
category endpoint. System Admin implicitly holds all permissions and bypasses both gates.
A `403 Forbidden` response must use a generic message without revealing which gate was
triggered, to prevent information leakage about project existence or membership.

Authorization checks (system permissions and project membership) are performed against
live data on every request — not cached in the session. If a user's role or permissions
are changed, the new authorization takes effect on their next request. There is no cached
authorization state that would allow stale-role access.

### Rate Limiting
All endpoints apply rate limiting configured at the infrastructure/middleware layer:

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../categories` (list) | 60 requests | per minute |
| `GET .../categories/{id}` (detail) | 60 requests | per minute |
| `GET .../categories/select` (dropdown) | 120 requests | per minute |
| `POST .../categories` (create) | 30 requests | per minute |
| `PATCH .../categories/{id}` (update) | 30 requests | per minute |
| `DELETE .../categories/{id}` (delete) | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### Referential Integrity
The `CATEGORY_IN_USE` check (US-5) prevents deletion of categories that are referenced
by test cases. This check must be performed within the same transaction as the soft-delete
to avoid TOCTOU races. The check counts only non-soft-deleted test cases; soft-deleted
test cases do not block category deletion.

---

## Out of Scope

- **Bulk create/update/delete categories** (only single-resource operations in Phase 1)
- **Category reordering or custom sort order** (alphabetical sort only)
- **Category colour or icon** (no visual metadata beyond name and description)
- **Cross-project category sharing** (categories are strictly per-project)
- **Structured audit log** (the `created_by`/`updated_by`/`deleted_by` columns provide
  basic attribution; a full audit log recording old/new values per field change is deferred
  to a cross-cutting audit feature in a future phase)
- **Category restore** (clearing `deleted_at` / `deleted_by` — deferred to a future phase;
  data is preserved but no restore endpoint exists in Phase 1)
- **Hard delete** (not implemented for any entity in Phase 1)
- **Category hierarchy / parent-child nesting** (flat list only)
- **UI views** (pages, forms, list views — covered in `ui-*` specs)
- **Project-level sharing of categories** (covered in `sharing` spec; categories follow
  project-level visibility)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission codes `category:create`, `category:read`,
  `category:read_list`, `category:update`, `category:delete`, `category:select` must be
  seeded in the `PERMISSIONS` table and assignable to roles.
- **IAM Users** -- `users.id` FK reference for `created_by`, `updated_by`, `deleted_by`
  audit columns.
- **Project CRUD** -- The `PROJECTS` table must exist; categories reference
  `projects(id)` via `project_id` FK. Project creation triggers auto-seeding of default
  categories from YAML config.
- **Project Members** -- Project membership and roles (Owner, Editor, Contributor,
  Viewer) are used for authorization scope checks.
- **Auth RBAC** -- System permission checks for `category:*` codes.
- **Auth Project Scope** -- Project membership scope check on all endpoints.
- **YAML Configuration** -- A YAML configuration file loaded at application startup
  containing a `categories` section with default category definitions. The file path is
  set via environment variable.
- **Test Case CRUD** -- Test cases reference `TEST_CATEGORIES` via a `category_id` FK.
  The `CATEGORY_IN_USE` constraint requires querying the test cases table before
  soft-deleting a category.
- **UI List Views** -- The categories list view follows the standard list-view pattern
  (checkbox column, action icons, ID/Name navigation, Add New button, filter controls).
