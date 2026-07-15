# Feature: Project CRUD

## Overview

Project CRUD provides the foundational workspace for HOA TCMS. Users with appropriate
permissions can create, read, update, and soft-delete projects. Each project acts as a
scope container for test cases, test plans, test runs, and metadata (categories, priorities,
templates, plan types). On creation, per-project metadata is auto-seeded from a YAML
configuration file, and the creator is automatically assigned the Owner role.

---

## User Stories

### US-1: Create Project

As a user with the `project:create` system permission, I want to create a new project,
so that my team has a workspace to manage test cases, plans, and executions.

**Acceptance Criteria (EARS)**

- WHEN a user with `project:create` permission sends a valid `POST /api/v1/projects`
  request with `name`, `description`, and `status`, THE SYSTEM SHALL insert the project
  into `PROJECTS`, auto-seed per-project metadata (test categories, test case templates,
  priority levels, test plan types) from the YAML configuration file within the same
  database transaction, assign the creator as the Owner in `PROJECT_MEMBERS`, and return
  `201 Created` with the project representation and a `Location` header pointing to the
  new resource.
- IF the user does not hold the `project:create` system permission, THE SYSTEM SHALL
  return `403 Forbidden`.
- IF the project `name` is empty or exceeds 255 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with field-level validation details.
- IF a project with the same name already exists (case-insensitive comparison),
  THE SYSTEM SHALL return `409 Conflict` with error code `DUPLICATE_PROJECT_NAME`.
- IF the YAML configuration file is unreadable, malformed, or missing required sections,
  THE SYSTEM SHALL return `500 Internal Server Error` and roll back the transaction.
- IF the creator user is soft-deleted or does not exist, THE SYSTEM SHALL return
  `500 Internal Server Error` (this indicates a corrupted session state; the creator
  identity is derived from the authenticated session).

### US-2: List Projects

As a user with `project:read_list` permission, I want to view a paginated list of
projects I belong to, so that I can browse available workspaces and navigate to them.

**Acceptance Criteria (EARS)**

- WHEN a user sends `GET /api/v1/projects`, THE SYSTEM SHALL return a paginated list
  of projects where the user is an active project member, excluding soft-deleted
  projects, ordered by `created_at` descending.
- IF the user does not hold the `project:read_list` system permission, THE SYSTEM SHALL
  return `403 Forbidden`.
- THE SYSTEM SHALL support pagination via query parameters `page` (default 1, minimum 1)
  and `limit` (default 25, minimum 1, maximum 100).
- THE SYSTEM SHALL include pagination metadata (`total`, `page`, `limit`) in the response.
- WHILE a project is soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL exclude it
  from the list for all users regardless of membership.
- IF the user is a System Admin, THE SYSTEM SHALL return all non-deleted projects
  (admin bypass).

### US-3: View Project Detail

As a project member, I want to view the full details of a project including its members
and their roles, so that I understand the project context and who has access.

**Acceptance Criteria (EARS)**

- WHEN a user with `project:read` permission sends `GET /api/v1/projects/{id}`,
  THE SYSTEM SHALL return the project fields (id, name, description, status, member
  list with roles, audit timestamps), excluding soft-deleted projects.
- IF the user does not hold the `project:read` system permission, THE SYSTEM SHALL
  return `403 Forbidden`.
- IF the user holds the `project:read` system permission but is not an active member
  of the project AND is not a System Admin, THE SYSTEM SHALL return `403 Forbidden`
  (same generic message to avoid revealing project existence).
- IF the project is soft-deleted or does not exist, THE SYSTEM SHALL return
  `404 Not Found` (same message for both to avoid information leakage).
- THE SYSTEM SHALL include all project members in the response (their `user_id`,
  `username`, `fullname`, and `role`).

### US-4: Update Project

As a project Owner or Editor (user with `project:update` permission), I want to update
project fields (name, description, status), so that I can keep project information current
and manage its lifecycle.

**Acceptance Criteria (EARS)**

- WHEN a user with `project:update` permission sends `PATCH /api/v1/projects/{id}` with
  one or more updatable fields, THE SYSTEM SHALL apply the changes and return `200 OK`
  with the updated project representation.
- IF the user does not hold the `project:update` system permission, THE SYSTEM SHALL
  return `403 Forbidden`.
- IF the user holds the `project:update` system permission but is not an Owner or
  Editor on the project AND is not a System Admin, THE SYSTEM SHALL return
  `403 Forbidden` (same generic message; lacking project role is indistinguishable
  from lacking system permission).
- IF the project is soft-deleted, THE SYSTEM SHALL return `404 Not Found` (per FR-54c:
  UPDATE on soft-deleted rows is rejected).
- IF the updated `name` conflicts with another project (case-insensitive), THE SYSTEM
  SHALL return `409 Conflict` with `DUPLICATE_PROJECT_NAME`.
- WHEN the project status is changed to `INACTIVE`, THE SYSTEM SHALL hide all child
  objects (test cases, test plans, test runs, test executions) from list views, detail
  views, and selection fields, WHILE preserving the underlying data and existing
  references (FKs remain intact).
- IF `status` is set to a value other than `ACTIVE` or `INACTIVE`, THE SYSTEM SHALL
  return `422 Unprocessable Entity`.
- THE SYSTEM SHALL NOT update `created_at`, `created_by`, or `id`.

### US-5: Delete Project (Soft-delete)

As a project Owner (user with `project:delete` permission), I want to delete a project
so that it and all its child objects are hidden from all views while the underlying data
is preserved for potential future restore.

**Acceptance Criteria (EARS)**

- WHEN a user with `project:delete` permission sends `DELETE /api/v1/projects/{id}`,
  THE SYSTEM SHALL set `deleted_at = NOW()` and `deleted_by = <current_user_id>` on the
  project row, set no `deleted_at` on child objects (no cascade), and return
  `204 No Content`.
- IF the user does not hold the `project:delete` system permission, THE SYSTEM SHALL
  return `403 Forbidden`.
- IF the user holds the `project:delete` system permission but is not an Owner on
  the project AND is not a System Admin, THE SYSTEM SHALL return `403 Forbidden`
  (same generic message).
- IF the project is already soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL NOT hard-delete any record.
- After soft-delete, all child objects (test cases, test plans, test runs, test
  executions, metadata) remain intact in the database with their original
  `deleted_at = NULL`. They are hidden from views because the parent project is
  filtered out (no cascade required per FR-53: parent deletion hides children).
- THE SYSTEM SHALL NOT cascade soft-delete to project members (project membership is
  preserved for potential restore).

---

## Security Considerations

### Input Sanitization (XSS Prevention)
All user-supplied text fields (`name`, `description`) must be sanitized on input
to strip disallowed HTML tags and malicious script content, and output-encoded at
the presentation layer to prevent Cross-Site Scripting (XSS). This implements the
project-wide policy defined in PRD §5.3 (XSS prevention). Defence-in-depth requires
both input sanitisation and output encoding.

### CSRF Protection
All state-changing endpoints (`POST /api/v1/projects`, `PATCH /api/v1/projects/{id}`,
`DELETE /api/v1/projects/{id}`) must be protected against Cross-Site Request
Forgery (CSRF). Session cookies must carry the `SameSite=Lax` attribute (or stricter).
Implementations must verify `Content-Type` headers and/or include anti-CSRF tokens
for defense in depth.

### Authorization Consistency
The system permission check is the first authorization gate; the project membership
and role check is the second gate. Both must be enforced server-side for every
project-scoped endpoint. The System Admin role bypasses both gates. A `403 Forbidden`
response must use a generic message without revealing which gate was triggered,
to prevent information leakage about project existence or membership.

### Rate Limiting
All endpoints should apply rate limiting to mitigate brute-force attacks on
resource enumeration. Consider `429 Too Many Requests` responses with a
`Retry-After` header.

---

## Out of Scope

- **Project member management** (add/remove members, change roles — covered in
  `project-members` spec)
- **Project-level sharing** (covered in `sharing` spec)
- **Project restore** (clearing `deleted_at` / `deleted_by` — deferred to a future
  phase; the data is preserved but no restore endpoint exists in Phase 1)
- **Hard delete** (not implemented for any entity in Phase 1)
- **YAML configuration file schema design** (the config file format is defined by the
  infrastructure team; this spec assumes it exists and is loaded at startup)
- **Metadata CRUD** (per-project categories, templates, priorities, plan types — covered
  in their respective `metadata-*` specs)
- **UI views** (pages, forms, list views — covered in `ui-*` specs)

---

## Dependencies

- **IAM Auth** — Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** — The permission codes `project:create`, `project:read`,
  `project:read_list`, `project:update`, `project:delete` must be seeded in the
  `PERMISSIONS` table and assignable to roles.
- **IAM Users** — `users.id` FK reference for `created_by`, `updated_by`, `deleted_by`
  audit columns and for `PROJECT_MEMBERS.user_id`.
- **YAML Configuration** — A YAML configuration file must be loaded at application
  startup, containing default metadata sections for test categories, test case templates,
  priority levels, and test plan types. The file path is set via environment variable.
  See [OQ-03 in PRD](../docs/PRD-Phase-01.md) — the schema is defined by the
  infrastructure team.
