# Feature: Audit Triggers

## Overview

Define PostgreSQL database triggers that auto-maintain `updated_at` and `updated_by` on
every UPDATE, and enforce the immutability of `created_at` and `created_by` at the database
level. These triggers are the database-layer complement to the application-layer enforcement
defined in the `audit-columns` spec, providing defence in depth: even if application code is
buggy, the database guarantees audit column integrity.

The triggers use `current_setting('app.current_user_id')` to read the authenticated user's
ID from the session context, which the application sets at the start of each database
transaction or session.

---

## User Stories

### US-01: Auto-Maintain `updated_at` and `updated_by` via DB Triggers

As a system, I want every UPDATE on any entity table to automatically set `updated_at`
to the current timestamp and `updated_by` to the current authenticated user, so that
the audit trail is maintained correctly even if application code forgets to set these
columns.

**Acceptance Criteria (EARS)**

- WHEN any row in an entity table is updated via an UPDATE statement, a `BEFORE UPDATE`
  trigger SHALL fire and set `NEW.updated_at = NOW()` and
  `NEW.updated_by = current_setting('app.current_user_id')::BIGINT`.
- WHEN the application performs a soft-delete (sets `deleted_at` and `deleted_by`),
  the trigger SHALL also fire and update `updated_at` and `updated_by` accordingly,
  because a soft-delete is an UPDATE operation.
- IF `current_setting('app.current_user_id')` is not set (e.g. a database migration
  or manual query without the application context), THE TRIGGER SHALL use a fallback
  value or skip updating `updated_by`:
  - The trigger should use `NULLIF(current_setting('app.current_user_id', true), '')::BIGINT`
    with the `true` parameter (missing_ok) to avoid errors when the setting is absent.
  - If the setting is absent, `updated_by` should remain unchanged (no overwrite with
    NULL).
- THE TRIGGER SHALL NOT modify `created_at` or `created_by` (those columns are
  untouched by the auto-maintenance trigger; their immutability is enforced by a
  separate trigger per US-02).
- The trigger function SHALL be implemented once as a shared function
  (`fn_audit_update()`) and reused by every entity table via per-table triggers.

### US-02: Enforce `created_at`/`created_by` Immutability in DB Triggers

As a system, I want the database to reject any UPDATE that attempts to change
`created_at` or `created_by`, so that the creation audit trail is tamper-proof at
the database level.

**Acceptance Criteria (EARS)**

- WHEN an UPDATE statement attempts to change `created_at` or `created_by` on any
  row, the `BEFORE UPDATE` trigger SHALL raise an exception with the message
  `'Cannot modify created_at or created_by'` and the UPDATE SHALL be aborted.
- IF neither `created_at` nor `created_by` are present in the UPDATE SET clause,
  the trigger SHALL not interfere with the operation (pass-through).
- The immutability enforcement SHALL be consolidated into a single shared trigger
  function (`fn_audit_immutability()`) and reused by every entity table via per-table
  triggers.
- The immutability trigger SHALL be distinct from the auto-maintenance trigger
  (`fn_audit_update()`) for clarity and separation of concerns, but both SHALL
  fire on `BEFORE UPDATE` on every entity table.
- The trigger SHALL NOT interfere with INSERT operations (it fires only on UPDATE).

---

## Out of Scope

- **Setting `app.current_user_id`** -- the application is responsible for calling
  `SELECT set_config('app.current_user_id', '<user_id>', true)` at the start of each
  transaction or session. This is part of the database connection / session management
  infrastructure.
- **Triggers for INSERT** -- `DEFAULT NOW()` on the columns handles the timestamp;
  `created_by`/`updated_by` are set explicitly by application INSERT statements.
- **Triggers for DELETE** -- hard DELETE is not used on entity tables (per `soft-delete`
  spec). Soft-delete is an UPDATE and is handled by the `BEFORE UPDATE` triggers.
- **Triggers on junction tables or reference tables** -- these tables do not carry
  `updated_at`/`updated_by` and do not need triggers.
- **Audit log triggers** that write old/new values to a separate audit log table --
  not in Phase 1. The audit columns provide who-created/who-last-modified; full change
  logging is deferred.

---

## Dependencies

- **Audit Columns** (`audit-columns` spec) -- defines the column convention that these
  triggers protect. The triggers assume every target table has `created_at`, `created_by`,
  `updated_at`, `updated_by` columns.
- **Soft Delete** (`soft-delete` spec) -- soft-delete operations are UPDATE statements
  and therefore fire these triggers. Soft-delete sets `deleted_at`/`deleted_by`; the
  trigger ensures `updated_at`/`updated_by` are also set correctly.
- **IAM Auth** -- the authenticated user ID is set via `app.current_user_id` at the
  start of each request's database transaction. The trigger reads this setting.
- **Database Migrations** -- each entity table's migration creates the two triggers
  (`trg_{table}_audit_update` and `trg_{table}_audit_immutability`) that call the
  shared trigger functions.
