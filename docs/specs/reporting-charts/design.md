# Design: Reporting & Charts

## Architecture

The Reporting feature is read-only. It introduces no new mutable tables and no
write paths. All endpoints execute aggregation queries against existing data
(TEST_CASES, TEST_CASE_RESULTS, TEST_CATEGORIES, TEST_PRIORITIES) scoped to a
single project. The feature relies on efficient database-level aggregation rather
than in-memory processing.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - get_summary   GET /api/v1/reports/project/{id}/summary            │   │
│  │  - get_trend     GET /api/v1/reports/project/{id}/trend              │   │
│  │  - get_coverage  GET /api/v1/reports/project/{id}/coverage           │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  ReportService:                           │  │  - ReportSummary (vo)   │  │
│  │  - get_project_summary                    │  │  - TrendDataPoint (vo)  │  │
│  │  - get_project_trend                      │  │  - CoverageGroup (vo)   │  │
│  │  - get_project_coverage                   │  └──────────────────────────┘  │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - ReportRepository (port)                │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlReportRepository  (implements ReportRepository)                  │   │
│  │    All methods are read-only aggregation SQL queries.                  │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All three endpoints require an authenticated session and project membership.
2. The handler validates the project exists, checks project membership (or System
   Admin bypass), and delegates to `ReportService`.
3. `ReportService` validates parameters and calls `ReportRepository` methods
   which execute aggregation SQL queries.
4. Results are assembled into response DTOs and returned.

---

## API Contract

---

### GET `/api/v1/reports/project/{projectId}/summary`

Summary statistics for a project's test execution results.

**Authentication:** Required (session)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Query Parameters:** None

**Success Response:** `200 OK`

```json
{
  "data": {
    "project_id": 42,
    "total": 150,
    "not_tested": 20,
    "in_progress": 15,
    "pass": 85,
    "fail": 18,
    "warning": 7,
    "ignore": 5,
    "pass_rate": 73.9
  }
}
```

**Notes:**
- `total` is the count of distinct test cases in the project's test runs (not the
  count of test case results).
- Each test case is counted exactly once using its most recent result (latest
  `updated_at`). If a test case appears in multiple test runs, only the most
  recent result overall is counted.
- `pass_rate` = `pass / (pass + fail + warning) * 100`. NOT TESTED, IN PROGRESS,
  and IGNORE are excluded from the denominator.
- If the denominator is zero (no test case has a definitive result yet),
  `pass_rate` returns `null`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User is not a project member and is not System Admin |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |

---

### GET `/api/v1/reports/project/{projectId}/trend`

Pass/fail trend data over time.

**Authentication:** Required (session)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Query Parameters:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `from_date` | string (date) | 30 days ago | ISO 8601 date (inclusive) |
| `to_date` | string (date) | today | ISO 8601 date (inclusive) |
| `granularity` | string | `day` | `day`, `week`, or `month` |
| `test_run_id` | integer | -- | Optional: scope to a specific test run |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "date": "2026-06-15",
      "pass_count": 42,
      "fail_count": 12,
      "warning_count": 3,
      "ignore_count": 2,
      "total_count": 59
    },
    {
      "date": "2026-06-16",
      "pass_count": 45,
      "fail_count": 10,
      "warning_count": 4,
      "ignore_count": 0,
      "total_count": 59
    }
  ]
}
```

**Notes:**
- Each data point represents a snapshot as of the end of that date bucket.
- Deduplication: within each date bucket, each test case is counted once using its
  most recent result with `updated_at <= bucket_end`.
- The response includes all date buckets in the range, even those with zero
  results (zero-fill gaps for continuous chart rendering).
- `test_run_id` filter applies to the result's ancestor test run. If provided, only
  results linked to that test run are counted.
- Data points are ordered chronologically (oldest first).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User is not a project member and is not System Admin |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `422` | `VALIDATION_ERROR` | `from_date` > `to_date`, invalid granularity, invalid date format |

---

### GET `/api/v1/reports/project/{projectId}/coverage`

Test coverage breakdown by category or priority.

**Authentication:** Required (session)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Query Parameters:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `dimension` | string | `category` | `category` or `priority` |
| `test_run_id` | integer | -- | Optional: scope to a specific test run |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "dimension_name": "API Tests",
      "total": 50,
      "pass": 40,
      "fail": 5,
      "warning": 2,
      "ignore": 1,
      "in_progress": 2,
      "not_tested": 0,
      "pass_rate": 85.1
    },
    {
      "dimension_name": "Security",
      "total": 20,
      "pass": 18,
      "fail": 1,
      "warning": 0,
      "ignore": 0,
      "in_progress": 1,
      "not_tested": 0,
      "pass_rate": 94.7
    },
    {
      "dimension_name": "UI Tests",
      "total": 0,
      "pass": 0,
      "fail": 0,
      "warning": 0,
      "ignore": 0,
      "in_progress": 0,
      "not_tested": 0,
      "pass_rate": null
    }
  ]
}
```

**Notes:**
- Groups are ordered alphabetically by `dimension_name`.
- All non-deleted categories/priorities in the project are included, even if they
  have zero test cases (showing coverage gaps). Entries with zero `total` appear
  with all zero counts.
- Deduplication is per (test_case, category/priority) pair. Each test case is
  counted once using its most recent result.
- If `dimension` is `category`, grouping is by `TEST_CASES.category_id`. If
  `priority`, grouping is by `TEST_CASES.priority_id`.
- `test_run_id` filter scopes to results within a specific test run.
- `pass_rate` = `pass / (pass + fail + warning) * 100`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User is not a project member and is not System Admin |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `422` | `VALIDATION_ERROR` | Invalid `dimension` value |

---

## Data Model

### No New Tables

The Reporting feature introduces **no new database tables**. All data is derived
from existing tables:

| Existing Table | Used For |
|---------------|----------|
| `TEST_CASES` | Test case list, category_id, priority_id references |
| `TEST_CASE_RESULTS` | Result status, updated_at timestamp per test case |
| `TEST_RUNS` | Test run scoping, run-to-project relationship |
| `TEST_CATEGORIES` | Category name resolution for coverage breakdown |
| `TEST_PRIORITIES` | Priority name resolution for coverage breakdown |

### Performance Indexes

The following indexes are recommended to support report queries efficiently.
They should be added to existing tables as part of this feature (migration task):

```sql
-- Index for finding the most recent result per test case (used by all three endpoints)
CREATE INDEX idx_test_case_results_tc_updated
  ON test_case_results (test_case_id, updated_at DESC);

-- Index for trend queries (date-range scan on updated_at)
CREATE INDEX idx_test_case_results_updated_status
  ON test_case_results (updated_at, status);

-- Index for coverage queries (join on category/priority)
CREATE INDEX idx_test_cases_category
  ON test_cases (category_id) WHERE deleted_at IS NULL;

CREATE INDEX idx_test_cases_priority
  ON test_cases (priority_id) WHERE deleted_at IS NULL;

-- Index for test-run-scoped aggregation
CREATE INDEX idx_test_case_results_test_run
  ON test_case_results (test_run_id, test_case_id);
```

---

## Sequence

### Get Project Summary

1. Client sends `GET /api/v1/reports/project/{projectId}/summary`.
2. `AuthMiddleware` validates session.
3. Handler validates `projectId`, checks project exists and is non-deleted.
4. Handler checks project membership (or System Admin bypass).
5. Handler calls `ReportService::get_project_summary(project_id)`.
6. Service calls `ReportRepository::get_summary(project_id)`.
7. Repository executes aggregation SQL:
   - Subquery: for each test case in the project's test runs, select the most
     recent result (by `updated_at DESC`, LIMIT 1 per test case).
   - Outer query: `GROUP BY status` and `COUNT(*)`.
8. Service maps the result set into `ReportSummary` DTO and computes `pass_rate`.
9. Handler returns `200 OK`.

### Get Project Trend

1. Client sends `GET /api/v1/reports/project/{projectId}/trend?from_date=...&to_date=...&granularity=day`.
2. Handler validates parameters (date range, granularity).
3. Handler calls `ReportService::get_project_trend(project_id, from, to, granularity, test_run_id)`.
4. Service calls `ReportRepository::get_trend(project_id, from, to, granularity, test_run_id)`.
5. Repository executes aggregation SQL using a single window-function pass:
   - Generate date bucket series (using `generate_series` in PostgreSQL) as a CTE.
   - For each test case, use `ROW_NUMBER() OVER (PARTITION BY test_case_id ORDER BY
     updated_at DESC)` to identify the most recent result within each bucket.
   - Filter to `row_number = 1` and `GROUP BY bucket + status`, counting in one pass.
   - This avoids per-bucket subqueries and keeps the plan linear in the number of
     results, not quadratic in the number of buckets.
6. Service zero-fills missing buckets and sorts chronologically.
7. Handler returns `200 OK`.

### Get Coverage Breakdown

1. Client sends `GET /api/v1/reports/project/{projectId}/coverage?dimension=category`.
2. Handler calls `ReportService::get_project_coverage(project_id, dimension, test_run_id)`.
3. Service calls `ReportRepository::get_coverage(project_id, dimension, test_run_id)`.
4. Repository executes aggregation SQL:
   - LEFT JOIN from the dimension table (categories or priorities) to test cases
     and their most recent result.
   - GROUP BY dimension and status.
   - Include zero-count rows for dimensions with no test cases.
5. Service maps into `CoverageGroup` DTOs, computes `pass_rate` per group.
6. Handler returns `200 OK`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `ReportSummary` | Domain (1) | Value object: `total`, `not_tested`, `in_progress`, `pass`, `fail`, `warning`, `ignore`, `pass_rate`. |
| `TrendDataPoint` | Domain (1) | Value object: `date`, `pass_count`, `fail_count`, `warning_count`, `ignore_count`, `total_count`. |
| `CoverageGroup` | Domain (1) | Value object: `dimension_name`, `total`, `pass`, `fail`, `warning`, `ignore`, `in_progress`, `not_tested`, `pass_rate`. |
| `ReportService` | Application (2) | Orchestrates report queries. Methods: `get_project_summary`, `get_project_trend`, `get_project_coverage`. All methods validate project membership before querying. |
| `ReportRepository` | Application (2) | Interface (port): `get_summary(project_id)`, `get_trend(project_id, from, to, granularity, test_run_id?)`, `get_coverage(project_id, dimension, test_run_id?)`. All read-only. |
| `ReportHandler` | Adapters (3) | HTTP handler: three GET methods. Validates parameters, checks project membership, calls `ReportService`, serializes responses. |
| `SqlReportRepository` | Infrastructure (4) | Implements `ReportRepository` using raw SQL aggregation queries for performance. All parameters use `$1`, `$2` placeholders. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| HTTP router registration | Register three new report routes under `/api/v1/reports/project/{projectId}/` -- all require session auth and project membership. |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User not a project member | `403` | `FORBIDDEN` | INFO | Generic message; also returned for non-existent project to prevent enumeration |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | |
| Invalid query parameter | `422` | `VALIDATION_ERROR` | INFO | Field-level details in response |
| Query timeout exceeded | `503` | `QUERY_TIMEOUT` | ERROR | Configurable timeout (default 10s) |

**Anti-patterns explicitly avoided:**

- **Do not** perform in-memory aggregation. All counting and grouping happens in
  the database.
- **Do not** use `SELECT *` on large tables. Only the required columns are
  selected for each query.
- **Do not** return data for projects the user is not a member of. The membership
  check is mandatory.
- **Do not** apply `N+1` query patterns. Each report endpoint executes at most a
  single database query (or a small fixed number).
- **Do not** expose internal IDs in report responses (only names and counts --
  no `category_id` or `priority_id`).

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `REPORT_QUERY_TIMEOUT_SECONDS` | `10` | Maximum query execution time before returning 503 |
| `REPORT_TREND_MAX_DAYS` | `90` | Maximum date range for trend queries at daily granularity; 12 months at monthly granularity |

**Future performance improvement:** For larger datasets (projects with 100K+ test cases
or date ranges beyond the current caps), a pre-aggregated snapshot table is recommended.
A nightly job would materialize per-day, per-test-case latest status into a dedicated
`report_daily_snapshots` table, making trend queries a simple range scan on pre-computed
data rather than an on-the-fly deduplication across the full results table. This is
deferred to a future phase.
