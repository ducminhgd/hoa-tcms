# Design: Project Members

## Architecture

The Project Members feature follows Clean Architecture layers. It introduces a single
new junction table (`PROJECT_MEMBERS`) and spans all four layers:

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                      │
│  ┌───────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                    │   │
│  │  - list_members_handler   GET  /api/v1/projects/{id}/members      │   │
│  │  - manage_members_handler POST /api/v1/projects/{id}/members      │   │
│  │  - update_role_handler    PATCH /api/v1/projects/{id}/members/{uid}│   │
│  │                                                                   │   │
│  │  Auth Guard:                                                       │   │
│  │  - ProjectScopeGuard   Verifies user is project member             │   │
│  └───────────────────────┬───────────────────────────────────────────┘   │
│                          │ calls                                         │
│                          ▼                                               │
│  Application (Layer 2)                      ┌────────────────────────┐  │
│  ┌──────────────────────────────────────┐   │  Domain (L1)           │  │
│  │  ListProjectMembersUseCase           │   │  - ProjectMember       │  │
│  │  ManageProjectMembersUseCase         │   │    entity              │  │
│  │  UpdateMemberRoleUseCase             │   └────────────────────────┘  │
│  │  ProjectMemberRepository (interface)  │                               │
│  └───────────────────────┬────────────────┘                               │
│                          │ delegates to                                   │
│                          ▼                                               │
│  Infrastructure (Layer 4)                                                │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  - SqlProjectMemberRepository  (implements ProjectMemberRepository)│   │
│  │  - Migration: PROJECT_MEMBERS table creation                      │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────┘
```

**Flow summary (add member):**

1. A `POST /api/v1/projects/{id}/members` request arrives. The auth middleware
   verifies the session and attaches the authenticated user.
2. The authorization layer checks the caller holds `project:update` system permission.
3. The `ProjectScopeGuard` verifies the caller is a project member with Owner role.
4. The handler deserializes the request body and calls `ManageProjectMembersUseCase`.
5. The use case validates: user exists, user is ACTIVE, not already a member,
   role is a valid enum, and removal does not leave the project without an Owner.
6. The use case calls `SqlProjectMemberRepository` to insert/delete rows in the
   `PROJECT_MEMBERS` table.
7. The handler returns `200 OK` with a summary of changes.

### Security Requirements

**Input sanitization:** Fields displayed in the member list (`username`, `fullname`)
are sourced from the USERS table and must be sanitized on input at the user
creation/update boundary. Output-encoding must be applied at the presentation
layer. See PRD §5.3 for the project-wide XSS prevention policy.

**CSRF protection:** All state-changing endpoints (`POST`, `PATCH`) must be
protected against CSRF. Session cookies must carry `SameSite=Lax` (or stricter).
`POST`/`PATCH` requests with `Content-Type: application/json` must verify the
`Content-Type` header to block simple form-based CSRF attacks. Additional
anti-CSRF tokens should be considered for defense in depth.

**Authorization layering:** Two independent gates protect member management:
the system permission (`project:update`) and the project role (Owner). Both
must pass (or the caller must be a System Admin). A `403 Forbidden` response
must use a generic message that does not distinguish between "missing system
permission" and "wrong project role", preventing project existence
enumeration.

**Last-Owner transaction safety:** The last-Owner check (count current Owners,
subtract those being removed/downgraded) must execute within the same database
transaction as the DELETE or UPDATE operation and use `SELECT ... FOR UPDATE`
or an equivalent row-level lock on the `project_members` rows to prevent
race conditions from concurrent requests.

---

## API Contract

### GET `/api/v1/projects/{project_id}/members`

List all members of a project.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `project_id` | integer | Project ID |

**Required Permission:** `project:read`

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "user_id": 10,
      "username": "jdoe",
      "fullname": "John Doe",
      "role": "Owner",
      "created_at": "2026-07-14T08:00:00Z"
    },
    {
      "user_id": 42,
      "username": "asmith",
      "fullname": "Alice Smith",
      "role": "Editor",
      "created_at": "2026-07-14T09:30:00Z"
    }
  ]
}
```

The list is ordered by `created_at` ascending (oldest first).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | Missing or invalid session |
| `403 Forbidden` | `FORBIDDEN` | User lacks `project:read` permission or is not a member of the project |
| `404 Not Found` | `NOT_FOUND` | Project does not exist |

---

### POST `/api/v1/projects/{project_id}/members`

Add and/or remove members in a single bulk operation. Removals are processed before
additions.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `project_id` | integer | Project ID |

**Required Permission:** `project:update` AND user must be an Owner of the project
(or hold `project:update` system permission).

**Request Body:**

```json
{
  "add": [
    { "user_id": 100, "role": "Editor" },
    { "user_id": 101, "role": "Viewer" }
  ],
  "remove": [42, 55]
}
```

Both `add` and `remove` arrays are optional. At least one must be non-empty.

**Success Response:** `200 OK`

```json
{
  "data": {
    "added": [
      { "user_id": 100, "role": "Editor" },
      { "user_id": 101, "role": "Viewer" }
    ],
    "removed": [42, 55]
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | Missing or invalid session |
| `403 Forbidden` | `FORBIDDEN` | User lacks `project:update` permission or is not an Owner |
| `404 Not Found` | `NOT_FOUND` | Project does not exist |
| `409 Conflict` | `DUPLICATE_MEMBER` | A user_id in `add` is already a member |
| `409 Conflict` | `LAST_OWNER_REMOVAL` | Removing the user would leave the project without an Owner |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | Both `add` and `remove` arrays empty; user_id not found; user is INACTIVE; invalid role value |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | A user_id in `add` belongs to a deactivated (INACTIVE) or soft-deleted user |

**Error body format (standard):**

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid request body",
    "details": [
      { "field": "add[0].user_id", "message": "User 999 does not exist" },
      { "field": "add[1].role", "message": "must be one of: Owner, Editor, Contributor, Viewer" }
    ]
  }
}
```

---

### PATCH `/api/v1/projects/{project_id}/members/{user_id}`

Change a member's role.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `project_id` | integer | Project ID |
| `user_id` | integer | User ID of the member whose role is to be changed |

**Required Permission:** `project:update` AND user must be an Owner of the project
(or hold `project:update` system permission).

**Request Body:**

```json
{
  "role": "Contributor"
}
```

**Success Response:** `200 OK`

```json
{
  "data": {
    "user_id": 42,
    "username": "asmith",
    "fullname": "Alice Smith",
    "role": "Contributor",
    "created_at": "2026-07-14T09:30:00Z"
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | Missing or invalid session |
| `403 Forbidden` | `FORBIDDEN` | User lacks `project:update` permission or is not an Owner |
| `404 Not Found` | `NOT_FOUND` | Project does not exist, or user_id is not a member of the project |
| `409 Conflict` | `LAST_OWNER_DOWNGRADE` | Changing the role away from Owner would leave the project without an Owner |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | `role` is missing or not one of the valid values |

---

## Data Model

### New table: PROJECT_MEMBERS

A junction table linking users to projects with a role. Hard-delete only (no
`deleted_at` / `deleted_by`). Because the `role` column is mutable, the table
carries both `created_at`/`created_by` (who added the member) and
`updated_at`/`updated_by` (for role change tracking) — an exception to the
junction-table rule in FR-54a, justified by the mutable role.

```sql
CREATE TABLE project_members (
    user_id    BIGINT       NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    project_id BIGINT       NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    role       VARCHAR(20)  NOT NULL CHECK (role IN ('Owner', 'Editor', 'Contributor', 'Viewer')),
    created_by BIGINT       NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_by BIGINT       REFERENCES users(id) ON DELETE RESTRICT,
    updated_at TIMESTAMPTZ  NOT NULL DEFAULT NOW(),

    -- Composite primary key
    CONSTRAINT pk_project_members PRIMARY KEY (user_id, project_id)
);

-- Index for lookups by project (list members, scope check)
CREATE INDEX idx_project_members_project_id ON project_members(project_id);

-- Index for lookups by user (all projects a user belongs to)
CREATE INDEX idx_project_members_user_id ON project_members(user_id);

-- Partial index for active membership checks (scope check performance)
CREATE INDEX idx_project_members_active ON project_members(project_id, user_id)
    INCLUDE (role);
```

**Design notes:**

- **Composite PK** `(user_id, project_id)` — enforces uniqueness at the database
  level. Prevents duplicate membership entries.
- **No surrogate `id` column** — the composite PK is the natural key for this
  junction. Queries filter by both user and project.
- **`role` is `VARCHAR` with a `CHECK` constraint** — not a DB-level `ENUM`.
  Follows the project convention to avoid expensive enum alterations.
- **`created_by`** — records who added the member. Set once at creation time
  and immutable thereafter.
- **`updated_at` and `updated_by`** — added to track role changes even though
  this is a junction table, because the role field is mutable. The `created_at`
  column records when the membership was first established and is immutable.
- **`updated_by` is nullable** — for the initial insert, `updated_by` is set to
  `created_by`. On subsequent role changes, it records who performed the change.
- **Partial index** `idx_project_members_active` — covering index for the most
  frequent query pattern: "is user X a member of project Y, and what is their
  role?" The `INCLUDE (role)` avoids a heap fetch for the role value.
- **Hard delete** — when a member is removed, the row is `DELETE`d entirely.
  No soft-delete on junction tables (per FR-52).

### Role definitions

| Role | Can manage members | Can modify project & objects | Can modify objects only | Can share | Read-only |
|------|-------------------|------------------------------|------------------------|-----------|-----------|
| Owner | Yes | Yes | Yes | Yes | — |
| Editor | No | Yes | Yes | Yes | — |
| Contributor | No | No | Yes | No | — |
| Viewer | No | No | No | No | Yes |

**Note:** The system permission check (`project:update`) is the primary gate for
member management. If a user holds the `project:update` system permission (e.g.,
System Admin), they bypass the Owner role check. For non-Admin users, the
membership role must be Owner.

---

## Sequence

### Add Members Flow

1. Client sends `POST /api/v1/projects/{id}/members` with
   `{"add": [{"user_id": 100, "role": "Editor"}]}`.
2. Auth middleware validates session (see auth flow in `iam-auth` spec).
3. Authorization middleware checks the caller has `project:update` system permission.
4. `ProjectScopeGuard` fetches the caller's membership record for this project.
   - If no record found → `403 Forbidden`.
   - If record found but role is not Owner and caller lacks `project:update` →
     `403 Forbidden`.
5. Handler deserializes request body and validates: `add` array entries, `role`
   values, at least one operation present, no duplicate user_ids across `add` and
   `remove`.
6. Handler calls `ManageProjectMembersUseCase::execute(project_id, caller_id, add_list, remove_list)`.
7. Use case validates each `add` entry:
   - User exists (`UserRepository::find_by_id`) → if not found, collect validation error.
   - User is ACTIVE and not deleted → if INACTIVE or deleted, collect validation error.
   - User is not already a member → query `ProjectMemberRepository::find_by_id`.
   - If any validation errors, return `422 Unprocessable Entity` with all errors
     aggregated (fail-fast for the entire request).
8. Use case validates `remove` entries:
   - For each user_id in `remove`, verify they are a member (skip if not — idempotent).
   - Check that removing those users does not leave the project with zero Owners:
     count of current Owners minus count of Owners being removed must be ≥ 1.
   - If last Owner would be removed, return `409 Conflict`.
9. Use case executes removals first: call `ProjectMemberRepository::delete(project_id, user_id)`
   for each user in the `remove` list.
10. Use case executes additions: call `ProjectMemberRepository::save(project_id, user_id, role, caller_id)`
    for each entry in the `add` list.
11. Use case wraps steps 9–10 in a database transaction.
12. Handler returns `200 OK` with the summary of added and removed members.

### Remove Members Flow (last-Owner protection)

1–6. Same as Add Members Flow.
7. Use case counts current Owners for this project.
8. Use case counts how many of the users in the `remove` list are Owners.
9. If `current_owner_count - removed_owner_count < 1`, return `409 Conflict`.
10. Proceed with deletion.
11. If the caller removes themselves from the project, the response is still `200 OK`
    (the caller's session remains valid but subsequent requests to project-scoped
    endpoints will fail with `403 Forbidden`).

### Change Role Flow

1. Client sends `PATCH /api/v1/projects/{project_id}/members/{user_id}` with
   `{"role": "Contributor"}`.
2–4. Same authorization checks as Add Members Flow.
5. Handler deserializes request body and validates `role`.
6. Handler calls `UpdateMemberRoleUseCase::execute(project_id, target_user_id, new_role, caller_id)`.
7. Use case fetches the current membership for `target_user_id` on `project_id`.
   - If not found → `404 Not Found`.
8. Use case checks last-Owner constraint:
   - If the current role is `Owner` and the new role is not `Owner`:
     count total Owners. If only 1 (the target), return `409 Conflict`.
9. Use case updates the role: call `ProjectMemberRepository::update_role(project_id, user_id, new_role, caller_id)`.
10. Handler returns `200 OK` with the updated membership record.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `ProjectMember` | Domain (1) | Entity representing a project membership: `user_id`, `project_id`, `role`, `created_by`, `created_at`, `updated_at`, `updated_by`. Provides `is_owner()`, `is_editor()`, `is_contributor()`, `is_viewer()` convenience methods. Constructor validates role against the allowed enum set. |
| `ProjectMemberRepository` | Application (2) | Interface (port) for project membership persistence. Methods: `find_by_project(project_id) -> Vec<ProjectMember>`, `find_by_id(project_id, user_id) -> Option<ProjectMember>`, `save(project_id, user_id, role, created_by)`, `delete(project_id, user_id)`, `update_role(project_id, user_id, new_role, updated_by)`, `count_owners(project_id) -> u64`, `exists(project_id, user_id) -> bool`. |
| `ListProjectMembersUseCase` | Application (2) | Orchestrates listing of members for a project. Fetches the member list with user details (username, fullname) via a join query. |
| `ManageProjectMembersUseCase` | Application (2) | Orchestrates bulk add + remove in a single transaction. Validates each entry independently and aggregates all errors before returning. Enforces last-Owner constraint. |
| `UpdateMemberRoleUseCase` | Application (2) | Orchestrates role change for a single member. Enforces last-Owner constraint when downgrading an Owner. |
| `ListMembersHandler` | Adapters (3) | HTTP handler for `GET /api/v1/projects/{project_id}/members`. Validates project exists, calls `ListProjectMembersUseCase`, returns member list. |
| `ManageMembersHandler` | Adapters (3) | HTTP handler for `POST /api/v1/projects/{project_id}/members`. Validates request body, calls `ManageProjectMembersUseCase`, returns summary. |
| `UpdateMemberRoleHandler` | Adapters (3) | HTTP handler for `PATCH /api/v1/projects/{project_id}/members/{user_id}`. Validates request body, calls `UpdateMemberRoleUseCase`, returns updated member. |
| `ProjectScopeGuard` | Adapters (3) | Guard/middleware component that checks whether a user is a member of a project (used by the broader authorization middleware defined in `auth-project-scope` feature). Fetches membership from `ProjectMemberRepository` and attaches the effective role to the request context. This guard is consumed by the `auth-project-scope` feature, not by this feature directly. |
| `SqlProjectMemberRepository` | Infrastructure (4) | Implements `ProjectMemberRepository` using PostgreSQL. All queries use parameterized placeholders (`$1`, `$2`, ...). Partial index `idx_project_members_active` is used for membership existence checks. |

### Modified Existing Components (from `auth-project-scope` feature)

| Component | Change |
|-----------|--------|
| Project scope authorization guard | Consume `ProjectMemberRepository` to look up user's project membership and role during scope check. |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Project does not exist | `404` | `NOT_FOUND` | INFO | Returned by all endpoints if project_id is invalid or soft-deleted |
| User lacks `project:read` permission | `403` | `FORBIDDEN` | INFO | For list endpoint |
| User lacks `project:update` permission | `403` | `FORBIDDEN` | INFO | For add/remove/change endpoints |
| User is not an Owner / has no project membership | `403` | `FORBIDDEN` | INFO | For add/remove/change endpoints when caller lacks system `project:update` |
| User ID in `add` does not exist | `422` | `VALIDATION_ERROR` | INFO | Field-level detail: `add[0].user_id` |
| User ID in `add` is INACTIVE or deleted | `422` | `VALIDATION_ERROR` | INFO | Deactivated users cannot be members |
| User ID in `add` is already a member | `409` | `DUPLICATE_MEMBER` | INFO | Includes user_id in response |
| Invalid role value | `422` | `VALIDATION_ERROR` | INFO | Role must be one of: Owner, Editor, Contributor, Viewer |
| Both `add` and `remove` empty | `422` | `VALIDATION_ERROR` | INFO | At least one operation required |
| Removing the last Owner | `409` | `LAST_OWNER_REMOVAL` | INFO | Project must always have at least one Owner |
| Downgrading the last Owner away from Owner | `409` | `LAST_OWNER_DOWNGRADE` | INFO | Same constraint as removal |
| Target user is not a member (role change) | `404` | `NOT_FOUND` | INFO | For PATCH endpoint when user_id is not a member |
| User ID in `remove` is not a member | — | — | — | Silently skipped (idempotent) |

**Idempotency note:** The `POST` endpoint is not idempotent by nature (adding
members changes state), but the `remove` operation treats non-existent members
as idempotent skips without error. Duplicate `add` entries for the same user_id
are rejected with `409 Conflict`.
