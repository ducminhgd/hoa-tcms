# Design: Test Run Executions

## Architecture

The Test Run Executions feature is a read-only display endpoint that lists Test
Executions linked to a Test Run. It introduces no new database tables -- it queries
the existing `test_executions` table via the `test_run_id` foreign key. The feature
spans three Clean Architecture layers (no domain entity needed, as it is a pure
query projection):

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handler:                                                         │   │
│  │  - list_test_run_executions  GET /api/v1/projects/{pid}/test-runs/{id}/│   │
│  │                               executions                               │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                                                        │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  ListTestRunExecutionsUseCase   (orchestrates the query)               │   │
│  │                                                                        │   │
│  │  Interfaces:                                                           │   │
│  │  - TestRunExecutionRepository (port -- read-only projection)           │   │
│  └──────────┬───────────────────────────────────────────────────────────┘   │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestRunExecutionRepository  (implements TestRunExecutionRepo)    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. Client sends `GET /api/v1/projects/{projectId}/test-runs/{id}/executions` with a
   session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates `projectId` and `id` as positive integers.
4. Handler verifies the Project exists and is not soft-deleted. If not found or
   soft-deleted -> `404 Not Found`.
5. Handler verifies the Test Run exists, belongs to the project, and is not
   soft-deleted. If not found, soft-deleted, or wrong project -> `404 Not Found`.
6. Handler calls `ListTestRunExecutionsUseCase::execute(project_id, run_id, page, limit, user_id)`.
7. Use case checks the user has `test_run:read` system permission.
8. Use case checks the user is a member of the project (any role) or is a System
   Admin.
9. Use case calls `TestRunExecutionRepository::find_by_run_id(run_id, page, limit)`.
10. Repository executes a parameterized JOIN query returning paginated execution
    summaries.
11. Handler returns `200 OK` with the paginated response.

---

## API Contract

### Common Error Response Format

All errors follow the standard project format:

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable description",
    "details": []
  }
}
```

---

### GET `/api/v1/projects/{projectId}/test-runs/{id}/executions`

List Test Executions linked to a Test Run, with pagination.

**Required Permission:** `test_run:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Test Run ID |

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
      "id": 15,
      "name": "Regression Run - Sprint 12",
      "status": "IN_PROGRESS",
      "tester_count": 3,
      "case_count": 42,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_at": "2026-07-15T08:30:00Z"
    },
    {
      "id": 10,
      "name": "Regression Run - Sprint 11",
      "status": "COMPLETED",
      "tester_count": 2,
      "case_count": 38,
      "created_at": "2026-07-07T09:00:00Z",
      "updated_at": "2026-07-11T17:00:00Z"
    }
  ],
  "meta": {
    "total": 5,
    "page": 1,
    "limit": 25
  }
}
```

**Notes:**
- Results are scoped to the given Test Run and exclude soft-deleted executions
  (`WHERE test_run_id = $1 AND deleted_at IS NULL`).
- Default sort is `created_at DESC` (newest first). No sort parameter is exposed.
- `tester_count` is the count of rows in the `execution_testers` junction table for
  each execution. If the execution has no testers, the value is `0`.
- `case_count` is the count of rows in the `execution_test_cases` table for each
  execution (imported test cases). If no cases have been imported, the value is `0`.
- The response is intentionally compact -- no nested objects or full related data.
  Consumers navigate to `/api/v1/projects/{projectId}/test-executions/{id}` for full
  execution detail.
- `page` and `limit` validation: `page` must be >= 1; `limit` must be between 1 and
  100 inclusive. Invalid values return `422`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_run:read` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, or Test Run does not exist, is soft-deleted, or belongs to a different project |
| `422` | `VALIDATION_ERROR` | Invalid pagination parameters (`page` < 1, `limit` < 1, `limit` > 100, non-integer values) |

---

## Data Model

### No New Table

This feature adds no new tables or columns. It is a read-only projection over the
existing schema defined in `test-execution-crud`. The relevant tables are:

```
TEST_RUNS ──< TEST_EXECUTIONS ──< EXECUTION_TESTERS
                  │
                  └──< EXECUTION_TEST_CASES
```

**Key foreign key:** `TEST_EXECUTIONS.test_run_id` -> `TEST_RUNS.id`

The query JOINs from `test_runs` (to verify existence and project scope) to
`test_executions` (filtered by `test_run_id` and `deleted_at IS NULL`), with
subquery or LEFT JOIN aggregations for `tester_count` and `case_count`.

### Query (Parameterized)

```sql
SELECT
    te.id,
    te.name,
    te.status,
    te.created_at,
    te.updated_at,
    COALESCE(tc.tester_count, 0)  AS tester_count,
    COALESCE(cc.case_count, 0)    AS case_count
FROM test_runs tr
JOIN test_executions te ON te.test_run_id = tr.id
LEFT JOIN LATERAL (
    SELECT COUNT(*) AS tester_count
    FROM execution_testers et
    WHERE et.execution_id = te.id
) tc ON true
LEFT JOIN LATERAL (
    SELECT COUNT(*) AS case_count
    FROM execution_test_cases etc
    WHERE etc.execution_id = te.id
) cc ON true
WHERE tr.id = $1
  AND tr.project_id = $2
  AND tr.deleted_at IS NULL
  AND te.deleted_at IS NULL
ORDER BY te.created_at DESC
LIMIT $3 OFFSET $4;
```

**Design notes:**
- The query includes a JOIN against `test_runs` to enforce the project scope and
  existence check in a single round-trip.
- `LATERAL` joins are used for the count subqueries so that each execution's counts
  are resolved independently without cross-contamination.
- If the project's RDBMS does not support `LATERAL`, use correlated scalar
  subqueries instead:
  ```sql
  SELECT
      te.id,
      te.name,
      te.status,
      te.created_at,
      te.updated_at,
      (SELECT COUNT(*) FROM execution_testers et WHERE et.execution_id = te.id) AS tester_count,
      (SELECT COUNT(*) FROM execution_test_cases etc WHERE etc.execution_id = te.id) AS case_count
  FROM ...
  ```
- A companion `COUNT(*)` query (same FROM/WHERE, no LIMIT/OFFSET) provides the
  `total` for pagination metadata.

---

## Sequence

### List Executions Flow

1. Client sends `GET /api/v1/projects/{projectId}/test-runs/{id}/executions?page=1&limit=25`
   with session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID,
   system admin flag).
3. Handler validates path parameters (`projectId` and `id` are positive integers).
   Invalid -> `404 Not Found`.
4. Handler validates query parameters (`page` >= 1, `limit` in 1..100).
   Invalid -> `422 Validation Error`.
5. Handler verifies the Project exists and is not soft-deleted via
   `ProjectRepository::exists_and_active(projectId)`. If not -> `404 Not Found`.
6. Handler verifies the Test Run exists and belongs to the project via
   `TestRunRepository::find_by_id(projectId, id)`. If not found, soft-deleted, or
   wrong project -> `404 Not Found`.
7. Handler calls `ListTestRunExecutionsUseCase::execute(project_id, run_id, page, limit, user_id)`.
8. Use case checks the user has `test_run:read` system permission via
   `AuthorizationService::has_permission(user_id, "test_run:read")`.
   Lacking -> `403 Forbidden`.
9. Use case checks the user is a member of the project (any role) or is a System
   Admin via `ProjectMemberRepository::exists(project_id, user_id)` or admin
   bypass.
   Not a member and not admin -> `403 Forbidden`.
10. Use case calls `TestRunExecutionRepository::find_by_run_id(run_id, page, limit)`.
11. Repository executes the parameterized JOIN query (see Data Model section) to
    fetch execution summaries with aggregated counts.
12. Repository executes a companion `COUNT(*)` query to get the total matching
    executions for pagination metadata.
13. Use case constructs the paginated response DTO (`data` + `meta`).
14. Handler returns `200 OK` with the response body.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestRunExecutionRepository` | Application (2) | Interface (port) for the read-only query. Single method: `find_by_run_id(run_id, page, limit) -> Result<(Vec<ExecutionSummary>, u64)>` where the tuple contains the execution list and total count. |
| `ListTestRunExecutionsUseCase` | Application (2) | Orchestrates the authorization checks (system permission + project membership) and delegates to the repository. Accepts `(project_id, run_id, page, limit, user_id)`, returns a paginated response DTO. |
| `ListTestRunExecutionsHandler` | Adapters (3) | HTTP handler for `GET /api/v1/projects/{projectId}/test-runs/{id}/executions`. Validates path and query parameters, verifies project and run existence, calls the use case, serializes the response. Registers the route under the test runs resource path. |
| `ExecutionSummary` (DTO/response struct) | Application (2) | Flat response object: `id` (i64), `name` (String), `status` (String), `tester_count` (u32), `case_count` (u32), `created_at` (DateTime), `updated_at` (DateTime). No nested objects or domain logic. |
| `SqlTestRunExecutionRepository` | Infrastructure (4) | Implements `TestRunExecutionRepository` using PostgreSQL. Executes the parameterized JOIN query with LATERAL/correlated subqueries for counts. All queries use parameterized placeholders (`$1`, `$2`, ...). |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| HTTP router registration | Add one new route: `GET /api/v1/projects/{projectId}/test-runs/{id}/executions` -- requires session auth. Register this route in the `test-run-crud` router group, after the `/{id}` dynamic route to avoid conflicts. |
| `test_run:read` permission usage | No new permission needed. This endpoint reuses `test_run:read` which should already be defined by `test-run-crud`. If `test-run-crud` has not yet defined this permission, it must be added there. |

---

## Route Registration

```text
# Registered in the test-run-crud router group
GET /api/v1/projects/{projectId}/test-runs/{id}/executions -> listExecutions
```

This route requires:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- Test Run existence validation (run belongs to project, not soft-deleted)
- System permission `test_run:read`
- Project membership (any role)

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks `test_run:read` permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User is not a project member | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| System Admin bypass | -- | -- | -- | Admin skips both permission and membership checks |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before any query |
| Test Run not found, soft-deleted, or wrong project | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Invalid pagination (`page` < 1, `limit` < 1, `limit` > 100, non-integer) | `422` | `VALIDATION_ERROR` | INFO | Includes field-level details |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Standard read rate limit (60 req/min) |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**
- **Do not reveal** whether a `404` is caused by a non-existent project, non-existent
  Test Run, soft-deleted Test Run, or wrong-project Test Run (same message for all).
- **Do not return full execution details** -- this is a summary list, not a detail
  view. Full execution detail is served by `test-execution-crud`.
- **Do not write to the database** -- this endpoint is strictly read-only.
- **Do not allow bypassing the project scope** -- every request checks that the
  caller is a project member (unless System Admin).
- **Do not introduce a new permission code** -- reuse `test_run:read`.
