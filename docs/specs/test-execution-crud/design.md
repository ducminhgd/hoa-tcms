# Design: Test Execution CRUD

## Architecture

The Test Execution CRUD feature follows Clean Architecture layering. Test executions are
nested under Test Runs, which are themselves nested under Projects. All endpoints follow
the resource path `/api/v1/projects/{projectId}/test-runs/{runId}/executions` and
require both system RBAC permission and project membership scope checks.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_executions    GET    /api/v1/projects/{pid}/test-runs/{rid}   │   │
│  │                               /executions                             │   │
│  │  - create_execution   POST   /api/v1/projects/{pid}/test-runs/{rid}   │   │
│  │                               /executions                             │   │
│  │  - get_execution      GET    /api/v1/projects/{pid}/test-runs/{rid}   │   │
│  │                               /executions/{id}                        │   │
│  │  - update_execution   PATCH  /api/v1/projects/{pid}/test-runs/{rid}   │   │
│  │                               /executions/{id}                        │   │
│  │  - delete_execution   DELETE /api/v1/projects/{pid}/test-runs/{rid}   │   │
│  │                               /executions/{id}                        │   │
│  │  - select_executions  GET    /api/v1/projects/{pid}/test-runs/{rid}   │   │
│  │                               /executions/select                      │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  TestExecutionService:                    │  │  - TestExecution        │  │
│  │  - create_execution                       │  │  - TesterAssignment     │  │
│  │  - list_executions                        │  └──────────────────────────┘  │
│  │  - get_execution                          │                                │
│  │  - update_execution                       │                                │
│  │  - delete_execution                       │                                │
│  │  - select_executions                      │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - TestExecutionRepository (port)         │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestExecutionRepository (implements TestExecutionRepository)     │   │
│  │  - TestCaseImportService (invoked during create)                       │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All execution endpoints require an authenticated session (checked by `AuthMiddleware`).
2. `POST` endpoints require the caller to be a Contributor, Editor, or Owner of the
   project (or a System Admin), on top of the system permission check. Viewers cannot
   create.
3. `PATCH` and `DELETE` endpoints require the caller to be at least a Contributor, with
   an additional ownership check for Contributors: they can only update or delete
   executions where `created_by` matches their user ID. Owners and Editors can update or
   delete any execution in the project.
4. `GET` endpoints require any project membership (or System Admin).
5. Create validates unique name within the Test Run, Test Run existence and project scope,
   and tester validity (user exists, ACTIVE, project member) within a single database
   transaction to prevent TOCTOU races.
6. Update validates unique name (if name is being changed) and tester validity within a
   single database transaction. Rejects updates on soft-deleted records.
7. Soft-delete requires no referential integrity checks (test case results that reference
   the execution are preserved and hidden by query filters).

### Security Requirements

**Input sanitization:** Execution `name` must be sanitized on input (strip disallowed
HTML tags) before storage. Output-encoding must be applied at the presentation layer.
See PRD 5.3 for the project-wide XSS prevention policy.

**CSRF protection:** All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must be
protected against CSRF. Session cookies must carry `SameSite=Lax` (or stricter).
`POST`/`PATCH`/`DELETE` requests with `Content-Type: application/json` must verify the
`Content-Type` header to block simple form-based CSRF attacks. Additional anti-CSRF
tokens should be considered for defense in depth.

**Authorization layering:** Three independent authorization gates apply:

1. System permission check (e.g., `test_execution:update`)
2. Project membership and role check (Contributor, Editor, Owner for mutations; any role
   for reads; Viewer excluded from all mutations)
3. Contributor ownership check (on update and delete only): if the user's project role is
   Contributor, verify `created_by` matches the authenticated user ID

All three gates must pass (or the caller must be a System Admin, who implicitly holds all
system permissions, bypasses all project membership checks, and bypasses the ownership
check). Authorization checks are performed against live data on every request -- permissions
and project membership are never cached in the session. If a user's role is changed
mid-session, the new role takes effect on their next request. A `403 Forbidden` response
must use a generic message that does not distinguish between "missing system permission"
and "wrong project role". The Contributor ownership check is the exception: it returns a
distinct message because the user has already passed the project membership gate.

**Tester validation:** Every user ID in `tester_ids` must be validated for project
membership. The user must exist (`users.deleted_at IS NULL`), be ACTIVE, and be a member
of the project (row exists in `PROJECT_MEMBERS` for the user and project). This prevents
assigning testers from outside the project.

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

### GET `/api/v1/projects/{projectId}/test-runs/{runId}/executions`

List test executions within a Test Run with pagination, filtering, and sorting.

**Required Permission:** `test_execution:read_list`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `runId` | integer | Test Run ID |

**Query Parameters:**

| Param | Type | Default | Max | Description |
|-------|------|---------|-----|-------------|
| `page` | integer | 1 | -- | Page number (1-based) |
| `limit` | integer | 25 | 100 | Items per page |
| `search` | string | -- | 255 | Case-insensitive substring match on `name` |
| `sort` | string | `-id` | -- | Sort field: `id`, `-id`, `name`, `-name`, `created_at`, `-created_at` |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 15,
      "name": "Sprint 12 Regression",
      "test_run_id": 8,
      "testers": [
        { "user_id": 10, "username": "alice" },
        { "user_id": 11, "username": "bob" }
      ],
      "created_by": 12,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_at": "2026-07-14T10:00:00Z"
    },
    {
      "id": 14,
      "name": "Hotfix v2.3.1",
      "test_run_id": 8,
      "testers": [
        { "user_id": 10, "username": "alice" }
      ],
      "created_by": 12,
      "created_at": "2026-07-13T08:00:00Z",
      "updated_at": "2026-07-15T08:00:00Z"
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
- Results are scoped to the given Test Run and exclude soft-deleted executions
  (`WHERE test_run_id = $1 AND deleted_at IS NULL`).
- The Test Run must belong to the project and not be soft-deleted.
- Default sort is `-id` (newest first, descending).
- `search` applies `ILIKE` on `name` when provided; when absent, no filter is applied.
  The search value is escaped in this order: first `\` is doubled to `\\`, then `%` is
  escaped to `\%`, then `_` is escaped to `\_`. The search value must not exceed 255
  characters. Leading and trailing whitespace is trimmed; an all-whitespace search is
  treated as "no filter."
- Tester details (`user_id` and `username`) are resolved via JOIN on
  `TEST_EXECUTION_TESTERS` and `USERS` and included in every list item so the UI
  can display tester names without an extra detail request.
- List response excludes `updated_by`, `deleted_at`, and `deleted_by` to keep the
  payload compact.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:read_list` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, or Test Run does not exist, is soft-deleted, or belongs to a different project |
| `422` | `VALIDATION_ERROR` | Invalid pagination, sort parameter, or search value |

---

### POST `/api/v1/projects/{projectId}/test-runs/{runId}/executions`

Create a new test execution within a Test Run. On creation, test cases from the linked
Test Run are imported as a snapshot (delegated to the `test-execution-import` flow).

**Required Permission:** `test_execution:create` AND project role Contributor, Editor, or
Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `runId` | integer | Test Run ID |

**Request Body:**

```json
{
  "name": "Sprint 12 Regression",
  "tester_ids": [10, 11]
}
```

| Field | Type | Required | Default | Constraints |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | -- | 1-500 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input (HTML stripped); unique per Test Run (case-insensitive) |
| `tester_ids` | array of integers | Yes | -- | At least one tester ID; each must reference an ACTIVE user who is a member of the project. Empty array is rejected. Duplicate IDs are deduplicated server-side. |

**Success Response:** `201 Created`

Headers: `Location: /api/v1/projects/42/test-runs/8/executions/15`

```json
{
  "data": {
    "id": 15,
    "name": "Sprint 12 Regression",
    "test_run_id": 8,
    "testers": [
      { "user_id": 10, "username": "alice" },
      { "user_id": 11, "username": "bob" }
    ],
    "created_by": 12,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 12,
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- `test_run_id` in the response mirrors the `runId` path parameter.
- `created_by` and `updated_by` are both set to the authenticated user's ID.
- The unique name check uses `LOWER(name) = LOWER($1)` and includes
  `WHERE test_run_id = $2 AND deleted_at IS NULL`.
- Soft-deleted executions with the same name do not block creation.
- After the execution row is inserted, the test case import flow is invoked (see
  `test-execution-import`). The request does not complete until the import finishes.
  If the import fails, the transaction is rolled back and the execution is not created.
- Tester IDs are deduplicated before insertion into the junction table.
- The Test Run must exist, not be soft-deleted, and belong to the specified project.
- Test Run validation runs within the same transaction as the insert to prevent TOCTOU
  races.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:create` permission or is not Contributor/Editor/Owner of the project |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, or Test Run does not exist, is soft-deleted, or belongs to a different project |
| `409` | `DUPLICATE_EXECUTION_NAME` | Execution name already exists in this Test Run (case-insensitive) |
| `422` | `VALIDATION_ERROR` | Name empty, exceeds max length, or `tester_ids` empty |
| `422` | `INVALID_TESTER` | One or more tester IDs are invalid (user does not exist, not ACTIVE, or not a project member) |

`409 Conflict` response body for duplicate name:

```json
{
  "error": {
    "code": "DUPLICATE_EXECUTION_NAME",
    "message": "An execution with the name 'Sprint 12 Regression' already exists in this Test Run.",
    "details": [
      { "field": "name", "message": "Execution name must be unique within the Test Run" }
    ]
  }
}
```

`422` response body for invalid tester:

```json
{
  "error": {
    "code": "INVALID_TESTER",
    "message": "One or more tester IDs are invalid.",
    "details": [
      { "field": "tester_ids", "message": "User 99 is not an ACTIVE member of this project" }
    ]
  }
}
```

---

### GET `/api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}`

Get full detail of a single test execution, including the list of assigned testers.

**Required Permission:** `test_execution:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `runId` | integer | Test Run ID |
| `id` | integer | Test Execution ID |

**Success Response:** `200 OK`

```json
{
  "data": {
    "id": 15,
    "name": "Sprint 12 Regression",
    "test_run_id": 8,
    "testers": [
      { "user_id": 10, "username": "alice" },
      { "user_id": 11, "username": "bob" }
    ],
    "created_by": 12,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 12,
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- Tester details (`user_id` and `username`) are resolved by joining
  `TEST_EXECUTION_TESTERS` with `USERS`.
- `deleted_at` and `deleted_by` are never returned to the client.
- The execution must belong to the specified Test Run; if the execution exists but
  belongs to a different Test Run, `404 Not Found` is returned.
- The Test Run must belong to the specified project.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:read` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, Test Run does not exist/is soft-deleted/belongs to different project, or execution does not exist/is soft-deleted/belongs to different Test Run |

---

### PATCH `/api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}`

Update an execution's name and/or assigned testers. Contributors can only update their
own executions.

**Required Permission:** `test_execution:update` AND project role Contributor (own only),
Editor, or Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `runId` | integer | Test Run ID |
| `id` | integer | Test Execution ID |

**Request Body** (all fields optional; at least one required):

```json
{
  "name": "Updated Regression",
  "tester_ids": [10, 12]
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `name` | string | No | 1-500 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input; unique per Test Run (case-insensitive) |
| `tester_ids` | array of integers | No | If provided: must contain at least one tester ID; each must reference an ACTIVE user who is a member of the project. Duplicate IDs are deduplicated. Omitting preserves the current tester set. |

**Success Response:** `200 OK`

Response body is the updated execution representation (same shape as GET detail).

**Notes:**
- At least one field must be provided (empty body returns `422`).
- `id`, `test_run_id`, `created_at`, and `created_by` are immutable.
- `deleted_at` and `deleted_by` cannot be modified through this endpoint.
- `updated_at` and `updated_by` are set automatically on change.
- The unique name check is only performed if `name` is provided and differs from the
  current value.
- When `tester_ids` is provided, the junction table is reconciled within the transaction:
  testers present in the new set but not currently assigned are inserted; testers
  currently assigned but absent from the new set are deleted. This is implemented as a
  delete-all-then-re-insert within the transaction for simplicity.
- Soft-deleted executions cannot be updated.
- For Contributors: the `created_by` field is checked against the authenticated user's
  ID. If they don't match, `403 Forbidden` is returned with a distinct message.
- Unrecognised fields in the request body cause a `422 Unprocessable Entity` response
  (strict mode).
- The execution must belong to the specified Test Run; if it belongs to a different Test
  Run, `404 Not Found` is returned.
- The Test Run must belong to the specified project.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:update` permission, is a Viewer, or (for Contributors) does not own the execution |
| `404` | `NOT_FOUND` | Project, Test Run, or execution does not exist, is soft-deleted, or belongs to a different parent |
| `409` | `DUPLICATE_EXECUTION_NAME` | Updated name conflicts with another execution in the same Test Run |
| `422` | `VALIDATION_ERROR` | No fields provided, invalid field values, or `tester_ids` empty |
| `422` | `INVALID_TESTER` | One or more tester IDs are invalid |

---

### DELETE `/api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}`

Soft-delete a test execution. Contributors can only delete their own executions.

**Required Permission:** `test_execution:delete` AND project role Contributor (own only),
Editor, or Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `runId` | integer | Test Run ID |
| `id` | integer | Test Execution ID |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Sets `deleted_at = NOW()` and `deleted_by = <current_user_id>`.
- For Contributors: the `created_by` field is checked against the authenticated user's
  ID. If they don't match, `403 Forbidden` is returned with a distinct message.
- No referential integrity check is performed before soft-deleting an execution. Test
  case results that reference this execution are preserved. They are hidden because
  downstream queries include `WHERE execution.deleted_at IS NULL`.
- The junction table rows in `TEST_EXECUTION_TESTERS` are left intact (they are hidden
  by the same query filter).
- Repeated DELETE on an already soft-deleted execution returns `404`.
- The execution must belong to the specified Test Run and project.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:delete` permission, is a Viewer, or (for Contributors) does not own the execution |
| `404` | `NOT_FOUND` | Project, Test Run, or execution does not exist, is soft-deleted, or belongs to a different parent |

---

### GET `/api/v1/projects/{projectId}/test-runs/{runId}/executions/select`

Return a compact list of all non-deleted executions within a Test Run for dropdown/selection
UI components.

**Required Permission:** `test_execution:select`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `runId` | integer | Test Run ID |

**Success Response:** `200 OK`

```json
{
  "data": [
    { "id": 15, "name": "Sprint 12 Regression" },
    { "id": 14, "name": "Hotfix v2.3.1" },
    { "id": 10, "name": "Acceptance v2.3" }
  ]
}
```

**Notes:**
- Unpaginated: returns all non-deleted executions for the Test Run. The SQL query uses
  `LIMIT 500`. If the result set has 500 rows, results are truncated, a warning is
  logged, and an `X-Result-Truncated: true` response header is set so the UI can
  surface this to the user.
- Each entry contains only `id` and `name` (minimal payload for dropdown rendering).
- Results are ordered by `name` ascending (case-insensitive, `ORDER BY LOWER(name)`).
- Soft-deleted executions are excluded.
- Route registration order matters: the `/select` path must be registered **before**
  the `/{id}` path to prevent `select` from being interpreted as an ID.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:select` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, Test Run does not exist, is soft-deleted, or belongs to a different project |

---

## Data Model

### New Table: TEST_EXECUTIONS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `name` | `VARCHAR(500)` | `NOT NULL` | Execution name; unique per Test Run (case-insensitive); see constraint below |
| `test_run_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_runs(id) ON DELETE RESTRICT` | Mandatory FK; every execution belongs to exactly one Test Run |
| `created_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who created the execution |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Last user who modified the execution |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by `BEFORE UPDATE` trigger |
| `deleted_by` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Nullable; set on soft-delete |
| `deleted_at` | `TIMESTAMPTZ` | | Nullable; set on soft-delete |

**Constraints:**

```sql
-- Case-insensitive unique name per Test Run (PostgreSQL)
-- Only enforced for non-deleted rows, allowing a soft-deleted row and a new row
-- with the same name to coexist.
CREATE UNIQUE INDEX uq_test_executions_name_run
  ON test_executions (test_run_id, LOWER(name))
  WHERE deleted_at IS NULL;

-- Foreign key indexes
CREATE INDEX idx_test_executions_test_run_id ON test_executions (test_run_id);
CREATE INDEX idx_test_executions_created_by ON test_executions (created_by);
CREATE INDEX idx_test_executions_updated_by ON test_executions (updated_by);
CREATE INDEX idx_test_executions_deleted_by ON test_executions (deleted_by);

-- Partial index for active executions (most queries filter out soft-deleted)
CREATE INDEX idx_test_executions_active ON test_executions (test_run_id, id DESC)
  WHERE deleted_at IS NULL;
```

### New Junction Table: TEST_EXECUTION_TESTERS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `execution_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_executions(id) ON DELETE RESTRICT` | FK to test execution |
| `user_id` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | FK to user (tester) |

**Constraints:**

```sql
-- Composite primary key ensures a user is not assigned twice to the same execution
PRIMARY KEY (execution_id, user_id)

-- Performance indexes
CREATE INDEX idx_test_execution_testers_execution_id
  ON test_execution_testers (execution_id);
CREATE INDEX idx_test_execution_testers_user_id
  ON test_execution_testers (user_id);
```

**Design notes:**

- **No separate `id` column** -- the composite `(execution_id, user_id)` primary key is
  both sufficient and prevents duplicate assignments naturally at the DB level.
- **`ON DELETE RESTRICT` on `execution_id` FK** prevents hard-deleting an execution that
  still has tester assignments. Since soft-delete is used, the RESTRICT is a
  defence-in-depth measure against accidental hard-deletes.
- **`ON DELETE RESTRICT` on `user_id` FK** prevents deleting a user who is assigned as
  a tester to any execution. This preserves audit trail integrity.
- **No soft-delete on the junction table** -- junction rows are hidden by the query
  filter on `test_executions.deleted_at IS NULL`. There is no independent lifecycle for
  tester assignments separate from the execution.
- **Tester reconciliation on update** -- when `tester_ids` is provided on PATCH, the
  junction rows are deleted and re-inserted within the same transaction. This is simpler
  than a diff-based approach and the table is small (typically < 10 rows per execution).

### BEFORE UPDATE Trigger

```sql
CREATE OR REPLACE FUNCTION trg_test_executions_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_test_executions_updated_at
  BEFORE UPDATE ON test_executions
  FOR EACH ROW
  EXECUTE FUNCTION trg_test_executions_updated_at();
```

The trigger ensures `updated_at` is always set to the current timestamp on every `UPDATE`,
regardless of which code path performs the update.

### Relationship to Other Tables

```
PROJECTS ──< TEST_RUNS ──< TEST_EXECUTIONS >── TEST_EXECUTION_TESTERS ──< USERS
                                   │
                                   └── TEST_CASE_RESULTS (via test-execution-import)
```

- `TEST_EXECUTIONS.test_run_id` -> `TEST_RUNS.id` (every execution belongs to one Test Run)
- `TEST_EXECUTION_TESTERS.execution_id` -> `TEST_EXECUTIONS.id` (tester assignment)
- `TEST_EXECUTION_TESTERS.user_id` -> `USERS.id` (tester identity)
- Test case results reference `TEST_EXECUTIONS.id` (created by the import flow; see
  `test-execution-import`)

---

## Sequence

### Create Test Execution Flow

1. Client sends `POST /api/v1/projects/{projectId}/test-runs/{runId}/executions` with
   `{"name": "Sprint 12 Regression", "tester_ids": [10, 11]}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler deserializes and validates the request body (name required, tester_ids
   required and non-empty, strict mode for unrecognised fields).
4. Handler calls `TestExecutionService::create_execution(project_id, run_id, cmd,
   current_user_id)`.
5. `TestExecutionService` checks the user has `test_execution:create` system permission
   (via `AuthorizationService`).
6. `TestExecutionService` checks the user is a Contributor, Editor, or Owner of the
   project (via `ProjectMemberRepository`). If Viewer or not a member, and not System
   Admin -> `403`.
7. `TestExecutionService` begins a database transaction.
8. Within the transaction:
   a. `SELECT ... FOR UPDATE` on the Test Run row to verify it exists, is not
      soft-deleted, and belongs to the specified project. If not found, soft-deleted, or
      wrong project -> roll back and return `404 Not Found`.
   b. Check for duplicate name:
      `TestExecutionRepository::find_by_name_in_run(run_id, name)`. If a non-deleted
      duplicate exists -> roll back and return `409 Conflict`.
   c. Validate tester IDs: for each unique ID in `tester_ids`, verify the user exists,
      is ACTIVE, and is a member of the project. If any ID fails -> roll back and return
      `422` with `INVALID_TESTER` and specific details per invalid ID. Use
      `SELECT ... FOR UPDATE` on project membership rows.
   d. Insert execution row via `TestExecutionRepository::save(execution)`. If the INSERT
      fails with a PostgreSQL duplicate key violation (error 23505), catch it and return
      `409 Conflict` with `DUPLICATE_EXECUTION_NAME` as a fallback.
   e. Insert tester assignments into `TEST_EXECUTION_TESTERS` (deduplicate IDs first).
   f. Invoke the test case import flow (delegated to
      `TestCaseImportService::import_from_run(execution_id, run_id)` -- see
      `test-execution-import`). The import snapshots test cases from the Test Run into
      the execution. If the import fails, roll back the entire transaction.
9. Transaction commits.
10. Handler constructs the `Location` header from the new execution ID and returns
    `201 Created`.

### Update Test Execution Flow

1. Client sends `PATCH /api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}`
   with `{"name": "...", "tester_ids": [10, 12]}` and session cookie.
2-4. Same as Create: validate session, project exists and not soft-deleted, Test Run
     exists and belongs to project and not soft-deleted, execution exists and belongs to
     Test Run and not soft-deleted.
5. Handler calls `TestExecutionService::update_execution(project_id, run_id,
   execution_id, cmd, current_user_id)`.
6. `TestExecutionService` checks `test_execution:update` system permission.
7. `TestExecutionService` checks the user is at least a Contributor of the project. If
   Viewer or not a member, and not System Admin -> `403`.
8. `TestExecutionService` checks if the user is a Contributor: if so, verify
   `execution.created_by == current_user_id`. If not matching -> `403` (ownership
   restriction). Owners and Editors skip this check.
9. `TestExecutionService` begins a database transaction.
10. Within the transaction:
    a. If `name` is provided and differs from current: check for duplicate name via
       `find_by_name_in_run`. If a different execution has the same name -> roll back
       and return `409 Conflict`.
    b. If `tester_ids` is provided: validate all IDs (same validation as create). If
       valid, delete all existing rows for this execution in `TEST_EXECUTION_TESTERS`
       and insert the new set (deduplicated).
    c. Apply updates via `execution.apply_update(cmd)` and save via repository.
11. Transaction commits.
12. Handler returns `200 OK` with the updated execution.

### Soft-Delete Test Execution Flow

1. Client sends `DELETE /api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}`
   with session cookie.
2-4. Same as Create: validate session, project, Test Run, and execution existence and
     scope. Execution must not already be soft-deleted.
5. Handler calls `TestExecutionService::delete_execution(project_id, run_id,
   execution_id, current_user_id)`.
6. `TestExecutionService` checks `test_execution:delete` system permission.
7. `TestExecutionService` checks the user is at least a Contributor of the project. If
   Viewer or not a member, and not System Admin -> `403`.
8. `TestExecutionService` checks if the user is a Contributor: if so, verify
   `execution.created_by == current_user_id`. If not matching -> `403` (ownership
   restriction).
9. `TestExecutionService` calls
   `TestExecutionRepository::soft_delete(execution_id, current_user_id)` which sets
   `deleted_at = NOW()`, `deleted_by = current_user_id`. No referential integrity check
   is performed.
10. Handler returns `204 No Content`.

### List Test Executions Flow

1. Client sends
   `GET /api/v1/projects/{projectId}/test-runs/{runId}/executions?page=1&limit=25&search=regression&sort=-id`
   with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler validates the project exists and is not soft-deleted.
4. Handler validates the Test Run exists, is not soft-deleted, and belongs to the
   project.
5. Handler calls
   `TestExecutionService::list_executions(project_id, run_id, query, current_user_id)`.
6. `TestExecutionService` checks `test_execution:read_list` system permission.
7. `TestExecutionService` checks the user is a member of the project (any role) or
   System Admin.
8. `TestExecutionService` calls
   `TestExecutionRepository::find_by_run(run_id, page, limit, search, sort)`.
9. Repository executes a parameterized query with
   `WHERE test_run_id = $1 AND deleted_at IS NULL`, optional `ILIKE` filter on `name`
   if `search` is provided, and `ORDER BY` based on `sort`. The query JOINs
   `TEST_EXECUTION_TESTERS` and `USERS` to include tester details.
10. Repository returns the paginated results and total count.
11. `TestExecutionService` returns the response DTO with `data` and `meta`.
12. Handler returns `200 OK`.

### Select Test Executions Flow

1. Client sends `GET /api/v1/projects/{projectId}/test-runs/{runId}/executions/select`
   with session cookie.
2-4. Same as List: validate session, project, and Test Run.
5. Handler calls
   `TestExecutionService::select_executions(project_id, run_id, current_user_id)`.
6. `TestExecutionService` checks `test_execution:select` system permission.
7. `TestExecutionService` checks the user is a project member (any role) or System
   Admin.
8. `TestExecutionService` calls
   `TestExecutionRepository::find_all_active_by_run(run_id)` which selects only `id`
   and `name`, ordered by `LOWER(name)`.
9. Handler returns `200 OK` with the flat array.

### Get Execution Detail Flow

1. Client sends
   `GET /api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}` with session
   cookie.
2-4. Same as List: validate session, project, and Test Run.
5. Handler calls
   `TestExecutionService::get_execution(project_id, run_id, execution_id,
   current_user_id)`.
6. `TestExecutionService` checks `test_execution:read` system permission.
7. `TestExecutionService` checks the user is a project member (any role) or System
   Admin.
8. `TestExecutionService` calls
   `TestExecutionRepository::find_by_id(execution_id, run_id)` which executes a query
   with LEFT JOIN on `TEST_EXECUTION_TESTERS` and `USERS` to resolve tester details.
9. Handler returns `200 OK`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestExecution` | Domain (1) | Entity: `id`, `name`, `test_run_id`, audit fields. Factory method `create(name, test_run_id, tester_ids, created_by)` performs domain validation (name not empty, tester_ids non-empty). Method `apply_update(cmd)` returns a modified entity with changed fields validated. No ORM or framework imports. |
| `TesterAssignment` | Domain (1) | Value object: `execution_id`, `user_id`. Simple pair struct with no domain logic beyond construction. |
| `TestExecutionService` | Application (2) | Orchestrates all execution use cases: `create_execution`, `list_executions`, `get_execution`, `update_execution`, `delete_execution`, `select_executions`. Each method checks the required system permission, project membership, and (for Contributor update/delete) ownership. Then delegates to the repository. On create, invokes `TestCaseImportService` after the execution row is inserted. |
| `TestExecutionRepository` | Application (2) | Interface (port): `find_by_id(execution_id, run_id)`, `find_by_name_in_run(run_id, name)`, `find_by_run(run_id, page, limit, search, sort)`, `find_all_active_by_run(run_id)`, `save(execution, tester_ids)`, `update(execution, tester_ids?)`, `soft_delete(execution_id, deleted_by)`. Also `validate_testers_in_project(tester_ids, project_id)` for tester validation. |
| `TestExecutionHandler` | Adapters (3) | HTTP handler with six methods (`list`, `create`, `get`, `update`, `delete`, `select`). Deserializes requests, calls `TestExecutionService`, serializes responses. The select handler is registered before the `/{id}` handler. |
| `SqlTestExecutionRepository` | Infrastructure (4) | Implements `TestExecutionRepository` using PostgreSQL. All queries include `WHERE deleted_at IS NULL` for active record filtering. Handles junction table operations for tester assignments. Uses parameterized queries exclusively. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (Application) | Add permission codes `test_execution:create`, `test_execution:read`, `test_execution:read_list`, `test_execution:update`, `test_execution:delete`, `test_execution:select` to the permission registry. Add role-based checks: Contributor/Editor/Owner can create; all members can read/read_list/select; Contributor (own only)/Editor/Owner can update/delete; Viewer is excluded from all mutations. |
| Seed migration (Infrastructure, from `iam-permissions`) | Add 6 new permission rows for `test_execution:*` codes. |
| HTTP router registration | Register six new routes under `/api/v1/projects/{projectId}/test-runs/{runId}/executions/` -- all require session auth. Route order: `/select` before `/{id}`. |

---

## Route Registration

```text
# Order matters: /select must be registered before /{id}
GET    /api/v1/projects/{projectId}/test-runs/{runId}/executions          -> list
POST   /api/v1/projects/{projectId}/test-runs/{runId}/executions          -> create
GET    /api/v1/projects/{projectId}/test-runs/{runId}/executions/select   -> select   (static path)
GET    /api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}     -> get      (dynamic path)
PATCH  /api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}     -> update
DELETE /api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}     -> delete
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- Test Run existence validation (Test Run not soft-deleted, belongs to project)
- System permission check (respective `test_execution:*` code)
- Project membership check:
  - `POST`: Contributor, Editor, or Owner
  - `PATCH`, `DELETE`: Contributor (own only), Editor, or Owner
  - `GET` (all read endpoints): any project role
- Contributor ownership check (on `PATCH` and `DELETE` only)

---

## New Permission Codes

These must be added to the `PERMISSIONS` table seed data. Use `INSERT ... ON CONFLICT
(code) DO NOTHING` and let the database assign IDs -- do not hardcode numeric IDs in the
migration:

| code | name |
|------|------|
| `test_execution:create` | Create Test Execution |
| `test_execution:read` | Read Test Execution |
| `test_execution:read_list` | Read Test Execution List |
| `test_execution:update` | Update Test Execution |
| `test_execution:delete` | Delete Test Execution |
| `test_execution:select` | Select Test Execution |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks required system permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks required project role | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Contributor does not own the execution | `403` | `FORBIDDEN` | INFO | Distinct message: "Contributors can only update/delete their own executions" |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before any execution operation |
| Test Run not found, soft-deleted, or wrong project | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Execution not found, soft-deleted, or wrong Test Run | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Duplicate execution name in Test Run | `409` | `DUPLICATE_EXECUTION_NAME` | INFO | Case-insensitive; only among non-deleted rows |
| Invalid tester ID(s) | `422` | `INVALID_TESTER` | INFO | Includes which specific IDs are invalid and why |
| Validation error (bad request body) | `422` | `VALIDATION_ERROR` | INFO | Includes field-level details; unrecognised fields rejected in strict mode |
| Invalid pagination, sort, or search value | `422` | `VALIDATION_ERROR` | INFO | page < 1 or page > 1000, limit < 1 or limit > 100, invalid sort, search > 255 chars |
| DB duplicate key violation (race condition) | `409` | `DUPLICATE_EXECUTION_NAME` | INFO | Caught from PostgreSQL error 23505 |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Per-endpoint limits; returns `Retry-After` header |
| Whitespace-only name | `422` | `VALIDATION_ERROR` | INFO | Name must contain at least one non-whitespace character |
| Test case import failure | `502` | `IMPORT_FAILED` | ERROR | Transaction is rolled back; execution is not created |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not reveal** whether a `404` is caused by a non-existent project, non-existent
  Test Run, non-existent execution, soft-deleted execution, or wrong-parent execution
  (same message for all).
- **Do not return `400`** for business logic errors like duplicate name or invalid
  testers -- use `409 Conflict` or `422 Unprocessable Entity` as specified.
- **Do not hard-delete** any record -- all deletions are soft-deletes.
- **Do not allow UPDATE on soft-deleted rows** -- the repository layer checks
  `deleted_at IS NULL` before applying mutations.
- **Do not omit `deleted_by`** -- every soft-delete sets both `deleted_at` and
  `deleted_by`.
- **Do not check for referential integrity on execution delete** -- test case results
  are preserved and hidden by query filters.
- **Do not use `GET` with a body** -- all state changes use `POST`, `PATCH`, or
  `DELETE`.
- **Do not allow cross-project tester assignment** -- tester validation always checks
  project membership.
- **Do not create an execution without testers** -- `tester_ids` must be non-empty on
  create and on update (when provided).
