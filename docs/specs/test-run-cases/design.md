# Design: Test Run Cases

## Architecture

The Test Run Cases feature follows Clean Architecture layers. It introduces a single
junction table (`TEST_RUN_TEST_CASES`) linking test runs to test cases, and spans all
four layers.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_cases   GET    /api/v1/projects/{pid}/test-runs/{id}/cases    │   │
│  │  - add_cases    POST   /api/v1/projects/{pid}/test-runs/{id}/cases    │   │
│  │  - remove_case  DELETE /api/v1/projects/{pid}/test-runs/{id}/cases/{cid}│  │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  RunTestCaseService:                      │  │  - RunTestCase (entity) │  │
│  │  - list_cases                             │  └──────────────────────────┘  │
│  │  - add_cases                              │                                │
│  │  - remove_case                            │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - RunTestCaseRepository (port)           │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlRunTestCaseRepository  (implements RunTestCaseRepository)        │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All endpoints require an authenticated session (checked by `AuthMiddleware`).
2. `GET` requires `test_run:read` system permission and project membership (any role).
3. `POST` and `DELETE` require `test_run:update` system permission and project membership
   with role Owner, Editor, or Contributor. Contributors additionally require the test run
   to be owned by them (`created_by` matches authenticated user ID).
4. The `POST` endpoint accepts a batch of test case IDs, validates all of them (existence,
   active, same project, not already in run), then inserts all in a single transaction.
5. The `DELETE` endpoint removes a single test case from a run (hard delete on the
   junction row). If the case is not in the run, `404 Not Found` is returned.

### Security Requirements

**Input sanitization:** This feature accepts only numeric IDs as input (no free-text
fields). No HTML/script sanitization is required at the handler boundary. The `summary`
field included in list responses is sanitized on input at the `test-case-crud` boundary.

**CSRF protection:** All state-changing endpoints (`POST`, `DELETE`) must be protected
against CSRF. Session cookies must carry `SameSite=Lax` (or stricter). `POST`/`DELETE`
requests with `Content-Type: application/json` must verify the `Content-Type` header to
block simple form-based CSRF attacks.

**Authorization layering:** Three independent authorization gates apply:

1. System permission check (`test_run:read` or `test_run:update`)
2. Project membership and role check (Owner, Editor, or Contributor for mutations; any
   role for reads; Viewer excluded from all mutations)
3. Contributor ownership check (on POST and DELETE only): if the user's project role is
   Contributor, verify `test_run.created_by` matches the authenticated user ID

All three gates must pass (or the caller must be a System Admin, who implicitly holds all
system permissions, bypasses all project membership checks, and bypasses the ownership
check). Authorization checks are performed against live data on every request -- permissions
and project membership are never cached in the session.

**Race condition protection:** The `POST` handler uses `SELECT ... FOR UPDATE` on the
`TEST_RUNS` row at the start of the transaction to serialize concurrent additions to the
same run. This prevents the TOCTOU race where two concurrent requests both pass the
duplicate check before either inserts.

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

### GET `/api/v1/projects/{projectId}/test-runs/{runId}/cases`

List test cases assigned to a test run, with resolved category and priority names.

**Required Permission:** `test_run:read`

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

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "test_case_id": 42,
      "summary": "User can log in with valid credentials",
      "category_id": 3,
      "category_name": "Login",
      "priority_id": 1,
      "priority_name": "Critical",
      "automated": true,
      "added_by": 15,
      "added_at": "2026-07-15T10:00:00Z"
    },
    {
      "test_case_id": 85,
      "summary": "Session expires after 30 minutes of inactivity",
      "category_id": null,
      "category_name": null,
      "priority_id": 2,
      "priority_name": "High",
      "automated": false,
      "added_by": 15,
      "added_at": "2026-07-15T10:05:00Z"
    }
  ],
  "meta": {
    "total": 25,
    "page": 1,
    "limit": 25
  }
}
```

**Notes:**
- Results are ordered by `added_at` ascending (oldest first -- the order cases were
  added to the run).
- `category_name` is resolved by LEFT JOIN on `test_categories`. If `category_id` is null
  or the category is soft-deleted, `category_name` is `null`.
- `priority_name` is resolved by LEFT JOIN on `test_priorities`. If `priority_id` is null
  or the priority is soft-deleted, `priority_name` is `null`.
- Soft-deleted test cases are excluded from the list. If a test case was soft-deleted
  after being added to the run, it is no longer returned.
- The test run must belong to the specified project; if it belongs to a different project,
  `404 Not Found` is returned.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_run:read` permission or is not a project member |
| `404` | `NOT_FOUND` | Project or test run does not exist, is soft-deleted, or run belongs to a different project |
| `422` | `VALIDATION_ERROR` | Invalid pagination parameters (page < 1, limit < 1 or limit > 100) |

---

### POST `/api/v1/projects/{projectId}/test-runs/{runId}/cases`

Add one or more test cases to a test run in a single batch operation.

**Required Permission:** `test_run:update` AND project role Owner, Editor, or Contributor
(Contributor only on runs they own)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `runId` | integer | Test Run ID |

**Request Body:**

```json
{
  "test_case_ids": [42, 85, 128]
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `test_case_ids` | array of integer | Yes | Non-empty array; each ID must be a positive integer |

**Success Response:** `201 Created`

```json
{
  "data": {
    "added": [
      { "test_case_id": 42, "summary": "User can log in with valid credentials" },
      { "test_case_id": 85, "summary": "Session expires after 30 minutes of inactivity" },
      { "test_case_id": 128, "summary": "Password reset email is sent within 60 seconds" }
    ]
  }
}
```

**Notes:**
- All insertions execute within a single database transaction (all-or-nothing).
- Before inserting, the system acquires a row-level lock on the test run (`SELECT ...
  FOR UPDATE`) to serialize concurrent modifications.
- Duplicate test case IDs within the request array are deduplicated before validation
  (submitting `[42, 42, 85]` is treated as `[42, 85]`).
- Each added entry records `added_by` as the authenticated user's ID and `added_at` as
  the current timestamp.
- Test cases that are soft-deleted are rejected (same as non-existent).
- No partial success: if any validation fails, the entire batch is rejected.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_run:update` permission, is a Viewer, or (for Contributors) does not own the run |
| `404` | `NOT_FOUND` | Project or test run does not exist, is soft-deleted, or run belongs to a different project |
| `409` | `TEST_CASE_ALREADY_IN_RUN` | One or more test cases are already in the run |
| `422` | `VALIDATION_ERROR` | `test_case_ids` array is empty or missing |
| `422` | `INVALID_TEST_CASE` | One or more test case IDs do not exist, are soft-deleted, or belong to a different project |

`409 Conflict` response body:

```json
{
  "error": {
    "code": "TEST_CASE_ALREADY_IN_RUN",
    "message": "One or more test cases are already in this test run.",
    "details": [
      { "field": "test_case_ids", "message": "Test cases [42, 85] are already in the run" }
    ]
  }
}
```

`422` response body for invalid test cases:

```json
{
  "error": {
    "code": "INVALID_TEST_CASE",
    "message": "One or more test cases are invalid or do not belong to this project.",
    "details": [
      { "field": "test_case_ids", "message": "Test cases [999, 888] do not exist, are deleted, or belong to a different project" }
    ]
  }
}
```

---

### DELETE `/api/v1/projects/{projectId}/test-runs/{runId}/cases/{testCaseId}`

Remove a single test case from a test run.

**Required Permission:** `test_run:update` AND project role Owner, Editor, or Contributor
(Contributor only on runs they own)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `runId` | integer | Test Run ID |
| `testCaseId` | integer | Test Case ID to remove from the run |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Hard-deletes the junction row. No soft-delete on junction tables.
- The test case itself is not affected -- only the link to the run is removed.
- No referential integrity check is performed against test execution snapshots.
  Removing a case from a run does not affect any execution that already imported it.
- If the test case is not currently in the run, `404 Not Found` is returned.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_run:update` permission, is a Viewer, or (for Contributors) does not own the run |
| `404` | `NOT_FOUND` | Project, test run, or case-in-run does not exist; or run belongs to a different project |

---

## Data Model

### New Table: TEST_RUN_TEST_CASES

A junction table linking test runs to test cases. Hard-delete only (no `deleted_at` /
`deleted_by`). The table carries `added_at` and `added_by` to track when and by whom
each test case was added to the run.

```sql
CREATE TABLE test_run_test_cases (
    run_id       BIGINT       NOT NULL REFERENCES test_runs(id) ON DELETE RESTRICT,
    test_case_id BIGINT       NOT NULL REFERENCES test_cases(id) ON DELETE RESTRICT,
    added_by     BIGINT       NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    added_at     TIMESTAMPTZ  NOT NULL DEFAULT NOW(),

    -- Composite primary key
    CONSTRAINT pk_test_run_test_cases PRIMARY KEY (run_id, test_case_id)
);

-- Index for lookups by run (list cases in a run)
CREATE INDEX idx_trtc_run_id ON test_run_test_cases(run_id);

-- Index for reverse lookups (find runs containing a test case)
CREATE INDEX idx_trtc_test_case_id ON test_run_test_cases(test_case_id);

-- Covering index for the most frequent query: list cases with test case details
CREATE INDEX idx_trtc_run_added ON test_run_test_cases(run_id, added_at)
    INCLUDE (test_case_id, added_by);
```

**Design notes:**

- **Composite PK** `(run_id, test_case_id)` -- enforces uniqueness at the database level.
  Prevents adding the same test case to a run twice.
- **No surrogate `id` column** -- the composite PK is the natural key for this junction.
- **`ON DELETE RESTRICT` on `run_id`** -- prevents hard-deleting a test run that still has
  linked test cases. Test run soft-delete does not cascade to this table.
- **`ON DELETE RESTRICT` on `test_case_id`** -- prevents hard-deleting a test case that
  is linked to a test run. Test case soft-delete does not remove links; instead, soft-deleted
  test cases are filtered out at query time (`WHERE tc.deleted_at IS NULL`).
- **`ON DELETE RESTRICT` on `added_by`** -- prevents deleting a user who has added test
  cases to runs, preserving audit trail integrity.
- **Covering index** `idx_trtc_run_added` -- supports the list query pattern: filter by
  `run_id`, order by `added_at`, with `test_case_id` and `added_by` included to avoid
  heap fetches.
- **Hard delete** -- when a test case is removed from a run, the row is `DELETE`d entirely.
  No soft-delete on junction tables (per FR-52, same convention as `PROJECT_MEMBERS`).
- **No `updated_at` / `updated_by`** -- unlike `PROJECT_MEMBERS`, this junction has no
  mutable columns. The link is either present (with `added_at`/`added_by`) or absent.

### Relationship to Other Tables

```
TEST_RUNS ──< TEST_RUN_TEST_CASES >── TEST_CASES
                  │
                  └── TEST_CATEGORIES (via test_cases.category_id)
                  └── TEST_PRIORITIES (via test_cases.priority_id)
```

- `TEST_RUN_TEST_CASES.run_id` -> `TEST_RUNS.id`
- `TEST_RUN_TEST_CASES.test_case_id` -> `TEST_CASES.id`
- `TEST_RUN_TEST_CASES.added_by` -> `USERS.id`

### List Query (JOIN Pattern)

```sql
SELECT
    trtc.test_case_id,
    tc.summary,
    tc.category_id,
    tcat.name AS category_name,
    tc.priority_id,
    tpri.name AS priority_name,
    tc.automated,
    trtc.added_by,
    trtc.added_at
FROM test_run_test_cases trtc
JOIN test_cases tc ON tc.id = trtc.test_case_id AND tc.deleted_at IS NULL
LEFT JOIN test_categories tcat ON tcat.id = tc.category_id AND tcat.deleted_at IS NULL
LEFT JOIN test_priorities tpri ON tpri.id = tc.priority_id AND tpri.deleted_at IS NULL
WHERE trtc.run_id = $1
ORDER BY trtc.added_at ASC
LIMIT $2 OFFSET $3;
```

### Add Validation Query

Before inserting into `TEST_RUN_TEST_CASES`, validate each test case:

```sql
-- Validate test cases exist, are active, and belong to the same project
SELECT id, summary
FROM test_cases
WHERE id = ANY($1)
  AND project_id = $2
  AND deleted_at IS NULL;
```

If the count of returned rows is less than the count of requested IDs, some IDs are
invalid. The missing IDs are reported in the error response.

---

## Sequence

### List Cases Flow

1. Client sends `GET /api/v1/projects/{projectId}/test-runs/{runId}/cases?page=1&limit=25`
   with session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates the project exists and is not soft-deleted.
4. Handler validates the test run exists, is not soft-deleted, and belongs to the project.
   If any check fails -> `404 Not Found`.
5. Handler calls `RunTestCaseService::list_cases(run_id, page, limit, current_user_id)`.
6. `RunTestCaseService` checks the user has `test_run:read` system permission
   (via `AuthorizationService`).
7. `RunTestCaseService` checks the user is a project member (any role) or System Admin.
8. `RunTestCaseService` calls `RunTestCaseRepository::find_by_run(run_id, page, limit)`.
9. Repository executes the JOIN query with `WHERE trtc.run_id = $1 AND tc.deleted_at IS NULL`,
   ordered by `trtc.added_at ASC`, with pagination.
10. Repository returns paginated results and total count.
11. `RunTestCaseService` returns the response DTO with `data` and `meta`.
12. Handler returns `200 OK`.

### Add Cases Flow

1. Client sends `POST /api/v1/projects/{projectId}/test-runs/{runId}/cases` with
   `{"test_case_ids": [42, 85, 128]}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context.
3. Handler deserializes and validates the request body (array present, non-empty, all
   positive integers). Deduplicates the array.
4. Handler validates the project exists and is not soft-deleted.
5. Handler validates the test run exists, is not soft-deleted, and belongs to the project.
   If any check fails -> `404 Not Found`.
6. Handler calls `RunTestCaseService::add_cases(project_id, run_id, test_case_ids,
   current_user_id)`.
7. `RunTestCaseService` checks `test_run:update` system permission.
8. `RunTestCaseService` checks the user is a Contributor, Editor, or Owner of the project.
   If Viewer or not a member, and not System Admin -> `403`.
9. `RunTestCaseService` checks if the user is a Contributor: if so, verify
   `test_run.created_by == current_user_id`. If not matching -> `403` (ownership
   restriction).
10. `RunTestCaseService` begins a database transaction.
11. Within the transaction:
    a. `SELECT ... FOR UPDATE` on the `TEST_RUNS` row to lock it against concurrent
       modifications to the same run's case set.
    b. Validate all test case IDs: query `TEST_CASES` to verify each exists, is not
       soft-deleted, and has `project_id` matching the run's project. If any are invalid
       -> roll back and return `422` with `INVALID_TEST_CASE`.
    c. Check for duplicates: query `TEST_RUN_TEST_CASES` for any of the IDs already in
       the run. If any found -> roll back and return `409` with
       `TEST_CASE_ALREADY_IN_RUN`.
    d. Insert all junction rows: `INSERT INTO test_run_test_cases (run_id, test_case_id,
       added_by) VALUES ...` for each test case ID.
    e. If any INSERT fails with a PostgreSQL duplicate key violation (error 23505), catch
       it and return `409 Conflict` as a fallback for concurrent inserts that bypassed the
       duplicate check.
    f. Fetch summaries for the added test cases to include in the response.
12. Transaction commits.
13. Handler returns `201 Created` with the list of added test case IDs and summaries.

### Remove Case Flow

1. Client sends `DELETE /api/v1/projects/{projectId}/test-runs/{runId}/cases/{testCaseId}`
   with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler validates the project exists and is not soft-deleted.
4. Handler validates the test run exists, is not soft-deleted, and belongs to the project.
   If any check fails -> `404 Not Found`.
5. Handler calls `RunTestCaseService::remove_case(project_id, run_id, test_case_id,
   current_user_id)`.
6. `RunTestCaseService` checks `test_run:update` system permission.
7. `RunTestCaseService` checks the user is a Contributor, Editor, or Owner of the project.
   If Viewer or not a member -> `403`.
8. `RunTestCaseService` checks if the user is a Contributor: if so, verify
   `test_run.created_by == current_user_id`. If not matching -> `403`.
9. `RunTestCaseService` calls `RunTestCaseRepository::delete(run_id, test_case_id)` which
   executes `DELETE FROM test_run_test_cases WHERE run_id = $1 AND test_case_id = $2`.
10. If no rows were deleted (test case not in run), return `404 Not Found`.
11. Handler returns `204 No Content`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `RunTestCase` | Domain (1) | Entity representing a test case linked to a run: `run_id`, `test_case_id`, `added_by`, `added_at`. Constructor validates all IDs are positive. No ORM or framework imports. |
| `RunTestCaseService` | Application (2) | Orchestrates use cases: `list_cases`, `add_cases`, `remove_case`. Each method checks the required system permission, project membership, and (for Contributor mutations) ownership. Delegates to the repository. |
| `RunTestCaseRepository` | Application (2) | Interface (port): `find_by_run(run_id, page, limit) -> (Vec<RunTestCaseWithDetail>, total)`, `exists(run_id, test_case_id) -> bool`, `insert_batch(entries: Vec<RunTestCase>)`, `delete(run_id, test_case_id) -> bool`. Also `validate_test_case_ids(ids: &[i64], project_id: i64) -> Result<Vec<(i64, String)>>` for pre-insert validation. |
| `RunTestCaseHandler` | Adapters (3) | HTTP handler with three methods (`list_cases`, `add_cases`, `remove_case`). Deserializes requests, calls `RunTestCaseService`, serializes responses. |
| `SqlRunTestCaseRepository` | Infrastructure (4) | Implements `RunTestCaseRepository` using PostgreSQL. Executes JOIN queries for listing, batch INSERT for adding, and DELETE for removing. All queries use parameterized placeholders. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (Application) | Add permission codes `test_run:read` and `test_run:update` to the permission registry. These codes are shared with `test-run-crud`. |
| Seed migration (Infrastructure, from `iam-permissions`) | Add 2 new permission rows for `test_run:*` codes. |
| HTTP router registration | Register three new routes under `/api/v1/projects/{projectId}/test-runs/{runId}/cases` -- all require session auth. |

---

## Route Registration

```text
GET    /api/v1/projects/{projectId}/test-runs/{runId}/cases           -> list_cases
POST   /api/v1/projects/{projectId}/test-runs/{runId}/cases           -> add_cases
DELETE /api/v1/projects/{projectId}/test-runs/{runId}/cases/{testCaseId} -> remove_case
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- Test run existence validation (run not soft-deleted, belongs to project)
- System permission check (`test_run:read` for GET, `test_run:update` for POST/DELETE)
- Project membership check:
  - `GET`: any project role
  - `POST`, `DELETE`: Owner, Editor, or Contributor
- Contributor ownership check (on `POST` and `DELETE` only)

---

## New Permission Codes

These must be added to the `PERMISSIONS` table seed data. Use `INSERT ... ON CONFLICT (code)
DO NOTHING` and let the database assign IDs -- do not hardcode numeric IDs in the migration:

| code | name |
|------|------|
| `test_run:read` | Read Test Run |
| `test_run:update` | Update Test Run |

These codes are shared with `test-run-crud`. The `test_run:read` permission also gates the
run detail view in `test-run-crud`; `test_run:update` also gates run field updates.

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks required system permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks required project role | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Contributor does not own the test run | `403` | `FORBIDDEN` | INFO | Distinct message: "Contributors can only modify their own test runs" |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before any operation |
| Test run not found, soft-deleted, or wrong project | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Test case not in run (on remove) | `404` | `NOT_FOUND` | INFO | Same 404 as other not-found cases |
| Test case already in run (on add) | `409` | `TEST_CASE_ALREADY_IN_RUN` | INFO | Lists conflicting IDs |
| Test case invalid (not found, deleted, wrong project) | `422` | `INVALID_TEST_CASE` | INFO | Lists invalid IDs |
| Empty or missing `test_case_ids` array | `422` | `VALIDATION_ERROR` | INFO | Field-level detail: `test_case_ids` |
| Invalid pagination parameters | `422` | `VALIDATION_ERROR` | INFO | page < 1 or page > 1000, limit < 1 or limit > 100 |
| DB duplicate key violation (race condition) | `409` | `TEST_CASE_ALREADY_IN_RUN` | INFO | Caught from PostgreSQL error 23505 |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Per-endpoint limits; returns `Retry-After` header |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not reveal** whether a `404` is caused by a non-existent project, non-existent test
  run, soft-deleted run, or wrong-project run (same message for all).
- **Do not return `400`** for business logic errors like duplicate test case or invalid
  test case -- use `409 Conflict` or `422 Unprocessable Entity` as specified.
- **Do not hard-delete** test cases or test runs -- only the junction row is hard-deleted.
- **Do not use soft-delete on the junction table** -- junction tables use hard delete.
- **Do not cascade** removals to test execution snapshots.
- **Do not allow partial success** on batch add -- the operation is all-or-nothing.
- **Do not use `GET` with a body** -- all state changes use `POST` or `DELETE`.
- **Do not skip the `SELECT ... FOR UPDATE` lock** on `POST` -- it prevents race
  conditions on concurrent additions.
