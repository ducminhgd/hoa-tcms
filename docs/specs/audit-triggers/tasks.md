# Tasks: Audit Triggers

> **Note:** This spec defines the shared PostgreSQL trigger functions and the application
> integration for setting `app.current_user_id`. Individual entity specs are responsible
> for adding per-table triggers in their own migrations. Tasks here cover the shared
> functions, the application-side session context plumbing, and testing.

---

## Database Layer -- Shared Trigger Functions

- [ ] 1. **Create migration for shared trigger functions** -- `design.md#trigger-functions`
  - Create the first migration (`001_create_trigger_functions.sql`) that must run
    before any entity table migration.
  - Implement `fn_audit_update()`:
    - `RETURNS TRIGGER`
    - Read `current_setting('app.current_user_id', true)` with `missing_ok = true`.
    - Set `NEW.updated_at := NOW()`.
    - If `current_user_id` is not NULL, set `NEW.updated_by := current_user_id`.
    - `RETURN NEW`.
    - Use `$$` dollar-quoting for the function body.
  - Implement `fn_audit_immutability()`:
    - `RETURNS TRIGGER`
    - Compare `NEW.created_at IS DISTINCT FROM OLD.created_at`. If different, raise
      exception `'Cannot modify created_at on table %'` with `ERRCODE = '23000'`.
    - Compare `NEW.created_by IS DISTINCT FROM OLD.created_by`. If different, raise
      exception with same pattern.
    - `RETURN NEW`.
  - Rollback migration: `DROP FUNCTION IF EXISTS fn_audit_update CASCADE;`
    `DROP FUNCTION IF EXISTS fn_audit_immutability CASCADE;`
    (CASCADE drops the per-table triggers that depend on them).

## Database Layer -- Per-Table Trigger Template

- [ ] 2. **Create migration helper for per-table triggers** -- `design.md#Per-Table Trigger Registration`
  - Document the SQL template for per-table triggers:
    ```sql
    CREATE TRIGGER trg_{table}_audit_immutability
        BEFORE UPDATE ON {table}
        FOR EACH ROW
        EXECUTE FUNCTION fn_audit_immutability();

    CREATE TRIGGER trg_{table}_audit_update
        BEFORE UPDATE ON {table}
        FOR EACH ROW
        EXECUTE FUNCTION fn_audit_update();
    ```
  - Note the alphabetical ordering: `immutability` fires before `update`.
  - Document which tables get triggers (all mutable entity tables with audit columns)
    and which do not (junction tables, reference tables, system tables).
  - This template is consumed by each entity table's migration.

## Application Layer -- Session Context

- [ ] 3. **Implement `set_user_context` for transactions** -- `design.md#Application Integration`
  - Create a utility function or middleware that wraps transaction initialisation:
    ```rust
    async fn set_transaction_user_context(
        tx: &mut Transaction<'_, Postgres>,
        user_id: i64,
    ) -> Result<(), Error> {
        sqlx::query("SELECT set_config('app.current_user_id', $1::TEXT, true)")
            .bind(user_id.to_string())
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
    ```
  - The third parameter `true` (is_local) scopes the setting to the current
    transaction. When the transaction ends, the setting is cleared.
  - Call this function at the start of every database transaction initiated by an
    authenticated HTTP request.
  - If a request does not have an authenticated user (e.g. login endpoint), do not
    call this function.
  - Handle the edge case where `user_id` is 0 or negative -- validate before calling
    `set_config`.

- [ ] 4. **Wire user context into the HTTP request lifecycle** -- `design.md#Application Integration`
  - In the Actix-Web (or equivalent) middleware or request guard, after
    `AuthMiddleware` has extracted the authenticated user ID from the session:
    - Store the user ID in request extensions / context.
    - Ensure every downstream database transaction initialisation calls
      `set_transaction_user_context(tx, user_id)`.
  - Test: start a request, verify `SHOW app.current_user_id` returns the expected
    value within the transaction.
  - Test: after the transaction commits/rolls back, verify the setting is cleared
    (not leaked to the next transaction on the same connection).

## Testing

- [ ] 5. **Write database integration test: `updated_at` auto-maintained** -- `requirements.md#US-01`
  - Create a test table with audit columns and the two triggers.
  - Insert a row with `created_by = 1`.
  - Set `app.current_user_id = '2'` with `set_config`.
  - Execute `UPDATE test_table SET name = 'Updated' WHERE id = 1`.
  - Assert `updated_at` is later than the original `updated_at` (which equalled
    `created_at` on insert).
  - Assert `updated_by = 2`.
  - Assert `created_at` and `created_by` are unchanged.

- [ ] 6. **Write database integration test: `updated_by` unchanged when setting absent** -- `requirements.md#US-01`
  - Create test table and row as above.
  - Do NOT set `app.current_user_id`.
  - Execute `UPDATE test_table SET name = 'Updated' WHERE id = 1`.
  - Assert `updated_at` is updated (set to NOW()).
  - Assert `updated_by` is unchanged (still the original value from INSERT).
  - Assert no error is raised.

- [ ] 7. **Write database integration test: `created_at` immutability enforced** -- `requirements.md#US-02`
  - Create test table and row.
  - Execute `UPDATE test_table SET created_at = '2020-01-01 00:00:00+00' WHERE id = 1`.
  - Assert the UPDATE is rejected with an exception.
  - Verify the error message contains `'Cannot modify created_at'`.
  - Verify the error SQLSTATE is `23000` (integrity_constraint_violation).

- [ ] 8. **Write database integration test: `created_by` immutability enforced** -- `requirements.md#US-02`
  - Create test table and row.
  - Execute `UPDATE test_table SET created_by = 999 WHERE id = 1`.
  - Assert the UPDATE is rejected with an exception.
  - Verify the error message contains `'Cannot modify created_by'`.

- [ ] 9. **Write database integration test: both triggers fire in correct order** -- `design.md#Sequence`
  - Purpose: verify that when both conditions apply (attempt to modify `created_at`
    AND normal UPDATE), the immutability trigger fires first and blocks the operation.
  - Create test table and row.
  - Execute `UPDATE test_table SET name = 'Updated', created_at = '2020-01-01' WHERE id = 1`.
  - Assert the UPDATE is rejected (immutability fires first).
  - Verify the `updated_at`/`updated_by` auto-maintenance does NOT run (the UPDATE
    was aborted).

---

## Dependencies on Other Specs

Each entity spec must add a task referencing this convention:

> **Example task** (appears in `project-crud/tasks.md`, etc.):
>
> - [ ] N. **Add audit triggers to `{TABLE}` migration** -- following the convention
>   from `audit-triggers/` spec
>   - Add `CREATE TRIGGER trg_{table}_audit_immutability ...` statement.
>   - Add `CREATE TRIGGER trg_{table}_audit_update ...` statement.
>   - Rollback migration: `DROP TRIGGER IF EXISTS ... ON {table}` statements.
>   - Integration test: verify that UPDATE auto-sets `updated_at`/`updated_by`.
>   - Integration test: verify that modifying `created_at`/`created_by` raises exception.
