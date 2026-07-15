# Feature: Auth Sharing Scope

## Overview

The Sharing Scope feature implements the third and innermost layer of the three-tier
authorization model: object-level sharing access. When an object (test case, test plan,
test run, etc.) is explicitly shared with a user, the sharing role assigned to that user
takes priority over their project membership role.

This feature extends `AuthorizationService` with `check_sharing_access`, which determines
whether a user can perform an operation on a specific object based on: (1) whether the
object is shared with the user, and (2) the share-specific role, which overrides the
project role. If the object is not shared, the check falls through to the project
membership role (delegated to `auth-project-scope`).

## User Stories

### US-1: Check sharing access on a specific object

As a **feature developer**, I want a method to check whether a user has access to a
specific object through sharing, so that shared objects grant the correct access level
that may differ from the user's project role.

**Acceptance Criteria (EARS)**

- WHEN `AuthorizationService::check_sharing_access(user_id, object_type, object_id,
  required_roles)` is called, THE SYSTEM SHALL return `true` if the object is shared with
  the user AND the share role is in `required_roles`.
- WHEN the object is shared with the user and the share role satisfies `required_roles`,
  THE SYSTEM SHALL return `true` even if the user's project role does not satisfy
  `required_roles` (sharing role overrides project role).
- WHEN the object is NOT shared with the user, THE SYSTEM SHALL fall through to check the
  user's project membership role (delegate to `check_project_membership`).
- WHEN the object is shared with the user but the share role does NOT satisfy
  `required_roles`, THE SYSTEM SHALL return `false` (the sharing role is the effective
  role -- it overrides the project role entirely, even if the project role would have
  passed).
- WHEN the user is a System Admin, THE SYSTEM SHALL return `true` without querying sharing
  or project membership (bypass via `auth-admin-bypass`).
- WHEN the object does not exist or is soft-deleted, THE SYSTEM SHALL return `false`.

### US-2: Sharing access is computed fresh on every request

As a **security architect**, I want sharing checks to be performed against live data on
every request, so that sharing changes (granting, revoking, changing share role) take
effect immediately.

**Acceptance Criteria (EARS)**

- WHEN sharing is granted or revoked mid-session, THE SYSTEM SHALL apply the change on
  the user's next request.
- WHEN a share role is changed mid-session, THE SYSTEM SHALL apply the new role on the
  user's next request.
- THE SYSTEM SHALL NOT cache sharing access in the session or in application memory beyond
  the current request.
- WHEN `check_sharing_access` returns `false`, THE SYSTEM SHALL return `403 Forbidden`
  with the same generic message used by all other authorization layers.

## Out of Scope

- System permission checking (covered by `auth-rbac`).
- Project membership scope check as a standalone operation (covered by
  `auth-project-scope`).
- System Admin bypass logic (covered by `auth-admin-bypass`).
- The sharing UI for granting/revoking shares (covered by `sharing-ui`).
- The sharing override data model and persistence (covered by `sharing-override`).
- Sharing role definitions (covered by `sharing-roles`).
- Orphaned objects (no project) -- sharing is the only access path for orphaned objects.
  This scenario is handled by the same `check_sharing_access` method: there is no project
  membership to fall through to, so only sharing access matters.

## Dependencies

- `iam-auth` -- Session-based authentication provides the `user_id`.
- `auth-rbac` -- System permission check runs before sharing scope check.
- `auth-project-scope` -- Fallback when object is not shared with the user.
- `auth-admin-bypass` -- System Admin bypass checked first.
- `sharing-override` -- Provides the sharing records data model and repository.
- `sharing-roles` -- Defines sharing role enum (Editor, Contributor, Viewer).
