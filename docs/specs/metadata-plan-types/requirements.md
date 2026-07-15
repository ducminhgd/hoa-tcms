# Feature: Metadata Plan Types

## Overview

Test Plan Types provide a fixed, config-driven set of seven plan type levels per project
(ACCEPTANCE, FUNCTIONAL, API, INTEGRATION, PERFORMANCE, REGRESSION, SECURITY) used to
classify the nature or methodology of test plans. The plan type set is seeded from a YAML
configuration file at project creation and is not freely created or deleted by users after
seeding. Project members with the Owner or Editor role can update the `description` field
of any plan type; the `name` field is immutable. Plan types are scoped to a single project
and have no cross-project visibility. Unlike priorities, plan types have no inherent
ordering -- they are sorted alphabetically by `name`.

---

## User Stories

### US-1: List Plan Types

As a project member, I want to view a paginated list of the seven plan type levels in the
current project, sorted alphabetically by name, so that I can understand the available plan
type classifications for test plans.

**Acceptance Criteria (EARS)**

- WHEN a user with `plan_type:read_list` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/plan-types`, THE SYSTEM SHALL return a paginated list
  of the plan types scoped to that project, ordered by `name` ascending.
- IF the user does not hold the `plan_type:read_list` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `plan_type:read_list` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden` (same generic message).
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
  `-name` (descending). Invalid sort values return `422`.
- IF the user is a System Admin, THE SYSTEM SHALL return all plan types for the project
  regardless of the admin's project membership (admin bypass).

### US-2: View Plan Type Detail

As a project member, I want to view the full details of a single plan type, including its
audit timestamps, so that I can confirm its configuration before assigning it to test plans.

**Acceptance Criteria (EARS)**

- WHEN a user with `plan_type:read` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/plan-types/{id}`, THE SYSTEM SHALL return the plan type
  fields (id, name, description, project_id, and audit timestamps -- created_at, created_by,
  updated_at, updated_by).
- IF the user does not hold the `plan_type:read` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `plan_type:read` but is not a member of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the plan type does not exist, THE SYSTEM SHALL return `404 Not Found` (same message
  to avoid information leakage).
- THE SYSTEM SHALL NOT return internal fields from a soft-delete strategy in the response
  body. Plan types are never soft-deleted in Phase 1 (they are fixed at 7 rows per project
  and persist for the entire project lifecycle).

### US-3: Update Plan Type

As a project Owner or Editor with `plan_type:update` permission, I want to update a plan
type's `description` field (but not its `name`), so that I can tailor the guidance text
shown to team members when they choose a plan type for a test plan.

**Acceptance Criteria (EARS)**

- WHEN a user with `plan_type:update` permission (or a System Admin) sends
  `PATCH /api/v1/projects/{projectId}/plan-types/{id}` with the `description` field,
  THE SYSTEM SHALL apply the change within a database transaction, set `updated_by` to the
  authenticated user's ID, set `updated_at = NOW()`, and return `200 OK` with the updated
  plan type representation.
- IF the user does not hold the `plan_type:update` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `plan_type:update` but is not an Owner or Editor of the project AND is
  not a System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the plan type does not exist, THE SYSTEM SHALL return `404 Not Found`.
- IF no updatable fields are provided (empty body), THE SYSTEM SHALL return
  `422 Unprocessable Entity`.
- IF the request body includes unrecognised fields, THE SYSTEM SHALL reject the request with
  `422 Unprocessable Entity` (strict mode -- rejects typos and unknown fields for
  defence-in-depth).
- THE SYSTEM SHALL NOT update `id`, `project_id`, `name`, `created_at`, `created_by`.
- THE SYSTEM SHALL NOT allow changing the `project_id` of a plan type (plan types are
  permanently scoped to their project).
- IF `description` is provided: omitting the field preserves the current value. Sending
  `"description": null` explicitly clears it (sets to `NULL`). Sending `"description": ""`
  stores an empty string. Values exceeding 2000 characters return `422`.
- IF the request body includes a `name` field, THE SYSTEM SHALL return `422 Unprocessable
  Entity` with a message indicating that `name` is not an updatable field (rather than
  silently ignoring it). This provides clear feedback that the user attempted to change an
  immutable field.

### US-4: Select Plan Types (Dropdown)

As a project member with `plan_type:select` permission, I want to retrieve a compact list
of all plan type levels in the project (without pagination) for use in dropdown and
selection UI components, so that I can assign plan types to test plans efficiently.

**Acceptance Criteria (EARS)**

- WHEN a user with `plan_type:select` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/plan-types/select`, THE SYSTEM SHALL return a flat
  array of all plan type levels in the project, each containing `id` and `name`, ordered by
  `name` ascending.
- IF the user does not hold the `plan_type:select` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `plan_type:select` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL NOT paginate this endpoint -- it is designed for dropdown population and
  returns all results (max 7 items, since the plan type set is fixed).
- THE SYSTEM SHALL sort results alphabetically by `name` ascending so consuming UIs can
  render plan types in a consistent order.

---

## Out of Scope

- **Create plan type** (the 7 plan type levels are seeded on project creation; no
  user-facing create endpoint)
- **Delete / soft-delete plan type** (plan types are fixed and permanent for the project
  lifecycle; they cannot be removed by users)
- **Adding or removing plan type levels** (the set of 7 is fixed by the system design)
- **Renaming plan types** (name is immutable after seeding)
- **Bulk update plan types** (only single-resource PATCH in Phase 1)
- **Plan type colour or icon** (no visual metadata beyond name and description)
- **Cross-project plan type sharing** (plan types are strictly per-project)
- **Structured audit log** (the `created_by`/`updated_by` columns provide basic
  attribution; a full audit log is deferred to a cross-cutting audit feature)
- **UI views** (pages, forms, list views -- covered in `ui-*` specs)

---

## Security Considerations

### Input Sanitization (XSS Prevention)

All user-supplied text fields (`description`) must be sanitized on input to strip
disallowed HTML tags and malicious script content, and output-encoded at the presentation
layer to prevent Cross-Site Scripting (XSS). This implements the project-wide policy
defined in PRD 5.3 (XSS prevention). Defence-in-depth requires both input sanitisation
and output encoding.

### CSRF Protection

All state-changing endpoints (`PATCH`) must be protected against Cross-Site Request
Forgery (CSRF). Session cookies must carry the `SameSite=Lax` attribute (or stricter).
Implementations must verify `Content-Type` headers and/or include anti-CSRF tokens for
defense in depth.

### Authorization Consistency

The system permission check is the first authorization gate; the project membership and
role check is the second gate. Both must be enforced server-side for every plan type
endpoint. System Admin implicitly holds all permissions and bypasses both gates. A
`403 Forbidden` response must use a generic message without revealing which gate was
triggered, to prevent information leakage about project existence or membership.

Authorization checks (system permissions and project membership) are performed against
live data on every request -- not cached in the session. If a user's role or permissions
are changed, the new authorization takes effect on their next request.

### Rate Limiting

All endpoints apply rate limiting configured at the infrastructure/middleware layer:

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../plan-types` (list) | 60 requests | per minute |
| `GET .../plan-types/{id}` (detail) | 60 requests | per minute |
| `GET .../plan-types/select` (dropdown) | 120 requests | per minute |
| `PATCH .../plan-types/{id}` (update) | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### Immutability of Name

The `name` field is immutable after seeding. This is enforced at three levels for defence
in depth:

1. **HTTP handler (adapter):** Reject requests containing a `name` field with
   `422 Unprocessable Entity` (strict mode -- does not silently ignore).
2. **Service (application):** The `UpdatePlanTypeCommand` DTO only exposes `description`;
   `name` is not accepted.
3. **Database trigger:** A `BEFORE UPDATE` trigger rejects any `UPDATE` that attempts to
   modify `name`, raising an exception. This provides a final safety net against bugs in
   upper layers.

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission codes `plan_type:read`, `plan_type:read_list`,
  `plan_type:update`, `plan_type:select` must be seeded in the `PERMISSIONS` table and
  assignable to roles.
- **IAM Users** -- `users.id` FK reference for `created_by`, `updated_by` audit columns.
- **Project CRUD** -- The `PROJECTS` table must exist; plan types reference
  `projects(id)` via `project_id` FK. Project creation triggers auto-seeding of the 7
  default plan type levels from YAML config.
- **Project Members** -- Project membership and roles (Owner, Editor, Contributor,
  Viewer) are used for authorization scope checks.
- **Auth RBAC** -- System permission checks for `plan_type:*` codes.
- **Auth Project Scope** -- Project membership scope check on all endpoints.
- **YAML Configuration** -- A YAML configuration file loaded at application startup
  containing a `plan_types` section with exactly 7 plan type definitions. The file path is
  set via environment variable.
- **Test Plan CRUD** -- Test plans reference `TEST_PLAN_TYPES` via a `plan_type_id` FK.
- **UI List Views** -- The plan types list view follows the standard list-view pattern
  (checkbox column, action icons, ID/Name navigation, filter controls).
- **UI Dropdown** -- Dropdown components render plan types in alphabetical order.
