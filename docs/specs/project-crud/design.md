# Design: Project CRUD

## Architecture

The Project CRUD feature follows the same Clean Architecture layering as
[IAM Auth](../iam-auth/design.md). It spans all four layers:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_projects  GET    /api/v1/projects                             │   │
│  │  - create_project POST   /api/v1/projects                             │   │
│  │  - get_project    GET    /api/v1/projects/{id}                        │   │
│  │  - update_project PATCH  /api/v1/projects/{id}                        │   │
│  │  - delete_project DELETE /api/v1/projects/{id}                        │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  ProjectService:                          │  │  - Project (entity)     │  │
│  │  - create_project                         │  │  - ProjectMember (vo)   │  │
│  │  - list_projects                          │  └──────────────────────────┘  │
│  │  - get_project                            │                                │
│  │  - update_project                         │                                │
│  │  - delete_project                         │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - ProjectRepository (port)               │                                │
│  │  - MetadataSeeder (port — config-based)   │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlProjectRepository  (implements ProjectRepository)                │   │
│  │  - ConfigFileSeeder      (implements MetadataSeeder; reads YAML)       │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All project endpoints require an authenticated session (checked by `AuthMiddleware`).
2. List and detail endpoints also check the system RBAC permission, then scope the
   query to projects the user is a member of (or all projects for System Admin).
3. Create endpoint checks `project:create` permission, inserts the project, seeds
   metadata from the YAML config, and assigns the creator as Owner — all in one
   database transaction.
4. Update and delete endpoints check the required system permission AND that the
   user has the appropriate project role (Owner / Editor).
5. Soft-delete sets `deleted_at` + `deleted_by` without cascading to children.

### Security Requirements

**Input sanitization:** Project `name` and `description` fields must be sanitized
on input (strip disallowed HTML tags) before storage. Output-encoding must be
applied at the presentation layer. See PRD §5.3 for the project-wide XSS prevention
policy.

**CSRF protection:** All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must
be protected against CSRF. Session cookies must carry `SameSite=Lax` (or stricter).
`POST`/`PATCH`/`DELETE` requests with `Content-Type: application/json` must verify
the `Content-Type` header to block simple form-based CSRF attacks. Additional
anti-CSRF tokens should be considered for defense in depth.

**Authorization layering:** The system permission check and the project role check
are independent gates. Both must pass (or the caller must be a System Admin). A
`403 Forbidden` response must use a generic message that does not distinguish
between "missing system permission" and "wrong project role".

---

## API Contract

### Common Error Response Format

All errors follow the standard format established in `iam-auth`:

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable description",
    "details": [
      { "field": "field_name", "message": "specific validation message" }
    ]
  }
}
```

---

### GET `/api/v1/projects`

List projects accessible to the authenticated user.

**Required Permission:** `project:read_list`

**Query Parameters:**

| Param | Type | Default | Max | Description |
|-------|------|---------|-----|-------------|
| `page` | integer | 1 | — | Page number (1-based) |
| `limit` | integer | 25 | 100 | Items per page |
| `status` | string | — | — | Optional filter: `ACTIVE`, `INACTIVE` |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 1,
      "name": "My Project",
      "description": "Project description",
      "status": "ACTIVE",
      "member_count": 4,
      "created_by": 42,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_at": "2026-07-14T10:00:00Z"
    }
  ],
  "meta": {
    "total": 50,
    "page": 1,
    "limit": 25
  }
}
```

**Notes:**
- Results are scoped to projects where the user is a member (or all non-deleted
  projects for System Admin).
- Soft-deleted projects are excluded.
- Ordered by `created_at DESC`.
- `member_count` is a computed count of active project members (not soft-deleted).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `project:read_list` permission |

---

### POST `/api/v1/projects`

Create a new project with auto-seeded metadata.

**Required Permission:** `project:create`

**Request Body:**

```json
{
  "name": "My Project",
  "description": "A description of the project (optional)",
  "status": "ACTIVE"
}
```

| Field | Type | Required | Default | Constraints |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | — | 1–255 characters; sanitized on input (HTML stripped); unique (case-insensitive) |
| `description` | string | No | `null` | Max 2000 characters; sanitized on input (HTML stripped) |
| `status` | string | No | `"ACTIVE"` | Must be `ACTIVE` or `INACTIVE` |

**Success Response:** `201 Created`

Headers: `Location: /api/v1/projects/42`

```json
{
  "data": {
    "id": 42,
    "name": "My Project",
    "description": "A description of the project (optional)",
    "status": "ACTIVE",
    "created_by": 1,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Auto-seeding side effects (transactional):**
- The creator is added to `PROJECT_MEMBERS` with role `Owner`.
- If `status` is `ACTIVE`, the following are seeded into their respective per-project
  tables from YAML config:
  - Default test categories
  - Default test case templates
  - 5 priority levels (HIGHEST, HIGH, MEDIUM, LOW, LOWEST)
  - Default test plan types (ACCEPTANCE, FUNCTIONAL, API, INTEGRATION, PERFORMANCE,
    REGRESSION, SECURITY)
- If `status` is `INACTIVE`, metadata is still seeded (so it exists when the project
  is later activated), but child objects are hidden until activation.
- `created_by` and `updated_by` are both set to the authenticated user's ID.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `project:create` permission |
| `409` | `DUPLICATE_PROJECT_NAME` | Project name already exists (case-insensitive) |
| `422` | `VALIDATION_ERROR` | Name empty, exceeds max length, invalid status, description too long |
| `500` | `INTERNAL_ERROR` | YAML config file unreadable, malformed, or missing required sections (transaction rolled back) |
| `503` | `DATABASE_UNAVAILABLE` | Database connection failed |

---

### GET `/api/v1/projects/{id}`

Get project detail with member list.

**Required Permission:** `project:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Project ID |

**Success Response:** `200 OK`

```json
{
  "data": {
    "id": 42,
    "name": "My Project",
    "description": "A description of the project",
    "status": "ACTIVE",
    "created_by": 1,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 1,
    "updated_at": "2026-07-14T12:00:00Z",
    "members": [
      {
        "user_id": 1,
        "username": "jdoe",
        "fullname": "John Doe",
        "role": "Owner"
      },
      {
        "user_id": 2,
        "username": "asmith",
        "fullname": "Alice Smith",
        "role": "Editor"
      }
    ]
  }
}
```

**Notes:**
- `members` lists all non-deleted project members (hard delete on junction table;
  soft-deleted users are excluded).
- If the project is soft-deleted, `404 Not Found` is returned regardless of membership.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `project:read` permission or is not a member of the project |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |

---

### PATCH `/api/v1/projects/{id}`

Update project fields.

**Required Permission:** `project:update`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Project ID |

**Request Body** (all fields optional; at least one required):

```json
{
  "name": "Updated Name",
  "description": "Updated description",
  "status": "INACTIVE"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `name` | string | No | 1–255 characters; sanitized on input (HTML stripped); unique (case-insensitive) |
| `description` | string | No | Max 2000 characters; sanitized on input (HTML stripped) |
| `status` | string | No | Must be `ACTIVE` or `INACTIVE` |

**Success Response:** `200 OK`

Response body is the updated project representation (same shape as GET detail,
without the `members` list).

**Notes:**
- At least one field must be provided (empty body returns `422`).
- `created_at` and `created_by` are immutable and must not change.
- Changing `status` to `INACTIVE` hides all child objects from views but preserves data.
- Soft-deleted projects cannot be updated (per FR-54c).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `project:update` permission or is not an Owner/Editor of the project |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `409` | `DUPLICATE_PROJECT_NAME` | Updated name conflicts with another project |
| `422` | `VALIDATION_ERROR` | No fields provided, invalid field values |

---

### DELETE `/api/v1/projects/{id}`

Soft-delete a project.

**Required Permission:** `project:delete`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Project ID |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Sets `deleted_at = NOW()` and `deleted_by = <current_user_id>`.
- No cascade to child objects (test cases, plans, runs, executions, metadata, members).
- Child objects remain fully accessible in the database; they are hidden from views
  because their parent project is filtered as soft-deleted (queries join through the
  project and exclude deleted parents).
- Repeated DELETE on an already soft-deleted project returns `404`.
- Project members are preserved (not soft-deleted) so membership data survives a
  potential future restore.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `project:delete` permission or is not an Owner of the project |
| `404` | `NOT_FOUND` | Project does not exist or is already soft-deleted |

---

## Data Model

### New Tables

#### PROJECTS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `name` | `VARCHAR(255)` | `NOT NULL` | Case-insensitive unique; see constraint below |
| `description` | `TEXT` | | Nullable, max 2000 chars enforced at app layer |
| `status` | `VARCHAR(20)` | `NOT NULL`, `DEFAULT 'ACTIVE'` | `CHECK (status IN ('ACTIVE', 'INACTIVE'))` |
| `created_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Authenticated user who created |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Last modifier |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by `BEFORE UPDATE` trigger |
| `deleted_by` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Nullable; set on soft-delete |
| `deleted_at` | `TIMESTAMPTZ` | | Nullable; set on soft-delete |

**Constraints:**

```sql
-- Case-insensitive unique name (PostgreSQL)
CREATE UNIQUE INDEX uq_projects_name_lower ON projects (LOWER(name));

-- Check constraint on status
ALTER TABLE projects ADD CONSTRAINT chk_projects_status
  CHECK (status IN ('ACTIVE', 'INACTIVE'));

-- Active projects partial index (for list/detail queries)
CREATE INDEX idx_projects_active ON projects (id) WHERE deleted_at IS NULL;
```

---

#### PROJECT_MEMBERS (junction table)

The `PROJECT_MEMBERS` table is **owned by the `project-members` feature**.
See `specs/project-members/design.md` for the canonical schema definition.

This feature only uses the following columns when inserting the creator as Owner
during project creation:

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `user_id` | `BIGINT` | `PK`, `NOT NULL` | Creator user ID |
| `project_id` | `BIGINT` | `PK`, `NOT NULL` | FK to `projects(id)` |
| `role` | `VARCHAR(20)` | `NOT NULL` | Set to `'Owner'` on project creation |
| `created_by` | `BIGINT` | `NOT NULL` | Authenticated user who created the project |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | |

Hard delete only — no `deleted_at` / `deleted_by`. Members are removed by `DELETE`
(covered in `project-members` spec).

---

## Sequence

### Create Project Flow (with auto-seeding)

1. Client sends `POST /api/v1/projects` with `{"name": "...", "description": "...",
   "status": "ACTIVE"}`. Session cookie is included.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. HTTP handler deserializes and validates the request body (name required, valid
   status, field lengths).
4. Handler calls `ProjectService::create_project(cmd: CreateProjectCommand)`.
5. `ProjectService` checks that the user has `project:create` system permission
   (via `AuthorizationService`).
6. `ProjectService` constructs a `Project` entity and a `ProjectMember` value object
   (role = Owner, created_by = current user).
7. `ProjectService` begins a **database transaction**.
8. Within the transaction:
    a. `ProjectRepository::find_by_name(name)` checks for duplicates
       (case-insensitive lookup, inside the transaction to avoid TOCTOU race).
    b. If duplicate found, return `DuplicateProjectName` error and roll back.
    c. `ProjectRepository::save(project)` inserts the project row.
    d. `ProjectRepository::add_member(project_id, user_id, role)` inserts the Owner
       membership row.
    e. `MetadataSeeder::seed(project_id, config)` copies defaults from the YAML config
       into per-project metadata tables in the correct order:
       i.   Priority levels (5 records — HIGHEST, HIGH, MEDIUM, LOW, LOWEST)
       ii.  Test plan types (7 records — ACCEPTANCE, FUNCTIONAL, API, INTEGRATION,
            PERFORMANCE, REGRESSION, SECURITY)
       iii. Test categories (from config)
       iv.  Test case templates (from config)
9. Transaction commits. If any step fails, the entire transaction is rolled back.
10. `ProjectService` returns the created `Project` to the handler.
11. Handler constructs the `Location` header from project ID and returns `201 Created`.

### Soft-Delete Project Flow

1. Client sends `DELETE /api/v1/projects/{id}` with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler calls `ProjectService::delete_project(project_id, current_user_id)`.
4. `ProjectService` checks the user holds `project:delete` system permission.
5. `ProjectService` calls `ProjectRepository::find_by_id(project_id)`.
6. If not found or `deleted_at IS NOT NULL`, return `NotFound` error.
7. `ProjectService` calls `ProjectRepository::soft_delete(project_id, current_user_id)`
   which sets `deleted_at = NOW()`, `deleted_by = current_user_id`.
8. No cascade to children or members.
9. Handler returns `204 No Content`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `Project` | Domain (1) | Entity: `id`, `name`, `description`, `status`, audit fields. Factory method `create(name, description, status, created_by)` performs domain validation (name not empty, valid status). |
| `ProjectMember` | Domain (1) | Value object: `user_id`, `project_id`, `role`. Not a full entity — created on project creation and managed via the `project-members` spec. |
| `ProjectService` | Application (2) | Orchestrates all project use cases: `create_project`, `list_projects`, `get_project`, `update_project`, `delete_project`. Each method checks the required system permission first, then delegates to the repository. |
| `ProjectRepository` | Application (2) | Interface (port): `find_by_id`, `find_by_name` (case-insensitive), `save`, `update`, `soft_delete`, `find_by_user_id` (paginated, scoped to user membership), `add_member`, `get_members`, `get_member_count`. |
| `MetadataSeeder` | Application (2) | Interface (port): `seed(project_id)`. Called during project creation to copy default metadata from the YAML config into per-project tables. The implementation is in Infrastructure. |
| `ProjectHandler` | Adapters (3) | Actix-Web handler struct with five methods (`list`, `create`, `get`, `update`, `delete`). Deserializes requests, calls `ProjectService`, serializes responses. |
| `SqlProjectRepository` | Infrastructure (4) | Implements `ProjectRepository` using SQLx or Diesel. All queries include `WHERE deleted_at IS NULL` for active record filtering. Methods accepting `project_id` check existence before mutating. |
| `ConfigFileSeeder` | Infrastructure (4) | Implements `MetadataSeeder`. Loads the YAML config file (path from env var) at startup. On `seed(project_id)`, inserts default records into per-project metadata tables within the caller's transaction. |
| `ProjectMemberRepository` | Application (2) | Interface (port) for member-level operations that are delegated from ProjectService but will be fully owned by the `project-members` spec. Initially provides: `add_member`, `get_members_by_project`. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (Application) | Add `check_project_permission(user_id, project_id, permission_code)` — verifies the user has the system permission AND is a project member with the required role. For `project:update`: requires Owner or Editor role. For `project:delete`: requires Owner role. |
| HTTP router registration | Register five new project routes under `/api/v1/projects/` — all require session auth. |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks required system permission | `403` | `FORBIDDEN` | INFO | Generic message; do not reveal what permission is missing |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Duplicate project name | `409` | `DUPLICATE_PROJECT_NAME` | INFO | Case-insensitive comparison |
| Validation error (bad request body) | `422` | `VALIDATION_ERROR` | INFO | Includes field-level details |
| YAML config unreadable or malformed | `500` | `INTERNAL_ERROR` | ERROR | Transaction rolled back; project not created |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All project operations fail |

**Anti-patterns explicitly avoided:**

- **Do not reveal** whether a `404` is caused by a non-existent project or a
  soft-deleted one (same message).
- **Do not return `400`** for business logic errors like duplicate names — use
  `409 Conflict` as specified.
- **Do not cascade soft-delete** to children — parent deletion hides children via
  query filtering.
- **Do not allow UPDATE on soft-deleted rows** — the repository layer checks
  `deleted_at IS NULL` before applying mutations.
- **Do not omit `deleted_by`** — every application-performed soft-delete sets
  both `deleted_at` and `deleted_by`.
