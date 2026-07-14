# Design: IAM Groups

## Architecture

The IAM Groups feature follows Clean Architecture layering. It spans all four layers:

```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                                   │
│  ┌──────────────────────────────────────────────────────┐                             │
│  │  HTTP Handlers:                                       │                             │
│  │  - list_groups_handler     GET    /api/v1/groups     │                             │
│  │  - create_group_handler    POST   /api/v1/groups     │                             │
│  │  - get_group_handler       GET    /api/v1/groups/{id}│                             │
│  │  - update_group_handler    PATCH  /api/v1/groups/{id}│                             │
│  │  - delete_group_handler    DELETE /api/v1/groups/{id}│                             │
│  │  - manage_members_handler  POST   .../{id}/members   │                             │
│  │  - manage_roles_handler    POST   .../{id}/roles     │                             │
│  └──────────┬───────────────────────────────────────────┘                             │
│             │ calls                                                                   │
│             ▼                                                                         │
│  Application (Layer 2)                           ┌──────────────────────────────┐     │
│  ┌───────────────────────────────────────────┐   │  Domain (Layer 1)            │     │
│  │  GroupService                             │   │  - Group entity              │     │
│  │  GroupRepository (interface)              │   │  - GroupStatus value object  │     │
│  │  MemberRepository (interface)             │   │                              │     │
│  │  GroupRoleRepository (interface)          │   └──────────────────────────────┘     │
│  └──────────┬───────────────────────────────┘                                       │
│             │ delegates to                                                           │
│             ▼                                                                        │
│  Infrastructure (Layer 4)                                                            │
│  ┌──────────────────────────────────────────────────────────────────────────────┐   │
│  │  PostgresGroupRepository   (implements GroupRepository)                       │   │
│  │  PostgresMemberRepository  (implements MemberRepository)                      │   │
│  │  PostgresGroupRoleRepo     (implements GroupRoleRepository)                   │   │
│  └──────────────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. An HTTP request arrives at the handler, which validates input and calls the corresponding
   `GroupService` method.
2. `GroupService` performs business logic (uniqueness checks, status validation) and delegates
   persistence to repository interfaces.
3. Repository implementations (`PostgresGroupRepository`, etc.) execute SQL against PostgreSQL.
4. Responses flow back through the handler, which maps domain objects to JSON responses.

**Key design decisions:**

- **Three separate repository interfaces** (Group, Member, GroupRole) to keep concerns
  separated — one per logical table. The Member and GroupRole repos handle only junction
  table operations (hard-delete insert/delete).
- **The authorization middleware** (not part of this feature) checks permissions *before*
  the handler runs. The handler assumes the caller is authenticated and authorized.
- **Soft-delete enforcement** follows FR-54c: the repository layer rejects UPDATE on rows
  where `deleted_at IS NOT NULL`, with two exceptions: the soft-delete itself and a
  future restore operation.
- **Audit columns** follow FR-54a/FR-54b: `created_at`/`created_by` are set on insert and
  never modified. `updated_at`/`updated_by` are auto-maintained by a DB `BEFORE UPDATE`
  trigger.
- **Junction tables** (USER_GROUPS, GROUP_ROLES) carry `created_at` only and are hard-deleted
  (no soft-delete columns).

---

## API Contract

### GET `/api/v1/groups`

List groups with pagination and optional status filtering.

**Required Permission:** `group:read_list`

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | integer | `1` | Page number (1-indexed) |
| `limit` | integer | `25` | Items per page (max 100) |
| `sort` | string | `name` | Sort field: `name`, `id`, `created_at`, `status` |
| `order` | string | `asc` | Sort direction: `asc` or `desc` |
| `status` | string | — | Filter by `ACTIVE` or `INACTIVE` (optional — when omitted, both statuses are returned) |

**Success Response:** `200 OK`
```json
{
  "data": [
    {
      "id": 1,
      "name": "QA Engineers",
      "description": "Quality assurance engineering team",
      "status": "ACTIVE",
      "member_count": 12,
      "created_at": "2026-07-14T10:00:00Z",
      "created_by": 1,
      "updated_at": "2026-07-14T10:00:00Z",
      "updated_by": 1
    }
  ],
  "meta": {
    "total": 5,
    "page": 1,
    "limit": 25
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | No valid session |
| `403 Forbidden` | `FORBIDDEN` | Missing `group:read_list` permission |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | Invalid `page`, `limit`, `sort`, `order`, or `status` parameter |

---

### POST `/api/v1/groups`

Create a new group.

**Required Permission:** `group:create`

**Request Headers:**

| Header | Value |
|--------|-------|
| Content-Type | `application/json` |

**Request Body:**
```json
{
  "name": "QA Engineers",
  "description": "Quality assurance engineering team"
}
```

**Success Response:** `201 Created`

`Location: /api/v1/groups/42`
```json
{
  "data": {
    "id": 42,
    "name": "QA Engineers",
    "description": "Quality assurance engineering team",
    "status": "ACTIVE",
    "member_count": 0,
    "created_at": "2026-07-14T10:00:00Z",
    "created_by": 1,
    "updated_at": "2026-07-14T10:00:00Z",
    "updated_by": 1
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | No valid session |
| `403 Forbidden` | `FORBIDDEN` | Missing `group:create` permission |
| `409 Conflict` | `DUPLICATE_NAME` | Group name already exists (case-insensitive) |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | Missing or invalid `name` (empty, too long) |

---

### GET `/api/v1/groups/{id}`

Get group detail including member and role lists.

**Required Permission:** `group:read`

**Success Response:** `200 OK`
```json
{
  "data": {
    "id": 42,
    "name": "QA Engineers",
    "description": "Quality assurance engineering team",
    "status": "ACTIVE",
    "member_count": 12,
    "role_count": 3,
    "created_at": "2026-07-14T10:00:00Z",
    "created_by": 1,
    "updated_at": "2026-07-14T10:00:00Z",
    "updated_by": 1,
    "members": [
      { "user_id": 7, "username": "jdoe", "fullname": "John Doe" },
      { "user_id": 9, "username": "asmith", "fullname": "Alice Smith" }
    ],
    "roles": [
      { "role_id": 2, "name": "Tester" },
      { "role_id": 5, "name": "Test Lead" }
    ]
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | No valid session |
| `403 Forbidden` | `FORBIDDEN` | Missing `group:read` permission |
| `404 Not Found` | `NOT_FOUND` | Group does not exist or is soft-deleted |

---

### PATCH `/api/v1/groups/{id}`

Update group fields (partial update).

**Required Permission:** `group:update`

**Request Headers:**

| Header | Value |
|--------|-------|
| Content-Type | `application/json` |

**Request Body (any subset of fields):**
```json
{
  "name": "QA Engineers - Senior",
  "description": "Senior quality assurance engineers",
  "status": "INACTIVE"
}
```

**Success Response:** `200 OK`
```json
{
  "data": {
    "id": 42,
    "name": "QA Engineers - Senior",
    "description": "Senior quality assurance engineers",
    "status": "INACTIVE",
    "member_count": 12,
    "created_at": "2026-07-14T10:00:00Z",
    "created_by": 1,
    "updated_at": "2026-07-14T12:30:00Z",
    "updated_by": 1
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | No valid session |
| `403 Forbidden` | `FORBIDDEN` | Missing `group:update` permission |
| `404 Not Found` | `NOT_FOUND` | Group does not exist or is soft-deleted |
| `409 Conflict` | `DUPLICATE_NAME` | Updated name conflicts with existing group |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | Invalid `status` value or malformed `name` |

---

### DELETE `/api/v1/groups/{id}`

Soft-delete a group. Sets `deleted_at` and `deleted_by`.

**Required Permission:** `group:delete`

**Success Response:** `204 No Content`

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | No valid session |
| `403 Forbidden` | `FORBIDDEN` | Missing `group:delete` permission |
| `404 Not Found` | `NOT_FOUND` | Group does not exist or is already soft-deleted |

---

### POST `/api/v1/groups/{id}/members`

Add or remove group members. Uses explicit add/remove arrays — both can be included
in the same request.

**Required Permission:** `group:update`

**Request Headers:**

| Header | Value |
|--------|-------|
| Content-Type | `application/json` |

**Request Body:**
```json
{
  "add": [7, 9],
  "remove": [3]
}
```

At least one of `add` or `remove` must be non-empty.

**Success Response:** `200 OK`
```json
{
  "data": {
    "added": 2,
    "removed": 1,
    "member_count": 13
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | No valid session |
| `403 Forbidden` | `FORBIDDEN` | Missing `group:update` permission |
| `404 Not Found` | `NOT_FOUND` | Group does not exist or is soft-deleted |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | Both `add` and `remove` empty; or one or more user IDs do not exist |
| `422 Unprocessable Entity` | `USER_NOT_FOUND` | One or more user IDs in `add` do not reference an active (non-deleted) user |

---

### POST `/api/v1/groups/{id}/roles`

Assign or remove roles from a group. Uses explicit add/remove arrays — both can be
included in the same request.

**Required Permission:** `group:update`

**Request Headers:**

| Header | Value |
|--------|-------|
| Content-Type | `application/json` |

**Request Body:**
```json
{
  "add": [2, 5],
  "remove": [1]
}
```

At least one of `add` or `remove` must be non-empty.

**Success Response:** `200 OK`
```json
{
  "data": {
    "added": 2,
    "removed": 1,
    "role_count": 3
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | No valid session |
| `403 Forbidden` | `FORBIDDEN` | Missing `group:update` permission |
| `404 Not Found` | `NOT_FOUND` | Group does not exist or is soft-deleted |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | Both `add` and `remove` empty; or one or more role IDs do not exist |
| `422 Unprocessable Entity` | `ROLE_NOT_FOUND` | One or more role IDs in `add` do not reference an existing (non-deleted) role |

---

## Data Model

### GROUPS table

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate PK |
| `name` | `VARCHAR(255)` | `NOT NULL`, `UNIQUE` | Unique, case-insensitive via unique index on `LOWER(name)` |
| `description` | `TEXT` | — | Optional, nullable |
| `status` | `VARCHAR(20)` | `NOT NULL`, `CHECK (status IN ('ACTIVE', 'INACTIVE'))` | Default `'ACTIVE'` |
| `created_by` | `BIGINT` | `NOT NULL`, `FK → users(id)` | Set on insert, immutable thereafter |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Set on insert, immutable thereafter |
| `updated_by` | `BIGINT` | `NOT NULL`, `FK → users(id)` | Set equal to `created_by` on insert; auto-maintained by `BEFORE UPDATE` trigger |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Set equal to `created_at` on insert; auto-maintained by `BEFORE UPDATE` trigger |
| `deleted_by` | `BIGINT` | `FK → users(id)` | Nullable. Set on soft-delete |
| `deleted_at` | `TIMESTAMPTZ` | — | Nullable. Set on soft-delete |

**DDL (PostgreSQL):**

```sql
CREATE TABLE groups (
    id          BIGINT          GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name        VARCHAR(255)    NOT NULL,
    description TEXT,
    status      VARCHAR(20)     NOT NULL DEFAULT 'ACTIVE'
                                CHECK (status IN ('ACTIVE', 'INACTIVE')),
    created_by  BIGINT          NOT NULL REFERENCES users(id),
    created_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_by  BIGINT          NOT NULL REFERENCES users(id),
    updated_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_by  BIGINT          REFERENCES users(id),
    deleted_at  TIMESTAMPTZ,
    CONSTRAINT uq_groups_name UNIQUE (name)
);

CREATE INDEX idx_groups_status ON groups(status) WHERE deleted_at IS NULL;
CREATE INDEX idx_groups_deleted_at ON groups(deleted_at);
```

### USER_GROUPS junction table

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `user_id` | `BIGINT` | `PK`, `FK → users(id)`, `NOT NULL` | Part of composite PK |
| `group_id` | `BIGINT` | `PK`, `FK → groups(id)`, `NOT NULL` | Part of composite PK |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Only audit column (per FR-52 junction table rule) |

**DDL:**

```sql
CREATE TABLE user_groups (
    user_id    BIGINT       NOT NULL REFERENCES users(id),
    group_id   BIGINT       NOT NULL REFERENCES groups(id),
    created_at TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, group_id)
);

-- Index for group-member lookups (list members of a group)
CREATE INDEX idx_user_groups_group_id ON user_groups(group_id);
```

### GROUP_ROLES junction table

The `group_roles` junction table is **owned by the `iam-roles` feature** (which defines its
migration and schema). It is documented here for reference:

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `group_id` | `BIGINT` | `PK`, `FK → groups(id) ON DELETE RESTRICT`, `NOT NULL` | Part of composite PK |
| `role_id` | `BIGINT` | `PK`, `FK → roles(id) ON DELETE RESTRICT`, `NOT NULL` | Part of composite PK |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Only audit column (per FR-52 junction table rule) |

**See `iam-roles/design.md` for the full table DDL and migration details.**

### Permission inheritance query pattern

The effective permissions for a user across direct roles and group roles are computed
at query time (no caching of permissions beyond request lifetime) using a UNION:

```sql
-- Direct role permissions
SELECT DISTINCT p.code
FROM role_permissions rp
JOIN permissions p ON p.id = rp.permission_id
JOIN user_roles ur ON ur.role_id = rp.role_id
WHERE ur.user_id = $1

UNION

-- Group role permissions
SELECT DISTINCT p.code
FROM role_permissions rp
JOIN permissions p ON p.id = rp.permission_id
JOIN group_roles gr ON gr.role_id = rp.role_id
JOIN user_groups ug ON ug.group_id = gr.group_id
WHERE ug.user_id = $1;
```

This query is used by the authorization middleware (not part of this feature) to
load the user's effective permission set on each authenticated request.

---

## Sequence

### Create Group Flow

1. Client sends `POST /api/v1/groups` with `{"name": "QA Engineers"}`.
2. Auth middleware validates the session (reused from `iam-auth`).
3. Authorization middleware checks the user has `group:create` permission.
4. Handler deserializes the request body and validates required fields.
5. Handler calls `GroupService::create(name, description, current_user_id)`.
6. `GroupService` checks name uniqueness via `GroupRepository::find_by_name(name)`.
7. If name exists, return `DuplicateName` error (→ handler returns `409 Conflict`).
8. `GroupService` creates a `Group` entity and calls `GroupRepository::save(group)`.
9. `GroupRepository` inserts into the `groups` table with `RETURNING id, created_at`.
10. Handler maps the saved `Group` to the response DTO and returns `201 Created`
    with `Location` header.

### Update Group Flow

1. Client sends `PATCH /api/v1/groups/{id}` with `{"name": "..."}`.
2. Auth and authorization middleware pass the request.
3. Handler deserializes the request body (partial — only provided fields are updated).
4. Handler calls `GroupService::update(id, fields, current_user_id)`.
5. `GroupService` loads the existing group via `GroupRepository::find_by_id(id)`.
6. If not found or soft-deleted, return `NotFound` error.
7. If name is being changed, check uniqueness via `GroupRepository::find_by_name(name)`
   — ensure the name doesn't belong to a different group.
8. `GroupService` applies the updates to the `Group` entity.
9. `GroupService` calls `GroupRepository::update(group)` — the DB trigger updates
   `updated_at`/`updated_by`.
10. Handler returns `200 OK` with the updated group.

### Manage Members Flow

1. Client sends `POST /api/v1/groups/{id}/members` with `{"add": [7, 9], "remove": [3]}`.
2. Auth and authorization middleware pass the request.
3. Handler validates: at least one of `add`/`remove` is non-empty, all user IDs exist.
4. Handler calls `GroupService::manage_members(group_id, add_ids, remove_ids)`.
5. `GroupService` verifies the group exists and is not soft-deleted.
6. For each user ID in `add`:
   - `MemberRepository::add(group_id, user_id)` — `INSERT ... ON CONFLICT DO NOTHING`.
7. For each user ID in `remove`:
   - `MemberRepository::remove(group_id, user_id)` — `DELETE FROM user_groups WHERE ...`.
8. Return `(added_count, removed_count)`.
9. Handler returns `200 OK`.

### Manage Roles Flow

1. Client sends `POST /api/v1/groups/{id}/roles` with `{"add": [2, 5], "remove": [1]}`.
2. Same pattern as Manage Members, using `GroupRoleRepository` and the `group_roles` table.
3. Handler returns `200 OK` with `(added_count, removed_count)`.

### Soft-Delete Group Flow

1. Client sends `DELETE /api/v1/groups/{id}`.
2. Auth and authorization middleware pass the request.
3. Handler calls `GroupService::soft_delete(group_id, current_user_id)`.
4. `GroupService` loads the group — if not found or already soft-deleted, return `NotFound`.
5. `GroupService` calls `GroupRepository::soft_delete(group)` which sets
   `deleted_at = NOW()` and `deleted_by = current_user_id`.
6. **No cascade delete** on `user_groups` or `group_roles` rows (per FR-53).
7. Handler returns `204 No Content`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `Group` | Domain (1) | Entity with fields: `id`, `name`, `description`, `status`, audit columns. Methods: `new()`, `update()`, `soft_delete()`, `restore()` (future). |
| `GroupStatus` | Domain (1) | Value object wrapping `ACTIVE`/`INACTIVE` with validation and `Display`/`FromStr` impls. |
| `GroupService` | Application (2) | Orchestrates all group use cases: create, update, soft-delete, manage members, manage roles. |
| `GroupRepository` | Application (2) | Interface (port) for group CRUD: `find_by_id`, `find_by_name`, `save`, `update`, `soft_delete`, `find_all` (paginated). |
| `MemberRepository` | Application (2) | Interface (port) for user-group membership: `add`, `remove`, `find_by_group`, `find_by_user`, `exists`. |
| `GroupRoleRepository` | Application (2) | Interface (port) for group-role assignments: `add`, `remove`, `find_by_group`, `find_by_role`, `exists`. |
| `ListGroupsHandler` | Adapters (3) | HTTP handler for `GET /api/v1/groups`. Validates query params, calls `GroupService`, returns paginated response. |
| `CreateGroupHandler` | Adapters (3) | HTTP handler for `POST /api/v1/groups`. Validates body, calls `GroupService`, returns `201`. |
| `GetGroupHandler` | Adapters (3) | HTTP handler for `GET /api/v1/groups/{id}`. Calls `GroupService`, returns detail with members+roles. |
| `UpdateGroupHandler` | Adapters (3) | HTTP handler for `PATCH /api/v1/groups/{id}`. Partial update, returns updated group. |
| `DeleteGroupHandler` | Adapters (3) | HTTP handler for `DELETE /api/v1/groups/{id}`. Soft-delete, returns `204`. |
| `ManageMembersHandler` | Adapters (3) | HTTP handler for `POST /api/v1/groups/{id}/members`. Add/remove members. |
| `ManageRolesHandler` | Adapters (3) | HTTP handler for `POST /api/v1/groups/{id}/roles`. Assign/remove roles. |
| `PostgresGroupRepository` | Infrastructure (4) | Implements `GroupRepository` using SQL queries against the `groups` table. |
| `PostgresMemberRepository` | Infrastructure (4) | Implements `MemberRepository` using SQL queries against the `user_groups` table. |
| `PostgresGroupRoleRepository` | Infrastructure (4) | Implements `GroupRoleRepository` using SQL queries against the `group_roles` table. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| HTTP router registration | Register all 7 group routes behind auth middleware. |
| Authorization middleware | Must support the 5 group permission codes: `group:read_list`, `group:create`, `group:read`, `group:update`, `group:delete`. |
| Seeded permissions | Add the 5 group permission codes to the `PERMISSIONS` seed data. |
| Permission inheritance query | The authorization middleware's permission-loading query must include the UNION with `user_groups` + `group_roles` pattern (see Data Model section). |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From auth middleware |
| Missing required permission | `403` | `FORBIDDEN` | INFO | From authorization middleware |
| Group not found / soft-deleted | `404` | `NOT_FOUND` | INFO | Detail or update on non-existent/soft-deleted group |
| Group name duplicate | `409` | `DUPLICATE_NAME` | WARN | Case-insensitive name collision |
| Invalid request body (validation) | `422` | `VALIDATION_ERROR` | INFO | Missing name, invalid status, empty add/remove, etc. |
| User IDs in `add` not found | `422` | `USER_NOT_FOUND` | INFO | Members endpoint — referencing non-existent users |
| Role IDs in `add` not found | `422` | `ROLE_NOT_FOUND` | INFO | Roles endpoint — referencing non-existent roles |
| Internal DB error | `500` | `INTERNAL_ERROR` | ERROR | Unexpected query failure |
| Redis unreachable (session check) | `503` | `SESSION_STORE_UNAVAILABLE` | ERROR | From auth middleware — all authenticated requests fail if Redis is down |

**Anti-patterns explicitly avoided:**

- **Do not return 404 for soft-deleted groups on list** — they are simply excluded from
  the list results. 404 is only returned when a client requests a specific soft-deleted group.
- **Do not cascade delete** junction rows when soft-deleting a group. The `user_groups`
  and `group_roles` rows remain intact (FR-53).
- **Do not expose internal error details** in production error responses (no SQL text,
  no stack traces).
- **Do not allow UPDATE on soft-deleted rows** — the repository layer rejects this
  with a clear error, except for the soft-delete and restore operations (FR-54c).
- **Sanitise user-controlled fields against XSS** — group `name` and `description` are
  user-controlled display fields. Strip HTML tags on input to prevent stored XSS. The
  presentation layer (Leptos SSR) provides output encoding, but input sanitisation adds
  defence-in-depth. Enforce maximum lengths: `name` 255 chars, `description` 2000 chars.
