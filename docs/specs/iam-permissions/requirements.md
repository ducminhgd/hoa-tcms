# Feature: IAM Permissions

## Overview

The permission catalog is a read-only reference table that enumerates every object-level
permission code in the system. It serves as the single source of truth for the RBAC engine:
Roles aggregate permissions via `role_permissions` join table, and the authorization middleware
checks whether a user holds the required permission code for each request. Because the catalog
is seeded at database migration time, it is never created, updated, or deleted through the
application UI/API; new permissions are added only via future migrations.

**Important:** The database migration is the **single source of truth** for the permission
catalog. CLI init does NOT re-seed permissions; it only creates the System Admin role
assignment and the first admin user.

## User Stories

### US-1: Browse the permission catalog

As a **System Admin**, I want to view the complete list of permissions, so that I understand
what granular actions are available when configuring roles.

**Acceptance Criteria (EARS)**

- WHEN a user with `permission:read_list` sends `GET /api/v1/permissions`, THE SYSTEM SHALL
  return a paginated list of all seeded permissions ordered by `code`.
- WHEN a user without `permission:read_list` sends `GET /api/v1/permissions`, THE SYSTEM SHALL
  return `403 Forbidden`.
- WHEN an unauthenticated client sends `GET /api/v1/permissions`, THE SYSTEM SHALL return
  `401 Unauthorized`.
- WHEN a request includes pagination parameters (`page`, `limit`), THE SYSTEM SHALL return the
  corresponding slice of the permission catalog.
- WHILE the permissions table is seeded (non-empty), THE SYSTEM SHALL always return at least
  the 51 permission rows defined in the seed migration.
- IF the permission list is empty (database not yet migrated), THE SYSTEM SHALL return
  `200 OK` with an empty `data` array and `meta.total = 0`.

### US-2: Permission codes are unique and immutable

As a **developer**, I want permission codes to be unique and immutable once seeded, so that
permission checks in the authorization layer are deterministic and no two permissions collide.

**Acceptance Criteria (EARS)**

- WHEN a seed migration inserts a permission row with a `code` that already exists in the
  table, THE SYSTEM SHALL fail the migration with a unique-constraint violation.
- WHEN a seed migration inserts a permission row, THE SYSTEM SHALL set `code` to a non-empty
  string matching the pattern `{resource}:{action}` (e.g. `test_case:create`).
- WHEN a seed migration inserts a permission row, THE SYSTEM SHALL set `name` to a
  human-readable string (e.g. `"Create Test Case"`).

### US-3: Permissions are read-only via the application

As a **System Admin**, I want to be certain that the permission catalog cannot be altered
through the application, so that the set of valid permission codes is tamper-proof.

**Acceptance Criteria (EARS)**

- WHEN an authenticated user sends `POST`, `PUT`, `PATCH`, or `DELETE` to
  `/api/v1/permissions` or any `/api/v1/permissions/{id}`, THE SYSTEM SHALL return
  `405 Method Not Allowed` for every method except `GET`.
- WHEN a job or script attempts to directly modify the `permissions` table outside of a
  migration, THE SYSTEM SHALL rely on database-level constraints (`UNIQUE` on `code`) and
  the absence of application write endpoints to prevent writes.
- THE SYSTEM SHALL expose **no** create, update, or delete endpoints for permissions in the
  REST API.

### US-4: Permission catalog is always available for authorization checks

As an **authorization middleware**, I want all permission codes to be accessible via a
consistent query interface, so that RBAC checks can resolve permission IDs from codes.

**Acceptance Criteria (EARS)**

- WHEN the system starts, THE SYSTEM SHALL have the `permissions` table fully seeded with all
  51 permission codes as defined in the seed migration.
- WHEN a role is assigned a permission, THE SYSTEM SHALL reference the permission by its
  numeric `id` (foreign key in `role_permissions`), not by its `code` string.
- WHEN the authorization middleware needs to resolve a permission code to an ID, THE SYSTEM
  SHALL provide a repository method `find_by_code(code: &str) -> Option<Permission>`.

## Out of Scope

- Creating, editing, or deleting permissions via the application UI or API.
- Role-to-permission assignment UI or logic (covered by `iam-roles` feature).
- Permission checks / authorization middleware (covered by `auth-rbac` feature).
- CLI commands to manage permissions.
- Soft-delete or audit columns beyond `created_at` on the `permissions` table.
- Any field other than `id`, `name`, `code`, and `created_at` on the `permissions` table.
- Hiding or filtering the permission list by resource (all permissions are always shown).

## Dependencies

- `database` — PostgreSQL schema must exist and migrations must be runnable.
- `iam-roles` feature — provides `role_permissions` junction table that references
  `permissions.id` as a foreign key.
- `auth-rbac` feature — consumes permission codes from this catalog for authorization checks.
- `meta-pagination` — shared pagination utilities for list endpoints.
