# Tasks: Audit Columns

> **Note:** This spec defines the project-wide audit column convention and shared
> infrastructure. Individual entity specs are responsible for adding audit columns
> to their own tables. Tasks here cover the convention definition and reusable
> components. Each entity spec's own tasks.md should include a task referencing
> this convention.

---

## Convention & Documentation

- [ ] 1. **Document the audit column convention** -- `requirements.md#US-01`, `design.md#Column Specification`
  - Publish a clear reference in the project developer guide summarising: the four
    columns, their types and constraints, the INSERT and UPDATE patterns, the exemption
    rules for different table categories, and the FK index requirement.
  - Include the SQL template for adding audit columns to a `CREATE TABLE` statement.
  - List exempt table categories (read-only reference tables, junction tables, system
    tables) with rationale.

## Domain Layer

- [ ] 2. **Implement `AuditFields` value object** -- `design.md#Components`
  - Fields: `created_at: DateTime<Utc>`, `created_by: i64`, `updated_at: DateTime<Utc>`,
    `updated_by: i64`.
  - Factory method `AuditFields::new(current_user_id: i64) -> Self` that sets all four
    fields: `created_at = updated_at = Utc::now()`,
    `created_by = updated_by = current_user_id`.
  - Derive `Debug`, `Clone`. Optionally derive or implement `Serialize` for response
    serialisation.
  - No framework imports; pure domain layer.

## Infrastructure Layer

- [ ] 3. **Create SQL migration template for audit columns** -- `design.md#Components`
  - Provide reusable SQL fragments for migration files:
    - `audit_columns_ddl()` returns the four-column DDL fragment:
      ```sql
      created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
      created_by  BIGINT       NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
      updated_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
      updated_by  BIGINT       NOT NULL REFERENCES users(id) ON DELETE RESTRICT
      ```
    - `audit_column_indexes(table_name: &str)` returns the FK index DDL:
      ```sql
      CREATE INDEX idx_{table}_created_by ON {table}(created_by);
      CREATE INDEX idx_{table}_updated_by ON {table}(updated_by);
      ```
  - These fragments are consumed by each entity table's migration.

- [ ] 4. **Add `AuditFields` to domain entities** -- `design.md#Domain Entity Design`
  - Add `audit: AuditFields` field to every existing domain entity: `Project`,
    `User`, `Group`, `Role`, etc.
  - Entity constructors accept `current_user_id` and call `AuditFields::new()`.
  - Entity UPDATE methods never modify `audit.created_at` or `audit.created_by`.
  - This task may be split across entity specs; this spec establishes the pattern
    and the first entity (e.g. `Project`) serves as the reference implementation.

## Testing

- [ ] 5. **Write unit test: `AuditFields::new` round-trip** -- `requirements.md#US-01`
  - Create `AuditFields::new(42)`.
  - Assert `created_by == 42` and `updated_by == 42`.
  - Assert `created_at == updated_at` (same timestamp within a small tolerance).
  - Assert both timestamps are recent (within 1 second of `Utc::now()`).

- [ ] 6. **Write repository integration test: INSERT sets audit columns** -- `requirements.md#US-01`
  - Use `PROJECTS` table as the test target.
  - Insert a project with `created_by = 1`.
  - Query the row and assert: `created_by == 1`, `updated_by == 1`,
    `created_at == updated_at`, `created_at IS NOT NULL`.
  - Verify the `DEFAULT NOW()` behaviour works (timestamp is recent and non-null).
  - Verify FK constraint: inserting with a non-existent `created_by` fails.

- [ ] 7. **Write repository integration test: UPDATE modifies only updated columns** -- `requirements.md#US-02`
  - Insert a project. Record `created_at`, `created_by`, `updated_at`, `updated_by`.
  - Update the project name with `updated_by = 2`.
  - Query the row and assert:
    - `created_at` is unchanged (exact same value).
    - `created_by` is unchanged (still `1`).
    - `updated_at` has advanced (later than the original `updated_at`).
    - `updated_by` is now `2`.
  - This test directly verifies the immutability enforcement at the application layer.

---

## Dependencies on Other Specs

Each entity spec must add a task referencing this convention:

> **Example task** (appears in `project-crud/tasks.md`, etc.):
>
> - [ ] N. **Add audit columns to `{TABLE}` migration** -- following the convention
>   from `audit-columns/` spec
>   - Include the four audit columns in the `CREATE TABLE` statement.
>   - Add FK indexes on `created_by` and `updated_by`.
>   - Insert repository: set all four audit columns from the authenticated user context.
>   - Update repository: set only `updated_at`/`updated_by`; never modify
>     `created_at`/`created_by`.
>   - Unit test: INSERT sets all four columns correctly.
>   - Unit test: UPDATE does not change `created_at`/`created_by`.
