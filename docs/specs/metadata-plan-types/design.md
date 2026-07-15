# Design: Metadata Plan Types

## Architecture

The Metadata Plan Types feature follows Clean Architecture layering. Plan types are
per-project metadata entities. The plan type set is fixed (7 levels per project) and seeded
from a YAML configuration file at project creation. Users cannot create or delete plan
types; they can only update the `description` field via `PATCH`. All endpoints are nested
under the project resource path (`/api/v1/projects/{projectId}/plan-types`) and require
both system RBAC permission and project membership scope checks.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_plan_types    GET    /api/v1/projects/{pid}/plan-types        │   │
│  │  - get_plan_type       GET    /api/v1/projects/{pid}/plan-types/{id}  │   │
│  │  - update_plan_type    PATCH  /api/v1/projects/{pid}/plan-types/{id}  │   │
│  │  - select_plan_types   GET    /api/v1/projects/{pid}/plan-types/select│   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌───────────────────────────┐ │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)         │ │
│  │  PlanTypeService:                        │  │  - TestPlanType (entity)  │ │
│  │  - list_plan_types                       │  └───────────────────────────┘ │
│  │  - get_plan_type                         │                                │
│  │  - update_plan_type                      │                                │
│  │  - select_plan_types                     │                                │
│  │                                          │                                │
│  │  Interfaces:                             │                                │
│  │  - PlanTypeRepository (port)             │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlPlanTypeRepository  (implements PlanTypeRepository)              │   │
│  │  - Plan type seeder        (part of ConfigFileSeeder; seeds 7 rows)    │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All plan type endpoints require an authenticated session (checked by `AuthMiddleware`).
2. `PATCH` endpoints additionally require that the caller be an Owner or Editor of the
   project (or a System Admin), on top of the system permission check. `GET` endpoints
   require any project membership (or System Admin).
3. Update validates that only `description` is provided (strict mode -- `name` is rejected
   with `422`), and applies the update within a single database transaction.
4. The `/select` endpoint returns a flat, unpaginated list of `{id, name}` tuples for
   dropdown components.
5. No create or delete endpoints exist -- the 7 plan types are seeded on project creation
   and are permanent for the project lifecycle.

### Security Requirements

**Input sanitization:** Plan type `description` fields must be sanitized on input (strip
disallowed HTML tags) before storage. Output-encoding must be applied at the presentation
layer. See PRD 5.3 for the project-wide XSS prevention policy. The `name` field is never
writable by users, so only `description` requires sanitization.

**CSRF protection:** The state-changing endpoint (`PATCH`) must be protected against CSRF.
Session cookies must carry `SameSite=Lax` (or stricter). `PATCH` requests with
`Content-Type: application/json` must verify the `Content-Type` header to block simple
form-based CSRF attacks. Additional anti-CSRF tokens should be considered for defense in
depth.

**Authorization layering:** The system permission check and the project membership role
check are independent gates. Both must pass (or the caller must be a System Admin, who
implicitly holds all system permissions and bypasses all project membership checks).
Authorization checks are performed against live data on every request -- permissions and
project membership are never cached in the session. A `403 Forbidden` response must use a
generic message that does not distinguish between "missing system permission" and "wrong
project role".

**Audit field visibility:** The `created_by` field is included in list and detail API
responses; `updated_by` is included in detail responses only (not in list or select). Both
fields are visible to all project members (including Viewers). This is intentional: within
a project, audit transparency aids collaboration. The fields carry numeric user IDs, not
emails or names, limiting direct PII exposure.

**Rate limiting:** All endpoints are protected by rate limiting configured at the
middleware layer. The state-changing endpoint (PATCH) allows 30 req/min; read endpoints
(GET list/detail) allow 60 req/min; the select endpoint allows 120 req/min due to
frequent UI usage. See requirements.md Security Considerations for the full table.

**Immutability enforcement:** The `name` field is immutable after seeding. Enforcement
occurs at three layers:

1. **HTTP handler:** Reject requests containing `name` with `422`.
2. **Service:** `UpdatePlanTypeCommand` DTO only exposes `description`.
3. **Database trigger:** `BEFORE UPDATE` trigger raises an exception if `name` is modified.
   See Data Model section for the trigger definition.

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

### GET `/api/v1/projects/{projectId}/plan-types`

List plan types in a project with pagination, filtering, and sorting.

**Required Permission:** `plan_type:read_list`

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
| `sort` | string | `name` | — | Sort field: `name`, `-name` |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 1,
      "name": "ACCEPTANCE",
      "description": "User acceptance testing to validate business requirements",
      "created_by": 42,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_at": "2026-07-14T10:00:00Z"
    },
    {
      "id": 2,
      "name": "API",
      "description": "API-level contract and integration tests",
      "created_by": 42,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_at": "2026-07-15T08:30:00Z"
    }
  ],
  "meta": {
    "total": 7,
    "page": 1,
    "limit": 25
  }
}
```

**Notes:**
- Results are scoped to the given project (`WHERE project_id = $1`).
- Default sort is `name` ascending (case-insensitive alphabetical order).
- `search` applies `ILIKE` on both `name` and `description` when provided; when absent, no
  filter is applied. Leading and trailing whitespace in the search value is trimmed. An
  all-whitespace search string after trimming is treated as "no filter." The characters
  `%`, `_`, and `\` in the search value are escaped before building the ILIKE pattern.
  Backslashes are doubled (`\\`) to prevent them from being consumed as PostgreSQL escape
  prefixes.
- The `sort` parameter accepts: `name` (ascending, default), `-name` (descending).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `plan_type:read_list` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `422` | `VALIDATION_ERROR` | Invalid pagination or sort parameter |

---

### GET `/api/v1/projects/{projectId}/plan-types/{id}`

Get full detail of a single plan type.

**Required Permission:** `plan_type:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Plan Type ID |

**Success Response:** `200 OK`

```json
{
  "data": {
    "id": 1,
    "name": "ACCEPTANCE",
    "description": "User acceptance testing to validate business requirements",
    "project_id": 42,
    "created_by": 1,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 1,
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- The plan type must belong to the specified project; if the plan type exists but belongs
  to a different project, `404 Not Found` is returned (same as non-existent).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `plan_type:read` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, or plan type does not exist / belongs to a different project |

---

### PATCH `/api/v1/projects/{projectId}/plan-types/{id}`

Update a plan type's `description`. The `name` field is immutable.

**Required Permission:** `plan_type:update` AND project role Owner or Editor

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Plan Type ID |

**Request Body** (`description` is the only accepted field; at least one field is required
-- since `description` is the only field, this means `description` is required):

```json
{
  "description": "Updated description for this plan type"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `description` | string | Yes (per above) | Max 2000 characters; sanitized on input (HTML stripped). Sending `null` explicitly clears it (sets `NULL`). Sending `""` stores empty string. |

**Success Response:** `200 OK`

Response body is the updated plan type representation (same shape as GET detail).

**Notes:**
- `description` must be provided (empty body returns `422`).
- `name` is not accepted in the request body; sending it returns `422` (strict mode --
  rejects instead of silently ignoring immutable fields).
- `id`, `project_id`, `name`, `created_at`, and `created_by` are immutable.
- `updated_at` and `updated_by` are set automatically on change.
- Unrecognised fields in the request body cause a `422 Unprocessable Entity` response
  (strict mode -- rejects typos and unknown fields for defence-in-depth).
- The plan type must belong to the specified project; if it belongs to a different
  project, `404 Not Found` is returned.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `plan_type:update` permission or is not Owner/Editor of the project |
| `404` | `NOT_FOUND` | Project or plan type does not exist, is soft-deleted, or plan type belongs to a different project |
| `422` | `VALIDATION_ERROR` | No `description` field provided, `description` too long, request includes `name`, or other validation error |

`422` response body when `name` is included:

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Request body contains fields that are not updatable.",
    "details": [
      { "field": "name", "message": "The 'name' field is immutable and cannot be updated." }
    ]
  }
}
```

---

### GET `/api/v1/projects/{projectId}/plan-types/select`

Return a compact list of all plan type levels for dropdown/selection UI components.

**Required Permission:** `plan_type:select`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Success Response:** `200 OK`

```json
{
  "data": [
    { "id": 1, "name": "ACCEPTANCE" },
    { "id": 3, "name": "API" },
    { "id": 4, "name": "FUNCTIONAL" },
    { "id": 5, "name": "INTEGRATION" },
    { "id": 6, "name": "PERFORMANCE" },
    { "id": 7, "name": "REGRESSION" },
    { "id": 2, "name": "SECURITY" }
  ]
}
```

**Notes:**
- Unpaginated: returns all plan types for the project (always 7 items).
- Each entry contains `id` and `name` (minimal payload for dropdown rendering).
- Results are ordered by `name` ascending (case-insensitive alphabetical order).
- Route registration order matters: the `/select` path must be registered **before** the
  `/{id}` path to prevent `select` from being interpreted as an ID.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `plan_type:select` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |

---

## Data Model

### New Table: TEST_PLAN_TYPES

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `project_id` | `BIGINT` | `NOT NULL`, `REFERENCES projects(id) ON DELETE RESTRICT` | Each plan type belongs to exactly one project |
| `name` | `VARCHAR(50)` | `NOT NULL` | Plan type name (ACCEPTANCE, FUNCTIONAL, API, INTEGRATION, PERFORMANCE, REGRESSION, SECURITY); immutable after insert. Unique per project; see constraint below. |
| `description` | `TEXT` | | Nullable; max 2000 chars enforced at app layer and by `CHECK` constraint below |
| `created_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who created the project (seeder sets this to the project creator) |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Last user who modified the plan type |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by `BEFORE UPDATE` trigger |

**Constraints:**

```sql
-- Description length guard (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_plan_types
  ADD CONSTRAINT chk_test_plan_types_description
  CHECK (description IS NULL OR char_length(description) <= 2000);

-- Unique name per project
CREATE UNIQUE INDEX uq_test_plan_types_name_project
  ON test_plan_types (project_id, LOWER(name));

-- Foreign key indexes
CREATE INDEX idx_test_plan_types_project_id ON test_plan_types (project_id);
CREATE INDEX idx_test_plan_types_created_by ON test_plan_types (created_by);
CREATE INDEX idx_test_plan_types_updated_by ON test_plan_types (updated_by);

-- Composite index for the most common query pattern: all plan types for a project ordered by name
CREATE INDEX idx_test_plan_types_project_name ON test_plan_types (project_id, LOWER(name));
```

**Design notes:**

- **No `rank` column.** Unlike priorities, plan types have no inherent ordering. They are
  sorted alphabetically by `name` in all list and select responses.
- **No soft-delete columns.** Unlike categories and templates, plan types are permanent and
  never soft-deleted. There are no `deleted_at` or `deleted_by` columns. The fixed set of
  7 plan types persists for the entire project lifecycle, and there is no user action that
  removes them.
- **No partial unique index.** Since plan types are never soft-deleted, the unique
  constraint on `(project_id, LOWER(name))` is unconditional (no `WHERE` clause), unlike
  categories which use partial indexes to exclude deleted rows.
- **`name` max 50 chars** is a tighter bound than categories (255 chars) because plan type
  names are fixed, short, and never edited.
- **Unique constraint on `name`** per project prevents duplicate seeding and maintains data
  integrity. Each project has exactly one plan type named "ACCEPTANCE", exactly one named
  "FUNCTIONAL", etc.
- **`project_id` FK with `RESTRICT`** prevents deleting a project that has plan types.
- **`ON DELETE RESTRICT` on user FKs** (`created_by`, `updated_by`) prevents deleting a
  user who has created or updated plan types. This preserves audit trail integrity.
- **Composite index `idx_test_plan_types_project_name`** on `(project_id, LOWER(name))`
  supports the most common query pattern: listing all plan types for a project sorted
  alphabetically.

### BEFORE UPDATE Trigger -- Immutability Guard

```sql
CREATE OR REPLACE FUNCTION trg_test_plan_types_immutable_fields()
RETURNS TRIGGER AS $$
BEGIN
  -- Reject changes to name (case-insensitive comparison handles collation differences)
  IF NEW.name IS DISTINCT FROM OLD.name THEN
    RAISE EXCEPTION 'The name field of test_plan_types is immutable and cannot be updated. (id=%)',
      OLD.id;
  END IF;

  -- Reject changes to project_id (plan types are permanently scoped)
  IF NEW.project_id IS DISTINCT FROM OLD.project_id THEN
    RAISE EXCEPTION 'The project_id field of test_plan_types is immutable and cannot be updated. (id=%)',
      OLD.id;
  END IF;

  -- Reject changes to created_at (set once on INSERT)
  IF NEW.created_at IS DISTINCT FROM OLD.created_at THEN
    RAISE EXCEPTION 'The created_at field of test_plan_types is immutable and cannot be updated. (id=%)',
      OLD.id;
  END IF;

  -- Reject changes to created_by (set once on INSERT)
  IF NEW.created_by IS DISTINCT FROM OLD.created_by THEN
    RAISE EXCEPTION 'The created_by field of test_plan_types is immutable and cannot be updated. (id=%)',
      OLD.id;
  END IF;

  -- Auto-set updated_at on any UPDATE
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_test_plan_types_before_update
  BEFORE UPDATE ON test_plan_types
  FOR EACH ROW
  EXECUTE FUNCTION trg_test_plan_types_immutable_fields();
```

This trigger serves three purposes:

1. **Immutability guard:** Raises an exception if `UPDATE` attempts to modify `name` or
   `project_id`. This is the last line of defence below the application and handler layers.
2. **`updated_at` maintenance:** Automatically sets `updated_at = NOW()` on every `UPDATE`,
   so the application does not need to set it explicitly.
3. **Self-documenting:** The trigger body documents which fields are immutable directly in
   the database schema.

### Relationship to Other Tables

```
PROJECTS ──< TEST_PLAN_TYPES ──< TEST_PLANS
```

- `TEST_PLAN_TYPES.project_id` → `PROJECTS.id` (each plan type belongs to one project)
- `TEST_PLANS.plan_type_id` → `TEST_PLAN_TYPES.id` (test plans reference a plan type;
  defined in the `test-plan-crud` spec). The FK must use `ON DELETE RESTRICT` to prevent
  accidental hard-deletion of a plan type that is still referenced.

---

## YAML Config Seed Format

Default plan types are defined in the YAML configuration file loaded at application
startup. The file path is set via environment variable:

| Env Var | Default | Description |
|---------|---------|-------------|
| `PLAN_TYPE_CONFIG_PATH` | `config/defaults.yaml` | Path to the YAML file containing default plan types |

The `plan_types` section must contain exactly 7 entries, one per plan type level, each with
`name` and optional `description`.

```yaml
# config/defaults.yaml
plan_types:
  - name: "ACCEPTANCE"
    description: "User acceptance testing to validate that the system meets business requirements and stakeholder expectations."
  - name: "FUNCTIONAL"
    description: "Functional testing to verify that each feature works according to its specifications."
  - name: "API"
    description: "API-level contract and integration tests to validate endpoint behaviour, request/response schemas, and error handling."
  - name: "INTEGRATION"
    description: "Integration testing to verify that multiple components or services work together correctly."
  - name: "PERFORMANCE"
    description: "Performance and load testing to measure system responsiveness, throughput, and stability under load."
  - name: "REGRESSION"
    description: "Regression testing to ensure that recent changes have not broken existing functionality."
  - name: "SECURITY"
    description: "Security testing to identify vulnerabilities, validate authentication/authorization, and ensure data protection."
```

**Seeding behaviour:**
- The `ConfigFileSeeder` (defined in `project-crud` design) loads the YAML file at startup.
- On project creation, the seeder inserts each plan type in the `plan_types` list into the
  `TEST_PLAN_TYPES` table with the new `project_id`, `created_by` set to the project
  creator, and `created_at`/`updated_at` set to `NOW()`.
- Plan types are inserted in the order they appear in the YAML file.
- If the `plan_types` section is missing or does not contain exactly 7 entries, the project
  creation transaction is rolled back with an error. The fixed set of 7 plan types is a
  hard requirement for every project.
- If a YAML entry is missing a `name`, or the `name` is not a string, the seeder rolls
  back the transaction with an error.
- If the `name` values do not form the exact set `{ACCEPTANCE, FUNCTIONAL, API, INTEGRATION,
  PERFORMANCE, REGRESSION, SECURITY}`, the seeder rolls back the project creation
  transaction with an error. The set of 7 plan type names is fixed; if customisation is
  desired, update the YAML config and restart the application before creating new projects.
- If a YAML entry has a `description` that is not a string, it is treated as `null`.
- If a YAML entry has a `description` exceeding 2000 characters, it is truncated to 2000
  characters with a warning log.
- Duplicate `name` (case-insensitive) within the plan_types list causes the seeder to roll
  back the transaction with an error.

---

## Sequence

### List Plan Types Flow

1. Client sends `GET /api/v1/projects/{projectId}/plan-types?page=1&limit=25&sort=name`
   with session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates the project exists and is not soft-deleted
   (call `ProjectRepository::find_by_id`). If not found → `404`.
4. Handler validates pagination and sort parameters.
5. Handler calls `PlanTypeService::list_plan_types(project_id, query)`.
6. `PlanTypeService` checks the user has `plan_type:read_list` system permission
   (via `AuthorizationService`).
7. `PlanTypeService` checks the user is a member of the project (any role) or System Admin
   (via `ProjectMemberRepository`).
8. `PlanTypeService` calls `PlanTypeRepository::find_by_project(project_id, page, limit, search, sort)`.
9. Repository executes a parameterized query with `WHERE project_id = $1`,
   `ILIKE` filters on name/description if `search` is provided, and `ORDER BY` based on
   `sort` (validated against a whitelist).
10. Repository returns the paginated results and total count.
11. `PlanTypeService` returns the response DTO with `data` and `meta`.
12. Handler returns `200 OK`.

### Get Plan Type Detail Flow

1. Client sends `GET /api/v1/projects/{projectId}/plan-types/{id}` with session cookie.
2-3. Same as List: validate session and project.
4. Handler calls `PlanTypeService::get_plan_type(project_id, plan_type_id, current_user_id)`.
5. `PlanTypeService` checks `plan_type:read` system permission.
6. `PlanTypeService` checks the user is a project member (any role) or System Admin.
7. `PlanTypeService` calls `PlanTypeRepository::find_by_id(project_id, plan_type_id)`.
8. Repository returns the plan type entity (scoped to project). If not found or belongs to
   a different project → returns `None`.
9. `PlanTypeService` returns the response DTO, or raises `PlanTypeNotFoundError` if `None`.
10. Handler maps `PlanTypeNotFoundError` to `404` and returns `200 OK` on success.

### Update Plan Type Flow

1. Client sends `PATCH /api/v1/projects/{projectId}/plan-types/{id}` with
   `{"description": "Updated description"}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. HTTP handler validates the project exists and is not soft-deleted
   (call `ProjectRepository::find_by_id`). If not found → `404`.
4. Handler deserializes and validates the request body:
   - If body is empty → `422`.
   - If `name` is present → `422` (immutable field).
   - If unrecognised fields are present → `422` (strict mode).
   - Validate `description` length (max 2000 chars).
5. Handler calls `PlanTypeService::update_plan_type(project_id, plan_type_id, cmd, current_user_id)`.
6. `PlanTypeService` checks the user has `plan_type:update` system permission
   (via `AuthorizationService`).
7. `PlanTypeService` checks the user is an Owner or Editor of the project
   (via `ProjectMemberRepository`). If not, and not System Admin → `403`.
8. `PlanTypeService` begins a database transaction.
9. Within the transaction:
   a. `SELECT ... FOR UPDATE` on the plan type row to lock it and verify it exists,
      belongs to the project, and is not deleted. If not found → roll back and
      return `404 Not Found`.
   b. Entity validates and applies the description change via
      `plan_type.apply_description_update(new_description)`.
   c. `PlanTypeService` calls `PlanTypeRepository::update(plan_type)` which uses
      `UPDATE ... RETURNING *`.
10. Transaction commits.
11. Handler returns `200 OK` with the updated plan type representation.

### Select Plan Types Flow

1. Client sends `GET /api/v1/projects/{projectId}/plan-types/select` with session cookie.
2-3. Same as List: validate session and project.
4. Handler calls `PlanTypeService::select_plan_types(project_id, current_user_id)`.
5. `PlanTypeService` checks `plan_type:select` system permission.
6. `PlanTypeService` checks the user is a project member (any role) or System Admin.
7. `PlanTypeService` calls `PlanTypeRepository::find_all_by_project(project_id)`
   which selects `id` and `name`, ordered by `name` ascending (case-insensitive).
8. Handler returns `200 OK` with the flat array.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestPlanType` | Domain (1) | Entity: `id`, `project_id`, `name`, `description`, audit fields. Factory method `create(project_id, name, description, created_by)` performs domain validation (name not empty, description length). Method `apply_description_update(description: Option<String>)` validates and returns a new entity with the updated description. No ORM or framework imports. |
| `PlanTypeService` | Application (2) | Orchestrates all plan type use cases: `list_plan_types`, `get_plan_type`, `update_plan_type`, `select_plan_types`. Each method checks the required system permission and project membership, then delegates to the repository. |
| `PlanTypeRepository` | Application (2) | Interface (port): `find_by_id(project_id, plan_type_id)`, `find_by_project(project_id, page, limit, search, sort)`, `update(plan_type)`, `find_all_by_project(project_id)`. No `save` (create is via seeder), no `soft_delete` (plan types are permanent). |
| `PlanTypeHandler` | Adapters (3) | HTTP handler with four methods (`list`, `get`, `update`, `select`). Deserializes requests, calls `PlanTypeService`, serializes responses. Rejects `name` in PATCH body with `422`. The select handler is registered before the `/{id}` handler. |
| `SqlPlanTypeRepository` | Infrastructure (4) | Implements `PlanTypeRepository` using PostgreSQL. Uses parameterized queries exclusively. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `ConfigFileSeeder` (Infrastructure, from `project-crud`) | Add `seed_plan_types(project_id, created_by, tx)` method that reads the `plan_types` section from the YAML config, validates exactly 7 entries with unique names, and inserts rows into `TEST_PLAN_TYPES`. |
| `AuthorizationService` (Application) | Add permission codes `plan_type:read`, `plan_type:read_list`, `plan_type:update`, `plan_type:select` to the permission registry. Add role-based checks: Owner and Editor can update; all members can read/read_list/select. |
| Seed migration (Infrastructure, from `iam-permissions`) | Add 4 new permission rows for `plan_type:*` codes. |
| HTTP router registration | Register four new routes under `/api/v1/projects/{projectId}/plan-types/` -- all require session auth. Route order: `/select` before `/{id}`. |

---

## Route Registration

```text
# Order matters: /select must be registered before /{id}
GET    /api/v1/projects/{projectId}/plan-types           → list
GET    /api/v1/projects/{projectId}/plan-types/select    → select   (static path)
GET    /api/v1/projects/{projectId}/plan-types/{id}      → get      (dynamic path)
PATCH  /api/v1/projects/{projectId}/plan-types/{id}      → update
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- System permission check (respective `plan_type:*` code)
- Project membership check (Owner/Editor for mutations; any role for reads)

---

## New Permission Codes

These must be added to the `PERMISSIONS` table seed data. Use `INSERT ... ON CONFLICT (code)
DO NOTHING` and let the database assign IDs -- do not hardcode numeric IDs in the migration:

| code | name |
|------|------|
| `plan_type:read` | Read Plan Type |
| `plan_type:read_list` | Read Plan Type List |
| `plan_type:update` | Update Plan Type |
| `plan_type:select` | Select Plan Type |

Note: No `create` or `delete` permission codes since plan types are fixed and seeded on
project creation. There are no user-facing create or delete operations.

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks required system permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks required project role | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before any plan type operation |
| Plan type not found or wrong project | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Validation error (bad request body) | `422` | `VALIDATION_ERROR` | INFO | Includes field-level details; unrecognised fields rejected in strict mode |
| Attempt to update `name` | `422` | `VALIDATION_ERROR` | INFO | Immutable field; handled at handler layer |
| Invalid pagination or sort parameter | `422` | `VALIDATION_ERROR` | INFO | page < 1 or page > 1000, limit < 1 or limit > 100, invalid sort value, search > 255 chars, non-integer page/limit |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Per-endpoint limits; returns `Retry-After` header |
| Empty request body on PATCH | `422` | `VALIDATION_ERROR` | INFO | At least the `description` field is required |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not reveal** whether a `404` is caused by a non-existent project, non-existent plan
  type, or wrong-project plan type (same message for all).
- **Do not return `400`** for business logic errors -- use `422 Unprocessable Entity` for
  validation errors.
- **Do not allow create or delete** of plan types (no `POST` or `DELETE` endpoints).
- **Do not silently ignore** `name` in a PATCH request -- reject with `422` so the caller
  knows the field is immutable.
- **Do not use `PUT`** for partial updates -- use `PATCH`.
- **Do not use `GET` with a body** -- all state changes use `PATCH`.
- **Do not hard-delete** any record -- plan types are permanent for the project lifecycle.
- **Do not omit `updated_by`** -- every update sets both `updated_at` (via trigger) and
  `updated_by` (via application).
