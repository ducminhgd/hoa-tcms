# Feature: Sharing Override

## Overview

The sharing override feature ensures that when an object is shared with a user, the
sharing role assigned on that object takes priority over the user's project membership
role for that object. This enables scenarios where a Viewer at the project level can
be granted Editor access on a specific test case, or a Contributor can be restricted
to Viewer access on a specific test plan.

The override logic is implemented in the authorization layer and is checked on every
request to a shared object.

---

## User Stories

### US-1: Sharing Role Overrides Project Role on Shared Objects

As a project member who has been granted a sharing role on a specific object, I want
that sharing role to take priority over my project membership role when accessing that
object, so that I can have elevated or restricted access on individual objects
regardless of my project-wide role.

**Acceptance Criteria (EARS)**

- WHEN a user accesses a shared object (test case, test plan, test run, etc.) and a
  sharing entry exists for that user (or a group they belong to) on that object, THE
  SYSTEM SHALL use the sharing role as the effective role for authorization
  decisions on that object, ignoring the user's project membership role.
- IF the user has a sharing role of `editor` on the object, THE SYSTEM SHALL grant
  full read, write, and sharing capabilities on that object regardless of their
  project membership role.
- IF the user has a sharing role of `contributor` on the object, THE SYSTEM SHALL
  grant read and update capabilities on that object regardless of their project
  membership role. Sharing operations are denied unless the user's project membership
  role independently grants sharing.
- IF the user has a sharing role of `viewer` on the object, THE SYSTEM SHALL grant
  read-only access on that object regardless of their project membership role. All
  mutation and sharing operations are denied.
- IF the user has a sharing role on the object AND their project membership role is
  higher (e.g., sharing role is Viewer but project role is Editor), THE SYSTEM SHALL
  still use the sharing role (Viewer) -- the sharing role always takes priority. This
  allows intentional restriction of access on specific objects.
- IF no sharing entry exists for the user (or any of their groups) on the object, THE
  SYSTEM SHALL fall back to the project membership role for authorization.

### US-2: Authorization Middleware Enforces Sharing Override

As a system architect, I want the authorization middleware to check sharing entries
before falling back to project membership when authorizing access to any shareable
object, so that the override logic is consistently applied across all endpoints
without each handler implementing it independently.

**Acceptance Criteria (EARS)**

- WHEN any request targets a shareable object (identified by object_type and
  object_id in the request path or body), THE SYSTEM SHALL perform the following
  authorization checks in order:
  1. System permission check (e.g., `test_case:update`)
  2. Effective role check: query sharing entries for the authenticated user (and
     their groups) on the target object. If a sharing entry exists, use its role as
     the effective role. Otherwise, use the project membership role.
  3. Verify the effective role is sufficient for the requested operation.
- THE SYSTEM SHALL resolve group-based sharing entries by checking all groups the
  authenticated user belongs to. If multiple sharing entries match (one direct user
  entry and one or more group entries), the highest role among all matching entries
  takes effect.
- THE SYSTEM SHALL cache nothing: the sharing entry check and group membership check
  are performed against live data on every request. If a sharing entry is added,
  modified, or removed, the change takes effect on the next request.
- THE SYSTEM SHALL treat the effective role as the single source of truth for the
  remainder of the request. Downstream code (use cases, services) receives the
  effective role and does not need to re-check project membership separately for that
  object.
- IF the user is a System Admin, THE SYSTEM SHALL bypass the sharing override check
  entirely (System Admin retains full access to all objects regardless of sharing
  entries).
- IF a sharing entry exists but the user's project membership has been removed
  (user is no longer a project member), THE SYSTEM SHALL still honour the sharing
  entry -- the object is accessible via sharing alone. This supports orphaned test
  cases and cross-project collaboration scenarios.

---

## Security Considerations

### Role Restriction via Sharing

An important security property: sharing roles can **restrict** access below the
project membership level. A project Editor can have their access reduced to Viewer on
a specific object via a sharing entry. This is intentional and supports scenarios
where access to sensitive test cases must be limited even for project leadership.

### Group Sharing Precedence

When multiple group sharing entries apply to a user, the highest role wins. This is a
permissive resolution strategy: the user gets the maximum access any of their groups
grants. Direct user sharing entries are compared alongside group entries using the
same highest-role-wins rule.

### Audit Trail

Every authorization decision that activates a sharing override (i.e., the effective
role differs from the project membership role) should be logged at DEBUG level for
audit purposes. The log entry includes: user_id, object_type, object_id,
project_role, sharing_role, effective_role.

### No Information Leakage

When a user is denied access due to a sharing role being insufficient, the error
response must not reveal whether the denial is due to the sharing role or the project
role. The same generic `403 Forbidden` message is returned in both cases.

---

## Out of Scope

- **Cascading sharing** -- Sharing an object does not automatically share its child
  objects (e.g., sharing a test plan does not share its linked test runs).
- **Sharing role expiration** -- Roles are permanent until explicitly revoked.
- **Sharing role conflict resolution UI** -- When multiple roles apply, the system
  resolves automatically (highest wins). No UI for manual conflict resolution.
- **Sharing on non-shareable objects** -- Only explicitly shareable object types are
  supported. The list of shareable types is defined in `sharing-ui`.

---

## Dependencies

- **IAM Auth** -- Authentication is required; the session provides the user ID and
  group memberships.
- **IAM Permissions** -- System permission check is the first authorization gate.
- **Project Members** -- Project membership role is the fallback when no sharing
  entry exists.
- **sharing-roles** -- Sharing roles (Editor, Contributor, Viewer) are the values
  assigned and enforced.
- **sharing-ui** -- The sharing UI creates and manages sharing entries that this
  feature consumes at authorization time.
- **Auth Sharing Scope** -- This feature is the implementation of the
  `auth-sharing-scope` cross-cutting concern.
