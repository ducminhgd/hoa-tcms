# Feature: Metadata Templates

## Overview

Test Case Templates provide reusable pre-fill content for test case descriptions within a
project. Each project has its own set of templates, seeded from a YAML configuration file at
project creation. Project members with appropriate roles (Owner, Editor) can manage templates
throughout the project lifecycle. When creating or editing a test case, a user selects a
template to pre-fill the test case description field. Templates are scoped to a single
project and have no cross-project visibility.

---

## User Stories

### US-1: Create Template

As a project Owner or Editor with the `template:create` system permission, I want to
create a new test case template within my project, so that testers can quickly pre-fill
test case descriptions with standardised content (e.g., "Login Test Steps", "API Test
Steps").

**Acceptance Criteria (EARS)**

- WHEN a user with `template:create` permission (or a System Admin) sends a valid
  `POST /api/v1/projects/{projectId}/templates` request with `name` and optional
  `description` (the template body/content), THE SYSTEM SHALL insert the template into
  `TEST_CASE_TEMPLATES` within a database transaction (to prevent TOCTOU races on the
  duplicate-name check), scope it to the given `project_id`, set `created_by` and
  `updated_by` to the authenticated user's ID, and return `201 Created` with the template
  representation and a `Location` header pointing to the new resource.
- IF the user does not hold the `template:create` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `template:create` but is not an Owner or Editor of the project AND is not
  a System Admin, THE SYSTEM SHALL return `403 Forbidden` (same generic message).
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF `name` is empty, contains only whitespace, or exceeds 255 characters, THE SYSTEM SHALL
  return `422 Unprocessable Entity` with field-level validation details. Leading and trailing
  whitespace is trimmed before length validation and storage.
- IF `description` is provided and exceeds 10000 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity`. An empty string `""` is stored as-is (distinct from `null`).
  Sending `"description": null` explicitly clears the field (sets it to `NULL` in the DB).
  Omitting `description` from the request body defaults to `null`.
- IF a template with the same `name` already exists in the same project (case-insensitive
  comparison, excluding soft-deleted templates), THE SYSTEM SHALL return `409 Conflict`
  with error code `DUPLICATE_TEMPLATE_NAME`.
- IF a template with the same `name` exists but is soft-deleted, THE SYSTEM SHALL allow
  creation of the new template (the soft-deleted record does not prevent re-creation of
  a template with the same name).
- IF the request body includes unrecognised fields, THE SYSTEM SHALL return
  `422 Unprocessable Entity` (strict mode -- rejects typos and unknown fields for
  defence-in-depth).

### US-2: List Templates

As a project member, I want to view a paginated list of test case templates in the current
project, sorted alphabetically by name, with the ability to filter and search, so that I
can browse available templates when creating test cases.

**Acceptance Criteria (EARS)**

- WHEN a user with `template:read_list` permission (or a System Admin) sends `GET /api/v1/projects/{projectId}/templates`,
  THE SYSTEM SHALL return a paginated list of non-deleted templates scoped to that project,
  ordered by `name` ascending.
- IF the user does not hold the `template:read_list` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `template:read_list` but is not a member of the project AND is not a
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
- WHILE a template is soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL exclude it
  from the list.
- IF the user is a System Admin, THE SYSTEM SHALL return all non-deleted templates
  for the project regardless of the admin's project membership (admin bypass).

### US-3: View Template Detail

As a project member, I want to view the full details of a single template, including its
audit timestamps and full body content, so that I can review the template before applying it
to a test case.

**Acceptance Criteria (EARS)**

- WHEN a user with `template:read` permission (or a System Admin) sends `GET /api/v1/projects/{projectId}/templates/{id}`,
  THE SYSTEM SHALL return the template fields (id, name, description, project_id, and audit
  timestamps -- created_at, created_by, updated_at, updated_by), excluding soft-deleted
  templates.
- IF the user does not hold the `template:read` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `template:read` but is not a member of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the template does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`
  (same message to avoid information leakage about soft-deleted records).
- THE SYSTEM SHALL NOT return `deleted_at` or `deleted_by` in the response body (these are
  internal fields).

### US-4: Update Template

As a project Owner or Editor with `template:update` permission, I want to update a
template's name and/or body content, so that I can keep the pre-fill content accurate
and useful for testers.

**Acceptance Criteria (EARS)**

- WHEN a user with `template:update` permission (or a System Admin) sends
  `PATCH /api/v1/projects/{projectId}/templates/{id}` with one or both updatable fields
  (`name`, `description`), THE SYSTEM SHALL apply the changes within a database transaction
  (to prevent TOCTOU races on the duplicate-name check), set `updated_by` to the
  authenticated user's ID, set `updated_at = NOW()`, and return `200 OK` with the updated
  template representation. On success, `updated_by` is set to the caller and `updated_at`
  reflects the change.
- IF the user does not hold the `template:update` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `template:update` but is not an Owner or Editor of the project AND is
  not a System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the template does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`
  (per FR-54c: UPDATE on soft-deleted rows is rejected).
- IF the updated `name` conflicts with another non-deleted template in the same project
  (case-insensitive comparison), THE SYSTEM SHALL return `409 Conflict` with
  `DUPLICATE_TEMPLATE_NAME`.
- IF no updatable fields are provided (empty body), THE SYSTEM SHALL return
  `422 Unprocessable Entity`.
- IF the request body includes unrecognised fields, THE SYSTEM SHALL reject the request with
  `422 Unprocessable Entity` (strict mode -- rejects typos and unknown fields for
  defence-in-depth).
- THE SYSTEM SHALL NOT update `id`, `project_id`, `created_at`, `created_by`, `deleted_at`,
  or `deleted_by`.
- THE SYSTEM SHALL NOT allow changing the `project_id` of a template (templates are
  permanently scoped to their project).
- IF `name` is provided: it must contain at least one non-whitespace character and must not
  exceed 255 characters after trimming leading/trailing whitespace. Whitespace-only values
  return `422`.
- IF `description` is provided: omitting the field preserves the current value. Sending
  `"description": null` explicitly clears it (sets to `NULL`). Sending `"description": ""`
  stores an empty string. Values exceeding 10000 characters return `422`.

### US-5: Delete Template (Soft-delete)

As a project Owner or Editor with `template:delete` permission, I want to soft-delete a
template so that it is hidden from all views while preserving its data. Unlike categories,
templates are standalone pre-fill sources and have no referential integrity constraints
blocking deletion.

**Acceptance Criteria (EARS)**

- WHEN a user with `template:delete` permission (or a System Admin) sends
  `DELETE /api/v1/projects/{projectId}/templates/{id}`, THE SYSTEM SHALL set
  `deleted_at = NOW()` and `deleted_by = <current_user_id>` and return `204 No Content`.
- IF the user does not hold the `template:delete` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `template:delete` but is not an Owner or Editor of the project AND is
  not a System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the template does not exist or is already soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL NOT hard-delete any record.
- Soft-deleting a template does not affect existing test cases. Templates are used only at
  the moment of test case creation/edit to pre-fill the description field; they are not
  referenced by FK after that point. Deleting a template simply removes it from the
  template selection dropdown for future test cases.

### US-6: Select Templates (Dropdown)

As a project member with `template:select` permission, I want to retrieve a compact list of
all non-deleted templates in the project (without pagination) for use in dropdown and
selection UI components, so that I can select a template to pre-fill test case descriptions.

**Acceptance Criteria (EARS)**

- WHEN a user with `template:select` permission (or a System Admin) sends `GET /api/v1/projects/{projectId}/templates/select`,
  THE SYSTEM SHALL return a flat array of all non-deleted templates in the project, each
  containing `id`, `name`, and `description`, ordered by `name` ascending.
- IF the user does not hold the `template:select` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `template:select` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL NOT paginate this endpoint -- it is designed for dropdown population and
  returns all results.
- THE SYSTEM SHALL exclude soft-deleted templates from the results.
- THE SYSTEM SHALL cap the result at 500 templates; if a project exceeds this limit, the
  response is truncated and a warning is logged. (The expected operational range is under
  200 templates per project.)
- THE SYSTEM SHALL include the `description` field in each entry so the UI can show a
  preview/snippet when the user hovers or selects a template in the dropdown.

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
template endpoint. System Admin implicitly holds all permissions and bypasses both gates.
A `403 Forbidden` response must use a generic message without revealing which gate was
triggered, to prevent information leakage about project existence or membership.

Authorization checks (system permissions and project membership) are performed against
live data on every request -- not cached in the session. If a user's role or permissions
are changed, the new authorization takes effect on their next request. There is no cached
authorization state that would allow stale-role access.

### Rate Limiting
All endpoints apply rate limiting configured at the infrastructure/middleware layer:

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../templates` (list) | 60 requests | per minute |
| `GET .../templates/{id}` (detail) | 60 requests | per minute |
| `GET .../templates/select` (dropdown) | 120 requests | per minute |
| `POST .../templates` (create) | 30 requests | per minute |
| `PATCH .../templates/{id}` (update) | 30 requests | per minute |
| `DELETE .../templates/{id}` (delete) | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### No Referential Integrity on Delete
Unlike categories, templates are not referenced by foreign key from any other table.
Templates are used at test-case creation/edit time to pre-fill the description field; the
template content is copied into the test case, and no ongoing FK relationship exists.
Therefore, template deletion does not require a referential-integrity gate. A template can
be soft-deleted freely regardless of how many test cases were created from it.

---

## Out of Scope

- **Bulk create/update/delete templates** (only single-resource operations in Phase 1)
- **Template versioning** (no version history for template content changes)
- **Template categories or tags** (templates are a flat list; categorisation through
  naming conventions only)
- **Cross-project template sharing** (templates are strictly per-project)
- **Template reordering or custom sort order** (alphabetical sort only)
- **Rich-text or Markdown rendering** of template body (storage and retrieval of plain
  text only; rich rendering is a UI-layer concern deferred to `test-case-template`)
- **Structured audit log** (the `created_by`/`updated_by`/`deleted_by` columns provide
  basic attribution; a full audit log recording old/new values per field change is deferred
  to a cross-cutting audit feature in a future phase)
- **Template restore** (clearing `deleted_at` / `deleted_by` -- deferred to a future phase;
  data is preserved but no restore endpoint exists in Phase 1)
- **Hard delete** (not implemented for any entity in Phase 1)
- **Template diff or comparison** (no side-by-side comparison of template versions)
- **UI views** (pages, forms, list views -- covered in `ui-*` specs)
- **Project-level sharing of templates** (covered in `sharing` spec; templates follow
  project-level visibility)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission codes `template:create`, `template:read`,
  `template:read_list`, `template:update`, `template:delete`, `template:select` must be
  seeded in the `PERMISSIONS` table and assignable to roles.
- **IAM Users** -- `users.id` FK reference for `created_by`, `updated_by`, `deleted_by`
  audit columns.
- **Project CRUD** -- The `PROJECTS` table must exist; templates reference
  `projects(id)` via `project_id` FK. Project creation triggers auto-seeding of default
  templates from YAML config.
- **Project Members** -- Project membership and roles (Owner, Editor, Contributor,
  Viewer) are used for authorization scope checks.
- **Auth RBAC** -- System permission checks for `template:*` codes.
- **Auth Project Scope** -- Project membership scope check on all endpoints.
- **YAML Configuration** -- A YAML configuration file loaded at application startup
  containing a `templates` section with default template definitions. The file path is
  set via environment variable.
- **Test Case CRUD** -- The `test-case-template` feature (separate spec) uses this
  template data to pre-fill test case descriptions. Templates are looked up at test-case
  creation/edit time; no FK from `TEST_CASES` to `TEST_CASE_TEMPLATES` exists.
- **UI List Views** -- The templates list view follows the standard list-view pattern
  (checkbox column, action icons, ID/Name navigation, Add New button, filter controls).
