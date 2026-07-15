# Feature: IAM Roles

## Overview

Role-based access control (RBAC) for the HOA TCMS system. Roles aggregate permissions and can be
assigned directly to users or to groups. Users receive the union of all permissions from their
directly assigned roles and from roles assigned to their groups. The System Admin role, seeded at
setup, holds all permissions and bypasses all authorization checks.

## User Stories

### US-1 — List Roles

As a System Admin or IAM manager, I want to view a paginated list of all roles, so that I can see
what roles exist in the system and manage them.

**Acceptance Criteria (EARS)**

- WHEN the client sends `GET /api/v1/roles` WITH a valid session AND the user has the
  `role:read_list` permission, THEN THE SYSTEM SHALL return a `200 OK` response with a paginated
  list of roles.
- WHEN the client sends `GET /api/v1/roles` WITHOUT a valid session, THEN THE SYSTEM SHALL return
  `401 Unauthorized`.
- WHEN the client sends `GET /api/v1/roles` WITH a valid session BUT the user lacks the
  `role:read_list` permission, THEN THE SYSTEM SHALL return `403 Forbidden`.
- IF the user is a System Admin, THEN THE SYSTEM SHALL return the full list of all roles (including
  the System Admin role itself) regardless of permissions.

### US-2 — Create Role

As a System Admin or IAM manager, I want to create a new role with a unique name and a set of
assigned permissions, so that I can define custom access profiles.

**Acceptance Criteria (EARS)**

- WHEN the client sends `POST /api/v1/roles` WITH a valid session, unique name, and at least one
  permission ID, AND the user has the `role:create` permission, THEN THE SYSTEM SHALL create the
  role, insert rows into `role_permissions`, and return `201 Created` with the new role's detail.
- IF the request body contains a name that already exists (case-insensitive), THEN THE SYSTEM SHALL
  return `409 Conflict` with code `DUPLICATE_ROLE_NAME`.
- IF the request body contains a permission ID that does not exist, THEN THE SYSTEM SHALL return
  `422 Unprocessable Entity` with code `VALIDATION_ERROR` and a field-level detail listing the
  invalid permission IDs.
- IF the request body is missing `name` or `permission_ids`, THEN THE SYSTEM SHALL return
  `422 Unprocessable Entity` with code `VALIDATION_ERROR`.
- IF a user attempts to create a role named "System Admin" (case-insensitive), THEN THE SYSTEM
  SHALL return `422 Unprocessable Entity` with code `RESERVED_NAME`.
- IF the role `name` contains HTML tags or script content, THE SYSTEM SHALL sanitize the input
  (strip HTML tags) to prevent stored XSS — role names are displayed in the UI throughout the
  application.
- WHEN the role is created, THE SYSTEM SHALL set `updated_at = created_at` and
  `updated_by = created_by` per the first-insert audit rule (FR-54b).

### US-3 — View Role Detail

As a System Admin or IAM manager, I want to view a role's details including its assigned
permissions, so that I can inspect what a role grants.

**Acceptance Criteria (EARS)**

- WHEN the client sends `GET /api/v1/roles/{id}` WITH a valid session AND the user has the
  `role:read` permission, THEN THE SYSTEM SHALL return `200 OK` with the role's name, audit
  columns, and the list of assigned permissions.
- IF a role with the given `{id}` does not exist OR has been soft-deleted, THEN THE SYSTEM SHALL
  return `404 Not Found`.
- IF the user lacks the `role:read` permission, THEN THE SYSTEM SHALL return `403 Forbidden`.
- WHEN returning the role detail, THE SYSTEM SHALL include the `permissions` array with each
  permission's `id`, `code`, and `name`.

### US-4 — Update Role

As a System Admin or IAM manager, I want to update a role's name and permission assignments, so
that I can adjust access profiles as needs change.

**Acceptance Criteria (EARS)**

- WHEN the client sends `PATCH /api/v1/roles/{id}` WITH a valid session, AND the user has the
  `role:update` permission, THEN THE SYSTEM SHALL update the role's name and/or replace its
  permission set, and return `200 OK` with the updated role detail.
- IF the new name conflicts with an existing role (case-insensitive), THEN THE SYSTEM SHALL return
  `409 Conflict` with code `DUPLICATE_ROLE_NAME`.
- IF the new name is "System Admin" (case-insensitive) AND the role being updated is not the
  System Admin role, THEN THE SYSTEM SHALL return `422 Unprocessable Entity` with code
  `RESERVED_NAME`.
- IF the role with the given `{id}` does not exist OR has been soft-deleted, THEN THE SYSTEM SHALL
  return `404 Not Found`.
- IF the user attempts to update the System Admin role's name, THEN THE SYSTEM SHALL return
  `403 Forbidden` — the System Admin role name is immutable.
- WHEN permissions are updated, THE SYSTEM SHALL replace the entire permission set: delete all
  existing `role_permissions` rows for this role and insert the new set, within a single
  transaction.

### US-5 — System Admin Role Protection

As a System Admin, I want the built-in System Admin role to be protected from modification and
deletion, so that the system always retains a full-access recovery role.

**Acceptance Criteria (EARS)**

- WHILE the application is being initialized, THE SYSTEM SHALL seed the "System Admin" role with
  all existing permissions.
- IF a user attempts to delete the System Admin role, THEN THE SYSTEM SHALL return `403 Forbidden`
  with code `PROTECTED_ROLE` (there is no DELETE endpoint for roles, but the application layer
  SHALL reject any attempt to soft-delete or hard-delete the System Admin role).
- IF a user attempts to rename the System Admin role, THEN THE SYSTEM SHALL return `403 Forbidden`
  with code `PROTECTED_ROLE`.
- IF a user attempts to modify the System Admin role's permission assignments, THEN THE SYSTEM
  SHALL return `403 Forbidden` with code `PROTECTED_ROLE`.
- WHEN the system seeds new permissions (via migration or later deployment), THE SYSTEM SHALL
  automatically grant them to the System Admin role.

### US-6 — Permission Inheritance from Group Roles

As a user, I want to receive permissions from roles assigned to my groups in addition to my
directly assigned roles, so that my access reflects my organizational memberships.

**Acceptance Criteria (EARS)**

- WHEN computing a user's effective permissions, THE SYSTEM SHALL UNION the permissions from
  all roles directly assigned to the user AND all roles assigned to every group the user belongs
  to.
- IF a role is assigned to both the user directly AND to one of the user's groups, THEN THE
  SYSTEM SHALL include the role's permissions only once (duplicate elimination).
- WHEN the system checks a permission for an action (e.g., `test_case:update`), THE SYSTEM SHALL
  evaluate the union set at query time — no pre-computation or caching of the union set.
- IF a role is removed from a group the user belongs to, THEN the permissions from that role
  SHALL be revoked from the user on the next permission check — no stale permission window beyond
  the current request.

## Out of Scope

- **Permission CRUD:** Permissions are a seeded read-only catalog. Creating, updating, or deleting
  permissions is covered by `iam-permissions`.
- **Role deletion:** There is no `DELETE /api/v1/roles/{id}` endpoint in Phase 1. Roles are not
  user-deletable. Roles can be deactivated by setting status (future) but not destroyed.
- **Role hierarchy or inheritance chains:** Roles do not inherit from other roles. The only
  inheritance model is user-level (direct + group-inherited = union).
- **Permission scope filtering:** The system does not filter the visible permission catalog based
  on which permissions a role already has. The seeded catalog is returned as-is to authorized users.
- **Bulk role assignment:** Assigning a role to multiple users or groups in a single operation is
  not supported in Phase 1. Each assignment is an individual junction row insert.

## Dependencies

- **Permission catalog** (`iam-permissions`): The seeded `PERMISSIONS` table must exist with its
  `id`, `code`, and `name` columns before `ROLE_PERMISSIONS` can reference it.
- **User entity** (`iam-users`): The `USERS` table must exist for `USER_ROLES` FK reference.
- **Group entity** (`iam-groups`): The `GROUPS` table must exist for `GROUP_ROLES` FK reference.
- **Audit columns pattern:** The `created_at`/`created_by`, `updated_at`/`updated_by`,
  `deleted_at`/`deleted_by` convention is shared across all mutable entities.
- **Junction table convention:** Composite PK, `created_at` only, hard-delete — shared across
  all N:M relationships (`USER_ROLES`, `GROUP_ROLES`, `ROLE_PERMISSIONS`).
