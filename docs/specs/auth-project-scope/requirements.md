# Feature: Auth Project Scope

## Overview

The Project Scope feature implements the second layer of the three-tier authorization model:
project membership verification. It checks whether a user is a member of a given project with
a role that satisfies the required role set. Every project-scoped endpoint (test cases, test
plans, test runs, categories, etc.) calls `AuthorizationService::check_project_membership` as
its second authorization gate, after the system permission check passes.

This feature uses the existing `project_members` table (defined in `project-members`) and
introduces no new database tables. It extends `AuthorizationService` with project-scoped
authorization methods consumed by all project-scoped features.

## User Stories

### US-1: Verify project membership with role requirement

As a **feature developer**, I want a single method to check whether the current user is a
member of a project with at least one of the required roles, so that I can restrict
project-scoped operations to authorized project members.

**Acceptance Criteria (EARS)**

- WHEN `AuthorizationService::check_project_membership(user_id, project_id, required_roles)`
  is called, THE SYSTEM SHALL return `true` if the user is a member of the project and their
  role is in the `required_roles` set.
- WHEN the user is a member of the project but their role is NOT in the `required_roles` set,
  THE SYSTEM SHALL return `false`.
- WHEN the user is not a member of the project at all, THE SYSTEM SHALL return `false`.
- WHEN `required_roles` is empty, THE SYSTEM SHALL require only that the user is a member
  of the project (any role).
- WHEN the user is a System Admin, THE SYSTEM SHALL return `true` without querying project
  membership (bypass via `auth-admin-bypass`).
- WHEN the project does not exist or is soft-deleted, THE SYSTEM SHALL return `false`.

### US-2: Project membership is computed fresh on every request

As a **security architect**, I want project membership checks to be performed against live
data on every request, so that membership changes (adding/removing members, changing roles)
take effect on the next request.

**Acceptance Criteria (EARS)**

- WHEN a user's project membership is added or removed mid-session, THE SYSTEM SHALL apply
  the change on their next request to a project-scoped endpoint.
- WHEN a user's project role is changed mid-session (e.g., from Editor to Viewer), THE
  SYSTEM SHALL apply the new role on their next request.
- THE SYSTEM SHALL NOT cache project membership in the session or in application memory
  beyond the current request.
- WHEN `check_project_membership` returns `false`, THE SYSTEM SHALL return `403 Forbidden`
  with the same generic message used by `auth-rbac` -- indistinguishable from a system
  permission denial.

## Out of Scope

- System permission checking (covered by `auth-rbac`).
- Object sharing scope check (covered by `auth-sharing-scope`).
- System Admin bypass logic (covered by `auth-admin-bypass`).
- Project membership management -- add/remove/change role (covered by `project-members`).
- Selecting a single required role per endpoint -- the `required_roles` parameter accepts
  an array because different operations on the same resource may require different roles
  (e.g., GET requires any member, POST requires Owner or Editor).

## Dependencies

- `iam-auth` -- Session-based authentication provides the `user_id`.
- `auth-rbac` -- System permission check runs before project scope check.
- `auth-admin-bypass` -- System Admin bypass checked first.
- `project-members` -- Provides the `project_members` table and `ProjectMemberRepository`.
- `project-crud` -- Provides project existence validation.
