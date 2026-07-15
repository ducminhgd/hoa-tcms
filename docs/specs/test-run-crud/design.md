# Design: Test Run CRUD

## Architecture

The Test Run CRUD feature follows Clean Architecture layering. Test runs are per-project
entities that group test cases for execution. All endpoints are nested under the project
resource path (`/api/v1/projects/{projectId}/test-runs`) and require both system RBAC
permission and project membership scope checks. Contributors additionally have
ownership-based restrictions on update and delete operations.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_test_runs     GET    /api/v1/projects/{pid}/test-runs         │   │
│  │  - create_test_run    POST   /api/v1/projects/{pid}/test-runs         │   │
│  │  - get_test_run       GET    /api/v1/projects/{pid}/test-runs/{id}    │   │
│  │  - update_test_run    PATCH  /api/v1/projects/{pid}/test-runs/{id}    │   │
│  │  - delete_test_run    DELETE /api/v1/projects/{pid}/test-runs/{id}    │   │
│  │  - select_test_runs   GET    /api/v1/projects/{pid}/test-runs/select  │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  TestRunService:                          │  │  - TestRun (entity)     │  │
│  │  - create_test_run                        │  └──────────────────────────┘  │
│  │  - list_test_runs                         │                                │
│  │  - get_test_run                           │                                │
│  │  - update_test_run                        │                                │
│  │  - delete_test_run                        │                                │
│  │  - select_test_runs                       │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - TestRunRepository (port)               │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestRunRepository  (implements TestRunRepository)                │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All test run endpoints require an authenticated session (checked by `AuthMiddleware`).
2. `POST` endpoints require the caller to be a Contributor, Editor, or Owner of the project
   (or a System Admin), on top of the system permission check. Viewers cannot create.
3. `PATCH` and `DELETE` endpoints require the caller to be at least a Contributor, with an
   additional ownership check for Contributors: they can only update or delete test runs
   where `created_by` matches their user ID. Owners and Editors can update or delete any
   test run in the project.
4. `GET` endpoints require any project membership (or System Admin).
5. Create validates unique summary, plan FK (same project), and user FKs (report_to,
   default_tester) within a single database transaction to prevent TOCTOU races.
6. Update validates unique summary (if summary is being changed), plan FK, and user FKs
   within a single database transaction. Rejects updates on soft-deleted records.
7. Soft-delete does not cascade to child entities (test run cases, test executions).
   Child entities remain and are hidden via query filtering through the parent test run.

### Security Requirements

**Input sanitization:** Test run `summary`, `version`, and `notes` fields must be sanitized
on input (strip disallowed HTML tags) before storage. Output-encoding must be applied at the
presentation layer. See PRD 5.3 for the project-wide XSS prevention policy.

**CSRF protection:** All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must be
protected against CSRF. Session cookies must carry `SameSite=Lax` (or stricter).
`POST`/`PATCH`/`DELETE` requests with `Content-Type: application/json` must verify the
`Content-Type` header to block simple form-based CSRF attacks. Additional anti-CSRF
tokens should be considered for defense in depth.

**Authorization layering:** Three independent authorization gates apply:

1. System permission check (e.g., `test_run:update`)
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

**Audit field visibility:** The `created_by` field is included in list and detail API
responses; `updated_by` is included in detail and create responses only (not in list or
select). Both fields are visible to all project members (including Viewers). The fields
carry numeric user IDs, not emails or names, limiting direct PII exposure.

**Rate limiting:** All endpoints are protected by rate limiting configured at the middleware
layer. State-changing endpoints (POST/PATCH/DELETE) allow 30 req/min; read endpoints (GET
list/detail) allow 60 req/min; the select endpoint allows 120 req/min due to frequent UI
usage. See requirements.md Security Considerations for the full table.

**FK validation (same-project constraint):** Before inserting or updating a test run with a
non-null `plan_id`, the system must verify that the referenced plan exists, is not
soft-deleted, and has a `project_id` matching the test run's `project_id`. This prevents
cross-project data injection. The validation query is:

```sql
SELECT 1 FROM test_plans
WHERE id = $1 AND project_id = $2 AND deleted_at IS NULL;
```

For `report_to` and `default_tester` user FKs, only existence and active-status are checked
(users are global, not project-scoped):

```sql
SELECT 1 FROM users
WHERE id = $1 AND deleted_at IS NULL;
```

These checks execute within the same database transaction as the insert/update.

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

### GET `/api/v1/projects/{projectId}/test-runs`

List test runs in a project with pagination, filtering, and sorting.

**Required Permission:** `test_run:read_list`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Query Parameters:**

| Param | Type | Default | Max | Description |
|-------|------|---------|-----|-------------|
| `page` | integer | 1 | -- | Page number (1-based) |
| `limit` | integer | 25 | 100 | Items per page |
| `plan_id` | integer | -- | -- | Filter by plan ID |
| `search` | string | -- | 255 | Case-insensitive substring match on `summary` |
| `sort` | string | `-id` | -- | Sort field: `id`, `-id`, `summary`, `-summary`, `created_at`, `-created_at` |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 12,
      "summary": "Sprint 12 Regression Run",
      "version": "v2.4.0",
      "plan_id": 3,
      "report_to": 15,
      "report_to_username": "jdoe",
      "report_to_fullname": "John Doe",
      "default_tester": 22,
      "default_tester_username": "asmith",
      "default_tester_fullname": "Alice Smith",
      "planned_start_date": "2026-07-20",
      "planned_end_date": "2026-07-30",
      "created_by": 15,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_at": "2026-07-14T10:00:00Z"
    },
    {
      "id": 11,
      "summary": "API Integration Smoke Test",
      "version": null,
      "plan_id": null,
      "report_to": 15,
      "report_to_username": "jdoe",
      "report_to_fullname": "John Doe",
      "default_tester": null,
      "default_tester_username": null,
      "default_tester_fullname": null,
      "planned_start_date": null,
      "planned_end_date": null,
      "created_by": 15,
      "created_at": "2026-07-14T09:00:00Z",
      "updated_at": "2026-07-14T09:00:00Z"
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
- Results are scoped to the given project and exclude soft-deleted test runs
  (`WHERE project_id = $1 AND deleted_at IS NULL`).
- Default sort is `-id` (newest first, descending).
- `search` applies `ILIKE` on `summary` when provided; when absent, no filter is applied.
  The search value is escaped: first `\` is doubled to `\\`, then `%` is escaped to `\%`,
  then `_` is escaped to `\_`. The search value must not exceed 255 characters. Leading and
  trailing whitespace is trimmed; an all-whitespace search is treated as "no filter."
- The `sort` parameter accepts: `id` (oldest first), `-id` (newest first, default),
  `summary` (A-Z), `-summary` (Z-A), `created_at` (oldest first), `-created_at` (newest
  first).
- `plan_id` filter: if the referenced plan does not exist, is soft-deleted, or belongs to a
  different project, the result set is empty (not an error).
- User names (`report_to_username`, `report_to_fullname`, etc.) are resolved via LEFT JOIN
  on the `users` table. If the referenced user is soft-deleted, the names are still returned
  (audit integrity).
- List response excludes `notes`, `updated_by`, `deleted_at`, and `deleted_by` to keep the
  payload compact.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_run:read_list` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `422` | `VALIDATION_ERROR` | Invalid pagination, sort parameter, or filter value |

---

### POST `/api/v1/projects/{projectId}/test-runs`

Create a new test run within a project.

**Required Permission:** `test_run:create` AND project role Contributor, Editor, or Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Request Body:**

```json
{
  "summary": "Sprint 12 Regression Run",
  "plan_id": 3,
  "report_to": 15,
  "default_tester": 22,
  "version": "v2.4.0",
  "notes": "Full regression suite for the Sprint 12 release candidate.",
  "planned_start_date": "2026-07-20",
  "planned_end_date": "2026-07-30"
}
```

| Field | Type | Required | Default | Constraints |
|-------|------|----------|---------|-------------|
| `summary` | string | Yes | -- | 1-500 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input (HTML stripped); unique per project (case-insensitive) |
| `report_to` | integer | Yes | -- | Must reference a non-deleted user |
| `plan_id` | integer | No | `null` | Must reference a non-deleted plan in the same project |
| `default_tester` | integer | No | `null` | Must reference a non-deleted user |
| `version` | string | No | `null` | Max 100 characters; sanitized on input (HTML stripped) |
| `notes` | string | No | `null` | Max 10000 characters; sanitized on input (HTML stripped). Empty string `""` stored as-is. Explicit `null` stores `NULL`. Omitting defaults to `null`. |
| `planned_start_date` | string | No | `null` | ISO 8601 date format `YYYY-MM-DD` |
| `planned_end_date` | string | No | `null` | ISO 8601 date format `YYYY-MM-DD`. If both dates are provided, `planned_end_date` must be >= `planned_start_date`. |

**Success Response:** `201 Created`

Headers: `Location: /api/v1/projects/42/test-runs/12`

```json
{
  "data": {
    "id": 12,
    "summary": "Sprint 12 Regression Run",
    "version": "v2.4.0",
    "project_id": 42,
    "plan_id": 3,
    "report_to": 15,
    "report_to_username": "jdoe",
    "report_to_fullname": "John Doe",
    "default_tester": 22,
    "default_tester_username": "asmith",
    "default_tester_fullname": "Alice Smith",
    "notes": "Full regression suite for the Sprint 12 release candidate.",
    "planned_start_date": "2026-07-20",
    "planned_end_date": "2026-07-30",
    "created_by": 15,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 15,
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- `project_id` in the response mirrors the path parameter.
- `created_by` and `updated_by` are both set to the authenticated user's ID.
- User names are resolved via JOIN on create (the referenced user is guaranteed to exist and
  be non-deleted at this point).
- The unique summary check uses `LOWER(summary) = LOWER($1)` and includes
  `WHERE project_id = $2 AND deleted_at IS NULL`.
- Soft-deleted test runs with the same summary do not block creation.
- Plan FK validation and user FK validation run within the same transaction as the insert to
  prevent TOCTOU races.
- Date range validation (`planned_end_date >= planned_start_date`) runs when both dates are
  provided.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_run:create` permission or is not Contributor/Editor/Owner of the project |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `409` | `DUPLICATE_TEST_RUN_SUMMARY` | Test run summary already exists in this project (case-insensitive) |
| `422` | `VALIDATION_ERROR` | Summary empty, exceeds max length; version/notes too long; invalid date format; unrecognised fields |
| `422` | `INVALID_PLAN` | `plan_id` does not exist, is soft-deleted, or belongs to a different project |
| `422` | `INVALID_USER` | `report_to` or `default_tester` does not exist or is soft-deleted |
| `422` | `INVALID_DATE_RANGE` | `planned_end_date` is before `planned_start_date` |

`422` response body for invalid plan:

```json
{
  "error": {
    "code": "INVALID_PLAN",
    "message": "The specified test plan does not exist or does not belong to this project.",
    "details": [
      { "field": "plan_id", "message": "Test plan must exist and belong to the same project" }
    ]
  }
}
```

---

### GET `/api/v1/projects/{projectId}/test-runs/{id}`

Get full detail of a single test run, including resolved user names.

**Required Permission:** `test_run:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Test Run ID |

**Success Response:** `200 OK`

```json
{
  "data": {
    "id": 12,
    "summary": "Sprint 12 Regression Run",
    "version": "v2.4.0",
    "project_id": 42,
    "plan_id": 3,
    "report_to": 15,
    "report_to_username": "jdoe",
    "report_to_fullname": "John Doe",
    "default_tester": 22,
    "default_tester_username": "asmith",
    "default_tester_fullname": "Alice Smith",
    "notes": "Full regression suite for the Sprint 12 release candidate.",
    "planned_start_date": "2026-07-20",
    "planned_end_date": "2026-07-30",
    "created_by": 15,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 15,
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- User names for `report_to` and `default_tester` are resolved by LEFT JOIN on `users`.
  If the user is soft-deleted, the names are still returned.
- If `default_tester` is null, both username and fullname are `null`.
- `deleted_at` and `deleted_by` are never returned to the client.
- The test run must belong to the specified project; if the test run exists but belongs
  to a different project, `404 Not Found` is returned (same as non-existent).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_run:read` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, or test run does not exist / is soft-deleted / belongs to a different project |

---

### PATCH `/api/v1/projects/{projectId}/test-runs/{id}`

Update a test run's fields. Contributors can only update their own test runs.

**Required Permission:** `test_run:update` AND project role Contributor (own only), Editor,
or Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Test Run ID |

**Request Body** (all fields optional; at least one required):

```json
{
  "summary": "Updated Sprint 12 Run",
  "plan_id": null,
  "report_to": 16,
  "default_tester": null,
  "version": "v2.5.0",
  "notes": "Updated notes for the release candidate.",
  "planned_start_date": "2026-07-22",
  "planned_end_date": "2026-08-01"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `summary` | string | No | 1-500 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input (HTML stripped); unique per project (case-insensitive) |
| `plan_id` | integer | No | Must reference a non-deleted plan in the same project. Sending `null` clears the reference. Omitting preserves the current value. |
| `report_to` | integer | No | Must reference a non-deleted user |
| `default_tester` | integer | No | Must reference a non-deleted user. Sending `null` clears the reference. Omitting preserves the current value. |
| `version` | string | No | Max 100 characters; sanitized on input (HTML stripped). Omitting preserves current. `null` clears. `""` stores empty string. |
| `notes` | string | No | Max 10000 characters; sanitized on input (HTML stripped). Omitting preserves current. `null` clears. `""` stores empty string. |
| `planned_start_date` | string | No | ISO 8601 date `YYYY-MM-DD`. Omitting preserves current. `null` clears. |
| `planned_end_date` | string | No | ISO 8601 date `YYYY-MM-DD`. Omitting preserves current. `null` clears. |

**Success Response:** `200 OK`

Response body is the updated test run representation (same shape as GET detail).

**Notes:**
- At least one field must be provided (empty body returns `422`).
- `id`, `project_id`, `created_at`, and `created_by` are immutable.
- `deleted_at` and `deleted_by` cannot be modified through this endpoint.
- `updated_at` and `updated_by` are set automatically on change.
- The unique summary check is only performed if `summary` is provided and differs
  from the current value.
- Soft-deleted test runs cannot be updated (per FR-54c).
- For Contributors: the `created_by` field is checked against the authenticated user's ID.
  If they don't match, `403 Forbidden` is returned with a distinct message.
- Unrecognised fields in the request body cause a `422 Unprocessable Entity` response
  (strict mode -- rejects typos and unknown fields for defence-in-depth).
- The test run must belong to the specified project; if it belongs to a different project,
  `404 Not Found` is returned.
- Date range validation: if both `planned_start_date` and `planned_end_date` end up non-null
  after applying the update (either because both were provided in the request, or one was
  provided and the other was already set), `planned_end_date >= planned_start_date` is
  enforced.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_run:update` permission, is a Viewer, or (for Contributors) does not own the test run |
| `404` | `NOT_FOUND` | Project or test run does not exist, is soft-deleted, or test run belongs to a different project |
| `409` | `DUPLICATE_TEST_RUN_SUMMARY` | Updated summary conflicts with another test run in the same project |
| `422` | `VALIDATION_ERROR` | No fields provided, invalid field values |
| `422` | `INVALID_PLAN` | `plan_id` does not exist, is soft-deleted, or belongs to a different project |
| `422` | `INVALID_USER` | `report_to` or `default_tester` does not exist or is soft-deleted |
| `422` | `INVALID_DATE_RANGE` | Resulting date range is invalid |

---

### DELETE `/api/v1/projects/{projectId}/test-runs/{id}`

Soft-delete a test run. Contributors can only delete their own test runs.

**Required Permission:** `test_run:delete` AND project role Contributor (own only), Editor,
or Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Test Run ID |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Sets `deleted_at = NOW()` and `deleted_by = <current_user_id>`.
- For Contributors: the `created_by` field is checked against the authenticated user's ID.
  If they don't match, `403 Forbidden` is returned with a distinct message.
- No cascade to child entities (test run cases, test executions). Those entities remain and
  are hidden because queries join through the test run and exclude soft-deleted parents.
- Repeated DELETE on an already soft-deleted test run returns `404`.
- The test run must belong to the specified project.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_run:delete` permission, is a Viewer, or (for Contributors) does not own the test run |
| `404` | `NOT_FOUND` | Project or test run does not exist, is soft-deleted, or belongs to a different project |

---

### GET `/api/v1/projects/{projectId}/test-runs/select`

Return a compact list of all non-deleted test runs for dropdown/selection UI components.

**Required Permission:** `test_run:select`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Success Response:** `200 OK`

```json
{
  "data": [
    { "id": 11, "summary": "API Integration Smoke Test" },
    { "id": 5, "summary": "Hotfix Verification Run" },
    { "id": 12, "summary": "Sprint 12 Regression Run" }
  ]
}
```

**Notes:**
- Unpaginated: returns all non-deleted test runs for the project. The SQL query uses
  `LIMIT 1000`. If the result set has 1000 rows, results are truncated, a warning is
  logged, and an `X-Result-Truncated: true` response header is set so the UI can
  surface this to the user.
- Each entry contains only `id` and `summary` (minimal payload for dropdown rendering).
- Results are ordered by `summary` ascending (case-insensitive, `ORDER BY LOWER(summary)`).
- Soft-deleted test runs are excluded.
- Route registration order matters: the `/select` path must be registered **before**
  the `/{id}` path to prevent `select` from being interpreted as an ID.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_run:select` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |

---

## Data Model

### New Table: TEST_RUNS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `project_id` | `BIGINT` | `NOT NULL`, `REFERENCES projects(id) ON DELETE RESTRICT` | Each test run belongs to exactly one project |
| `plan_id` | `BIGINT` | `REFERENCES test_plans(id) ON DELETE RESTRICT` | Nullable; must belong to same project (enforced at app layer) |
| `summary` | `VARCHAR(500)` | `NOT NULL` | Test run summary; unique per project (case-insensitive); see constraint below |
| `report_to` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who receives reports/notifications for this run |
| `default_tester` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Nullable; default tester assigned to cases in this run |
| `version` | `VARCHAR(100)` | | Nullable; free-text version identifier (e.g., "v2.4.0") |
| `notes` | `TEXT` | | Nullable; max 10000 chars enforced at app layer and by `CHECK` constraint below |
| `planned_start_date` | `DATE` | | Nullable; planned start date of the test run |
| `planned_end_date` | `DATE` | | Nullable; planned end date of the test run |
| `created_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who created the test run |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Last user who modified the test run |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by `BEFORE UPDATE` trigger |
| `deleted_by` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Nullable; set on soft-delete |
| `deleted_at` | `TIMESTAMPTZ` | | Nullable; set on soft-delete |

**Constraints:**

```sql
-- Notes length guard (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_runs
  ADD CONSTRAINT chk_test_runs_notes
  CHECK (notes IS NULL OR char_length(notes) <= 10000);

-- Version length guard (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_runs
  ADD CONSTRAINT chk_test_runs_version
  CHECK (version IS NULL OR char_length(version) <= 100);

-- Planned date range guard (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_runs
  ADD CONSTRAINT chk_test_runs_dates
  CHECK (planned_start_date IS NULL OR planned_end_date IS NULL
         OR planned_end_date >= planned_start_date);

-- Case-insensitive unique summary per project (PostgreSQL)
-- Only enforced for non-deleted rows
CREATE UNIQUE INDEX uq_test_runs_summary_project
  ON test_runs (project_id, LOWER(summary))
  WHERE deleted_at IS NULL;

-- Foreign key indexes
CREATE INDEX idx_test_runs_project_id ON test_runs (project_id);
CREATE INDEX idx_test_runs_plan_id ON test_runs (plan_id);
CREATE INDEX idx_test_runs_report_to ON test_runs (report_to);
CREATE INDEX idx_test_runs_default_tester ON test_runs (default_tester);
CREATE INDEX idx_test_runs_created_by ON test_runs (created_by);
CREATE INDEX idx_test_runs_updated_by ON test_runs (updated_by);
CREATE INDEX idx_test_runs_deleted_by ON test_runs (deleted_by);

-- Composite index for common filter pattern (list with plan filter)
CREATE INDEX idx_test_runs_project_plan ON test_runs (project_id, plan_id)
  WHERE deleted_at IS NULL;

-- Partial index for active test runs (most queries filter out soft-deleted)
CREATE INDEX idx_test_runs_active ON test_runs (project_id, id DESC)
  WHERE deleted_at IS NULL;
```

**Design notes:**

- **Partial unique index** `uq_test_runs_summary_project` enforces summary uniqueness only
  among non-deleted rows. This allows creating a new test run with the same summary as a
  previously soft-deleted one.
- **`project_id` FK with `RESTRICT`** prevents deleting a project that has test runs.
  Project soft-delete does not cascade (test runs remain with their original
  `deleted_at IS NULL`; they are hidden because queries filter on
  `WHERE project.deleted_at IS NULL`).
- **`plan_id` FK with `RESTRICT`** prevents hard-deleting a plan that is referenced by test
  runs. The plan soft-delete is already prevented by a check in the test plan feature.
- **`ON DELETE RESTRICT` on user FKs** (`report_to`, `default_tester`, `created_by`,
  `updated_by`, `deleted_by`) prevents deleting a user who is referenced by test runs.
- **`DATE` type for planning dates** rather than `TIMESTAMPTZ`: planning dates represent
  calendar days without time-of-day precision. The ISO 8601 `YYYY-MM-DD` format is used in
  API payloads.
- **`deleted_at` and `deleted_by`** follow the project-wide soft-delete convention: both
  set together on soft-delete, both `NULL` for active records.

### BEFORE UPDATE Trigger

```sql
CREATE OR REPLACE FUNCTION trg_test_runs_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_test_runs_updated_at
  BEFORE UPDATE ON test_runs
  FOR EACH ROW
  EXECUTE FUNCTION trg_test_runs_updated_at();
```

### Relationship to Other Tables

```
PROJECTS ──< TEST_RUNS >── TEST_PLANS
                │
                ├── USERS (report_to)
                ├── USERS (default_tester)
                ├── USERS (created_by)
                ├── USERS (updated_by)
                └── USERS (deleted_by)
```

- `TEST_RUNS.project_id` -> `PROJECTS.id` (each test run belongs to one project)
- `TEST_RUNS.plan_id` -> `TEST_PLANS.id` (optional reference; must belong to same project)
- `TEST_RUNS.report_to` -> `USERS.id` (required; user who receives run reports)
- `TEST_RUNS.default_tester` -> `USERS.id` (optional; default tester for run cases)

---

## Sequence

### Create Test Run Flow

1. Client sends `POST /api/v1/projects/{projectId}/test-runs` with
   `{"summary": "...", "report_to": 15, ...}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler deserializes and validates the request body (summary required, report_to required,
   lengths, date format, strict mode for unrecognised fields).
4. Handler calls `TestRunService::create_test_run(project_id, cmd, current_user_id)`.
5. `TestRunService` checks the user has `test_run:create` system permission
   (via `AuthorizationService`).
6. `TestRunService` checks the user is a Contributor, Editor, or Owner of the project
   (via `ProjectMemberRepository`). If Viewer or not a member, and not System Admin ->
   `403`.
7. `TestRunService` begins a database transaction.
8. Within the transaction:
   a. `SELECT ... FOR UPDATE` on the project row to verify it exists and is not
      soft-deleted. If not found or soft-deleted -> roll back and return `404 Not Found`.
   b. Check for duplicate summary:
      `TestRunRepository::find_by_summary_in_project(project_id, summary)`. If a non-deleted
      duplicate exists -> roll back and return `409 Conflict`.
   c. Validate `report_to` user exists and is not soft-deleted. If invalid -> roll back and
      return `422` with `INVALID_USER`.
   d. If `default_tester` is provided and non-null: validate user exists and is not
      soft-deleted. If invalid -> roll back and return `422` with `INVALID_USER`.
   e. If `plan_id` is provided and non-null: validate the plan exists, is non-deleted, and
      belongs to the same project. If invalid -> roll back and return `422` with
      `INVALID_PLAN`.
   f. Validate date range: if both dates provided, `planned_end_date >= planned_start_date`.
      If invalid -> roll back and return `422` with `INVALID_DATE_RANGE`.
   g. Construct a `TestRun` entity and call `TestRunRepository::save(test_run)`. If the
      INSERT fails with a PostgreSQL duplicate key violation (error 23505), catch it and
      return `409 Conflict` with `DUPLICATE_TEST_RUN_SUMMARY` as a fallback for concurrent
      inserts.
9. Transaction commits.
10. Handler constructs the `Location` header from the new test run ID and returns
    `201 Created`.

### Update Test Run Flow

1. Client sends `PATCH /api/v1/projects/{projectId}/test-runs/{id}` with
   `{"summary": "...", "version": "v2.5.0"}` and session cookie.
2-4. Same as Create: validate session, project exists, test run exists and belongs to
     project and is not soft-deleted.
5. Handler calls `TestRunService::update_test_run(project_id, test_run_id, cmd, current_user_id)`.
6. `TestRunService` checks `test_run:update` system permission.
7. `TestRunService` checks the user is at least a Contributor of the project. If Viewer or
   not a member, and not System Admin -> `403`.
8. `TestRunService` checks if the user is a Contributor: if so, verify
   `test_run.created_by == current_user_id`. If not matching -> `403` (ownership
   restriction). Owners and Editors skip this check.
9. `TestRunService` begins a database transaction.
10. Within the transaction:
    a. If `summary` is provided and differs from current: check for duplicate summary via
       `TestRunRepository::find_by_summary_in_project`. If a different test run has the same
       summary -> roll back and return `409 Conflict`.
    b. If `plan_id` is provided: validate (if non-null) or clear (if null). Same validation
       as create.
    c. If `report_to` is provided: validate user exists and is not soft-deleted. Same
       validation as create.
    d. If `default_tester` is provided: validate (if non-null) or clear (if null). Same
       validation as create.
    e. If `planned_start_date` or `planned_end_date` is provided: recompute the resulting
       date pair (merging provided values with current values) and validate
       `planned_end_date >= planned_start_date` if both are non-null. If invalid -> roll
       back and return `422` with `INVALID_DATE_RANGE`.
    f. Apply updates via `test_run.apply_update(cmd)` and save via repository.
11. Transaction commits.
12. Handler returns `200 OK` with the updated test run.

### Soft-Delete Test Run Flow

1. Client sends `DELETE /api/v1/projects/{projectId}/test-runs/{id}` with session cookie.
2-4. Same as Create: validate session, project exists, test run exists and belongs to
     project and is not soft-deleted.
5. Handler calls `TestRunService::delete_test_run(project_id, test_run_id, current_user_id)`.
6. `TestRunService` checks `test_run:delete` system permission.
7. `TestRunService` checks the user is at least a Contributor of the project. If Viewer or
   not a member, and not System Admin -> `403`.
8. `TestRunService` checks if the user is a Contributor: if so, verify
   `test_run.created_by == current_user_id`. If not matching -> `403` (ownership
   restriction).
9. `TestRunService` calls `TestRunRepository::soft_delete(test_run_id, current_user_id)`
   which sets `deleted_at = NOW()`, `deleted_by = current_user_id`.
10. Handler returns `204 No Content`.

### List Test Runs Flow

1. Client sends `GET /api/v1/projects/{projectId}/test-runs?page=1&limit=25&plan_id=3&search=regression&sort=-id`
   with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler validates the project exists and is not soft-deleted.
4. Handler calls `TestRunService::list_test_runs(project_id, query, current_user_id)`.
5. `TestRunService` checks `test_run:read_list` system permission.
6. `TestRunService` checks the user is a member of the project (any role) or System Admin.
7. `TestRunService` calls `TestRunRepository::find_by_project(project_id, page, limit, filters, search, sort)`.
8. Repository executes a parameterized query with `WHERE project_id = $1 AND deleted_at IS NULL`,
   optional `plan_id` filter, `ILIKE` filter on summary if `search` is provided, and
   `ORDER BY` based on `sort`. User names are resolved via LEFT JOIN on `users`.
9. Repository returns the paginated results and total count.
10. `TestRunService` returns the response DTO with `data` and `meta`.
11. Handler returns `200 OK`.

### Select Test Runs Flow

1. Client sends `GET /api/v1/projects/{projectId}/test-runs/select` with session cookie.
2-3. Same as List: validate session and project.
4. Handler calls `TestRunService::select_test_runs(project_id, current_user_id)`.
5. `TestRunService` checks `test_run:select` system permission.
6. `TestRunService` checks the user is a project member (any role) or System Admin.
7. `TestRunService` calls `TestRunRepository::find_all_active_by_project(project_id)` which
   selects only `id` and `summary`, ordered by `LOWER(summary)`.
8. Handler returns `200 OK` with the flat array.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestRun` | Domain (1) | Entity: `id`, `project_id`, `plan_id`, `summary`, `report_to`, `default_tester`, `version`, `notes`, `planned_start_date`, `planned_end_date`, audit fields. Factory method `create(project_id, summary, report_to, plan_id, default_tester, version, notes, planned_start_date, planned_end_date, created_by)` performs domain validation (summary not empty, dates valid). Method `apply_update(cmd)` returns a modified entity with changed fields validated. No ORM or framework imports. |
| `TestRunService` | Application (2) | Orchestrates all test run use cases: `create_test_run`, `list_test_runs`, `get_test_run`, `update_test_run`, `delete_test_run`, `select_test_runs`. Each method checks the required system permission, project membership, and (for Contributor update/delete) ownership. Then delegates to the repository. |
| `TestRunRepository` | Application (2) | Interface (port): `find_by_id(project_id, test_run_id)`, `find_by_summary_in_project(project_id, summary)`, `find_by_project(project_id, page, limit, filters, search, sort)`, `find_all_active_by_project(project_id)`, `save(test_run)`, `update(test_run)`, `soft_delete(test_run_id, deleted_by)`. Also `validate_plan_in_project(plan_id, project_id)` and `validate_user_exists(user_id)` for FK validation. |
| `TestRunHandler` | Adapters (3) | HTTP handler with six methods (`list`, `create`, `get`, `update`, `delete`, `select`). Deserializes requests, calls `TestRunService`, serializes responses. The select handler is registered before the `/{id}` handler. |
| `SqlTestRunRepository` | Infrastructure (4) | Implements `TestRunRepository` using PostgreSQL. All queries include `WHERE deleted_at IS NULL` for active record filtering. Uses parameterized queries exclusively. Resolves `report_to_username`, `report_to_fullname`, `default_tester_username`, `default_tester_fullname` via LEFT JOIN on `users` on detail and list queries. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (Application) | Add permission codes `test_run:create`, `test_run:read`, `test_run:read_list`, `test_run:update`, `test_run:delete`, `test_run:select` to the permission registry. Add role-based checks: Contributor/Editor/Owner can create; all members can read/read_list/select; Contributor (own only)/Editor/Owner can update/delete; Viewer is excluded from all mutations. |
| Seed migration (Infrastructure, from `iam-permissions`) | Add 6 new permission rows for `test_run:*` codes. |
| HTTP router registration | Register six new routes under `/api/v1/projects/{projectId}/test-runs/` -- all require session auth. Route order: `/select` before `/{id}`. |

---

## Route Registration

```text
# Order matters: /select must be registered before /{id}
GET    /api/v1/projects/{projectId}/test-runs          -> list
POST   /api/v1/projects/{projectId}/test-runs          -> create
GET    /api/v1/projects/{projectId}/test-runs/select   -> select   (static path)
GET    /api/v1/projects/{projectId}/test-runs/{id}     -> get      (dynamic path)
PATCH  /api/v1/projects/{projectId}/test-runs/{id}     -> update
DELETE /api/v1/projects/{projectId}/test-runs/{id}     -> delete
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- System permission check (respective `test_run:*` code)
- Project membership check:
  - `POST`: Contributor, Editor, or Owner
  - `PATCH`, `DELETE`: Contributor (own only), Editor, or Owner
  - `GET` (all read endpoints): any project role
- Contributor ownership check (on `PATCH` and `DELETE` only)

---

## New Permission Codes

These must be added to the `PERMISSIONS` table seed data. Use `INSERT ... ON CONFLICT (code)
DO NOTHING` and let the database assign IDs -- do not hardcode numeric IDs in the migration:

| code | name |
|------|------|
| `test_run:create` | Create Test Run |
| `test_run:read` | Read Test Run |
| `test_run:read_list` | Read Test Run List |
| `test_run:update` | Update Test Run |
| `test_run:delete` | Delete Test Run |
| `test_run:select` | Select Test Run |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks required system permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks required project role | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Contributor does not own the test run | `403` | `FORBIDDEN` | INFO | Distinct message: "Contributors can only update/delete their own test runs" |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before any test run operation |
| Test run not found, soft-deleted, or wrong project | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Duplicate test run summary in project | `409` | `DUPLICATE_TEST_RUN_SUMMARY` | INFO | Case-insensitive; only among non-deleted rows |
| Plan does not exist or wrong project | `422` | `INVALID_PLAN` | INFO | Checked during create and update |
| report_to or default_tester user invalid | `422` | `INVALID_USER` | INFO | User does not exist or is soft-deleted |
| Invalid date range | `422` | `INVALID_DATE_RANGE` | INFO | planned_end_date before planned_start_date |
| Validation error (bad request body) | `422` | `VALIDATION_ERROR` | INFO | Includes field-level details; unrecognised fields rejected in strict mode |
| Invalid pagination, sort parameter, or filter value | `422` | `VALIDATION_ERROR` | INFO | page < 1 or page > 1000, limit < 1 or limit > 100, invalid sort, search > 255 chars, non-integer page/limit |
| Invalid date format | `422` | `VALIDATION_ERROR` | INFO | Date not in YYYY-MM-DD format |
| DB duplicate key violation (race condition) | `409` | `DUPLICATE_TEST_RUN_SUMMARY` | INFO | Caught from PostgreSQL error 23505; mapped to same 409 response |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Per-endpoint limits; returns `Retry-After` header |
| Whitespace-only summary | `422` | `VALIDATION_ERROR` | INFO | Summary must contain at least one non-whitespace character |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not reveal** whether a `404` is caused by a non-existent project, non-existent test
  run, soft-deleted test run, or wrong-project test run (same message for all).
- **Do not return `400`** for business logic errors like duplicate summary, invalid plan,
  or invalid user -- use `409 Conflict` or `422 Unprocessable Entity` as specified.
- **Do not hard-delete** any record -- all deletions are soft-deletes.
- **Do not allow UPDATE on soft-deleted rows** -- the repository layer checks
  `deleted_at IS NULL` before applying mutations.
- **Do not omit `deleted_by`** -- every soft-delete sets both `deleted_at` and
  `deleted_by`.
- **Do not cascade soft-delete** to child entities (test run cases, test executions) --
  parent deletion hides children via query filtering.
- **Do not use `GET` with a body** -- all state changes use `POST`, `PATCH`, or `DELETE`.
- **Do not allow cross-project FK injection** -- plan FK validation always checks
  `project_id` matches the test run's project.
