# Design: Test Run Statistics

## Architecture

The Test Run Statistics feature is a read-only computed endpoint. It introduces no new
database tables, no domain entities that need persistence, and no state-changing
operations. The feature follows Clean Architecture layering with a thin stack: a handler
in the Adapters layer, a service in the Application layer, and an aggregation query in the
Infrastructure layer.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handler:                                                         │   │
│  │  - get_statistics   GET /api/v1/projects/{pid}/test-runs/{id}/stats   │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  TestRunStatisticsService:                │  │  (no new entities --     │  │
│  │  - get_statistics                        │  │   uses existing result   │  │
│  │                                           │  │   status values from     │  │
│  │  Interfaces:                              │  │   TestCaseResult)        │  │
│  │  - TestRunStatisticsRepository (port)     │  └──────────────────────────┘  │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestRunStatisticsRepository (implements port)                    │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. The endpoint requires an authenticated session (checked by `AuthMiddleware`).
2. The handler validates the project exists and is not soft-deleted.
3. The handler validates the test run exists, belongs to the project, and is not
   soft-deleted.
4. The handler calls `TestRunStatisticsService::get_statistics(project_id, test_run_id, current_user_id)`.
5. The service checks `test_run:read_statistics` system permission.
6. The service checks the user is a member of the project (any role) or a System Admin.
7. The service delegates to `TestRunStatisticsRepository::aggregate_by_status(test_run_id)`,
   which executes a single aggregation query.
8. The handler returns `200 OK` with the statistics payload.

---

## API Contract

### Common Error Response Format

All errors follow the standard project format:

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable description"
  }
}
```

---

### GET `/api/v1/projects/{projectId}/test-runs/{id}/statistics`

Return aggregated statistics of test case result statuses within a test run.

**Required Permission:** `test_run:read_statistics`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Test Run ID |

**Query Parameters:** None.

**Request Body:** None.

**Success Response:** `200 OK`

```json
{
  "data": {
    "total": 50,
    "not_tested": 10,
    "in_progress": 5,
    "pass": 20,
    "fail": 8,
    "warning": 4,
    "ignore": 3
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `total` | integer | Total number of test cases linked to the test run (sum of all status counts). Non-negative. |
| `not_tested` | integer | Count of test cases that have never been executed in any linked execution, or whose result status is NOT_TESTED. Non-negative. |
| `in_progress` | integer | Count of test case results with status IN_PROGRESS. Non-negative. |
| `pass` | integer | Count of test case results with status PASS. Non-negative. |
| `fail` | integer | Count of test case results with status FAIL. Non-negative. |
| `warning` | integer | Count of test case results with status WARNING. Non-negative. |
| `ignore` | integer | Count of test case results with status IGNORE. Non-negative. |

**Invariant:** `total` = `not_tested` + `in_progress` + `pass` + `fail` + `warning` + `ignore`

**Notes:**

- Statistics are computed fresh on every request. There is no caching, no denormalized
  counters, and no materialized view.
- The aggregation base is the set of test cases linked to the test run via
  `TEST_RUN_TEST_CASES` (excluding soft-deleted links, if applicable).
- Each linked test case is counted exactly once. If multiple test executions exist for the
  same run, the latest result per test case is used (determined by the most recent
  `TEST_EXECUTION` by `created_at`, or by a defined priority rule).
- Soft-deleted test executions and soft-deleted test case results are excluded.
- Test cases with no matching result in any execution are counted as `not_tested`.
- If the test run has no linked test cases, all counts are zero.
- The `data` wrapper allows future metadata fields (e.g., `computed_at` timestamp) to be
  added without a breaking change.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_run:read_statistics` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, or test run does not exist, is soft-deleted, or belongs to a different project |

---

## Data Model

### No New Tables

This feature introduces no new database tables. It is a read-only aggregation over
existing tables.

### Relationships (for the aggregation query)

```
TEST_RUNS ──< TEST_RUN_TEST_CASES >── TEST_CASES
      │
      └──< TEST_EXECUTIONS >──< TEST_CASE_RESULTS
```

- `TEST_RUN_TEST_CASES` links test cases to a test run (the base set for statistics).
- `TEST_EXECUTIONS` are created from a test run and import a subset of its test cases.
- `TEST_CASE_RESULTS` records the outcome of executing a test case within an execution,
  including a `status` column.

### Aggregation Query (Conceptual)

The exact query will be refined during implementation based on the actual schema of
`TEST_CASE_RESULTS`, `TEST_EXECUTIONS`, and `TEST_RUN_TEST_CASES`. The conceptual approach:

1. Start from `TEST_RUN_TEST_CASES` where `test_run_id = $1` (and not soft-deleted, if
   applicable).
2. For each linked test case, find the latest result across all non-deleted test executions
   linked to the run. The "latest" execution is determined by `created_at DESC`.
3. If no result exists for a test case, its status is `NOT_TESTED`.
4. Group by status and count.
5. Sum all counts for `total`.

**Key constraints for implementation:**
- All inputs must be parameterized (`$1`, `$2`, ...). No string interpolation.
- Soft-deleted rows in `TEST_EXECUTIONS` and `TEST_CASE_RESULTS` must be excluded via
  `WHERE deleted_at IS NULL` clauses (if those tables support soft-delete).
- The query must be scoped to a single test run (no cross-run data leakage).
- Performance: the query should execute in a single round-trip to the database. At Phase 1
  scale (hundreds of test cases per run, dozens of executions), a well-indexed aggregation
  query will complete in under 100ms.

### Result Statuses

| Status | Semantics |
|--------|-----------|
| `NOT_TESTED` | Test case has been linked to the run but never executed, or execution imported but result never set |
| `IN_PROGRESS` | Test case execution is in progress |
| `PASS` | Test case passed |
| `FAIL` | Test case failed |
| `WARNING` | Test case passed with warnings |
| `IGNORE` | Test case result is ignored / skipped |

These status values are defined by the `test-case-result-update` feature. The statistics
feature consumes them as-is; it does not define or enforce the status enum.

---

## Sequence

### Get Statistics Flow

1. Client sends `GET /api/v1/projects/{projectId}/test-runs/{id}/statistics` with session
   cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates the project exists and is not soft-deleted (via
   `ProjectRepository::find_by_id` or a shared project validation guard). If not found or
   soft-deleted -> `404 Not Found`.
4. Handler validates the test run exists, belongs to the project, and is not soft-deleted
   (via `TestRunRepository::find_by_id_in_project`). If not found, soft-deleted, or wrong
   project -> `404 Not Found` (same generic message).
5. Handler calls `TestRunStatisticsService::get_statistics(project_id, test_run_id, current_user_id)`.
6. `TestRunStatisticsService` checks `test_run:read_statistics` system permission via
   `AuthorizationService`. If denied and not System Admin -> `403 Forbidden`.
7. `TestRunStatisticsService` checks the user is a member of the project (any role) via
   `ProjectMemberRepository`. If not a member and not System Admin -> `403 Forbidden`.
8. `TestRunStatisticsService` calls `TestRunStatisticsRepository::aggregate_by_status(test_run_id)`.
9. Repository executes the aggregation query and returns a `TestRunStatistics` struct with
   all seven counts.
10. `TestRunStatisticsService` returns the statistics DTO.
11. Handler serializes and returns `200 OK`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestRunStatistics` | Domain (1) | Plain data object: `total`, `not_tested`, `in_progress`, `pass`, `fail`, `warning`, `ignore`. All non-negative integers. Invariant: total equals sum of status counts. No methods beyond construction. No framework imports. |
| `TestRunStatisticsService` | Application (2) | Single method `get_statistics(project_id, test_run_id, current_user_id)`. Checks permission, membership, delegates to repository, returns `TestRunStatistics`. |
| `TestRunStatisticsRepository` | Application (2) | Interface (port): `aggregate_by_status(test_run_id) -> TestRunStatistics`. Single method. |
| `TestRunStatisticsHandler` | Adapters (3) | HTTP handler with single method `get_statistics`. Validates project and test run existence. Calls service. Serializes response. |
| `SqlTestRunStatisticsRepository` | Infrastructure (4) | Implements `TestRunStatisticsRepository`. Executes the aggregation query against PostgreSQL. Uses parameterized queries exclusively. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (Application) | Add permission code `test_run:read_statistics` to the permission registry. |
| Seed migration (Infrastructure) | Add one new permission row for `test_run:read_statistics`. |
| HTTP router registration | Register one new route: `GET /api/v1/projects/{projectId}/test-runs/{id}/statistics`. |

---

## Route Registration

```text
GET /api/v1/projects/{projectId}/test-runs/{id}/statistics  ->  get_statistics
```

This route requires:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- Test run existence validation (test run not soft-deleted, belongs to project)
- System permission check (`test_run:read_statistics`)
- Project membership check (any project role)
- System Admin bypasses all membership checks

The route is a sub-resource of the test run, so it is registered under the
`/test-runs/{id}/` path prefix.

---

## New Permission Code

This must be added to the `PERMISSIONS` table seed data. Use `INSERT ... ON CONFLICT (code)
DO NOTHING` and let the database assign IDs -- do not hardcode numeric IDs in the migration:

| code | name |
|------|------|
| `test_run:read_statistics` | Read Test Run Statistics |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks `test_run:read_statistics` permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks project membership | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before test run validation |
| Test run not found, soft-deleted, or wrong project | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Returns `Retry-After` header |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not cache** statistics at any layer. Every request computes fresh from the database.
- **Do not create denormalized counters** or materialized views for Phase 1.
- **Do not expose** individual test case details or result data in the statistics response.
- **Do not reveal** whether a `404` is caused by a non-existent project, non-existent test
  run, soft-deleted test run, or wrong-project test run (same message for all).
- **Do not use string interpolation** or dynamic SQL for the aggregation query. The test
  run ID is the only variable input and must be parameterized.
