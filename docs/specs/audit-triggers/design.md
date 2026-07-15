# Design: Audit Triggers

## Architecture

Audit triggers are a **cross-cutting database infrastructure concern**. They provide
the last line of defence for audit column integrity. The design consists of two shared
PostgreSQL trigger functions and a mechanism for the application to communicate the
current user's ID to the database.

The design spans two layers:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  Application Session → DB Transaction:                                 │   │
│  │  - At transaction start, the app calls:                                │   │
│  │    SELECT set_config('app.current_user_id', $1::TEXT, true)            │   │
│  │                                                                        │   │
│  │  Shared Trigger Functions:                                             │   │
│  │  - fn_audit_update()      → auto-sets updated_at, updated_by           │   │
│  │  - fn_audit_immutability() → rejects changes to created_at, created_by │   │
│  │                                                                        │   │
│  │  Per-Table Triggers (created by each entity's migration):              │   │
│  │  - trg_{table}_audit_update      → BEFORE UPDATE, calls fn_audit_update│   │
│  │  - trg_{table}_audit_immutability → BEFORE UPDATE, calls fn_immutability│  │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  Domain / Application (Layers 1-3)                                           │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - Repository layer also enforces immutability (defence in depth)      │   │
│  │  - Application sets app.current_user_id at transaction start           │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. Each HTTP request begins a database transaction (or session).
2. The application calls `SET app.current_user_id = '<user_id>'` to communicate the
   authenticated user's ID to PostgreSQL.
3. On any UPDATE to an entity table, the `BEFORE UPDATE` trigger fires:
   a. `fn_audit_immutability()` checks if `NEW.created_at` differs from
      `OLD.created_at` or `NEW.created_by` differs from `OLD.created_by`. If so, it
      raises an exception and aborts the UPDATE.
   b. `fn_audit_update()` sets `NEW.updated_at = NOW()` and
      `NEW.updated_by = current_setting('app.current_user_id', true)::BIGINT`
      (if the setting is available).
4. The UPDATE proceeds with the corrected `updated_at`/`updated_by` values.
5. Application-layer repository code also avoids modifying `created_at`/`created_by`
   (defence in depth -- the trigger is the safety net, not the primary enforcement).

---

## Trigger Functions

### Shared Function: `fn_audit_update()`

Auto-maintains `updated_at` and `updated_by` on every UPDATE.

```sql
CREATE OR REPLACE FUNCTION fn_audit_update()
RETURNS TRIGGER AS $$
DECLARE
    current_user_id BIGINT;
BEGIN
    -- Read the authenticated user ID from the session-level setting.
    -- The 'true' parameter means missing_ok: returns NULL if not set.
    current_user_id := NULLIF(
        current_setting('app.current_user_id', true), ''
    )::BIGINT;

    -- Always update the timestamp.
    NEW.updated_at := NOW();

    -- Update updated_by only if we have a user context.
    -- If the setting is absent (e.g. migration script), keep the old value.
    IF current_user_id IS NOT NULL THEN
        NEW.updated_by := current_user_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
```

**Design decisions:**
- `current_setting(..., true)` with `missing_ok = true` prevents errors when the
  setting is not defined (e.g. during migrations or manual DB operations).
- If `current_user_id` is NULL (setting absent), `updated_by` is left unchanged.
  This is safe because application-performed UPDATEs always have the setting.
- `updated_at` is always set to `NOW()` regardless of user context -- a timestamp
  is always available.

### Shared Function: `fn_audit_immutability()`

Rejects UPDATEs that attempt to modify `created_at` or `created_by`.

```sql
CREATE OR REPLACE FUNCTION fn_audit_immutability()
RETURNS TRIGGER AS $$
BEGIN
    -- Compare NEW vs OLD for created_at (timestamptz).
    IF NEW.created_at IS DISTINCT FROM OLD.created_at THEN
        RAISE EXCEPTION 'Cannot modify created_at on table %', TG_TABLE_NAME
            USING ERRCODE = '23000';  -- integrity_constraint_violation
    END IF;

    -- Compare NEW vs OLD for created_by (bigint).
    IF NEW.created_by IS DISTINCT FROM OLD.created_by THEN
        RAISE EXCEPTION 'Cannot modify created_by on table %', TG_TABLE_NAME
            USING ERRCODE = '23000';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
```

**Design decisions:**
- Uses `IS DISTINCT FROM` (not `<>`) to correctly handle NULL comparisons.
  (Although `created_at` and `created_by` are NOT NULL, using `IS DISTINCT FROM`
  is the safest pattern and is idiomatic PostgreSQL.)
- Raises `integrity_constraint_violation` (SQLSTATE `23000`), which maps naturally
  to a `500 Internal Server Error` at the application level.
- Includes `TG_TABLE_NAME` in the error message for debugging (the table name helps
  identify which trigger fired).
- Returns `NEW` to allow the UPDATE to proceed if no violation.

---

## Per-Table Trigger Registration

Each entity table migration creates two triggers that call the shared functions:

```sql
-- Template for each entity table (e.g., for the 'projects' table)

-- Trigger 1: auto-maintain updated_at / updated_by
CREATE TRIGGER trg_projects_audit_update
    BEFORE UPDATE ON projects
    FOR EACH ROW
    EXECUTE FUNCTION fn_audit_update();

-- Trigger 2: enforce created_at / created_by immutability
CREATE TRIGGER trg_projects_audit_immutability
    BEFORE UPDATE ON projects
    FOR EACH ROW
    EXECUTE FUNCTION fn_audit_immutability();
```

**Trigger ordering:** PostgreSQL fires triggers in alphabetical order by trigger name.
The immutability check (`trg_*_audit_immutability`) fires before the auto-maintenance
trigger (`trg_*_audit_update`) because `immutability` sorts before `update`. This is
the correct order: reject invalid changes first, then auto-set fields.

**Tables that get triggers:** Every mutable entity table that has `created_at`,
`created_by`, `updated_at`, `updated_by` columns. Junction tables, reference tables,
and system tables do not get these triggers (they lack the columns).

---

## Application Integration: Setting `app.current_user_id`

### Per-Transaction Approach (Recommended)

The application calls `SET LOCAL app.current_user_id = '<user_id>'` at the start of
every database transaction that services an authenticated request:

```rust
// At transaction start, after extracting user from session
async fn begin_transaction(pool: &PgPool, user_id: i64) -> Result<Transaction, Error> {
    let mut tx = pool.begin().await?;

    sqlx::query("SELECT set_config('app.current_user_id', $1::TEXT, true)")
        .bind(user_id.to_string())
        .execute(&mut *tx)
        .await?;

    Ok(tx)
}
```

The third parameter to `set_config` (`is_local = true`) scopes the setting to the
current transaction. When the transaction commits or rolls back, the setting is
automatically cleared.

**Alternative (session-level):** If the database connection pool maintains affinity
(a connection is reused for multiple requests from different users), use
`SET LOCAL` (transaction-scoped) to avoid leaking the user ID across requests.

### Fallback for Unauthenticated Requests

Endpoints that do not require authentication (e.g. `POST /api/v1/auth/login`) do not
set `app.current_user_id`. UPDATEs from these endpoints should not occur (login does
not update entity tables), but if they do, `updated_by` will remain unchanged and
`updated_at` will still be set.

### Migration Scripts and CLI Tools

Migration scripts and CLI tools (e.g. the `iam-cli-init` bootstrap command) run outside
the HTTP request context and do not set `app.current_user_id`. The trigger handles this:
- `updated_at` is set to `NOW()`.
- `updated_by` is left unchanged (the application is expected to set it explicitly
  in these contexts, or the initial INSERT provides it).

---

## Sequence

### UPDATE Flow with Triggers

1. Application begins a transaction and calls
   `SELECT set_config('app.current_user_id', '42', true)`.
2. Application executes an UPDATE statement:
   ```sql
   UPDATE projects SET name = 'New Name' WHERE id = 1;
   ```
3. PostgreSQL fires the `BEFORE UPDATE` triggers in alphabetical order:
   a. `trg_projects_audit_immutability`:
      - Compares `NEW.created_at` vs `OLD.created_at` -- equal, OK.
      - Compares `NEW.created_by` vs `OLD.created_by` -- equal, OK.
      - Returns `NEW`.
   b. `trg_projects_audit_update`:
      - Reads `current_setting('app.current_user_id', true)` -- returns `'42'`.
      - Sets `NEW.updated_at := NOW()`.
      - Sets `NEW.updated_by := 42`.
      - Returns `NEW`.
4. The UPDATE commits with the corrected `updated_at`/`updated_by` values.

### Attempted `created_at` Modification Flow

1. Application has a bug that includes `created_at` in the UPDATE:
   ```sql
   UPDATE projects SET name = 'New Name', created_at = '2020-01-01' WHERE id = 1;
   ```
2. `trg_projects_audit_immutability` fires:
   - Compares `NEW.created_at` ('2020-01-01') vs `OLD.created_at` (original value).
   - They differ. Raises exception:
     `'Cannot modify created_at on table projects'`.
   - The UPDATE is aborted. The application receives a database error.
3. The application maps the `integrity_constraint_violation` SQLSTATE to a
   `500 Internal Server Error` and logs the incident for investigation.

---

## Components

### New Database Components

| Component | Type | Role |
|-----------|------|------|
| `fn_audit_update()` | PostgreSQL function | Shared trigger function. Auto-sets `updated_at = NOW()` and `updated_by = current_setting('app.current_user_id')`. |
| `fn_audit_immutability()` | PostgreSQL function | Shared trigger function. Rejects UPDATEs that modify `created_at` or `created_by`. |
| `trg_{table}_audit_update` | Per-table trigger | `BEFORE UPDATE ... FOR EACH ROW EXECUTE FUNCTION fn_audit_update()`. |
| `trg_{table}_audit_immutability` | Per-table trigger | `BEFORE UPDATE ... FOR EACH ROW EXECUTE FUNCTION fn_audit_immutability()`. |

### New Application Components

| Component | Layer | Role |
|-----------|-------|------|
| `UserContextSetter` | Infrastructure (4) | Utility that calls `SET app.current_user_id` at transaction start. Wraps the database transaction initialisation. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| Transaction / connection pool initialisation | Add `SET app.current_user_id` call at the start of each authenticated request's transaction. |
| Every entity table migration | Add two `CREATE TRIGGER` statements calling the shared functions. |
| Global migration (runs first) | Create the two shared trigger functions (`fn_audit_update`, `fn_audit_immutability`). |

### Migration Ordering

The shared trigger functions must be created before any entity table migration runs
(because entity migrations create per-table triggers that reference the functions).

Migration sequencing:
1. `001_create_trigger_functions.sql` -- creates `fn_audit_update()` and
   `fn_audit_immutability()`.
2. Entity table migrations (e.g. `002_create_users.sql`, `003_create_projects.sql`,
   etc.) -- each creates its two per-table triggers.

---

## Error Handling

| Error Case | Where Caught | Behaviour |
|------------|-------------|-----------|
| `app.current_user_id` not set | Trigger function | `updated_by` unchanged; `updated_at` set to `NOW()`. No error. |
| `app.current_user_id` is not a valid BIGINT | Trigger function | `::BIGINT` cast fails; UPDATE aborted with cast error. Application logs and returns `500`. |
| UPDATE attempts to change `created_at` | Trigger function | Exception raised; UPDATE aborted. Application returns `500`. |
| UPDATE attempts to change `created_by` | Trigger function | Exception raised; UPDATE aborted. Application returns `500`. |
| Trigger function does not exist (missed migration) | Database | `ERROR: function fn_audit_update() does not exist`. Application returns `500`. |

**Anti-patterns explicitly avoided:**

- **Do not hard-code table names** in the shared trigger functions. The functions use
  `TG_TABLE_NAME` for error messages and work on any table with the expected columns.
- **Do not use `TG_ARGV`** (trigger arguments) for column names. The trigger functions
  assume the standard column names (`created_at`, `created_by`, `updated_at`,
  `updated_by`) and do not need parameterisation.
- **Do not fire triggers on INSERT** -- `DEFAULT NOW()` handles `created_at`/`updated_at`
  on INSERT; `created_by`/`updated_by` are set explicitly by the INSERT statement.
- **Do not fire triggers on DELETE** -- hard DELETE is not used on entity tables.
- **Do not set `app.current_user_id` at session level** (unless the connection pool
  guarantees user affinity). Prefer `SET LOCAL` (transaction-scoped) to avoid leaking
  user context across requests.
