# Feature: IAM Groups

## Overview

Provide Group CRUD and user-group membership management for the HOA TCMS. Groups aggregate
users and receive Role assignments. A User inherits permissions from Roles assigned to their
Groups, combined with directly assigned Roles (union). Groups have ACTIVE/INACTIVE status and
support soft-delete.

## User Stories

### US-01: List Groups

As a System Admin, I want to view a paginated list of groups, so that I can see all groups
in the system and navigate to individual group details.

**Acceptance Criteria (EARS)**

- WHEN I submit a `GET /api/v1/groups` request with a valid session and the `group:read_list`
  permission, THE SYSTEM SHALL return `200 OK` with a paginated list of groups.
- WHEN I specify `page`, `limit`, `sort`, and `order` query parameters, THE SYSTEM SHALL
  return the requested page of results sorted accordingly (default: `page=1`, `limit=25`,
  `sort=name`, `order=asc`).
- WHEN a group has been soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL exclude
  it from the list.
- WHEN a group has status `INACTIVE`, THE SYSTEM SHALL still include it in the list
  (status filtering is a client-side concern, unless a filter parameter is provided).
- IF I do not hold the `group:read_list` permission, THE SYSTEM SHALL return
  `403 Forbidden`.
- IF I am not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.
- THE SYSTEM SHALL support an optional `status` query parameter to filter by
  `ACTIVE`/`INACTIVE`.

### US-02: Create Group

As a System Admin, I want to create a new group, so that I can organize users into logical
collections for role assignment.

**Acceptance Criteria (EARS)**

- WHEN I submit a `POST /api/v1/groups` request with a valid session, the `group:create`
  permission, and a JSON body containing `name` (required, unique) and optionally
  `description`, THE SYSTEM SHALL create the group with status `ACTIVE`, set audit columns
  (`created_at`/`created_by`, `updated_at`/`updated_by`), and return `201 Created` with the
  created group and a `Location` header pointing to `/api/v1/groups/{id}`.
- IF I submit a `name` that already exists (case-insensitive), THE SYSTEM SHALL return
  `409 Conflict` with error code `DUPLICATE_NAME`.
- IF I submit a `name` that is empty or exceeds the maximum length (e.g., 255 characters),
  THE SYSTEM SHALL return `422 Unprocessable Entity` with a validation error.
- IF I submit a `name` or `description` containing HTML tags or script content,
  THE SYSTEM SHALL sanitize the input (strip HTML tags) to prevent stored XSS — all
  user-controlled display fields are output-encoded at the presentation layer, but
  defence-in-depth requires input sanitisation.
- IF I do not hold the `group:create` permission, THE SYSTEM SHALL return `403 Forbidden`.
- IF I am not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.

### US-03: View Group Detail

As a System Admin, I want to view a group's details including its member list and role list,
so that I can understand who belongs to the group and what roles it has.

**Acceptance Criteria (EARS)**

- WHEN I submit a `GET /api/v1/groups/{id}` request with a valid session and the
  `group:read` permission, THE SYSTEM SHALL return `200 OK` with the group's fields
  (`id`, `name`, `description`, `status`, `created_at`, `created_by`, `updated_at`,
  `updated_by`, `member_count`, `role_count`) and arrays of `members` and `roles`.
- WHEN the group has been soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL return
  `404 Not Found`.
- IF I specify a non-existent group ID, THE SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL include the member count and role count in the response (computed from
  junction tables).
- THE SYSTEM SHALL return the members array as `[{user_id, username, fullname}]`.
- THE SYSTEM SHALL return the roles array as `[{role_id, role_name}]`.
- IF I do not hold the `group:read` permission, THE SYSTEM SHALL return `403 Forbidden`.
- IF I am not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.

### US-04: Update Group

As a System Admin, I want to update a group's name, description, or status, so that I can
keep group information current.

**Acceptance Criteria (EARS)**

- WHEN I submit a `PATCH /api/v1/groups/{id}` request with a valid session, the
  `group:update` permission, and a JSON body containing any subset of updatable fields
  (`name`, `description`, `status`), THE SYSTEM SHALL update only the provided fields,
  set `updated_at`/`updated_by` via DB trigger, and return `200 OK` with the updated group.
- IF I submit a `name` that already exists on a different group (case-insensitive),
  THE SYSTEM SHALL return `409 Conflict` with error code `DUPLICATE_NAME`.
- IF I attempt to update a soft-deleted group (`deleted_at IS NOT NULL`), THE SYSTEM SHALL
  return `404 Not Found` (the group is not visible).
- IF I set `status` to a value other than `ACTIVE` or `INACTIVE`, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a validation error.
- IF I do not hold the `group:update` permission, THE SYSTEM SHALL return `403 Forbidden`.
- IF I am not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.

### US-05: Manage Group Members

As a System Admin, I want to add users to or remove users from a group, so that users
inherit the group's roles.

**Acceptance Criteria (EARS)**

- WHEN I submit a `POST /api/v1/groups/{id}/members` request with a valid session, the
  `group:update` permission, and a JSON body containing `add` (array of user IDs to add)
  and/or `remove` (array of user IDs to remove), THE SYSTEM SHALL:
  - Insert rows into `USER_GROUPS` for each user ID in `add` that is not already a member.
  - Hard-delete rows from `USER_GROUPS` for each user ID in `remove` that is currently a member.
  - Skip already-member user IDs in `add` without error.
  - Skip non-member user IDs in `remove` without error.
  - Return `200 OK` with `{ added: N, removed: N }` counts.
- IF I attempt to add a user that does not exist, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a validation error listing invalid user IDs.
- IF I attempt to modify a soft-deleted group, THE SYSTEM SHALL return `404 Not Found`.
- IF I do not hold the `group:update` permission, THE SYSTEM SHALL return `403 Forbidden`.
- IF I am not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.

### US-06: Manage Group Roles

As a System Admin, I want to assign roles to or remove roles from a group, so that the
group's members collectively inherit those role permissions.

**Acceptance Criteria (EARS)**

- WHEN I submit a `POST /api/v1/groups/{id}/roles` request with a valid session, the
  `group:update` permission, and a JSON body containing `add` (array of role IDs to assign)
  and/or `remove` (array of role IDs to remove), THE SYSTEM SHALL:
  - Insert rows into `GROUP_ROLES` for each role ID in `add` that is not already assigned.
  - Hard-delete rows from `GROUP_ROLES` for each role ID in `remove` that is currently assigned.
  - Skip already-assigned role IDs in `add` without error.
  - Skip non-assigned role IDs in `remove` without error.
  - Return `200 OK` with `{ added: N, removed: N }` counts.
- IF I attempt to assign a role that does not exist, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a validation error listing invalid role IDs.
- IF I attempt to modify a soft-deleted group, THE SYSTEM SHALL return `404 Not Found`.
- IF I do not hold the `group:update` permission, THE SYSTEM SHALL return `403 Forbidden`.
- IF I am not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.

### US-07: Delete Group (Soft-Delete)

As a System Admin, I want to soft-delete a group, so that it is hidden from the system
without losing historical membership or role assignment data.

**Acceptance Criteria (EARS)**

- WHEN I submit a `DELETE /api/v1/groups/{id}` request with a valid session and the
  `group:delete` permission, THE SYSTEM SHALL set `deleted_at` to `NOW()` and `deleted_by`
  to the current user's ID on the group record, and return `204 No Content`.
- IF the group is already soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF I attempt to delete a group that does not exist, THE SYSTEM SHALL return
  `404 Not Found`.
- WHILE a group is soft-deleted, all junction rows in `USER_GROUPS` and `GROUP_ROLES` for
  that group REMAIN intact (no cascade delete — per FR-53).
- IF I do not hold the `group:delete` permission, THE SYSTEM SHALL return `403 Forbidden`.
- IF I am not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.

### US-08: Permission Inheritance from Groups

As a System Admin, I want group members to automatically inherit permissions from roles
assigned to their groups, so that I can manage permissions at the group level rather than
individually per user.

**Acceptance Criteria (EARS)**

- WHEN a user is a member of a group, THE SYSTEM SHALL include permissions from roles
  assigned to that group in the user's effective permission set.
- IF a user belongs to multiple groups, THE SYSTEM SHALL union the permissions from all
  roles across all their groups.
- IF a user has both a directly assigned role and a group-assigned role that grant the same
  permission, THE SYSTEM SHALL include that permission only once in the effective set
  (union semantics — no duplication).
- WHEN a user is removed from a group, THE SYSTEM SHALL exclude the permissions that were
  inherited from that group's roles from the user's effective permission set on the next
  request (no caching of stale permissions beyond the request lifetime).
- WHEN a role is removed from a group, THE SYSTEM SHALL exclude the permissions that were
  granted through that role from all group members' effective permission sets on the next
  request.

## Out of Scope

- Group hierarchy or nested groups (groups within groups).
- Group-level project membership (groups are not directly added to projects).
- Automatic membership management (e.g., rules-based or SCIM provisioning).
- Group-specific UI customizations or branding.
- Transferring group ownership or delegation of group management.
- Bulk import/export of group members or roles via CSV/JSON.
- Audit log of membership or role assignment changes (deferred to cross-cutting audit).

## Dependencies

- **GROUPS table** in PostgreSQL — must be created with columns: `id`, `name`, `description`,
  `status`, audit columns (`created_at`/`created_by`, `updated_at`/`updated_by`,
  `deleted_at`/`deleted_by`). Unique constraint on `name`.
- **USER_GROUPS junction table** — columns: `user_id`, `group_id`, `created_at`. Composite
  PK on `(user_id, group_id)`. Hard delete (no soft-delete columns per FR-52 junction table
  rule).
- **GROUP_ROLES junction table** — columns: `group_id`, `role_id`, `created_at`. Composite
  PK on `(group_id, role_id)`. Hard delete. Table is owned and created by the `iam-roles`
  feature; iam-groups queries it for role-management operations.
- **USERS table** — must exist with `id`, `username`, `fullname`, `status`, `deleted_at`
  (from `iam-users` feature). Members reference this table.
- **ROLES table** — must exist with `id`, `name` (from `iam-roles` feature). Group role
  assignments reference this table.
- **Auth middleware** — session verification and user context on the request.
- **Authorization middleware** — permission checks for `group:read_list`, `group:create`,
  `group:read`, `group:update`, `group:delete`.
- **Audit column trigger** — DB trigger for `updated_at`/`updated_by` auto-maintenance
  (cross-cutting).
- **Permission codes** — `group:read_list`, `group:create`, `group:read`, `group:update`,
  `group:delete` must exist in the `PERMISSIONS` table.
