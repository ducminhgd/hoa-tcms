# Feature: Audit Columns

## Overview

Define and enforce the project-wide audit column convention for all mutable tables in
HOA TCMS. Every mutable entity table carries four audit columns: `created_at`, `created_by`,
`updated_at`, and `updated_by`. These columns provide a complete audit trail of who created
and who last modified every row, without needing a separate audit log table.

`created_at` and `created_by` are set once on INSERT and never modified thereafter.
`updated_at` and `updated_by` are set on INSERT (to match the created values) and
updated on every subsequent UPDATE.

This feature defines the **convention and application-layer enforcement**; it works in
tandem with the `audit-triggers` spec, which provides database-level trigger enforcement.

---

## User Stories

### US-01: Define the Audit Column Convention

As a developer, I want a single, unambiguous project-wide convention for audit columns
on every mutable table, so that every entity table has a consistent audit trail.

**Acceptance Criteria (EARS)**

- EVERY mutable entity table in the database SHALL include the following four audit
  columns:

  | Column | Type | Constraints | Notes |
  |--------|------|-------------|-------|
  | `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after INSERT |
  | `created_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Immutable after INSERT |
  | `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Set to `created_at` on INSERT; updated on every UPDATE |
  | `updated_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Set to `created_by` on INSERT; updated on every UPDATE |

- WHEN a row is inserted, `updated_at` SHALL equal `created_at` and `updated_by` SHALL
  equal `created_by`.
- WHEN a row is updated, `updated_at` and `updated_by` SHALL be set to the current
  timestamp and the authenticated user's ID. `created_at` and `created_by` SHALL NOT
  change.
- The following table categories are exempt from the full audit column set:

  | Category | Columns included | Examples |
  |----------|-----------------|----------|
  | Read-only reference tables | `created_at` only | `PERMISSIONS` |
  | Junction / association tables | `created_at` only | `PROJECT_MEMBERS`, `USER_GROUPS` |
  | System tables (not application-managed) | None | Migration tracking tables |

- `users.created_by` is nullable (the CLI bootstrap admin has no creator).
- All `_by` columns are `BIGINT NOT NULL` FK referencing `users(id)`, except
  `deleted_by` (nullable) and `users.created_by` (nullable).
- FK constraints on `_by` columns use `ON DELETE RESTRICT` to prevent deleting a user
  who has created or modified records.

### US-02: Enforce Immutability of `created_at`/`created_by` at Application Layer

As a system, I want the application layer to reject any attempt to modify `created_at`
or `created_by` after INSERT, so that the creation audit trail is tamper-proof even if
the database triggers are bypassed.

**Acceptance Criteria (EARS)**

- WHEN the application constructs an UPDATE statement for any entity, THE SYSTEM SHALL
  NOT include `created_at` or `created_by` in the SET clause.
- IF a repository implementation inadvertently includes `created_at` or `created_by`
  in an UPDATE SET clause, THE SYSTEM SHALL reject the operation at the repository
  layer and return an `INTERNAL_ERROR`.
- WHEN the application constructs an INSERT statement, `created_at`, `created_by`,
  `updated_at`, and `updated_by` SHALL all be set to the current timestamp and the
  authenticated user's ID.
- IF an INSERT is attempted with `created_by` set to NULL (for non-bootstrap records),
  THE SYSTEM SHALL reject the operation (NOT NULL constraint at database level;
  validation at application level for a clear error message).
- The repository layer testing SHALL verify that UPDATE operations do not modify
  `created_at` or `created_by` by inserting a row, updating it, and asserting the
  created columns are unchanged.

---

## Out of Scope

- **Trigger-level enforcement of `updated_at`/`updated_by` auto-maintenance** -- covered
  in the `audit-triggers` spec.
- **Trigger-level enforcement of `created_at`/`created_by` immutability** -- covered in
  the `audit-triggers` spec.
- **Full audit log table** (separate table recording every change with old/new values) --
  not in Phase 1. The audit columns provide who-created/who-last-modified; a full change
  log is deferred to a future phase.
- **`updated_by` context for soft-delete** -- covered in the `soft-delete` spec (which
  sets `updated_at`/`updated_by` alongside `deleted_at`/`deleted_by`).
- **Implementation of audit columns on individual entity tables** -- each entity spec
  is responsible for adding the four columns to its own migrations. This spec defines
  the convention.

---

## Dependencies

- **IAM Users** -- `users.id` FK reference for `created_by` and `updated_by`.
- **Soft Delete** (`soft-delete` spec) -- `deleted_at`/`deleted_by` are additional
  columns on the same entity tables and share the same FK pattern.
- **Audit Triggers** (`audit-triggers` spec) -- DB triggers provide the database-level
  enforcement of `updated_at`/`updated_by` auto-maintenance and `created_at`/`created_by`
  immutability. Application-layer enforcement (this spec) and trigger enforcement are
  complementary, defence-in-depth measures.
