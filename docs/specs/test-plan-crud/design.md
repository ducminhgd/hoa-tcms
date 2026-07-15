# Design: Test Plan CRUD

## Architecture

The Test Plan CRUD feature follows Clean Architecture layering. Test plans are multi-project
entities managed through a top-level resource path (not nested under projects, because they
span multiple projects). All endpoints are under `/api/v1/test-plans/` and require both
system RBAC permission and project membership scope checks across all linked projects.

```
+-----------------------------------------------------------------------------+
|  Adapters (Layer 3)                                                          |
|  +-----------------------------------------------------------------------+   |
|  |  HTTP Handlers:                                                        |   |
|  |  - list_test_plans     GET    /api/v1/test-plans                      |   |
|  |  - create_test_plan    POST   /api/v1/test-plans                      |   |
|  |  - get_test_plan       GET    /api/v1/test-plans/{id}                 |   |
|  |  - update_test_plan    PATCH  /api/v1/test-plans/{id}                 |   |
|  |  - delete_test_plan    DELETE /api/v1/test-plans/{id}                 |   |
|  |  - select_test_plans   GET    /api/v1/test-plans/select               |   |
|  +----------------------+------------------------------------------------+   |
|                         | calls                                               |
|                         v                                                     |
|  Application (Layer 2)                         +--------------------------+  |
|  +------------------------------------------+  |  Domain (Layer 1)        |  |
|  |  TestPlanService:                         |  |  - TestPlan (entity)    |  |
|  |  - create_test_plan                       |  |  - TestPlanStatus (enum)|  |
|  |  - list_test_plans                        |  +--------------------------+  |
|  |  - get_test_plan                          |                                |
|  |  - update_test_plan                       |                                |
|  |  - delete_test_plan                       |                                |
|  |  - select_test_plans                      |                                |
|  |                                           |                                |
|  |  Interfaces:                              |                                |
|  |  - TestPlanRepository (port)              |                                |
|  +----------+-------------------------------+                                |
|             | delegates to                                                    |
|             v                                                                 |
|  Infrastructure (Layer 4)                                                     |
|  +-----------------------------------------------------------------------+   |
|  |  - SqlTestPlanRepository  (implements TestPlanRepository)              |   |
|  +-----------------------------------------------------------------------+   |
+-----------------------------------------------------------------------------+
```

**Flow summary:**

1. All test plan endpoints require an authenticated session (checked by `AuthMiddleware`).
2. `POST`, `PATCH`, and `DELETE` endpoints require the caller to be an Owner or Editor in at
   least one of the test plan's linked projects (or a System Admin), on top of the system
   permission check.
3. `GET` endpoints require the caller to be a member of at least one linked project (any
   role) for detail; list and select automatically filter to test plans where the user has
   membership.
4. Create validates unique name, project IDs (at least one, all non-deleted), and plan type
   FK within a single database transaction to prevent TOCTOU races. Project associations are
   inserted into the junction table within the same transaction.
5. Update validates unique name (if changing), plan type FK, project set (if changing), and
   status transitions within a single database transaction. Project associations are
   sync-replaced atomically.
6. Soft-delete requires no referential integrity checks in Phase 1 (test plans are not yet
   referenced by other entities). Project associations are preserved.

### Security Requirements

**Input sanitization:** Test plan `name`, `version`, and `description` fields must be
sanitized on input (strip disallowed HTML tags) before storage. Output-encoding must be
applied at the presentation layer. See PRD 5.3 for the project-wide XSS prevention policy.

**CSRF protection:** All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must be
protected against CSRF. Session cookies must carry `SameSite=Lax` (or stricter).
`POST`/`PATCH`/`DELETE` requests with `Content-Type: application/json` must verify the
`Content-Type` header to block simple form-based CSRF attacks. Additional anti-CSRF tokens
should be considered for defense in depth.

**Authorization layering:** Two independent authorization gates apply to mutations:

1. System permission check (e.g., `test_plan:update`)
2. Multi-project membership and role check: the user must be an Owner or Editor in at least
   one of the test plan's linked projects (or a System Admin)

For reads, a third gate applies:

3. Visibility gate: the user must be a member of at least one linked project (or System
   Admin). If not, `404 Not Found` is returned (indistinguishable from non-existent).

For list and select, the SQL query inherently filters to test plans where the user has
membership in at least one linked project, so no post-query filter is needed.

All gates must pass (or the caller must be a System Admin, who implicitly holds all system
permissions and bypasses all membership checks). Authorization checks are performed against
live data on every request -- permissions and project membership are never cached in the
session. A `403 Forbidden` response must use a generic message that does not distinguish
between "missing system permission" and "wrong project role."

**Audit field visibility:** The `created_by` field is included in list and detail API
responses; `updated_by` is included in detail and create responses only (not in list or
select). Both fields are visible to all project members. The fields carry numeric user IDs,
not emails or names, limiting direct PII exposure.

**Rate limiting:** All endpoints are protected by rate limiting configured at the middleware
layer. State-changing endpoints (POST/PATCH/DELETE) allow 30 req/min; read endpoints
(GET list/detail) allow 60 req/min; the select endpoint allows 120 req/min due to frequent
UI usage. See requirements.md Security Considerations for the full table.

**Status transition enforcement:** Status changes are validated against a state machine
(see Status Transitions section below). The client cannot bypass transition rules. Invalid
transitions return `422` with `INVALID_STATUS_TRANSITION`.

**Plan type cross-project validation:** Before inserting or updating a test plan with a
`plan_type_id`, the system must verify that the referenced plan type exists, is not
soft-deleted, and its `project_id` is among the test plan's linked projects. This prevents
cross-project data injection.

```sql
-- Validate plan_type_id
SELECT 1 FROM test_plan_types
WHERE id = $1 AND project_id = ANY($2) AND deleted_at IS NULL;
```

This check executes within the same database transaction as the insert/update.

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

### GET `/api/v1/test-plans`

List test plans accessible to the authenticated user with pagination, filtering, and
sorting.

**Required Permission:** `test_plan:read_list`

**Query Parameters:**

| Param | Type | Default | Max | Description |
|-------|------|---------|-----|-------------|
| `page` | integer | 1 | -- | Page number (1-based) |
| `limit` | integer | 25 | 100 | Items per page |
| `status` | string | -- | -- | Filter by status: `TODO`, `IN_PROGRESS`, `DONE`, `CANCEL` |
| `plan_type_id` | integer | -- | -- | Filter by plan type ID |
| `project_id` | integer | -- | -- | Filter by linked project ID (user must be a member of that project) |
| `search` | string | -- | 255 | Case-insensitive substring match on `name` and `description` |
| `sort` | string | `-updated_at` | -- | Sort field: `-updated_at`, `updated_at`, `name`, `-name`, `status`, `-status` |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 15,
      "name": "Sprint 12 Regression",
      "version": "2.1",
      "plan_type_id": 5,
      "plan_type_name": "REGRESSION",
      "status": "IN_PROGRESS",
      "project_ids": [1, 3],
      "created_by": 42,
      "created_at": "2026-07-10T09:00:00Z",
      "updated_at": "2026-07-15T08:30:00Z"
    },
    {
      "id": 12,
      "name": "API v2 Integration Tests",
      "version": "1.0",
      "plan_type_id": 2,
      "plan_type_name": "API",
      "status": "TODO",
      "project_ids": [3],
      "created_by": 42,
      "created_at": "2026-07-08T14:00:00Z",
      "updated_at": "2026-07-08T14:00:00Z"
    }
  ],
  "meta": {
    "total": 45,
    "page": 1,
    "limit": 25
  }
}
```

**Notes:**
- Results are scoped to test plans where the authenticated user is a member of at least one
  linked project (`WHERE EXISTS (SELECT 1 FROM test_plan_projects tpp JOIN project_members pm
  ON tpp.project_id = pm.project_id WHERE tpp.plan_id = tp.id AND pm.user_id = $1)`).
- Soft-deleted test plans are excluded (`WHERE tp.deleted_at IS NULL`).
- Default sort is `-updated_at` (most recently updated first).
- `plan_type_name` is resolved via JOIN on `TEST_PLAN_TYPES`.
- `project_ids` is aggregated from the junction table.
- List response excludes `description`, `updated_by`, `deleted_at`, and `deleted_by` to keep
  the payload compact.
- `search` applies `ILIKE` on both `name` and `description` when provided; when absent, no
  filter is applied. The search value is escaped: first `\` is doubled to `\\`, then `%` is
  escaped to `\%`, then `_` is escaped to `\_`. Leading and trailing whitespace is trimmed;
  an all-whitespace search is treated as "no filter."
- The `sort` parameter is validated against a whitelist.
- System Admin sees all non-deleted test plans (bypasses membership filter).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_plan:read_list` permission |
| `422` | `VALIDATION_ERROR` | Invalid pagination, sort parameter, or filter value |

---

### POST `/api/v1/test-plans`

Create a new test plan.

**Required Permission:** `test_plan:create` AND project role Owner or Editor in at least one
specified project

**Request Body:**

```json
{
  "name": "Sprint 12 Regression",
  "project_ids": [1, 3],
  "plan_type_id": 5,
  "version": "2.1",
  "description": "Full regression test plan for Sprint 12 covering both frontend and backend projects."
}
```

| Field | Type | Required | Default | Constraints |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | -- | 1-255 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input (HTML stripped); unique globally (case-insensitive) among non-deleted test plans |
| `project_ids` | array of integer | Yes | -- | Non-empty; each ID must reference a non-deleted project; duplicates are deduplicated silently |
| `plan_type_id` | integer | Yes | -- | Must reference a non-deleted plan type whose project is in `project_ids` |
| `version` | string | No | `"1.0"` | Max 50 characters; leading/trailing whitespace trimmed; sanitized on input (HTML stripped) |
| `description` | string | No | `null` | Max 10000 characters; sanitized on input (HTML stripped). Empty string `""` stored as-is. Explicit `null` stores `NULL`. |

**Success Response:** `201 Created`

Headers: `Location: /api/v1/test-plans/15`

```json
{
  "data": {
    "id": 15,
    "name": "Sprint 12 Regression",
    "version": "2.1",
    "plan_type_id": 5,
    "plan_type_name": "REGRESSION",
    "status": "TODO",
    "description": "Full regression test plan for Sprint 12 covering both frontend and backend projects.",
    "project_ids": [1, 3],
    "created_by": 42,
    "created_at": "2026-07-15T10:00:00Z",
    "updated_by": 42,
    "updated_at": "2026-07-15T10:00:00Z"
  }
}
```

**Notes:**
- `status` is always set to `TODO` on create -- not accepted from client input.
- `created_by` and `updated_by` are both set to the authenticated user's ID.
- `plan_type_name` is resolved via JOIN on create.
- Project associations are inserted into `TEST_PLAN_PROJECTS` within the same transaction.
- The unique name check uses `LOWER(name) = LOWER($1) AND deleted_at IS NULL`.
- Soft-deleted test plans with the same name do not block creation.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_plan:create` permission or is not Owner/Editor in any specified project |
| `409` | `DUPLICATE_TEST_PLAN_NAME` | Test plan name already exists (case-insensitive, among non-deleted) |
| `422` | `VALIDATION_ERROR` | Name empty, exceeds max length; version too long; description too long |
| `422` | `EMPTY_PROJECT_IDS` | `project_ids` is missing or an empty array |
| `422` | `INVALID_PROJECT` | A project ID in `project_ids` does not exist or is soft-deleted |
| `422` | `INVALID_PLAN_TYPE` | `plan_type_id` does not exist, is soft-deleted, or belongs to a project not in `project_ids` |

---

### GET `/api/v1/test-plans/{id}`

Get full detail of a single test plan, including linked projects and resolved plan type name.

**Required Permission:** `test_plan:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Plan ID |

**Success Response:** `200 OK`

```json
{
  "data": {
    "id": 15,
    "name": "Sprint 12 Regression",
    "version": "2.1",
    "plan_type_id": 5,
    "plan_type_name": "REGRESSION",
    "status": "IN_PROGRESS",
    "description": "Full regression test plan for Sprint 12 covering both frontend and backend projects.",
    "project_ids": [1, 3],
    "created_by": 42,
    "created_at": "2026-07-10T09:00:00Z",
    "updated_by": 42,
    "updated_at": "2026-07-15T08:30:00Z"
  }
}
```

**Notes:**
- `plan_type_name` is resolved by JOIN on `test_plan_types`. If the plan type is soft-deleted
  (defensive check), `plan_type_name` is still resolved (plan types are not soft-deletable
  in Phase 1).
- `project_ids` is aggregated from the `TEST_PLAN_PROJECTS` junction table.
- `deleted_at` and `deleted_by` are never returned to the client.
- If the user is not a member of any linked project and is not a System Admin, `404` is
  returned (same message as non-existent).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_plan:read` permission |
| `404` | `NOT_FOUND` | Test plan does not exist, is soft-deleted, or user is not a member of any linked project |

---

### PATCH `/api/v1/test-plans/{id}`

Update a test plan's fields, including status transitions and project associations.

**Required Permission:** `test_plan:update` AND project role Owner or Editor in at least one
linked project

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Plan ID |

**Request Body** (all fields optional; at least one required):

```json
{
  "name": "Sprint 12 Full Regression",
  "version": "3.0",
  "plan_type_id": 4,
  "description": "Updated description for the full regression suite.",
  "project_ids": [1, 3, 5]
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `name` | string | No | 1-255 characters, must contain at least one non-whitespace character; sanitized; unique globally (case-insensitive) among non-deleted |
| `version` | string | No | Max 50 characters; sanitized. `null` clears (sets `NULL`). Omitted preserves current. `""` stores empty string. |
| `plan_type_id` | integer | No | Must reference a non-deleted plan type whose project is in the test plan's linked projects (current or new set) |
| `description` | string | No | Max 10000 characters; sanitized. `null` clears. `""` stores empty string. Omitted preserves current. |
| `project_ids` | array of integer | No | Non-empty if provided; each ID must reference a non-deleted project; replaces the entire set atomically. User must be Owner/Editor in at least one project in the new set. |

**Success Response:** `200 OK`

Response body is the updated test plan representation (same shape as GET detail).

**Notes:**
- At least one field must be provided (empty body returns `422`).
- Status changes use the dedicated `POST /api/v1/test-plans/{id}/transition-status` endpoint
  (see test-plan-status spec). The `status` field is rejected in the PATCH body with
  `422 FIELD_NOT_UPDATABLE`.
- `id`, `created_at`, and `created_by` are immutable.
- `deleted_at` and `deleted_by` cannot be modified through this endpoint.
- `updated_at` and `updated_by` are set automatically on change.
- The unique name check is only performed if `name` is provided and differs from the current
  value.
- Soft-deleted test plans cannot be updated.
- When `project_ids` is updated, all existing project associations are deleted and new ones
  inserted in a single transaction (sync-replace).
- If the plan type's project is not in the resulting project set (current or new), the
  request is rejected with `PLAN_TYPE_PROJECT_MISMATCH`.
- Unrecognised fields in the request body cause a `422 Unprocessable Entity` response
  (strict mode).
- The user must be Owner/Editor in at least one of the CURRENT linked projects (before
  applying the update) AND in at least one of the NEW linked projects (if `project_ids`
  is being changed). This prevents privilege escalation.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_plan:update` permission or is not Owner/Editor in a linked project |
| `404` | `NOT_FOUND` | Test plan does not exist, is soft-deleted, or user is not a member of any linked project |
| `409` | `DUPLICATE_TEST_PLAN_NAME` | Updated name conflicts with another non-deleted test plan |
| `422` | `VALIDATION_ERROR` | No fields provided, invalid field values |
| `422` | `FIELD_NOT_UPDATABLE` | `status` field provided in request body (must use `POST .../transition-status`) |
| `422` | `EMPTY_PROJECT_IDS` | `project_ids` is provided but empty |
| `422` | `INVALID_PROJECT` | A project ID in `project_ids` does not exist or is soft-deleted |
| `422` | `INVALID_PLAN_TYPE` | `plan_type_id` does not exist, is soft-deleted, or does not belong to a linked project |
| `422` | `PLAN_TYPE_PROJECT_MISMATCH` | The plan type's project is not in the test plan's linked projects after the update |

---

### DELETE `/api/v1/test-plans/{id}`

Soft-delete a test plan.

**Required Permission:** `test_plan:delete` AND project role Owner or Editor in at least one
linked project

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Plan ID |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Sets `deleted_at = NOW()` and `deleted_by = <current_user_id>`.
- Project associations in `TEST_PLAN_PROJECTS` are preserved (not deleted) for audit trail
  integrity.
- Repeated DELETE on an already soft-deleted test plan returns `404`.
- If the user is not a member of any linked project, `404` is returned (same as non-existent
  -- no information leakage).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_plan:delete` permission or is not Owner/Editor in a linked project |
| `404` | `NOT_FOUND` | Test plan does not exist, is already soft-deleted, or user is not a member of any linked project |

---

### GET `/api/v1/test-plans/select`

Return a compact list of all non-deleted test plans accessible to the authenticated user for
dropdown/selection UI components.

**Required Permission:** `test_plan:select`

**Success Response:** `200 OK`

```json
{
  "data": [
    { "id": 12, "name": "API v2 Integration Tests" },
    { "id": 8, "name": "Performance Baseline v1" },
    { "id": 15, "name": "Sprint 12 Regression" }
  ]
}
```

**Notes:**
- Unpaginated: returns all non-deleted test plans where the user is a member of at least one
  linked project. The SQL query uses `LIMIT 1000`. If the result set has 1000 rows, results
  are truncated, a warning is logged, and an `X-Result-Truncated: true` response header is
  set.
- Each entry contains only `id` and `name` (minimal payload for dropdown rendering).
- Results are ordered by `name` ascending (case-insensitive, `ORDER BY LOWER(name)`).
- Soft-deleted test plans are excluded.
- Route registration order matters: the `/select` path must be registered **before** the
  `/{id}` path to prevent `select` from being interpreted as an ID.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_plan:select` permission |

---

## Data Model

### New Table: TEST_PLANS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `name` | `VARCHAR(255)` | `NOT NULL` | Test plan name; unique among non-deleted rows (case-insensitive) |
| `version` | `VARCHAR(50)` | `NOT NULL`, `DEFAULT '1.0'` | Version string; app-defined format |
| `plan_type_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_plan_types(id) ON DELETE RESTRICT` | The plan type classification |
| `description` | `TEXT` | | Nullable; max 10000 chars enforced at app layer and by `CHECK` constraint below |
| `status` | `VARCHAR(20)` | `NOT NULL`, `DEFAULT 'TODO'` | Lifecycle status: `TODO`, `IN_PROGRESS`, `DONE`, `CANCEL` |
| `created_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who created the test plan |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Last user who modified the test plan |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by `BEFORE UPDATE` trigger |
| `deleted_by` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Nullable; set on soft-delete |
| `deleted_at` | `TIMESTAMPTZ` | | Nullable; set on soft-delete |

### New Junction Table: TEST_PLAN_PROJECTS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `plan_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_plans(id) ON DELETE RESTRICT` | FK to test plan |
| `project_id` | `BIGINT` | `NOT NULL`, `REFERENCES projects(id) ON DELETE RESTRICT` | FK to project |

**Constraints:**

```sql
-- Composite primary key on junction table
ALTER TABLE test_plan_projects
  ADD PRIMARY KEY (plan_id, project_id);

-- Description length guard (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_plans
  ADD CONSTRAINT chk_test_plans_description
  CHECK (description IS NULL OR char_length(description) <= 10000);

-- Status values guard (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_plans
  ADD CONSTRAINT chk_test_plans_status
  CHECK (status IN ('TODO', 'IN_PROGRESS', 'DONE', 'CANCEL'));

-- Case-insensitive unique name among non-deleted test plans
CREATE UNIQUE INDEX uq_test_plans_name
  ON test_plans (LOWER(name))
  WHERE deleted_at IS NULL;

-- Foreign key indexes
CREATE INDEX idx_test_plans_plan_type_id ON test_plans (plan_type_id);
CREATE INDEX idx_test_plans_created_by ON test_plans (created_by);
CREATE INDEX idx_test_plans_updated_by ON test_plans (updated_by);
CREATE INDEX idx_test_plans_deleted_by ON test_plans (deleted_by);
CREATE INDEX idx_test_plans_status ON test_plans (status)
  WHERE deleted_at IS NULL;

-- Junction table indexes
CREATE INDEX idx_test_plan_projects_project_id ON test_plan_projects (project_id);

-- Composite index for common list query: find all plans for a project the user belongs to
CREATE INDEX idx_test_plans_active_updated
  ON test_plans (updated_at DESC)
  WHERE deleted_at IS NULL;
```

**Design notes:**

- **Global unique name.** Unlike test cases (unique per project), test plan names are unique
  globally among non-deleted rows. This is because test plans are not project-scoped and
  users need to distinguish them by name across the entire system.
- **No `project_id` on `TEST_PLANS`.** Multi-project support is through the junction table
  `TEST_PLAN_PROJECTS`. A test plan must be linked to at least one project.
- **`version` is a free-form string** defaulting to `"1.0"`. It is not parsed or compared
  by the system; version comparison logic is the responsibility of the client.
- **`status` is a `VARCHAR` with a `CHECK` constraint** rather than a database ENUM type,
  following the project-wide convention of avoiding DB ENUMs.
- **`plan_type_id` is mandatory** (NOT NULL). Every test plan must have a plan type.
- **`ON DELETE RESTRICT` on `plan_type_id` FK** prevents hard-deleting a plan type that is
  referenced by test plans.
- **`ON DELETE RESTRICT` on user FKs** prevents deleting a user who has created, updated, or
  deleted test plans, preserving audit trail integrity.
- **Partial unique index `uq_test_plans_name`** enforces name uniqueness only among
  non-deleted rows. A soft-deleted test plan and a new test plan can share the same name.
- **Junction table uses composite PK** on `(plan_id, project_id)`, automatically
  deduplicating duplicate project associations at the database level.
- **No dedicated audit columns on `TEST_PLAN_PROJECTS`** -- the project associations are
  replaced atomically (sync-replace), so tracking individual association changes is not
  needed in Phase 1.

### BEFORE UPDATE Trigger

```sql
CREATE OR REPLACE FUNCTION trg_test_plans_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_test_plans_updated_at
  BEFORE UPDATE ON test_plans
  FOR EACH ROW
  EXECUTE FUNCTION trg_test_plans_updated_at();
```

### Relationship to Other Tables

```
TEST_PLAN_TYPES ──< TEST_PLANS >──< TEST_PLAN_PROJECTS >── PROJECTS
```

- `TEST_PLANS.plan_type_id` -> `TEST_PLAN_TYPES.id` (each test plan has one plan type)
- `TEST_PLAN_PROJECTS.plan_id` -> `TEST_PLANS.id` (junction to test plan)
- `TEST_PLAN_PROJECTS.project_id` -> `PROJECTS.id` (junction to project)

---

## Status Transitions

The test plan status follows a validated state machine. Transitions are enforced at the
application layer (service), with the `CHECK` constraint providing a defence-in-depth safety
net at the database level.

```
                    +---------+
         +--------->|  TODO   |<---------+
         |          +----+----+          |
         |               |               |
         |               |               |
         |        +------v------+        |
         |        | IN_PROGRESS |        |
         |        +------+------+        |
         |               |               |
         |               |               |
         |        +------v------+        |
         +---------+    DONE    |        |
         |        +-------------+        |
         |                               |
         |        +-------------+        |
         +---------+   CANCEL   +--------+
                  +-------------+
```

Allowed transitions:

| From | To | Description |
|------|----|-------------|
| `TODO` | `IN_PROGRESS` | Start working on the test plan |
| `TODO` | `CANCEL` | Cancel a plan before work begins |
| `IN_PROGRESS` | `DONE` | Mark the test plan as completed |
| `IN_PROGRESS` | `CANCEL` | Cancel an in-progress plan |
| `DONE` | `IN_PROGRESS` | Reopen a completed plan for additional work |
| `CANCEL` | `TODO` | Reactivate a cancelled plan |

Invalid transitions return `422` with `INVALID_STATUS_TRANSITION`.

Self-transitions (e.g., TODO to TODO) are silently accepted (no-op) rather than rejected.

**Status transition validation implementation:**

```python
# Domain-level transition map
ALLOWED_TRANSITIONS = {
    TestPlanStatus.TODO:         {TestPlanStatus.IN_PROGRESS, TestPlanStatus.CANCEL},
    TestPlanStatus.IN_PROGRESS:  {TestPlanStatus.DONE, TestPlanStatus.CANCEL},
    TestPlanStatus.DONE:         {TestPlanStatus.IN_PROGRESS},
    TestPlanStatus.CANCEL:       {TestPlanStatus.TODO},
}

def can_transition(current: TestPlanStatus, target: TestPlanStatus) -> bool:
    """Check if transitioning from current to target is allowed."""
    if current == target:
        return True  # Self-transition is a no-op
    return target in ALLOWED_TRANSITIONS.get(current, set())
```

---

## Sequence

### Create Test Plan Flow

1. Client sends `POST /api/v1/test-plans` with
   `{"name": "...", "project_ids": [1,3], "plan_type_id": 5, ...}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler deserializes and validates the request body (name required, project_ids required
   and non-empty, plan_type_id required, strict mode for unrecognised fields).
4. Handler calls `TestPlanService::create_test_plan(cmd, current_user_id)`.
5. `TestPlanService` checks the user has `test_plan:create` system permission
   (via `AuthorizationService`).
6. `TestPlanService` checks the user is an Owner or Editor in at least one of the specified
   projects (via `ProjectMemberRepository`). If not, and not System Admin -> `403`.
7. `TestPlanService` begins a database transaction.
8. Within the transaction:
   a. Validate all project IDs: `ProjectRepository::find_all_active_by_ids(project_ids)`.
      If any project does not exist or is soft-deleted -> roll back and return `422` with
      `INVALID_PROJECT`. Duplicates are removed before validation.
   b. Check for duplicate name: `TestPlanRepository::find_by_name(name)`. If a non-deleted
      duplicate exists -> roll back and return `409 Conflict`.
   c. Validate `plan_type_id`: fetch the plan type and verify it is non-deleted and its
      `project_id` is in the `project_ids` set. If invalid -> roll back and return `422`
      with `INVALID_PLAN_TYPE`.
   d. Construct a `TestPlan` entity (status = TODO) and call
      `TestPlanRepository::save(test_plan, project_ids)`.
   e. Within the same transaction, insert rows into `TEST_PLAN_PROJECTS` for each project ID.
   f. If the INSERT fails with a PostgreSQL duplicate key violation (error 23505 on the
      unique name index), catch it and return `409 Conflict` with
      `DUPLICATE_TEST_PLAN_NAME` as a fallback for concurrent inserts.
9. Transaction commits.
10. Handler constructs the `Location` header from the new test plan ID and returns
    `201 Created`.

### Update Test Plan Flow

1. Client sends `PATCH /api/v1/test-plans/{id}` with
   `{"name": "...", "status": "DONE", "project_ids": [1,3,5]}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates the request body (at least one field, strict mode for unrecognised
   fields, field-level validation for name/version/description lengths).
4. Handler calls `TestPlanService::update_test_plan(test_plan_id, cmd, current_user_id)`.
5. `TestPlanService` checks `test_plan:update` system permission.
6. `TestPlanService` fetches the current test plan to verify it exists and is not
   soft-deleted. If not found or soft-deleted -> `404`.
7. `TestPlanService` checks the user is an Owner or Editor in at least one of the test plan's
   current linked projects (via `ProjectMemberRepository`). If not, and not System Admin ->
   `403`.
8. If `project_ids` is being changed: additionally verify the user is an Owner or Editor in
   at least one project in the new set. If not -> `403`.
9. `TestPlanService` begins a database transaction.
10. Within the transaction:
    a. If `name` is provided and differs from current: check for duplicate name via
       `TestPlanRepository::find_by_name`. If a different test plan has the same name ->
       roll back and return `409 Conflict`.
    b. If `plan_type_id` is provided: validate the plan type exists, is non-deleted, and its
       `project_id` is in the test plan's project set (current or new). If invalid -> roll
       back and return `422` with `INVALID_PLAN_TYPE`.
            c. If `status` is provided: reject with `422 FIELD_NOT_UPDATABLE` (status changes must
               use the dedicated `POST .../transition-status` endpoint).
    d. If `project_ids` is provided: validate all IDs exist and are non-deleted. Delete all
       existing `TEST_PLAN_PROJECTS` rows for this plan. Insert new rows for each project ID.
       After the sync, verify the `plan_type_id`'s project is still in the set (check again
       for `PLAN_TYPE_PROJECT_MISMATCH`).
    e. Apply remaining field updates via `test_plan.apply_update(cmd)` and save via
       repository.
11. Transaction commits.
12. Handler returns `200 OK` with the updated test plan.

### Soft-Delete Test Plan Flow

1. Client sends `DELETE /api/v1/test-plans/{id}` with session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler calls `TestPlanService::delete_test_plan(test_plan_id, current_user_id)`.
4. `TestPlanService` checks `test_plan:delete` system permission.
5. `TestPlanService` fetches the current test plan to verify it exists and is not
   soft-deleted. If not found or soft-deleted -> `404`.
6. `TestPlanService` checks the user is an Owner or Editor in at least one of the test plan's
   linked projects. If not, and not System Admin -> `403`.
7. `TestPlanService` calls `TestPlanRepository::soft_delete(test_plan_id, current_user_id)`
   which sets `deleted_at = NOW()`, `deleted_by = current_user_id`.
8. `TEST_PLAN_PROJECTS` rows are NOT deleted (preserved for audit trail).
9. Handler returns `204 No Content`.

### List Test Plans Flow

1. Client sends `GET /api/v1/test-plans?page=1&limit=25&status=IN_PROGRESS&search=regression&sort=-updated_at`
   with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler validates pagination and filter parameters.
4. Handler calls `TestPlanService::list_test_plans(query, current_user_id)`.
5. `TestPlanService` checks `test_plan:read_list` system permission.
6. `TestPlanService` calls `TestPlanRepository::find_accessible_by_user(user_id, page, limit, filters, search, sort)`.
7. Repository executes a parameterized query that joins `TEST_PLANS` with
   `TEST_PLAN_PROJECTS` and `PROJECT_MEMBERS` to filter to test plans where the user is a
   member of at least one linked project. System Admin bypasses the membership filter.
8. Repository aggregates `project_ids` via array aggregation and resolves `plan_type_name`
   via JOIN.
9. Repository returns the paginated results and total count.
10. Handler returns `200 OK`.

### Select Test Plans Flow

1. Client sends `GET /api/v1/test-plans/select` with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler calls `TestPlanService::select_test_plans(current_user_id)`.
4. `TestPlanService` checks `test_plan:select` system permission.
5. `TestPlanService` calls `TestPlanRepository::find_all_accessible_by_user(user_id)`
   which selects only `id` and `name`, filtered to test plans where the user is a member,
   ordered by `LOWER(name)`, limited to 1000.
6. Handler returns `200 OK` with the flat array.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestPlan` | Domain (1) | Entity: `id`, `name`, `version`, `plan_type_id`, `description`, `status`, audit + soft-delete fields. Factory method `create(name, version, plan_type_id, description, created_by)` performs domain validation. Method `apply_update(cmd)` validates changed fields and returns a modified entity. Method `transition_status(new_status)` validates the transition and returns a new entity with updated status. No ORM or framework imports. |
| `TestPlanStatus` | Domain (1) | Enum: `TODO`, `IN_PROGRESS`, `DONE`, `CANCEL`. Contains the transition map and a `can_transition_to(target) -> bool` method. |
| `TestPlanService` | Application (2) | Orchestrates all test plan use cases: `create_test_plan`, `list_test_plans`, `get_test_plan`, `update_test_plan`, `delete_test_plan`, `select_test_plans`. Each method checks system permission and multi-project membership, then delegates to the repository. |
| `TestPlanRepository` | Application (2) | Interface (port): `find_by_id(test_plan_id)`, `find_by_name(name)`, `find_accessible_by_user(user_id, page, limit, filters, search, sort)`, `find_all_accessible_by_user(user_id)`, `save(test_plan, project_ids)`, `update(test_plan, project_ids?)`, `soft_delete(test_plan_id, deleted_by)`. Also `validate_plan_type_in_projects(plan_type_id, project_ids)`. |
| `TestPlanHandler` | Adapters (3) | HTTP handler with six methods (`list`, `create`, `get`, `update`, `delete`, `select`). Deserializes requests, calls `TestPlanService`, serializes responses. The select handler is registered before the `/{id}` handler. |
| `SqlTestPlanRepository` | Infrastructure (4) | Implements `TestPlanRepository` using PostgreSQL. Uses parameterized queries exclusively. Handles the multi-project membership filtering via JOINs with `TEST_PLAN_PROJECTS` and `PROJECT_MEMBERS`. Aggregates `project_ids` and resolves `plan_type_name`. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (Application) | Add permission codes `test_plan:create`, `test_plan:read`, `test_plan:read_list`, `test_plan:update`, `test_plan:delete`, `test_plan:select` to the permission registry. Add role-based checks: Owner/Editor for create/update/delete; any member for reads. |
| `ProjectMemberRepository` (Application) | Add method `find_user_roles_in_projects(user_id, project_ids) -> Map<project_id, role>` to efficiently check membership across multiple projects. Or reuse existing per-project membership lookup in a loop. |
| Seed migration (Infrastructure, from `iam-permissions`) | Add 6 new permission rows for `test_plan:*` codes. |
| HTTP router registration | Register six new routes under `/api/v1/test-plans/` -- all require session auth. Route order: `/select` before `/{id}`. |

---

## Route Registration

```text
# Order matters: /select must be registered before /{id}
GET    /api/v1/test-plans          -> list
POST   /api/v1/test-plans          -> create
GET    /api/v1/test-plans/select   -> select   (static path)
GET    /api/v1/test-plans/{id}     -> get      (dynamic path)
PATCH  /api/v1/test-plans/{id}     -> update
DELETE /api/v1/test-plans/{id}     -> delete
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- System permission check (respective `test_plan:*` code)
- Multi-project membership check:
  - `POST`: Owner or Editor in at least one specified project
  - `PATCH`, `DELETE`: Owner or Editor in at least one linked project
  - `GET` (read endpoints): any membership in at least one linked project
  - `GET` (list, select): membership filter built into SQL query

---

## New Permission Codes

These must be added to the `PERMISSIONS` table seed data. Use `INSERT ... ON CONFLICT (code)
DO NOTHING` and let the database assign IDs -- do not hardcode numeric IDs in the migration:

| code | name |
|------|------|
| `test_plan:create` | Create Test Plan |
| `test_plan:read` | Read Test Plan |
| `test_plan:read_list` | Read Test Plan List |
| `test_plan:update` | Update Test Plan |
| `test_plan:delete` | Delete Test Plan |
| `test_plan:select` | Select Test Plan |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks required system permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks required project role | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| User not a member of any linked project (detail/delete) | `404` | `NOT_FOUND` | INFO | Same as non-existent to prevent information leakage |
| Test plan not found or soft-deleted | `404` | `NOT_FOUND` | INFO | |
| Duplicate test plan name | `409` | `DUPLICATE_TEST_PLAN_NAME` | INFO | Case-insensitive; among non-deleted rows |
| Empty or missing project_ids | `422` | `EMPTY_PROJECT_IDS` | INFO | Applies to both create and update |
| Project does not exist or is soft-deleted | `422` | `INVALID_PROJECT` | INFO | Checked during create and update |
| Plan type does not exist, is soft-deleted, or belongs to a non-linked project | `422` | `INVALID_PLAN_TYPE` | INFO | Checked during create and update |
| Plan type's project not in the test plan's project set after update | `422` | `PLAN_TYPE_PROJECT_MISMATCH` | INFO | Only applicable on update when project_ids changes |
| Invalid status transition | `422` | `INVALID_STATUS_TRANSITION` | INFO | Validated against the state machine |
| Validation error (bad request body) | `422` | `VALIDATION_ERROR` | INFO | Includes field-level details; unrecognised fields rejected in strict mode |
| Invalid pagination or sort parameter | `422` | `VALIDATION_ERROR` | INFO | page < 1 or page > 1000, limit < 1 or limit > 100, invalid sort value, search > 255 chars |
| DB duplicate key violation (race condition) | `409` | `DUPLICATE_TEST_PLAN_NAME` | INFO | Caught from PostgreSQL error 23505 |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Per-endpoint limits; returns `Retry-After` header |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not reveal** whether a `404` is caused by a non-existent test plan, soft-deleted test
  plan, or user not being a member of any linked project (same message for all).
- **Do not return `400`** for business logic errors -- use `409 Conflict` or
  `422 Unprocessable Entity` as specified.
- **Do not hard-delete** any record -- all deletions are soft-deletes.
- **Do not allow UPDATE on soft-deleted rows.**
- **Do not omit `deleted_by`** -- every soft-delete sets both `deleted_at` and `deleted_by`.
- **Do not delete project associations on soft-delete** -- `TEST_PLAN_PROJECTS` rows are
  preserved for audit trail integrity.
- **Do not use `GET` with a body** -- all state changes use `POST`, `PATCH`, or `DELETE`.
- **Do not allow cross-project plan type injection** -- plan type FK validation always
  checks that the plan type's project is among the test plan's linked projects.
- **Do not accept `status` on create** -- the initial status is always `TODO`.
