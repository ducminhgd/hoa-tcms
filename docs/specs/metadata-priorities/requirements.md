# Feature: Metadata Priorities

## Overview

Test Priorities provide a fixed, config-driven set of five priority levels per project
(HIGHEST, HIGH, MEDIUM, LOW, LOWEST) used to classify the urgency or importance of test
cases. The priority set is seeded from a YAML configuration file at project creation and
is not freely created or deleted by users after seeding. Project members with the Owner or
Editor role can update the `description` field of any priority; the `name` and `rank` fields
are immutable. Priorities are scoped to a single project and have no cross-project
visibility. The `rank` column (integer, 1--5) controls display ordering, with 1 being the
most urgent (HIGHEST) and 5 the least urgent (LOWEST).

---

## User Stories

### US-1: List Priorities

As a project member, I want to view a paginated list of the five priority levels in the
current project, sorted by rank ascending, so that I can understand the available priority
classification for test cases.

**Acceptance Criteria (EARS)**

- WHEN a user with `priority:read_list` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/priorities`, THE SYSTEM SHALL return a paginated list
  of the priorities scoped to that project, ordered by `rank` ascending.
- IF the user does not hold the `priority:read_list` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `priority:read_list` but is not a member of the project AND is not a
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
- THE SYSTEM SHALL support an optional `sort` query parameter with values `rank` (default),
  `-rank` (descending), `name`, `-name` (descending). Invalid sort values return `422`.
- IF the user is a System Admin, THE SYSTEM SHALL return all priorities for the project
  regardless of the admin's project membership (admin bypass).

### US-2: View Priority Detail

As a project member, I want to view the full details of a single priority level, including
its rank and audit timestamps, so that I can confirm its configuration before assigning it
to test cases.

**Acceptance Criteria (EARS)**

- WHEN a user with `priority:read` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/priorities/{id}`, THE SYSTEM SHALL return the priority
  fields (id, name, description, rank, project_id, and audit timestamps -- created_at,
  created_by, updated_at, updated_by).
- IF the user does not hold the `priority:read` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `priority:read` but is not a member of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the priority does not exist, THE SYSTEM SHALL return `404 Not Found` (same message
  to avoid information leakage).
- THE SYSTEM SHALL NOT return internal fields from a soft-delete strategy in the response
  body. Priorities are never soft-deleted in Phase 1 (they are fixed at 5 rows per project
  and persist for the entire project lifecycle).

### US-3: Update Priority

As a project Owner or Editor with `priority:update` permission, I want to update a
priority's `description` field (but not its `name` or `rank`), so that I can tailor the
guidance text shown to team members when they choose a priority for a test case.

**Acceptance Criteria (EARS)**

- WHEN a user with `priority:update` permission (or a System Admin) sends
  `PATCH /api/v1/projects/{projectId}/priorities/{id}` with the `description` field,
  THE SYSTEM SHALL apply the change within a database transaction, set `updated_by` to the
  authenticated user's ID, set `updated_at = NOW()`, and return `200 OK` with the updated
  priority representation.
- IF the user does not hold the `priority:update` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `priority:update` but is not an Owner or Editor of the project AND is
  not a System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the priority does not exist, THE SYSTEM SHALL return `404 Not Found`.
- IF no updatable fields are provided (empty body), THE SYSTEM SHALL return
  `422 Unprocessable Entity`.
- IF the request body includes unrecognised fields, THE SYSTEM SHALL reject the request with
  `422 Unprocessable Entity` (strict mode -- rejects typos and unknown fields for
  defence-in-depth).
- THE SYSTEM SHALL NOT update `id`, `project_id`, `name`, `rank`, `created_at`, `created_by`.
- THE SYSTEM SHALL NOT allow changing the `project_id` of a priority (priorities are
  permanently scoped to their project).
- The request body must contain the `description` field (it is the only updatable field,
  so "omit to preserve" does not apply — the client must resend the existing value to
  keep it unchanged). Sending `"description": null` explicitly clears it (sets to `NULL`).
  Sending `"description": ""` stores an empty string. Values exceeding 2000 characters
  return `422`.
- IF the request body includes a `name` field, THE SYSTEM SHALL return `422 Unprocessable
  Entity` with a message indicating that `name` is not an updatable field (rather than
  silently ignoring it). This provides clear feedback that the user attempted to change an
  immutable field.
- IF the request body includes a `rank` field, THE SYSTEM SHALL return `422 Unprocessable
  Entity` with a message indicating that `rank` is not an updatable field.

### US-4: Select Priorities (Dropdown)

As a project member with `priority:select` permission, I want to retrieve a compact list of
all priority levels in the project (without pagination) for use in dropdown and selection
UI components, so that I can assign priorities to test cases efficiently.

**Acceptance Criteria (EARS)**

- WHEN a user with `priority:select` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/priorities/select`, THE SYSTEM SHALL return a flat
  array of all priority levels in the project, each containing `id`, `name`, and `rank`,
  ordered by `rank` ascending.
- IF the user does not hold the `priority:select` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `priority:select` but is not a member of the project AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL NOT paginate this endpoint -- it is designed for dropdown population and
  returns all results (max 5 items, since the priority set is fixed).
- THE SYSTEM SHALL include the `rank` field in the response so consuming UIs can render
  priorities in the correct urgency order.

---

## Out of Scope

- **Create priority** (the 5 priority levels are seeded on project creation; no user-facing
  create endpoint)
- **Delete / soft-delete priority** (priorities are fixed and permanent for the project
  lifecycle; they cannot be removed by users)
- **Adding or removing priority levels** (the set of 5 is fixed by the system design)
- **Reordering priorities** (rank is immutable after seeding)
- **Renaming priorities** (name is immutable after seeding)
- **Bulk update priorities** (only single-resource PATCH in Phase 1)
- **Priority colour or icon** (no visual metadata beyond name, description, and rank)
- **Cross-project priority sharing** (priorities are strictly per-project)
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
role check is the second gate. Both must be enforced server-side for every priority
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
| `GET .../priorities` (list) | 60 requests | per minute |
| `GET .../priorities/{id}` (detail) | 60 requests | per minute |
| `GET .../priorities/select` (dropdown) | 120 requests | per minute |
| `PATCH .../priorities/{id}` (update) | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### Immutability of Name and Rank

The `name` and `rank` fields are immutable after seeding. This is enforced at three
levels for defence in depth:
1. **HTTP handler (adapter):** Reject requests containing `name` or `rank` fields with
   `422 Unprocessable Entity` (strict mode -- does not silently ignore).
2. **Service (application):** The `UpdatePriorityCommand` DTO only exposes `description`;
   `name` and `rank` are not accepted.
3. **Database trigger:** A `BEFORE UPDATE` trigger rejects any `UPDATE` that attempts to
   modify `name` or `rank`, raising an exception. This provides a final safety net against
   bugs in upper layers.

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission codes `priority:read`, `priority:read_list`,
  `priority:update`, `priority:select` must be seeded in the `PERMISSIONS` table and
  assignable to roles.
- **IAM Users** -- `users.id` FK reference for `created_by`, `updated_by` audit columns.
- **Project CRUD** -- The `PROJECTS` table must exist; priorities reference
  `projects(id)` via `project_id` FK. Project creation triggers auto-seeding of the 5
  default priority levels from YAML config.
- **Project Members** -- Project membership and roles (Owner, Editor, Contributor,
  Viewer) are used for authorization scope checks.
- **Auth RBAC** -- System permission checks for `priority:*` codes.
- **Auth Project Scope** -- Project membership scope check on all endpoints.
- **YAML Configuration** -- A YAML configuration file loaded at application startup
  containing a `priorities` section with exactly 5 priority definitions. The file path is
  set via environment variable.
- **Test Case CRUD** -- Test cases reference `TEST_PRIORITIES` via a `priority_id` FK.
- **UI List Views** -- The priorities list view follows the standard list-view pattern
  (checkbox column, action icons, ID/Name navigation, filter controls).
- **UI Dropdown** -- Dropdown components render priorities in rank order (1--5).
