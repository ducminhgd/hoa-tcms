# Design: IAM Permissions

## Architecture

The IAM Permissions feature follows Clean Architecture layering. Because the permission table
is read-only and seeded at migration time, the feature is narrow: a single list endpoint backed
by a repository that reads from PostgreSQL. No write paths exist.

```
┌─────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                 │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                               │   │
│  │  - list_permissions_handler    GET /api/v1/permissions       │   │
│  └──────────┬───────────────────────────────────────────────────┘   │
│             │ calls                                                 │
│             ▼                                                       │
│  Application (Layer 2)                         ┌─────────────────┐ │
│  ┌─────────────────────────────────────────┐   │  Domain (L1)   │ │
│  │  ListPermissionsUseCase                 │   │  - Permission  │ │
│  │  PermissionRepository (interface)       │   └─────────────────┘ │
│  └──────────┬─────────────────────────────┘                       │
│             │ delegates to                                        │
│             ▼                                                     │
│  Infrastructure (Layer 4)                                         │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │  - SqlPermissionRepository (implements PermissionRepository) │ │
│  │  - Seed migration (V2__seed_permissions.sql)                 │ │
│  └──────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

**Key design decisions:**

- **No write endpoints** — permissions are seeded by migration only. The HTTP layer rejects
  all non-GET methods with `405 Method Not Allowed`.
- **No caching** — the permission table is small (≤ 100 rows) and read-heavy. PostgreSQL's
  page cache is sufficient; introducing Redis or in-memory caching is premature optimisation.
- **Pagination** is included for consistency with all list endpoints, even though the catalog
  is small (51 rows at launch). The same shared pagination utilities used by other list
  endpoints are reused here.
- **No soft-delete** — per FR-54a, read-only reference tables carry only `created_at`.
  Permissions are never removed, only added in future migrations.
- **No audit columns** beyond `created_at` — no `updated_at`/`updated_by`/`deleted_at`/`deleted_by`
  because the rows are written exactly once by a migration and never modified afterwards.

---

## API Contract

### GET `/api/v1/permissions`

List all seeded permissions. Read-only endpoint.

**Request Headers:**
| Header | Value |
|--------|-------|
| Cookie | `hoa-tcms-session=<session_id>` (required) |
| Accept | `application/json` |

**Query Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | integer | `1` | Page number (1-indexed) |
| `limit` | integer | `25` | Items per page (max 100) |

**Success Response:** `200 OK`
```json
{
  "data": [
    {
      "id": 1,
      "name": "Create User",
      "code": "user:create"
    },
    {
      "id": 2,
      "name": "Read User",
      "code": "user:read"
    }
  ],
  "meta": {
    "total": 51,
    "page": 1,
    "limit": 25
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | Session cookie is missing, invalid, or expired |
| `403 Forbidden` | `FORBIDDEN` | Authenticated but lacks `permission:read_list` |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | `page` < 1 or `limit` < 1 or `limit` > 100 |
| `405 Method Not Allowed` | `METHOD_NOT_ALLOWED` | Any method other than `GET` |

**Error body format (consistent with `iam-auth`):**
```json
{
  "error": {
    "code": "FORBIDDEN",
    "message": "Insufficient permissions"
  }
}
```

For validation errors:
```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid query parameters",
    "details": [
      { "field": "limit", "message": "must be between 1 and 100" }
    ]
  }
}
```

---

## Data Model

### New table: `permissions`

Read-only reference table. No soft-delete, no audit pairs — only `created_at`.

```sql
CREATE TABLE permissions (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name       VARCHAR(255) NOT NULL,
    code       VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX uq_permissions_code ON permissions (code);
```

**Indexes:**
| Name | Columns | Purpose |
|------|---------|---------|
| `uq_permissions_code` | `code` | Enforce unique codes (used for lookup in authorization) |

### Seed data (51 rows)

The complete permission catalog, ordered by resource then action:

| # | code | name |
|---|------|------|
| 1 | `user:create` | Create User |
| 2 | `user:read` | Read User |
| 3 | `user:read_list` | Read User List |
| 4 | `user:update` | Update User |
| 5 | `user:delete` | Delete User |
| 6 | `user:select` | Select User |
| 7 | `group:create` | Create Group |
| 8 | `group:read` | Read Group |
| 9 | `group:read_list` | Read Group List |
| 10 | `group:update` | Update Group |
| 11 | `group:delete` | Delete Group |
| 12 | `group:select` | Select Group |
| 13 | `role:create` | Create Role |
| 14 | `role:read` | Read Role |
| 15 | `role:read_list` | Read Role List |
| 16 | `role:update` | Update Role |
| 17 | `role:delete` | Delete Role |
| 18 | `role:select` | Select Role |
| 19 | `permission:read_list` | Read Permission List |
| 20 | `project:create` | Create Project |
| 21 | `project:read` | Read Project |
| 22 | `project:read_list` | Read Project List |
| 23 | `project:update` | Update Project |
| 24 | `project:delete` | Delete Project |
| 25 | `project:select` | Select Project |
| 26 | `test_case:create` | Create Test Case |
| 27 | `test_case:read` | Read Test Case |
| 28 | `test_case:read_list` | Read Test Case List |
| 29 | `test_case:update` | Update Test Case |
| 30 | `test_case:delete` | Delete Test Case |
| 31 | `test_case:select` | Select Test Case |
| 32 | `test_plan:create` | Create Test Plan |
| 33 | `test_plan:read` | Read Test Plan |
| 34 | `test_plan:read_list` | Read Test Plan List |
| 35 | `test_plan:update` | Update Test Plan |
| 36 | `test_plan:delete` | Delete Test Plan |
| 37 | `test_plan:select` | Select Test Plan |
| 38 | `test_run:create` | Create Test Run |
| 39 | `test_run:read` | Read Test Run |
| 40 | `test_run:read_list` | Read Test Run List |
| 41 | `test_run:update` | Update Test Run |
| 42 | `test_run:delete` | Delete Test Run |
| 43 | `test_run:select` | Select Test Run |
| 44 | `test_execution:create` | Create Test Execution |
| 45 | `test_execution:read` | Read Test Execution |
| 46 | `test_execution:read_list` | Read Test Execution List |
| 47 | `test_execution:update` | Update Test Execution |
| 48 | `test_execution:delete` | Delete Test Execution |
| 49 | `test_execution:select` | Select Test Execution |
| 50 | `share:create` | Create Share |
| 51 | `share:delete` | Delete Share |

### Relationship to other tables

The `role_permissions` junction table (defined in the `iam-roles` feature) references
`permissions.id` as a foreign key:

```
ROLES ──┐
        │                       PERMISSIONS
        ├── role_permissions ─────┘ (FK: permission_id)
        │    (composite PK: role_id, permission_id)
```

---

## Sequence

### List Permissions Flow

1. Client sends `GET /api/v1/permissions?page=1&limit=25` with the session cookie.
2. **Auth middleware** validates the session (see `iam-auth` design.md for middleware flow).
3. The request reaches the **authorization middleware** which checks that the authenticated
   user has the `permission:read_list` system permission:
   - Load the user's effective permissions (direct roles + group-inherited roles).
   - Check if `permission:read_list` is in the union set.
   - If absent, return `403 Forbidden`.
4. If authorized, the handler calls `ListPermissionsUseCase::execute(page, limit)`.
5. `ListPermissionsUseCase` calls `PermissionRepository::find_all(page, limit)`.
6. `SqlPermissionRepository` executes:
   ```sql
   SELECT id, name, code, created_at
   FROM permissions
   ORDER BY code
   LIMIT $1 OFFSET $2;
   ```
   Also executes a count query:
   ```sql
   SELECT COUNT(*) FROM permissions;
   ```
7. The use case constructs the response DTO with `data` and `meta` (total, page, limit).
8. The handler serialises the DTO to JSON and returns `200 OK`.

### Permission Code Lookup Flow (used by `auth-rbac`)

1. The authorization middleware receives a request with a required permission code
   (e.g. `test_case:update`).
2. The middleware calls `PermissionRepository::find_by_code("test_case:update")`.
3. `SqlPermissionRepository` executes:
   ```sql
   SELECT id, name, code, created_at
   FROM permissions
   WHERE code = $1;
   ```
4. If found, the middleware uses the `id` to check the `role_permissions` join table for the
   current user. If not found, the permission code is invalid (should never happen with
   well-formed seed data) — the request is denied with `403 Forbidden`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `Permission` | Domain (1) | Entity representing a permission row: `id`, `name`, `code`, `created_at`. No behaviour beyond construction and accessors. |
| `PermissionRepository` | Application (2) | Interface (port) defining `find_all(page, limit) -> (Vec<Permission>, u64)` and `find_by_code(code) -> Option<Permission>`. |
| `ListPermissionsUseCase` | Application (2) | Orchestrates fetching paginated permissions from the repository and returns a response DTO. |
| `PermissionsResponse` / `PermissionDTO` | Application (2) | Response DTO: `PermissionDTO { id, name, code }` and the paginated wrapper `{ data, meta }`. |
| `ListPermissionsHandler` | Adapters (3) | HTTP handler for `GET /api/v1/permissions`. Extracts query params, validates, calls use case, serialises response. |
| `SqlPermissionRepository` | Infrastructure (4) | Implements `PermissionRepository` using SQL queries via `sqlx` or `diesel`. |
| Seed migration | Infrastructure (4) | SQL migration file (`V2__seed_permissions.sql`) that inserts all 51 permission rows. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| HTTP router registration | Register `GET /api/v1/permissions` route with auth middleware + `permission:read_list` guard. Explicitly **exclude** all other HTTP methods for this path (return `405`). |
| `role_permissions` table (in `iam-roles`) | FK constraint references `permissions.id` — ensure seed migration runs before the `iam-roles` migration that creates `role_permissions`. |

### Compile-time interface checks

```rust
// Rust's trait system verifies these at compile time automatically.
// For documentation purposes:
// - `SqlPermissionRepository` implements `PermissionRepository`
```

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing/invalid session cookie | `401` | `NOT_AUTHENTICATED` | INFO | From auth middleware (see `iam-auth` design). |
| Authenticated but lacks `permission:read_list` | `403` | `FORBIDDEN` | INFO | Generic message: `"Insufficient permissions"`. |
| Invalid pagination parameters (`page < 1`, `limit < 1`, `limit > 100`) | `422` | `VALIDATION_ERROR` | INFO | Field-level detail list. |
| Non-GET method on `/api/v1/permissions` | `405` | `METHOD_NOT_ALLOWED` | INFO | Response includes `Allow: GET` header. |
| Database unreachable | `503` | `DATABASE_UNAVAILABLE` | ERROR | All endpoints fail; application health check catches this. |
| Unexpected query failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen; indicates a bug or infrastructure issue. |

**Anti-patterns explicitly avoided:**

- **Do not return `404` for the permissions list endpoint** — the resource `/api/v1/permissions`
  always exists. An empty list is a valid response (`200 OK` with `data: []`).
- **Do not expose the list endpoint without authentication** — the permission catalog is an
  internal reference table, not public configuration.
- **Do not allow any write operation** — the permissions table is read-only from the application.
  Enforce at the router level (no registered routes for POST/PUT/PATCH/DELETE on
  `/api/v1/permissions`).
- **Do not cache permission rows in application memory** — the table is small and PostgreSQL
  page cache is sufficient. Premature caching adds invalidation complexity with zero benefit.
