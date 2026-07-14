# Feature: Project Members

## Overview

Manage user membership on projects with role-based access (Owner, Editor, Contributor,
Viewer). Project membership is the second layer of the three-layer authorization model
— it grants scope-based access to all objects within a project. Only users with
appropriate authority (project Owner or `project:update` system permission) can add,
remove, or change member roles.

## User Stories

### US-01: List Project Members

As a project member, I want to see the list of all users who are members of a project
and their roles, so that I know who has access to the project and at what level.

**Acceptance Criteria (EARS)**

- WHEN I submit a `GET /api/v1/projects/{project_id}/members` request with a valid
  session and the `project:read` system permission, AND I am a member of the project,
  THE SYSTEM SHALL return `200 OK` with a list of all members including their `user_id`,
  `username`, `fullname`, `role`, and `created_at`.
- IF the project does not exist, THE SYSTEM SHALL return `404 Not Found`.
- IF the user does not hold the `project:read` system permission, THE SYSTEM SHALL
  return `403 Forbidden`.
- IF the user is not a member of the project and the object is not shared with them,
  THE SYSTEM SHALL return `403 Forbidden`.

### US-02: Add Members to Project

As a project Owner, I want to add users to the project with a specific role, so that
they can access project resources according to their assigned permissions.

**Acceptance Criteria (EARS)**

- WHEN I submit a `POST /api/v1/projects/{project_id}/members` request with a valid
  session and an `add` array containing user IDs and roles, THE SYSTEM SHALL add each
  listed user as a member of the project with the specified role, and return `200 OK`
  with a summary of added members.
- IF I am not an Owner of the project AND do not hold the `project:update` system
  permission, THE SYSTEM SHALL return `403 Forbidden`.
- IF I submit a `user_id` that does not exist in the system, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a field-level validation error for that entry.
- IF I submit a `user_id` belonging to a user with `status = INACTIVE` or
  `deleted_at IS NOT NULL`, THE SYSTEM SHALL return `422 Unprocessable Entity` with
  a field-level error — deactivated users cannot be added as members.
- IF I submit a `user_id` that is already a member of the project, THE SYSTEM SHALL
  return `409 Conflict` with a message indicating the duplicate entry.
- IF I submit a role value other than `Owner`, `Editor`, `Contributor`, or `Viewer`,
  THE SYSTEM SHALL return `422 Unprocessable Entity` with a field-level validation error.

### US-03: Remove Members from Project

As a project Owner, I want to remove users from the project, so that they no longer
have access to project resources through membership.

**Acceptance Criteria (EARS)**

- WHEN I submit a `POST /api/v1/projects/{project_id}/members` request with a valid
  session and a `remove` array containing user IDs, THE SYSTEM SHALL remove each
  listed user from the project and return `200 OK` with a summary of removed members.
- IF I am not an Owner of the project AND do not hold the `project:update` system
  permission, THE SYSTEM SHALL return `403 Forbidden`.
- IF I submit a `user_id` in the `remove` array that is not a current member of the
  project, THE SYSTEM SHALL treat that entry as idempotent (skip it without error).
- IF the `remove` array contains the last Owner of the project, THE SYSTEM SHALL
  return `409 Conflict` — a project must always have at least one Owner.
- IF the `remove` array contains the user making the request (self-removal), THE SYSTEM
  SHALL proceed with the removal (the user loses access immediately).

### US-04: Change Member Role

As a project Owner, I want to change a member's role, so that I can adjust their
level of access as project needs evolve.

**Acceptance Criteria (EARS)**

- WHEN I submit a `PATCH /api/v1/projects/{project_id}/members/{user_id}` request with
  a valid session and a `role` field, THE SYSTEM SHALL update the member's role and
  return `200 OK` with the updated membership record.
- IF I am not an Owner of the project AND do not hold the `project:update` system
  permission, THE SYSTEM SHALL return `403 Forbidden`.
- IF the target user is not a member of the project, THE SYSTEM SHALL return
  `404 Not Found`.
- IF I attempt to change the role of the last Owner away from Owner, THE SYSTEM SHALL
  return `409 Conflict` — a project must always have at least one Owner.
- IF I submit a role value other than `Owner`, `Editor`, `Contributor`, or `Viewer`,
  THE SYSTEM SHALL return `422 Unprocessable Entity` with a field-level validation error.

### US-05: Project Membership Scope Check

As a system component performing authorization, I want to verify that a user is a
member of an object's project, so that I can enforce scope-based access control.

**Acceptance Criteria (EARS)**

- WHEN a user attempts an action on a project-scoped object, THE SYSTEM SHALL check
  whether the user is a member of the object's project BEFORE granting access.
- IF the user is a member of the project, THE SYSTEM SHALL resolve the effective role
  from the membership record to determine what actions the user can perform on
  project objects.
- IF the user is not a member of the project, THE SYSTEM SHALL fall through to the
  object-sharing scope check (third layer of authorization).
- IF the user is a System Admin, THE SYSTEM SHALL skip the project membership check
  entirely and grant full access.
- WHEN a project membership check is performed, THE SYSTEM SHALL ignore members whose
  owning project has `deleted_at IS NOT NULL`.

### US-06: Bulk Manage Members (Add + Remove in One Request)

As a project Owner, I want to add and remove members in a single request, so that
I can efficiently update the team composition.

**Acceptance Criteria (EARS)**

- WHEN I submit a `POST /api/v1/projects/{project_id}/members` request containing
  both an `add` array and a `remove` array, THE SYSTEM SHALL process removals first,
  then additions, and return `200 OK` with a combined summary.
- IF the user making the request is listed in both `add` and `remove` for the same
  project, THE SYSTEM SHALL process this consistently (removal then re-add if
  appropriate).
- WHEN the `add` array is empty, THE SYSTEM SHALL process only removals. WHEN the
  `remove` array is empty, THE SYSTEM SHALL process only additions.

## Security Considerations

### Input Sanitization (XSS Prevention)
The member list response includes `username` and `fullname` from the USERS table.
These must be sanitized on input (when users are created/updated) and output-encoded
at the presentation layer to prevent Cross-Site Scripting (XSS). See PRD §5.3 for
the project-wide XSS prevention policy.

### CSRF Protection
All state-changing endpoints (`POST /api/v1/projects/{project_id}/members`,
`PATCH /api/v1/projects/{project_id}/members/{user_id}`) must be protected against
Cross-Site Request Forgery (CSRF). Session cookies must carry the `SameSite=Lax`
attribute (or stricter). Requests with `Content-Type: application/json` must verify
the `Content-Type` header to block simple form-based CSRF attacks.

### Authorization Consistency
The member management endpoints enforce a two-gate authorization check: (1) the
caller must hold the `project:update` system permission, AND (2) the caller must
be an Owner of the project (or be a System Admin, who bypasses both gates). Both
checks are enforced server-side. A `403 Forbidden` response must use a generic
message without revealing which gate was triggered.

### Last-Owner Protection
The system must prevent removing or downgrading the last Owner of a project.
This check must happen BEFORE executing any removal or role change, within the
same database transaction to prevent race conditions. Concurrent requests that
would simultaneously remove or downgrade the last Owner must be serialized.

### Deactivated User Handling
When a user is deactivated (`status = INACTIVE` or `deleted_at IS NOT NULL`),
the authorization middleware (as defined in PRD §6.5) rejects access at the
authentication layer. However, the membership record is preserved to maintain
data integrity. The system must not inadvertently reactivate access by adding a
deactivated user as a member — this is explicitly rejected per US-02.

### Rate Limiting
All member management endpoints should apply rate limiting to mitigate
brute-force attacks on user enumeration. Consider `429 Too Many Requests`
responses with a `Retry-After` header.

## Out of Scope

- Soft-delete on PROJECT_MEMBERS (junction table uses hard delete only; the record
  is removed entirely).
- Bulk role update endpoint (each role change requires a targeted PATCH request).
- Invitation workflow or pending membership status (members are added directly
  without acceptance flow).
- Membership history or audit log of member additions/removals (deferred to a
  cross-cutting audit feature).
- Automatic role assignment on project creation (the creator is seeded as Owner
  via `project-crud` feature).
- Group-based project membership (project membership is per-user only in Phase 1).
- Project membership for non-user entities (e.g., service accounts, API tokens).
- Cascading removal of object sharing entries when a member is removed (sharing
  entries are independent of project membership).
- Rate limiting on member management endpoints.

## Dependencies

- **PROJECTS table** — must exist with a valid `id` PK (from `project-crud` feature).
- **USERS table** — must exist with `id`, `username`, `fullname`, `status`,
  `deleted_at` columns (from `iam-users` feature).
- **Auth middleware** — session verification must be in place to authenticate
  requests before reaching member management handlers.
- **System permission `project:read`** — required for listing members (from
  `iam-permissions` feature).
- **System permission `project:update`** — required for adding/removing members
  (from `iam-permissions` feature).
- **`auth-project-scope` feature** — the project membership scope check component
  consumes the PROJECT_MEMBERS data to verify user access on project-scoped objects.
