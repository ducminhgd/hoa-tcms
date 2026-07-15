# Feature: Sharing Roles

## Overview

Sharing Roles define the permission levels a user or group receives on a shared object.
These roles are distinct from project membership roles (Owner, Editor, Contributor,
Viewer) and apply per-object, overriding the project membership role for that specific
object.

Three sharing roles exist:

- **Editor** -- Full read/write access plus the ability to share the object with
  others.
- **Contributor** -- Read and update access but cannot share the object with others.
- **Viewer** -- Read-only access; cannot modify or share the object.

Sharing roles are enforced at the authorization layer and stored per sharing entry in
the database.

---

## User Stories

### US-1: Define Sharing Role Enumeration

As a system architect, I want a well-defined sharing role enumeration (Editor,
Contributor, Viewer) that is distinct from project membership roles, so that sharing
authorization logic has a clear, type-safe reference for all permission checks.

**Acceptance Criteria (EARS)**

- THE SYSTEM SHALL define three sharing role levels: `EDITOR`, `CONTRIBUTOR`, and
  `VIEWER`.
- THE SYSTEM SHALL NOT reuse project membership role values (Owner, Editor,
  Contributor, Viewer) for sharing roles. Sharing roles are a separate enumeration
  with their own storage and semantics.
- THE SYSTEM SHALL store sharing roles as a lookup table (`SHARE_ROLES`) or an inline
  enum/string in the `SHARING_ENTRIES` table. The role value must be validated at
  insert/update time.
- THE SYSTEM SHALL enforce that every sharing entry has exactly one valid sharing
  role.
- THE SYSTEM SHALL reject any attempt to create or update a sharing entry with an
  unrecognised role value, returning `422 Unprocessable Entity`.

### US-2: Apply Sharing Roles to Shared Objects

As a user who shares an object with another user or group, I want to assign a sharing
role (Editor, Contributor, or Viewer) to each recipient, so that I can grant
fine-grained access on a per-object and per-recipient basis.

**Acceptance Criteria (EARS)**

- WHEN a sharing entry is created for an object (test case, test plan, test run, etc.)
  and a target user or group, THE SYSTEM SHALL require a sharing role (`editor`,
  `contributor`, or `viewer`) in the request.
- IF the sharing role is `editor`, THE SYSTEM SHALL grant the recipient full read,
  write, and sharing capabilities on that object (equivalent to object-level Editor).
- IF the sharing role is `contributor`, THE SYSTEM SHALL grant the recipient read and
  update capabilities on that object, but deny the ability to modify sharing entries
  for that object.
- IF the sharing role is `viewer`, THE SYSTEM SHALL grant the recipient read-only
  access on that object; any mutation or sharing operation is denied.
- THE SYSTEM SHALL store the assigned sharing role with the sharing entry and use it
  at authorization time to determine the effective permissions for that user on that
  object.

---

## Security Considerations

### Role Escalation Prevention

A user cannot assign a sharing role higher than their own effective role on the
object. For example, a Contributor on a shared object cannot grant Editor access to
another user. The maximum sharing role a user can assign is bounded by their own
effective role (project role or sharing role, whichever is higher per the override
rules in `sharing-override`).

### Role Confusion Prevention

Sharing roles (`editor`, `contributor`, `viewer`) are intentionally named the same as
the lower three project membership roles but are stored and evaluated separately. The
authorization layer (see `sharing-override`) handles the precedence logic. This naming
consistency is a deliberate UX choice: the semantics are the same (Editor = full
access, Contributor = edit, Viewer = read), but the scope differs (per-object vs.
project-wide).

### Audit Trail

Every sharing entry creation, update, and deletion must record `created_by` and
`updated_by` for audit purposes. Role changes on a sharing entry are logged so that an
audit trail exists for who granted what access to whom and when.

---

## Out of Scope

- **Applying sharing roles at the project level** -- Sharing roles are per-object
  only. Project-wide access is governed by project membership roles.
- **Custom sharing roles** -- Only the three predefined roles exist in Phase 1.
  Extending the role set is deferred.
- **Sharing role inheritance through groups** -- Group sharing assigns the role to all
  group members uniformly. Nested group hierarchies and role conflict resolution in
  nested groups are deferred.
- **Sharing role expiration / TTL** -- Roles are permanent until explicitly revoked.
  Time-bound sharing entries are deferred.

---

## Dependencies

- **IAM Auth** -- Authentication is required to create or modify sharing entries.
- **IAM Users & IAM Groups** -- Sharing entries reference user IDs or group IDs as
  targets.
- **Project Members** -- Project membership roles are the baseline for authorization;
  sharing roles override them per-object.
- **sharing-override** -- The sharing role is consumed by the authorization override
  logic to compute effective permissions.
- **sharing-ui** -- The sharing role is selected in the sharing UI when adding or
  updating a sharing entry.
- **Auth Sharing Scope** -- The authorization layer reads sharing roles to determine
  access.
