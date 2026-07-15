# Design: Audit Columns

## Architecture

Audit columns are a **cross-cutting data model convention**, not a standalone feature
with its own endpoints. The design defines the column specification, the repository-layer
enforcement pattern, and the approach for embedding audit fields into entities.

The design spans two layers:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Application (Layer 2)                                                        │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  EntityService::create(command, current_user_id):                      │   │
│  │  - Sets created_by, created_at, updated_by, updated_at = current_user  │   │
│  │  - Calls EntityRepository::save(entity)                                │   │
│  │                                                                        │   │
│  │  EntityService::update(id, command, current_user_id):                  │   │
│  │  - Calls EntityRepository::update(entity)                              │   │
│  │  - Service does NOT set created_at/created_by in the update command    │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │                                                     │
│                         ▼                                                     │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  SqlEntityRepository::save(entity):                                    │   │
│  │  - INSERT includes created_at, created_by, updated_at, updated_by      │   │
│  │                                                                        │   │
│  │  SqlEntityRepository::update(entity):                                  │   │
│  │  - UPDATE sets updated_at = NOW(), updated_by = $current_user          │   │
│  │  - UPDATE does NOT include created_at, created_by in SET clause        │   │
│  │  - Repository enforces: if created columns appear in SET, panic/error  │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Defence in depth:** Immutability of `created_at`/`created_by` is enforced at both:
- **Application layer** (repository code never writes to these columns on UPDATE).
- **Database layer** (BEFORE UPDATE trigger raises exception if they change -- see
  `audit-triggers` spec).

---

## Column Specification

### Full Audit Column Set (Mutable Entity Tables)

Every mutable entity table that represents a domain object with a lifecycle must include:

```sql
created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
created_by  BIGINT       NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
updated_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
updated_by  BIGINT       NOT NULL REFERENCES users(id) ON DELETE RESTRICT
```

### Exemptions

| Table category | Columns included | Rationale |
|----------------|-----------------|-----------|
| Read-only reference tables (seeded, never updated by application) | `created_at` only | No user modifies these rows. Example: `PERMISSIONS`. |
| Junction / association tables (hard DELETE, no UPDATE) | `created_at` only | Membership is binary. Rows are inserted and deleted, never updated. Example: `PROJECT_MEMBERS`, `USER_GROUPS`. |
| `USERS` table | All four, but `created_by` nullable | The CLI bootstrap admin has no creator. All other users have a non-null `created_by` (the admin who created them). |

### Foreign Key Indexes

Every `_by` column must have a supporting index for join performance:

```sql
CREATE INDEX idx_{table}_created_by ON {table}(created_by);
CREATE INDEX idx_{table}_updated_by ON {table}(updated_by);
```

---

## Domain Entity Design

### `AuditFields` Value Object

A reusable value object that can be embedded in every domain entity:

```rust
// domain/audit.rs
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct AuditFields {
    pub created_at: DateTime<Utc>,
    pub created_by: i64,
    pub updated_at: DateTime<Utc>,
    pub updated_by: i64,
}

impl AuditFields {
    /// Create new audit fields for a freshly inserted row.
    /// Both created_* and updated_* are set to the same values.
    pub fn new(current_user_id: i64) -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            created_by: current_user_id,
            updated_at: now,
            updated_by: current_user_id,
        }
    }
}
```

### Embedding in Entity Structs

Every domain entity that maps to a mutable table embeds `AuditFields`:

```rust
// domain/project.rs
pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub status: ProjectStatus,
    pub audit: AuditFields,          // created_at/created_by/updated_at/updated_by
    pub deletion: Option<DeletionFields>, // deleted_at/deleted_by (from soft-delete spec)
}
```

The `audit` field is never modified directly by business logic. It is:
- Set by the repository on INSERT (from the authenticated user context).
- Updated by the repository on UPDATE (the `updated_*` fields only).
- Read by the presentation layer for display (e.g. "Created by John Doe on 2026-07-14").

---

## Repository Pattern

### INSERT

All four audit columns are set to the current timestamp and user:

```rust
// Pseudocode
async fn save(&self, entity: &Entity, created_by: i64) -> Result<Entity, RepositoryError> {
    let row = sqlx::query_as(
        "INSERT INTO {table} (name, description, status,
         created_at, created_by, updated_at, updated_by)
         VALUES ($1, $2, $3, NOW(), $4, NOW(), $4)
         RETURNING id, created_at, created_by, updated_at, updated_by"
    )
        .bind(&entity.name)
        .bind(&entity.description)
        .bind(&entity.status)
        .bind(created_by)
        .fetch_one(&mut *self.tx)
        .await?;
    Ok(row)
}
```

**Key points:**
- `created_at = updated_at = NOW()` on INSERT.
- `created_by = updated_by = <current_user_id>` on INSERT.
- All four values come from the authenticated session context, never from client input.

### UPDATE

Only `updated_at` and `updated_by` appear in the SET clause. `created_at` and
`created_by` are never included:

```rust
// Pseudocode
async fn update(&self, entity: &Entity, updated_by: i64) -> Result<Entity, RepositoryError> {
    let row = sqlx::query_as(
        "UPDATE {table}
         SET name = $1, description = $2, status = $3,
             updated_at = NOW(), updated_by = $4
         WHERE id = $5 AND deleted_at IS NULL
         RETURNING id, name, description, status,
                   created_at, created_by, updated_at, updated_by"
    )
        .bind(&entity.name)
        .bind(&entity.description)
        .bind(&entity.status)
        .bind(updated_by)
        .bind(entity.id)
        .fetch_optional(&mut *self.tx)
        .await?;
    // ...
}
```

**Enforcement pattern:** The repository MUST NOT include `created_at` or `created_by`
in any UPDATE SET clause. This is enforced by:
1. **Code review:** UPDATE queries are reviewed to verify only `updated_at`/`updated_by`
   appear.
2. **Repository unit tests:** Each repository test inserts a row, updates it, and
   asserts `created_at` and `created_by` are unchanged.
3. **Database trigger:** The `BEFORE UPDATE` trigger (see `audit-triggers` spec) is
   the last line of defence, raising an exception if `created_at` or `created_by` are
   modified.

---

## Components

### New Shared Components

| Component | Layer | Role |
|-----------|-------|------|
| `AuditFields` | Domain (1) | Value object holding the four audit timestamps and user references. Factory method `new(current_user_id)` sets all four to the same values for INSERT. |
| `AuditColumns` (migration helper) | Infrastructure (4) | SQL template module. Provides reusable fragments for adding audit columns to a migration: `audit_columns_sql()`, `audit_column_indexes_sql(table_name)`. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| Every domain entity (`Project`, `TestCase`, `TestPlan`, `TestRun`, `TestExecution`, `User`, `Group`, `Role`) | Embed `AuditFields` (or equivalent fields). |
| Every entity repository | INSERT queries set all four audit columns. UPDATE queries set only `updated_at`/`updated_by`. Repository tests verify `created_at`/`created_by` immutability. |
| Every entity migration | Include the four audit columns in the `CREATE TABLE` statement. Include FK indexes on `created_by` and `updated_by`. |
| Response DTOs (`ProjectResponse`, etc.) | Include `created_by`, `created_at`, `updated_by`, `updated_at` fields (or `updated_at` only for list responses). |

---

## Error Handling

| Error Case | Layer | Behaviour |
|------------|-------|-----------|
| `created_by` is NULL on INSERT (non-bootstrap record) | Database | NOT NULL constraint violation. Application should validate before INSERT. |
| `created_by` FK references non-existent user | Database | FK constraint violation. Indicates a bug (session references deleted user). |
| Attempt to modify `created_at`/`created_by` in UPDATE | Application | Repository code never includes them. If accidentally included, unit test catches it. |
| Attempt to modify `created_at`/`created_by` in UPDATE | Database | Trigger raises exception (see `audit-triggers` spec). |
| `updated_by` FK references non-existent user | Database | FK constraint violation. Indicates a bug (session references deleted user). |

**Anti-patterns explicitly avoided:**

- **Do not accept `created_at`/`created_by`/`updated_at`/`updated_by` from client
  input.** These are set server-side from the authenticated session.
- **Do not use `SERIAL`/`BIGSERIAL` for `created_by`/`updated_by`.** They are FK
  references to `users.id`, not auto-generated values.
- **Do not expose `deleted_at`/`deleted_by` in responses** unless the entity is
  being viewed in an admin/audit context. Standard detail views show only active
  records and therefore do not include deletion fields.
- **Do not include `updated_by`/`updated_at` on junction tables** that only support
  INSERT and DELETE (no UPDATE). These tables carry only `created_at`.
