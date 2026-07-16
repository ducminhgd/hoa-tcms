# Tasks: Soft Delete

> **Note:** This spec defines the project-wide soft-delete convention and shared
> infrastructure. Individual entity specs are responsible for implementing soft-delete
> on their own tables. Tasks here cover the convention definition and reusable components.
> Each entity spec's own tasks.md should include a task referencing this convention.

---

## Convention & Documentation

- [ ] 1. **Document the soft-delete convention** -- `requirements.md#US-01`, `design.md#Column Convention`
  - Publish a clear, single-page reference in the project wiki or developer guide
    summarising: which columns to include, the partial index pattern, the repository
    pattern for `soft_delete`, the query filtering rule, and which table categories
    are exempt.
  - Include the SQL template for the `deleted_at`/`deleted_by` column pair.
  - List exempt table categories (junction tables, `OBJECT_SHARING`, read-only
    reference tables) with rationale.

## Domain Layer

- [ ] 2. **Define `SoftDelete` trait** -- `design.md#Components`
  - Define a trait (Rust) or interface with a single method:
    `fn soft_delete(&mut self, deleted_by: i64)`.
  - Entity types that support soft-delete implement this trait.
  - The trait lives in the Domain layer (no infrastructure dependencies).
  - This is optional if entities handle deletion through their repository interface
    rather than the entity itself. In that case, the convention is expressed through
    the repository pattern (Task 3).

## Infrastructure Layer

- [ ] 3. **Implement `ActiveRecordFilter` query helper** -- `design.md#Components`, `requirements.md#US-03`
  - Create a reusable query helper (function or macro) that appends
    `WHERE deleted_at IS NULL` to SQLx or Diesel queries.
  - Handle both simple table queries and JOIN scenarios where the filter applies
    to a specific table alias (e.g. `WHERE p.deleted_at IS NULL`).
  - Write unit tests verifying the generated SQL includes the correct WHERE clause.

- [x] 4. **Create migration template for entity tables** -- `design.md#Column Convention`
  - Provide an SQL template for adding `deleted_at`/`deleted_by` columns to entity
    tables, including the FK constraint and partial index.
  - Template:
    ```sql
    ALTER TABLE {table} ADD COLUMN deleted_at TIMESTAMPTZ;
    ALTER TABLE {table} ADD COLUMN deleted_by BIGINT
        REFERENCES users(id) ON DELETE RESTRICT;
    CREATE INDEX idx_{table}_active ON {table}(id)
        WHERE deleted_at IS NULL;
    CREATE INDEX idx_{table}_deleted_by ON {table}(deleted_by);
    ```
  - This template is consumed by each entity table's migration. Entity migrations
    substitute `{table}` with their actual table name.

## Testing

- [ ] 5. **Write integration test: soft-delete happy path** -- `requirements.md#US-02`
  - Use `PROJECTS` table (first entity to implement soft-delete) as the test target.
  - Create a project, soft-delete it, verify `deleted_at` and `deleted_by` are set.
  - Verify `deleted_at` is a recent timestamp and `deleted_by` matches the
    authenticated user.
  - Verify `updated_at` and `updated_by` are also updated.

- [ ] 6. **Write integration test: soft-delete idempotency** -- `requirements.md#US-02`
  - Soft-delete a project, then attempt to soft-delete it again.
  - Verify the second attempt returns `404 Not Found` (already deleted).
  - Verify the `deleted_at` and `deleted_by` values are from the first delete
    (not overwritten).

- [ ] 7. **Write integration test: deleted records invisible** -- `requirements.md#US-03`
  - Create two projects, soft-delete one.
  - Call the list endpoint; verify only the non-deleted project is returned.
  - Call the detail endpoint for the deleted project; verify `404 Not Found`.
  - Verify the `404` message is identical to a non-existent project ID.

- [ ] 8. **Write integration test: no cascade to children** -- `requirements.md#US-02`, `requirements.md#US-03`
  - Create a project with child entities (test cases, test plans).
  - Soft-delete the project.
  - Query child entities directly (without joining the deleted parent).
  - Verify child entities still have `deleted_at IS NULL` (not cascade-deleted).
  - Query child entities through the parent table join.
  - Verify children are hidden by the parent's `deleted_at IS NOT NULL` filter.

- [ ] 9. **Write integration test: UPDATE rejected on soft-deleted row** -- `requirements.md#US-03`
  - Create a project, soft-delete it.
  - Call PATCH on the soft-deleted project with a name change.
  - Verify `404 Not Found` (per FR-54c, UPDATE on soft-deleted rows is rejected).

---

## Dependencies on Other Specs

Each entity spec must add a task referencing this convention when implementing its
repository:

> **Example task** (appears in `project-crud/tasks.md`, etc.):
>
> - [ ] N. **Implement soft-delete on `{Entity}Repository`** -- following the convention
>   from `soft-delete/` spec
>   - Migration: add `deleted_at TIMESTAMPTZ`, `deleted_by BIGINT FK`,
>     partial index `WHERE deleted_at IS NULL`.
>   - Repository: all SELECT queries include `WHERE deleted_at IS NULL`.
>   - Repository: implement `soft_delete(id, deleted_by)` per the pattern.
>   - Service: call `soft_delete` from the delete use case.
>   - Handler: return `404` when entity is already soft-deleted.
