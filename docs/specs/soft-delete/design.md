# Design: Soft Delete

## Architecture

Soft-delete is a **cross-cutting infrastructure concern**, not a standalone feature with
its own endpoints. It defines the column convention, repository-layer filtering, and the
shared mechanism used by every entity's DELETE endpoint.

The design spans two layers:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  Every DELETE handler in the system:                                   │   │
│  │  - Calls the entity's service, which delegates to the repository       │   │
│  │  - Returns 204 No Content on success, 404 if already deleted           │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                                                        │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  EntityService::delete(id, current_user_id):                           │   │
│  │  - Calls EntityRepository::soft_delete(id, current_user_id)            │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ delegates to                                        │
│                         ▼                                                     │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  SqlEntityRepository::soft_delete(id, deleted_by):                     │   │
│  │  - Checks deleted_at IS NULL (returns NotFound if already deleted)     │   │
│  │  - Executes UPDATE SET deleted_at=NOW(), deleted_by=$2                 │   │
│  │  - All SELECT queries include WHERE deleted_at IS NULL                 │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. A DELETE request arrives at the entity's HTTP handler.
2. The handler calls the entity's service, which checks permissions.
3. The service calls `EntityRepository::soft_delete(id, current_user_id)`.
4. The repository checks that `deleted_at IS NULL` for the given ID. If already
   deleted, returns a `NotFound` error.
5. The repository executes `UPDATE {table} SET deleted_at = NOW(), deleted_by = $2
   WHERE id = $1 AND deleted_at IS NULL`. The `updated_at` and `updated_by` columns
   are set automatically by the `BEFORE UPDATE` trigger; the application does not
   include them in the UPDATE.
6. No cascade: only the single row is updated.
7. The handler returns `204 No Content` (or `200 OK` depending on entity convention).

---

## Column Convention

Every entity table (tables that represent domain objects with a lifecycle, as opposed
to pure junction tables) must include these columns:

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `deleted_at` | `TIMESTAMPTZ` | Nullable, no default | `NULL` = active record. Set to `NOW()` on soft-delete. |
| `deleted_by` | `BIGINT` | Nullable, `REFERENCES users(id) ON DELETE RESTRICT` | `NULL` = active record. Set to current user ID on application-performed soft-delete. |

### Partial Index

Every entity table must have a partial index optimising queries for active records:

```sql
CREATE INDEX idx_{table}_active ON {table}(id) WHERE deleted_at IS NULL;
```

where `{table}` is the table name (e.g. `projects`, `test_cases`, `test_plans`).

### Tables That Do NOT Use Soft-Delete

The following table categories use **hard DELETE** and do **not** carry `deleted_at`/`deleted_by`:

| Category | Examples | Reason |
|----------|----------|--------|
| Junction / association tables | `PROJECT_MEMBERS`, `USER_GROUPS`, `ROLE_PERMISSIONS`, `GROUP_ROLES` | Membership is binary (member or not). No audit trail needed for removal. |
| Sharing table | `OBJECT_SHARING` | Sharing grants are additive and revoked by deletion. No need to preserve revoked grants. |
| Read-only reference tables | `PERMISSIONS` | Seeded at deployment, never deleted by application. |

---

## Repository Pattern

### Soft-Delete Operation

Every entity repository that implements soft-delete must follow this pattern:

```rust
// Pseudocode -- actual implementation in infrastructure layer
async fn soft_delete(&self, id: i64, deleted_by: i64) -> Result<(), RepositoryError> {
    // 1. Check the row exists and is not already deleted
    let row = sqlx::query("SELECT deleted_at FROM {table} WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *self.tx)
        .await?;

    match row {
        None => return Err(RepositoryError::NotFound),
        Some(row) if row.deleted_at.is_some() => return Err(RepositoryError::NotFound),
        _ => {}
    }

    // 2. Soft-delete: set deleted_at, deleted_by only.
    //    updated_at and updated_by are handled automatically by the
    //    BEFORE UPDATE trigger (fn_audit_update). The application MUST NOT
    //    set them here to prevent timestamp divergence between the
    //    application-set value and the trigger-set value.
    let result = sqlx::query(
        "UPDATE {table}
         SET deleted_at = NOW(), deleted_by = $2
         WHERE id = $1 AND deleted_at IS NULL"
    )
        .bind(id)
        .bind(deleted_by)
        .execute(&mut *self.tx)
        .await?;

    if result.rows_affected() == 0 {
        return Err(RepositoryError::NotFound); // race condition guard
    }

    Ok(())
}
```

**Key points:**
- The `WHERE deleted_at IS NULL` clause in the UPDATE statement prevents a TOCTOU race
  where two requests delete the same row concurrently.
- The application sets only `deleted_at` and `deleted_by`. The `BEFORE UPDATE` trigger
  (`fn_audit_update` from the `audit-triggers` spec) automatically sets `updated_at =
  NOW()` and `updated_by = current_setting('app.current_user_id')`. The application
  MUST NOT set `updated_at`/`updated_by` in the soft-delete UPDATE to prevent timestamp
  divergence between the application-set value and the trigger-set value.
- The explicit existence check is a defence-in-depth measure -- the `rows_affected()`
  guard is the authoritative check.

### Query Filtering

Every SELECT query on an entity table must include `WHERE deleted_at IS NULL` (or an
equivalent JOIN condition when joining through a parent that may be deleted):

```rust
// Direct lookup (standalone entity, no parent)
async fn find_by_id(&self, id: i64) -> Result<Option<Entity>, RepositoryError> {
    sqlx::query_as("SELECT * FROM {table} WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(&mut *self.tx)
        .await
}

// Parent-scoped entity lookup (e.g., test case under a project)
// Must JOIN the parent table to ensure the parent is not soft-deleted.
async fn find_by_id(&self, id: i64, parent_id: i64) -> Result<Option<Entity>, RepositoryError> {
    sqlx::query_as(
        "SELECT child.*
         FROM {child_table} child
         JOIN {parent_table} parent ON child.{parent_id_col} = parent.id
         WHERE child.id = $1 AND child.{parent_id_col} = $2
           AND child.deleted_at IS NULL
           AND parent.deleted_at IS NULL"
    )
        .bind(id)
        .bind(parent_id)
        .fetch_optional(&mut *self.tx)
        .await
}
// If the parent is soft-deleted, the JOIN produces no rows and the result
// is None, returning 404 identically to a directly-deleted child.

// List with pagination
async fn find_all(&self, page: i64, limit: i64) -> Result<PaginatedResult<Entity>, RepositoryError> {
    sqlx::query_as(
        "SELECT * FROM {table} WHERE deleted_at IS NULL
         ORDER BY created_at DESC LIMIT $1 OFFSET $2"
    )
        .bind(limit)
        .bind((page - 1) * limit)
        .fetch_all(&mut *self.tx)
        .await
}
```

**No-cascade enforcement:** When filtering child entities by parent, the query joins
through the parent table and the parent's `WHERE deleted_at IS NULL` condition
automatically excludes children of deleted parents. No additional condition on child
rows is needed:

```sql
-- Children of soft-deleted parents are implicitly hidden
SELECT child.*
FROM test_cases child
JOIN projects parent ON child.project_id = parent.id
WHERE parent.deleted_at IS NULL  -- hides children of deleted parents
  AND child.deleted_at IS NULL;  -- hides directly deleted children
```

---

## Sequence

### Soft-Delete Flow (generic)

1. Client sends `DELETE /api/v1/{resource}/{id}` with session cookie.
2. `AuthMiddleware` validates the session and attaches user context.
3. HTTP handler calls `EntityService::delete(id, current_user_id)`.
4. `EntityService` checks the required system permission and entity-level authorisation
   (project role, sharing role, or admin bypass).
5. `EntityService` calls `EntityRepository::soft_delete(id, current_user_id)`.
6. Repository checks `SELECT deleted_at FROM {table} WHERE id = $1`:
   - Row not found: return `NotFound` error.
   - `deleted_at IS NOT NULL`: return `NotFound` error (already deleted).
   - `deleted_at IS NULL`: proceed.
7. Repository executes `UPDATE {table} SET deleted_at = NOW(), deleted_by = $2
   WHERE id = $1 AND deleted_at IS NULL`. The `BEFORE UPDATE` trigger
   (`fn_audit_update`) automatically sets `updated_at` and `updated_by`.
8. If `rows_affected() == 0`: return `NotFound` error (concurrent delete race).
9. Service returns success.
10. Handler returns `204 No Content` (or `200 OK` with entity representation,
    per entity convention).

---

## Components

### New Shared Components

| Component | Layer | Role |
|-----------|-------|------|
| `SoftDelete` | Domain (1) | Trait (Rust) or interface defining the soft-delete contract: a method signature `soft_delete(&mut self, deleted_by: i64)` that entity types can implement. |
| `ActiveRecordFilter` | Infrastructure (4) | Reusable query fragment or helper that appends `WHERE deleted_at IS NULL` (or equivalent) to SQLx queries. Not a standalone type; a utility module used by repository implementations. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| Every entity repository (`SqlProjectRepository`, `SqlTestCaseRepository`, etc.) | Implement `soft_delete` with the pattern described above. Add `WHERE deleted_at IS NULL` to all SELECT queries. |
| Every entity service | Add `delete` method that calls `Repository::soft_delete`. |
| Every DELETE HTTP handler | Return `404` when the entity is already soft-deleted. Return `204` on success. |
| Database migrations for all entity tables | Add `deleted_at` and `deleted_by` columns, FK constraint, and partial index. |

### How Individual Entity Specs Use This Convention

Each entity spec (e.g. `project-crud`, `test-case-crud`) references soft-delete in:
- Its **requirements.md**: DELETE user story references soft-delete behaviour.
- Its **design.md**: Data model includes `deleted_at`/`deleted_by` columns; sequence
  diagram shows the soft-delete flow.
- Its **tasks.md**: Repository implementation task includes "implement `soft_delete`
  following the convention from `soft-delete` spec".

This spec provides the shared infrastructure; entity specs consume it.

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Entity not found (never existed) | `404` | `NOT_FOUND` | INFO | Generic message; do not distinguish from soft-deleted |
| Entity already soft-deleted | `404` | `NOT_FOUND` | INFO | Same generic message as not-found |
| Concurrent delete race (rows_affected == 0) | `404` | `NOT_FOUND` | INFO | Another request deleted it first |
| UPDATE on soft-deleted entity (non-restore) | `404` | `NOT_FOUND` | INFO | Per FR-54c; repository rejects the UPDATE |
| User has no delete permission | `403` | `FORBIDDEN` | INFO | Checked by service layer before repository call |
| Database error during soft-delete | `500` | `INTERNAL_ERROR` | ERROR | Unexpected DB failure |

**Anti-patterns explicitly avoided:**

- **Do not hard-delete** any entity row. All DELETE endpoints perform `UPDATE SET deleted_at`.
- **Do not cascade soft-delete** to child rows. Query filtering handles hiding children.
- **Do not return different messages** for "not found" vs "soft-deleted" (prevents
  information leakage).
- **Do not set `deleted_by` to NULL** when the application performs the soft-delete.
  `deleted_by` is NULL only when set outside the application (e.g. manual DB operation).
- **Do not set `updated_at`/`updated_by` in the soft-delete UPDATE statement.** The
  `BEFORE UPDATE` trigger (`fn_audit_update`) sets these automatically. Setting them in
  the application would cause timestamp divergence between the application-computed value
  and the trigger-computed value.
