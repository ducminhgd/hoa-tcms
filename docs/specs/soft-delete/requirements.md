# Feature: Soft Delete

## Overview

Define and enforce the project-wide soft-delete convention for all entity tables in HOA TCMS.
Every entity table carries `deleted_at TIMESTAMPTZ` (nullable) and `deleted_by BIGINT FK`
(nullable). Application DELETE endpoints always perform a soft-delete -- they set
`deleted_at = NOW()` and `deleted_by = <current_user_id>`, never a hard DELETE. All queries
filter `WHERE deleted_at IS NULL` to hide deleted records. Soft-deleting a parent does not
cascade to children; children are hidden implicitly because the parent is filtered from
queries.

This feature defines the **convention and enforcement**; it does not implement soft-delete
on individual entity tables (that belongs to each entity's own spec).

---

## User Stories

### US-01: Define the Soft-Delete Convention

As a developer, I want a single, unambiguous project-wide convention for soft-delete columns
and behaviour, so that every entity table implements soft-delete consistently.

**Acceptance Criteria (EARS)**

- EVERY entity table in the database SHALL include the columns `deleted_at TIMESTAMPTZ`
  (nullable, no default) and `deleted_by BIGINT` (nullable, `REFERENCES users(id)` with
  `ON DELETE RESTRICT`).
- WHEN an entity is actively usable, `deleted_at` and `deleted_by` SHALL be `NULL`.
- WHEN an entity is soft-deleted by the application, `deleted_at` SHALL be set to the
  current timestamp (`NOW()`) and `deleted_by` SHALL be set to the authenticated user's ID.
- Junction tables and `OBJECT_SHARING` SHALL NOT carry `deleted_at`/`deleted_by` columns;
  they use hard DELETE instead (per FR-52).
- Read-only reference tables (e.g. `PERMISSIONS`) SHALL NOT carry `deleted_at`/`deleted_by`
  columns.
- THE SYSTEM SHALL NOT perform hard DELETE on any entity table. No `DELETE FROM` statements
  on entity tables are permitted outside of migration rollbacks.
- A partial index `idx_{table}_active ON {table}(id) WHERE deleted_at IS NULL` SHALL be
  created on every entity table to optimise active-record queries.

### US-02: Soft-Delete on DELETE Endpoints

As a user with delete permission on an entity, I want the DELETE endpoint to soft-delete
the record rather than destroy it, so that data is preserved for audit and potential restore.

**Acceptance Criteria (EARS)**

- WHEN a DELETE endpoint is called on a non-deleted entity, THE SYSTEM SHALL execute
  `UPDATE {table} SET deleted_at = NOW(), deleted_by = <current_user_id> WHERE id = $1`
  and return the appropriate success response (typically `204 No Content`).
- IF the entity is already soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL
  return `404 Not Found`.
- THE SYSTEM SHALL NOT cascade soft-delete to child entities. Only the target row's
  `deleted_at`/`deleted_by` columns are updated.
- The `deleted_by` column SHALL always be populated when the application performs a
  soft-delete. NULL `deleted_by` is only acceptable from direct database operations
  outside the application.

### US-03: Filter Soft-Deleted Records from All Queries

As a user of the system, I want soft-deleted records to be invisible in all list views,
detail views, and selection fields, so that deleted data does not clutter the UI or
affect business logic.

**Acceptance Criteria (EARS)**

- WHEN any SELECT query retrieves entity rows for application use, THE SYSTEM SHALL
  include `WHERE deleted_at IS NULL` (or equivalent JOIN condition) in the query.
- WHEN a detail endpoint requests a specific entity by ID, THE SYSTEM SHALL return
  `404 Not Found` if the entity is soft-deleted, with the same error message as a
  non-existent entity.
- WHEN an UPDATE endpoint is called on a soft-deleted entity, THE SYSTEM SHALL return
  `404 Not Found` (per FR-54c, UPDATE on soft-deleted rows is rejected except for the
  soft-delete and restore operations themselves).
- WHEN a parent entity is soft-deleted, its child entities SHALL be hidden from views
  because queries join through the parent and the parent is filtered out. No cascade
  update on child rows is required.
- ALL child entity detail/lookup endpoints MUST either JOIN the parent table and include
  `WHERE parent.deleted_at IS NULL`, or perform a separate parent-existence-and-active
  check before returning the child. A child whose parent is soft-deleted SHALL return
  `404 Not Found` identically to a directly-deleted child.
- Export, reporting, and other read paths SHALL also exclude soft-deleted records.
- A restore operation that clears `deleted_at` and `deleted_by` to NULL SHALL be deferred
  to a future phase.

---

## Out of Scope

- **Restore endpoint** (clearing `deleted_at`/`deleted_by` to NULL -- deferred to a future
  phase).
- **Hard delete** of any entity (not implemented in Phase 1).
- **Cascading soft-delete** to child records (explicitly disallowed per FR-53).
- **Soft-delete on junction tables** (e.g. `PROJECT_MEMBERS`, `USER_GROUPS`,
  `ROLE_PERMISSIONS`) -- these use hard DELETE.
- **Soft-delete on `OBJECT_SHARING`** -- uses hard DELETE.
- **Purging** of soft-deleted records (deferred to a future data retention phase).
- **Implementation of soft-delete on individual entity tables** -- each entity spec
  (e.g. `project-crud`, `test-case-crud`) is responsible for implementing the convention
  on its own tables. This spec defines the convention and provides shared infrastructure.

---

## Dependencies

- **IAM Users** -- `users.id` FK reference for `deleted_by`.
- **Audit Columns** (`audit-columns` spec) -- `deleted_at`/`deleted_by` are part of the
  full audit column set on every entity table (alongside `created_at`/`created_by`,
  `updated_at`/`updated_by`).
- **Audit Triggers** (`audit-triggers` spec) -- DB triggers auto-maintain `updated_at`
  and enforce `created_at`/`created_by` immutability. Soft-delete operations must set
  `updated_at`/`updated_by` in addition to `deleted_at`/`deleted_by` so the trigger
  has context for the change.
