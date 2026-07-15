# Design: Metadata Categories

## Architecture

The Metadata Categories feature follows Clean Architecture layering. Categories are
per-project metadata entities. All endpoints are nested under the project resource path
(`/api/v1/projects/{projectId}/categories`) and require both system RBAC permission and
project membership scope checks.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_categories     GET    /api/v1/projects/{pid}/categories       │   │
│  │  - create_category     POST   /api/v1/projects/{pid}/categories       │   │
│  │  - get_category        GET    /api/v1/projects/{pid}/categories/{id}  │   │
│  │  - update_category     PATCH  /api/v1/projects/{pid}/categories/{id}  │   │
│  │  - delete_category     DELETE /api/v1/projects/{pid}/categories/{id}  │   │
│  │  - select_categories   GET    /api/v1/projects/{pid}/categories/select│   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  CategoryService:                         │  │  - TestCategory (entity)│  │
│  │  - create_category                        │  └──────────────────────────┘  │
│  │  - list_categories                        │                                │
│  │  - get_category                           │                                │
│  │  - update_category                        │                                │
│  │  - delete_category                        │                                │
│  │  - select_categories                      │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - CategoryRepository (port)              │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlCategoryRepository  (implements CategoryRepository)              │   │
│  │  - Category seeder        (part of ConfigFileSeeder; seeds from YAML)  │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All category endpoints require an authenticated session (checked by `AuthMiddleware`).
2. `POST`, `PATCH`, and `DELETE` endpoints additionally require that the caller be an
   Owner or Editor of the project (or a System Admin), on top of the system permission
   check. `GET` endpoints require any project membership (or System Admin).
3. Create validates unique name and inserts within a single database transaction to
   prevent TOCTOU races between the duplicate check and the insert.
4. Update validates unique name (if name is being changed) and applies the update within a
   single database transaction to prevent TOCTOU races. Rejects updates on soft-deleted records.
5. Soft-delete checks that no non-deleted test cases reference the category and prevents
   deletion if any exist.
6. The `/select` endpoint returns a flat, unpaginated list of `{id, name}` pairs for
   dropdown components.

### Security Requirements

**Input sanitization:** Category `name` and `description` fields must be sanitized on
input (strip disallowed HTML tags) before storage. Output-encoding must be applied at the
presentation layer. See PRD 5.3 for the project-wide XSS prevention policy.

**CSRF protection:** All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must be
protected against CSRF. Session cookies must carry `SameSite=Lax` (or stricter).
`POST`/`PATCH`/`DELETE` requests with `Content-Type: application/json` must verify the
`Content-Type` header to block simple form-based CSRF attacks. Additional anti-CSRF
tokens should be considered for defense in depth.

**Authorization layering:** The system permission check and the project membership role
check are independent gates. Both must pass (or the caller must be a System Admin, who
implicitly holds all system permissions and bypasses all project membership checks).
Authorization checks are performed against live data on every request — permissions and
project membership are never cached in the session. If a user's role is changed
mid-session, the new role takes effect on their next request. A `403 Forbidden` response
must use a generic message that does not distinguish between "missing system permission"
and "wrong project role".

**Audit field visibility:** The `created_by` field is included in list and detail API
responses; `updated_by` is included in detail and create responses only (not in list or
select). Both fields are visible to all project members (including Viewers). This is
intentional: within a project, audit transparency (who created/modified a category) aids
collaboration. The fields carry numeric user IDs, not emails or names, limiting direct PII
exposure.

**Rate limiting:** All endpoints are protected by rate limiting configured at the
middleware layer. State-changing endpoints (POST/PATCH/DELETE) allow 30 req/min; read
endpoints (GET list/detail) allow 60 req/min; the select endpoint allows 120 req/min
due to frequent UI usage. See requirements.md Security Considerations for the full table.

**Referential integrity (delete):** The `CATEGORY_IN_USE` check must count only
non-soft-deleted test cases and must execute within the same database transaction as the
soft-delete. Use a `SELECT COUNT(*) FROM test_cases WHERE category_id = $1 AND deleted_at IS NULL`
query, and if the count is greater than zero, reject with `409 Conflict`.

---

## API Contract

### Common Error Response Format

All errors follow the standard project format:

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

### GET `/api/v1/projects/{projectId}/categories`

List categories in a project with pagination, filtering, and sorting.

**Required Permission:** `category:read_list`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Query Parameters:**

| Param | Type | Default | Max | Description |
|-------|------|---------|-----|-------------|
| `page` | integer | 1 | — | Page number (1-based) |
| `limit` | integer | 25 | 100 | Items per page |
| `search` | string | — | 255 | Case-insensitive substring match on `name` and `description` |
| `sort` | string | `name` | — | Sort field: `name`, `-name`, `created_at`, `-created_at` |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 1,
      "name": "Smoke Tests",
      "description": "Basic sanity checks run on every build",
      "created_by": 42,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_at": "2026-07-14T10:00:00Z"
    },
    {
      "id": 2,
      "name": "Regression",
      "description": "Full regression suite",
      "created_by": 42,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_at": "2026-07-15T08:30:00Z"
    }
  ],
  "meta": {
    "total": 12,
    "page": 1,
    "limit": 25
  }
}
```

**Notes:**
- Results are scoped to the given project and exclude soft-deleted categories
  (`WHERE project_id = $1 AND deleted_at IS NULL`).
- Default sort is `name` ascending (case-insensitive).
- `search` applies `ILIKE` on both `name` and `description` when provided;
  when absent, no filter is applied. The characters `%`, `_`, and `\` in the search value
  are escaped before building the ILIKE pattern (the user is searching for literal
  text, not writing SQL wildcards). Backslashes are doubled (`\\`) to prevent them
  from being consumed as PostgreSQL escape prefixes. The search value must not exceed 255 characters.
- The `sort` parameter accepts: `name` (ascending, default), `-name` (descending),
  `created_at` (oldest first), `-created_at` (newest first).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `category:read_list` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `422` | `VALIDATION_ERROR` | Invalid pagination or sort parameter |

---

### POST `/api/v1/projects/{projectId}/categories`

Create a new category within a project.

**Required Permission:** `category:create` AND project role Owner or Editor

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Request Body:**

```json
{
  "name": "Smoke Tests",
  "description": "Basic sanity checks run on every build"
}
```

| Field | Type | Required | Default | Constraints |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | — | 1–255 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input (HTML stripped); unique per project (case-insensitive) |
| `description` | string | No | `null` | Max 2000 characters; sanitized on input (HTML stripped). Empty string `""` stored as-is. Explicit `null` stores `NULL`. Omitting the field defaults to `null`. |

**Success Response:** `201 Created`

Headers: `Location: /api/v1/projects/42/categories/15`

```json
{
  "data": {
    "id": 15,
    "name": "Smoke Tests",
    "description": "Basic sanity checks run on every build",
    "project_id": 42,
    "created_by": 1,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 1,
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- `project_id` in the response mirrors the path parameter.
- `created_by` and `updated_by` are both set to the authenticated user's ID.
- The unique name check uses `LOWER(name) = LOWER($1)` and includes
  `WHERE project_id = $2 AND deleted_at IS NULL`.
- Soft-deleted categories with the same name do not block creation.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `category:create` permission or is not Owner/Editor of the project |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `409` | `DUPLICATE_CATEGORY_NAME` | Category name already exists in this project (case-insensitive) |
| `422` | `VALIDATION_ERROR` | Name empty, exceeds max length, description too long |

`409 Conflict` response body for duplicate name:

```json
{
  "error": {
    "code": "DUPLICATE_CATEGORY_NAME",
    "message": "A category with the name 'Smoke Tests' already exists in this project.",
    "details": [
      { "field": "name", "message": "Category name must be unique within the project" }
    ]
  }
}
```

---

### GET `/api/v1/projects/{projectId}/categories/{id}`

Get full detail of a single category.

**Required Permission:** `category:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Category ID |

**Success Response:** `200 OK`

```json
{
  "data": {
    "id": 15,
    "name": "Smoke Tests",
    "description": "Basic sanity checks run on every build",
    "project_id": 42,
    "created_by": 1,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 1,
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- `deleted_at` and `deleted_by` are never returned to the client.
- The category must belong to the specified project; if the category exists but belongs
  to a different project, `404 Not Found` is returned (same as non-existent).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `category:read` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, or category does not exist / is soft-deleted / belongs to a different project |

---

### PATCH `/api/v1/projects/{projectId}/categories/{id}`

Update a category's name and/or description.

**Required Permission:** `category:update` AND project role Owner or Editor

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Category ID |

**Request Body** (all fields optional; at least one required):

```json
{
  "name": "Updated Name",
  "description": "Updated description"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `name` | string | No | 1–255 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input (HTML stripped); unique per project (case-insensitive) |
| `description` | string | No | Max 2000 characters; sanitized on input (HTML stripped). Omitting the field preserves the current value. Sending `null` explicitly clears it (sets `NULL`). Sending `""` stores empty string. |

**Success Response:** `200 OK`

Response body is the updated category representation (same shape as GET detail).

**Notes:**
- At least one field must be provided (empty body returns `422`).
- `id`, `project_id`, `created_at`, and `created_by` are immutable.
- `deleted_at` and `deleted_by` cannot be modified through this endpoint.
- `updated_at` and `updated_by` are set automatically on change.
- The unique name check is only performed if `name` is provided and differs
  from the current value.
- Soft-deleted categories cannot be updated (per FR-54c).
- Unrecognised fields in the request body cause a `422 Unprocessable Entity` response
  (strict mode — rejects typos and unknown fields for defence-in-depth).
- The category must belong to the specified project; if it belongs to a different
  project, `404 Not Found` is returned.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `category:update` permission or is not Owner/Editor of the project |
| `404` | `NOT_FOUND` | Project or category does not exist, is soft-deleted, or category belongs to a different project |
| `409` | `DUPLICATE_CATEGORY_NAME` | Updated name conflicts with another category in the same project |
| `422` | `VALIDATION_ERROR` | No fields provided, invalid field values |

---

### DELETE `/api/v1/projects/{projectId}/categories/{id}`

Soft-delete a category.

**Required Permission:** `category:delete` AND project role Owner or Editor

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Category ID |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Sets `deleted_at = NOW()` and `deleted_by = <current_user_id>`.
- Before soft-deleting, checks that no non-deleted test cases reference this category
  (`SELECT COUNT(*) FROM test_cases WHERE category_id = $1 AND deleted_at IS NULL`).
  If any exist, returns `409 Conflict` with `CATEGORY_IN_USE`.
- The check and the soft-delete execute within the same database transaction to prevent
  TOCTOU races.
- Repeated DELETE on an already soft-deleted category returns `404`.
- The category must belong to the specified project.
- Existing test case references (FKs) remain intact after soft-delete; they continue
  to display the category name for historical/audit purposes.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `category:delete` permission or is not Owner/Editor of the project |
| `404` | `NOT_FOUND` | Project or category does not exist, is soft-deleted, or belongs to a different project |
| `409` | `CATEGORY_IN_USE` | One or more non-deleted test cases reference this category |

`409 Conflict` response body:

```json
{
  "error": {
    "code": "CATEGORY_IN_USE",
    "message": "Cannot delete category because it is referenced by 3 test case(s). Remove or reassign those test cases first.",
    "details": [
      { "field": "category_id", "message": "Referenced by 3 non-deleted test cases" }
    ]
  }
}
```

---

### GET `/api/v1/projects/{projectId}/categories/select`

Return a compact list of all non-deleted categories for dropdown/selection UI components.

**Required Permission:** `category:select`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Success Response:** `200 OK`

```json
{
  "data": [
    { "id": 1, "name": "API Tests" },
    { "id": 5, "name": "Login" },
    { "id": 3, "name": "Performance" },
    { "id": 2, "name": "Regression" },
    { "id": 4, "name": "Smoke Tests" }
  ]
}
```

**Notes:**
- Unpaginated: returns all non-deleted categories for the project. Capped at 500 results;
  if a project exceeds this limit, results are truncated and a warning is logged.
- Each entry contains only `id` and `name` (minimal payload for dropdown rendering).
- Results are ordered by `name` ascending (case-insensitive).
- Soft-deleted categories are excluded.
- Route registration order matters: the `/select` path must be registered **before**
  the `/{id}` path to prevent `select` from being interpreted as an ID.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `category:select` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |

---

## Data Model

### New Table: TEST_CATEGORIES

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `project_id` | `BIGINT` | `NOT NULL`, `REFERENCES projects(id) ON DELETE RESTRICT` | Each category belongs to exactly one project |
| `name` | `VARCHAR(255)` | `NOT NULL` | Category name; unique per project (case-insensitive); see constraint below |
| `description` | `TEXT` | | Nullable; max 2000 chars enforced at app layer and by `CHECK` constraint below |
| `created_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who created the category |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Last user who modified the category |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by `BEFORE UPDATE` trigger |
| `deleted_by` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Nullable; set on soft-delete |
| `deleted_at` | `TIMESTAMPTZ` | | Nullable; set on soft-delete |

**Constraints:**

```sql
-- Description length guard (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_categories
  ADD CONSTRAINT chk_test_categories_description
  CHECK (description IS NULL OR char_length(description) <= 2000);

-- Case-insensitive unique name per project (PostgreSQL)
-- Only enforced for non-deleted rows, allowing a soft-deleted row and a new row
-- with the same name to coexist.
CREATE UNIQUE INDEX uq_test_categories_name_project
  ON test_categories (project_id, LOWER(name))
  WHERE deleted_at IS NULL;

-- Foreign key indexes
CREATE INDEX idx_test_categories_project_id ON test_categories (project_id);
CREATE INDEX idx_test_categories_created_by ON test_categories (created_by);
CREATE INDEX idx_test_categories_updated_by ON test_categories (updated_by);
CREATE INDEX idx_test_categories_deleted_by ON test_categories (deleted_by);

-- Partial index for active categories (most queries filter out soft-deleted)
CREATE INDEX idx_test_categories_active ON test_categories (project_id, LOWER(name))
  WHERE deleted_at IS NULL;
```

**Design notes:**

- **Partial unique index** `uq_test_categories_name_project` enforces name uniqueness
  only among non-deleted rows. This allows creating a new category with the same name
  as a previously soft-deleted one.
- **`project_id` FK with `RESTRICT`** prevents deleting a project that has categories.
  Project soft-delete does not cascade (categories remain with their original
  `deleted_at IS NULL`; they are hidden because queries filter on
  `WHERE project.deleted_at IS NULL`).
- **`ON DELETE RESTRICT` on user FKs** (`created_by`, `updated_by`, `deleted_by`)
  prevents deleting a user who has created, updated, or deleted categories. This
  preserves audit trail integrity. Note that this means a user who performed any
  category operation can never be hard-deleted; offboarding must handle this gracefully.
- **`deleted_at` and `deleted_by`** follow the project-wide soft-delete convention:
  both set together on soft-delete, both `NULL` for active records.
- **No `status` column** -- categories do not have an ACTIVE/INACTIVE state. They are
  either present (non-deleted) or soft-deleted. The project `status` field controls
  visibility of all child metadata.

### BEFORE UPDATE Trigger

```sql
CREATE OR REPLACE FUNCTION trg_test_categories_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_test_categories_updated_at
  BEFORE UPDATE ON test_categories
  FOR EACH ROW
  EXECUTE FUNCTION trg_test_categories_updated_at();
```

The trigger ensures `updated_at` is always set to the current timestamp on every `UPDATE`,
regardless of which code path performs the update. The application layer does not need to
set `updated_at` explicitly.

### Relationship to Other Tables

```
PROJECTS ──< TEST_CATEGORIES ──< TEST_CASES
```

- `TEST_CATEGORIES.project_id` → `PROJECTS.id` (each category belongs to one project)
- `TEST_CASES.category_id` → `TEST_CATEGORIES.id` (test cases reference a category;
  defined in the `test-case-crud` spec). The FK must use `ON DELETE RESTRICT` to prevent
  accidental hard-deletion of a category that is still referenced.

---

## YAML Config Seed Format

Default categories are defined in the YAML configuration file loaded at application
startup. The file path is set via environment variable:

| Env Var | Default | Description |
|---------|---------|-------------|
| `CATEGORY_CONFIG_PATH` | `config/defaults.yaml` | Path to the YAML file containing default categories |

The `categories` section is a list of objects, each with `name` and optional
`description`.

```yaml
# config/defaults.yaml
categories:
  - name: "Smoke Tests"
    description: "Basic sanity checks run on every build"
  - name: "Regression"
    description: "Full regression test suite"
  - name: "Login"
    description: "Tests related to user authentication and login flows"
  - name: "Checkout"
    description: "E-commerce checkout flow tests"
  - name: "API Tests"
    description: "Direct API endpoint tests"
  - name: "Performance"
    description: "Load and stress testing"
  - name: "Security"
    description: "Security and penetration tests"
  - name: "Database"
    description: "Database migration and data integrity tests"
```

**Seeding behaviour:**
- The `ConfigFileSeeder` (defined in `project-crud` design) loads the YAML file at
  startup.
- On project creation, the seeder inserts each category in the `categories` list into
  the `TEST_CATEGORIES` table with the new `project_id`, `created_by` set to the project
  creator, and `created_at`/`updated_at` set to `NOW()`.
- Categories are inserted in the order they appear in the YAML file.
- If the `categories` section is missing or empty, no categories are seeded (this is not
  an error — projects can exist without default categories).
- If a YAML entry is missing a `name`, or the `name` is not a string, the seeder skips
  that entry with a warning log (does not fail the project creation transaction).
- If a YAML entry has a duplicate `name` within the same config file, only the first
  occurrence is inserted; duplicates are skipped with a warning log.
- If a YAML entry has a `description` that is not a string, it is treated as `null`.
- If a YAML entry has a `description` exceeding 2000 characters, it is truncated to 2000
  characters with a warning log.

---

## Sequence

### Create Category Flow

1. Client sends `POST /api/v1/projects/{projectId}/categories` with
   `{"name": "Smoke Tests", "description": "..."}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. HTTP handler validates the project exists and is not soft-deleted
   (call `ProjectRepository::find_by_id`). If not found → `404`.
4. Handler deserializes and validates the request body (name required, lengths).
5. Handler calls `CategoryService::create_category(project_id, cmd)`.
6. `CategoryService` checks the user has `category:create` system permission
   (via `AuthorizationService`).
7. `CategoryService` checks the user is an Owner or Editor of the project
   (via `ProjectMemberRepository`). If not, and not System Admin → `403`.
8. `CategoryService` begins a database transaction.
9. Within the transaction:
   a. Check for duplicate name: `CategoryRepository::find_by_name_in_project(project_id, name)`.
      If a non-deleted duplicate exists → roll back and return `409 Conflict`.
   b. Construct a `TestCategory` entity and call `CategoryRepository::save(category)`.
10. Transaction commits.
11. Handler constructs the `Location` header from the new category ID and returns
    `201 Created`.

### Soft-Delete Category Flow

1. Client sends `DELETE /api/v1/projects/{projectId}/categories/{id}` with session
   cookie.
2–4. Same as Create: validate session, project exists, category exists and belongs
     to project and is not soft-deleted.
5. Handler calls `CategoryService::delete_category(project_id, category_id, user_id)`.
6. `CategoryService` checks `category:delete` system permission.
7. `CategoryService` checks the user is Owner or Editor of the project.
8. `CategoryService` begins a database transaction.
9. Within the transaction:
   a. Query `SELECT COUNT(*) FROM test_cases WHERE category_id = $1 AND deleted_at IS NULL`.
   b. If count > 0 → roll back and return `409 Conflict` with `CATEGORY_IN_USE`.
   c. Call `CategoryRepository::soft_delete(category_id, user_id)` which sets
      `deleted_at = NOW()`, `deleted_by = current_user_id`.
10. Transaction commits.
11. Handler returns `204 No Content`.

### List Categories Flow

1. Client sends `GET /api/v1/projects/{projectId}/categories?page=1&limit=25&search=smoke&sort=name`
   with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler validates the project exists and is not soft-deleted.
4. Handler calls `CategoryService::list_categories(project_id, query)`.
5. `CategoryService` checks `category:read_list` system permission.
6. `CategoryService` checks the user is a member of the project (any role) or System Admin.
7. `CategoryService` calls `CategoryRepository::find_by_project(project_id, page, limit, search, sort)`.
8. Repository executes a parameterized query with `WHERE project_id = $1 AND deleted_at IS NULL`,
   `ILIKE` filters on name/description if `search` is provided, and `ORDER BY` based on `sort`.
9. Repository returns the paginated results and total count.
10. `CategoryService` returns the response DTO with `data` and `meta`.
11. Handler returns `200 OK`.

### Select Categories Flow

1. Client sends `GET /api/v1/projects/{projectId}/categories/select` with session cookie.
2–3. Same as List: validate session and project.
4. Handler calls `CategoryService::select_categories(project_id)`.
5. `CategoryService` checks `category:select` system permission.
6. `CategoryService` checks the user is a project member (any role) or System Admin.
7. `CategoryService` calls `CategoryRepository::find_all_active_by_project(project_id)`
   which selects only `id` and `name`, ordered by `LOWER(name)`.
8. Handler returns `200 OK` with the flat array.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestCategory` | Domain (1) | Entity: `id`, `project_id`, `name`, `description`, audit fields. Factory method `create(project_id, name, description, created_by)` performs domain validation (name not empty, description length). No ORM or framework imports. |
| `CategoryService` | Application (2) | Orchestrates all category use cases: `create_category`, `list_categories`, `get_category`, `update_category`, `delete_category`, `select_categories`. Each method checks the required system permission and project membership, then delegates to the repository. |
| `CategoryRepository` | Application (2) | Interface (port): `find_by_id(project_id, category_id)`, `find_by_name_in_project(project_id, name)`, `find_by_project(project_id, page, limit, search, sort)`, `save(category)`, `update(category)`, `soft_delete(category_id, user_id)`, `find_all_active_by_project(project_id)`, `count_referencing_test_cases(category_id)`. |
| `CategoryHandler` | Adapters (3) | HTTP handler with six methods (`list`, `create`, `get`, `update`, `delete`, `select`). Deserializes requests, calls `CategoryService`, serializes responses. The select handler is registered before the `/{id}` handler. |
| `SqlCategoryRepository` | Infrastructure (4) | Implements `CategoryRepository` using PostgreSQL. All queries include `WHERE deleted_at IS NULL` for active record filtering. Uses parameterized queries exclusively. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `ConfigFileSeeder` (Infrastructure, from `project-crud`) | Add `seed_categories(project_id, created_by, tx)` method that reads the `categories` section from the YAML config and inserts rows into `TEST_CATEGORIES`. |
| `AuthorizationService` (Application) | Add permission codes `category:create`, `category:read`, `category:read_list`, `category:update`, `category:delete`, `category:select` to the permission registry. Add role-based checks: Owner and Editor can create/update/delete; all members can read/read_list/select. |
| Seed migration (Infrastructure, from `iam-permissions`) | Add 6 new permission rows for `category:*` codes. |
| HTTP router registration | Register six new routes under `/api/v1/projects/{projectId}/categories/` — all require session auth. Route order: `/select` before `/{id}`. |

---

## Route Registration

```text
# Order matters: /select must be registered before /{id}
GET    /api/v1/projects/{projectId}/categories           → list
POST   /api/v1/projects/{projectId}/categories           → create
GET    /api/v1/projects/{projectId}/categories/select    → select   (static path)
GET    /api/v1/projects/{projectId}/categories/{id}      → get      (dynamic path)
PATCH  /api/v1/projects/{projectId}/categories/{id}      → update
DELETE /api/v1/projects/{projectId}/categories/{id}      → delete
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- System permission check (respective `category:*` code)
- Project membership check (Owner/Editor for mutations; any role for reads)

---

## New Permission Codes

These must be added to the `PERMISSIONS` table seed data. Use `INSERT ... ON CONFLICT (code)
DO NOTHING` and let the database assign IDs — do not hardcode numeric IDs in the migration:

| code | name |
|------|------|
| `category:create` | Create Category |
| `category:read` | Read Category |
| `category:read_list` | Read Category List |
| `category:update` | Update Category |
| `category:delete` | Delete Category |
| `category:select` | Select Category |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks required system permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks required project role | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before any category operation |
| Category not found, soft-deleted, or wrong project | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Duplicate category name in project | `409` | `DUPLICATE_CATEGORY_NAME` | INFO | Case-insensitive; only among non-deleted rows |
| Category still referenced by test cases | `409` | `CATEGORY_IN_USE` | INFO | Count of referencing test cases included in details |
| Validation error (bad request body) | `422` | `VALIDATION_ERROR` | INFO | Includes field-level details; unrecognised fields rejected in strict mode |
| Invalid pagination or sort parameter | `422` | `VALIDATION_ERROR` | INFO | page > 1000, limit > 100, invalid sort value, search > 255 chars |
| Whitespace-only name | `422` | `VALIDATION_ERROR` | INFO | Name must contain at least one non-whitespace character |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not reveal** whether a `404` is caused by a non-existent project, non-existent
  category, soft-deleted category, or wrong-project category (same message for all).
- **Do not return `400`** for business logic errors like duplicate names or in-use
  categories — use `409 Conflict` as specified.
- **Do not hard-delete** any record — all deletions are soft-deletes.
- **Do not allow UPDATE on soft-deleted rows** — the repository layer checks
  `deleted_at IS NULL` before applying mutations.
- **Do not omit `deleted_by`** — every soft-delete sets both `deleted_at` and
  `deleted_by`.
- **Do not cascade** soft-delete to referencing test cases — the `CATEGORY_IN_USE`
  check actively prevents deletion when references exist.
- **Do not use `GET` with a body** — all state changes use `POST`, `PATCH`, or `DELETE`.
