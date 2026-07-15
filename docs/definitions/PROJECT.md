# Project

A project is the top-level workspace container in HOA TCMS. It scopes test cases, test plans, test runs,
test executions, and per-project metadata (categories, priorities, templates, plan types). Access to
project-scoped objects is governed by project membership.

## Fields

1. Name: name of the project. Text field. Required. Case-insensitive unique across all projects (duplicate
   returns 409 Conflict). Max 255 characters.
2. Description: additional information about the project. Text area. Optional.
3. Status: ACTIVE or INACTIVE. Selection field. Required, default ACTIVE.
   - ACTIVE: project and its objects are visible to members.
   - INACTIVE: project and all child objects are hidden from list views, detail views, and selection fields.
     Underlying data and existing FK references remain intact.

## Creation Behavior

When a project is created:

1. Only users with the `project:create` system permission may create projects (PRD FR-15).
2. Per-project metadata is auto-seeded from the YAML configuration file within the same database transaction:
   - Test Categories (from config)
   - Test Case Templates (from config)
   - Priority levels (HIGHEST, HIGH, MEDIUM, LOW, LOWEST — from config)
   - Test Plan types (ACCEPTANCE, FUNCTIONAL, API, INTEGRATION, PERFORMANCE, REGRESSION, SECURITY — from config)
3. The creating user is automatically assigned as a project member with the Owner role.
4. If the YAML configuration file is unreadable, malformed, or missing required sections, the transaction
   is rolled back and creation fails.

## Project Members

A project may contain multiple members. Each member has a role that determines their access level within
the project scope.

### Roles

| Role | Permissions |
|------|-------------|
| **Owner** | Full control. Can manage members (add, remove, change roles), modify project fields, modify all project objects, share objects, and delete the project. |
| **Editor** | Can modify project fields and all project objects. Can share objects. Cannot manage members. |
| **Contributor** | Can modify project objects (create, update test cases, runs, executions). Cannot share objects. Cannot modify project fields. Cannot manage members. |
| **Viewer** | Read-only access to project and all its objects. Cannot modify, share, or manage members. |

### Rules

1. A project must always have at least one Owner. Removing or downgrading the last Owner is rejected
   (409 Conflict).
2. Deactivated users (`status = INACTIVE` or `deleted_at IS NOT NULL`) cannot be added as project members.
3. Project membership records use hard delete — when a member is removed, the junction row is deleted.
4. The creating user is seeded as Owner on project creation.

## Deletion

- Soft-delete only. No hard delete.
- On soft-delete: `deleted_at` is set to the current timestamp, `deleted_by` is set to the user who
  performed the deletion.
- No cascade delete. Child objects (test cases, test plans, test runs, test executions, metadata) retain
  their original `deleted_at = NULL`. They are hidden from views because the parent project is filtered out
  (implicit hiding, per PRD FR-53).
- Project membership records are preserved for potential future restore.
- Only project Owners (with `project:delete` system permission) or System Admins may delete a project.
- UPDATE operations on soft-deleted projects are rejected (per PRD FR-54c).

## Authorization

Access to a project and its objects follows the three-layer authorization model:

| Layer | Check |
|-------|-------|
| **System RBAC** | User must hold the required system permission (e.g., `project:read`, `project:update`). |
| **Project Membership** | User must be a member of the project (or the object must be shared with them). |
| **Sharing Override** | If the object is shared with the user, the sharing role overrides the project membership role for that object. |

System Admin bypasses all checks (full access to all projects and objects).

## Audit Columns

All project rows carry audit column pairs (per PRD FR-54a):

| Column | Type | Description |
|--------|------|-------------|
| `created_at` | `TIMESTAMPTZ NOT NULL` | Set on insert. Immutable after insert. |
| `created_by` | `BIGINT NOT NULL` FK → `users.id` | User who created the project. Immutable after insert. |
| `updated_at` | `TIMESTAMPTZ NOT NULL` | Set to `created_at` on insert. Auto-updated by DB trigger on subsequent updates. |
| `updated_by` | `BIGINT NOT NULL` FK → `users.id` | Set to `created_by` on insert. Auto-updated by DB trigger on subsequent updates. |
| `deleted_at` | `TIMESTAMPTZ` (nullable) | NULL for active projects. Set on soft-delete. |
| `deleted_by` | `BIGINT` FK → `users.id` (nullable) | NULL for active projects. Set to user who performed soft-delete. |
