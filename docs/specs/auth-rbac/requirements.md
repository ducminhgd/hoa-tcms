# Feature: Auth RBAC

## Overview

The System RBAC feature implements the first layer of the three-tier authorization model:
permission checking. It resolves a user's effective permissions by computing the union of
permissions from their directly assigned roles and roles inherited through group memberships.
Every API endpoint that requires a system permission calls `AuthorizationService::check_permission`
as its first authorization gate.

This feature uses the existing IAM tables (`user_roles`, `group_roles`, `role_permissions`,
`permissions`) and introduces no new database tables. It is a pure application-layer feature
that provides the `AuthorizationService` consumed by all other features.

## User Stories

### US-1: Check a system permission for a user

As a **feature developer**, I want a single method to check whether the current user holds a
specific system permission code, so that I can gate every protected endpoint with a consistent
authorization call.

**Acceptance Criteria (EARS)**

- WHEN `AuthorizationService::check_permission(user_id, permission_code)` is called, THE SYSTEM
  SHALL return `true` if the user holds the permission and `false` otherwise.
- WHEN the user holds the permission through a directly assigned role (`user_roles`), THE SYSTEM
  SHALL return `true`.
- WHEN the user holds the permission through a role inherited from a group (`user_groups` +
  `group_roles`), THE SYSTEM SHALL return `true`.
- WHEN the user holds the permission through both direct and group-inherited roles, THE SYSTEM
  SHALL compute the union (deduplicated set) and return `true` if the permission exists in
  either source.
- WHEN the user does not hold the permission through any role, THE SYSTEM SHALL return `false`.
- WHEN the permission code does not exist in the `permissions` table, THE SYSTEM SHALL return
  `false` (unknown permission codes are treated as not held).

### US-2: Permission resolution is computed fresh on every request

As a **security architect**, I want permission checks to be performed against live data on every
request, so that role changes (adding/removing permissions, changing group membership) take
effect on the next request without stale cached state.

**Acceptance Criteria (EARS)**

- WHEN a user's role assignments change mid-session (via IAM Users or IAM Groups), THE SYSTEM
  SHALL apply the new permissions on their next request.
- WHEN a role's permission assignments change mid-session (via IAM Roles), THE SYSTEM SHALL
  apply the new permissions on the user's next request.
- THE SYSTEM SHALL NOT cache the computed permission set in the session or in application memory
  beyond the current request.
- THE SYSTEM SHALL NOT store the permission set in Redis (the session store).

### US-3: Authorization check failure produces a consistent 403 response

As a **security architect**, I want all permission check failures to produce identical `403
Forbidden` responses, so that attackers cannot enumerate permissions or distinguish between
different authorization failure modes.

**Acceptance Criteria (EARS)**

- WHEN `check_permission` returns `false`, THE SYSTEM SHALL return `403 Forbidden` with the
  generic message `"Insufficient permissions"` and error code `FORBIDDEN`.
- WHEN the user is not authenticated, THE SYSTEM SHALL return `401 Unauthorized` (handled by
  the auth middleware, outside this feature).
- THE SYSTEM SHALL NOT include the required permission code, the user's roles, or any
  diagnostic information in the `403` response body.
- THE SYSTEM SHALL log the denial at INFO level including user ID and permission code (for
  audit/debugging), but SHALL NOT return this information to the client.

## Out of Scope

- Project membership scope check (covered by `auth-project-scope`).
- Object sharing scope check (covered by `auth-sharing-scope`).
- System Admin bypass logic (covered by `auth-admin-bypass`).
- Role assignment to users or groups (covered by `iam-users` and `iam-groups`).
- Permission assignment to roles (covered by `iam-roles`).
- Creating or modifying permission codes (covered by `iam-permissions`).
- Rate limiting on authorization checks (handled by middleware layer).
- Caching of computed permissions (intentionally out of scope for Phase 1).

## Dependencies

- `iam-auth` — Session-based authentication provides the `user_id` in the request context.
- `iam-roles` — Provides `user_roles`, `group_roles`, `role_permissions` tables and their junction data.
- `iam-permissions` — Provides the `permissions` table with seeded permission codes.
- `iam-groups` — Provides `user_groups` junction table for group membership.
- `iam-users` — Provides `users` table for user existence validation.
- `auth-admin-bypass` — System Admin bypass is checked before RBAC resolution.
