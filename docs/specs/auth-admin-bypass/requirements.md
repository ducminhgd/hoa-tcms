# Feature: Auth Admin Bypass

## Overview

The Admin Bypass feature is the simplest and most foundational authorization rule: if the
authenticated user is a System Admin, they bypass **all** subsequent permission and scope
checks. This is the first check in every `AuthorizationService` method and is implemented as
a single predicate: "is this user a System Admin?"

A System Admin implicitly holds every system permission, is a member of every project with
the Owner role, and has full access to every shared or shareable object. This eliminates the
need to assign individual permissions or project memberships to System Admin accounts.

## User Stories

### US-1: System Admin bypasses all authorization gates

As a **System Admin**, I want to access any resource and perform any operation in the system
without needing individual permission assignments or project memberships, so that I can
administer the system without being blocked by authorization checks.

**Acceptance Criteria (EARS)**

- WHEN a System Admin calls any API endpoint, THE SYSTEM SHALL bypass the system permission
  check and allow the request.
- WHEN a System Admin calls any project-scoped endpoint, THE SYSTEM SHALL bypass the project
  membership check and allow the request regardless of membership.
- WHEN a System Admin accesses any shared or shareable object, THE SYSTEM SHALL bypass the
  sharing scope check and allow the request.
- WHEN a user who is NOT a System Admin makes a request, THE SYSTEM SHALL proceed to the
  normal authorization checks (RBAC, project scope, sharing scope).
- WHEN a user's System Admin role is revoked mid-session, THE SYSTEM SHALL deny access
  starting from the next request (no stale bypass).
- WHEN a user is granted the System Admin role mid-session, THE SYSTEM SHALL grant full
  access starting from the next request.

## Out of Scope

- System Admin role assignment to users (covered by `iam-users` role assignment or
  `iam-cli-init`).
- The System Admin role definition itself (covered by `iam-roles` -- the "System Admin"
  role is seeded by migration with all permission IDs in `role_permissions`).
- Any UI for System Admin bypass -- the bypass is transparent. System Admins see no
  difference in API responses; they simply never receive `403 Forbidden` for
  authorization reasons.
- Auditing of admin bypass events (deferred to audit logging feature in Phase 2).

## Dependencies

- `iam-roles` -- The "System Admin" role exists in the `roles` table and has rows in
  `role_permissions` linking to all permissions.
- `iam-auth` -- Session-based authentication provides the `user_id` needed to check roles.
- `auth-rbac` -- This feature is called by `AuthorizationService::check_permission` as the
  first step before resolving permissions.
- `auth-project-scope` -- Called by `check_project_membership` as the first step.
- `auth-sharing-scope` -- Called by `check_sharing_access` as the first step.
