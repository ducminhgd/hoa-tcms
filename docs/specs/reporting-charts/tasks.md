# Tasks: Reporting & Charts

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable
unit of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement report value objects --
       `requirements.md#US-1` through `US-3`, `design.md#Components`
  - `ReportSummary`: `total`, `not_tested`, `in_progress`, `pass`, `fail`,
    `warning`, `ignore`, `pass_rate` (Option<f64>)
  - `TrendDataPoint`: `date` (NaiveDate), `pass_count`, `fail_count`,
    `warning_count`, `ignore_count`, `total_count`
  - `CoverageGroup`: `dimension_name`, `total`, `pass`, `fail`, `warning`,
    `ignore`, `in_progress`, `not_tested`, `pass_rate` (Option<f64>)
  - No framework imports; pure Rust structs
  - Implement `pass_rate` computation logic on the value objects (handles
    zero denominator returning None)

- [ ] 2. Define `DateGranularity` enum --
       `requirements.md#US-2`, `design.md#API Contract`
  - Variants: `Day`, `Week`, `Month`
  - FromStr implementation for query parameter parsing
  - No framework imports

- [ ] 3. Define `CoverageDimension` enum --
       `requirements.md#US-3`, `design.md#API Contract`
  - Variants: `Category`, `Priority`
  - FromStr implementation for query parameter parsing
  - No framework imports

---

## Layer 2 -- Application

- [ ] 4. Define `ReportRepository` interface (port) --
       `design.md#Components`
  - Methods:
    - `get_summary(project_id: i64) -> Result<ReportSummary>`
    - `get_trend(project_id: i64, from_date: NaiveDate, to_date: NaiveDate,
      granularity: DateGranularity, test_run_id: Option<i64>) ->
      Result<Vec<TrendDataPoint>>`
    - `get_coverage(project_id: i64, dimension: CoverageDimension,
      test_run_id: Option<i64>) -> Result<Vec<CoverageGroup>>`
  - All methods are read-only
  - Return domain value objects (not DTOs)

- [ ] 5. Implement `ReportService` --
       `requirements.md#US-1` through `US-3`, `design.md#Sequence`
  - `get_project_summary(project_id)`: delegates to repository, returns
    `ReportSummary`
  - `get_project_trend(project_id, from, to, granularity, test_run_id)`:
    validates date range (from <= to, max range 365 days), delegates to
    repository, zero-fills gaps
  - `get_project_coverage(project_id, dimension, test_run_id)`: validates
    dimension, delegates to repository
  - All methods require project membership check (done at handler level)
  - Zero-fill logic for trend: generate all date buckets in range, merge with
    query results, fill missing with zero counts

- [ ] 6. Define response DTOs --
       `design.md#API Contract`
  - `ReportSummaryResponse`: mirrors ReportSummary fields
  - `TrendResponse`: `data: Vec<TrendDataPointResponse>`
  - `TrendDataPointResponse`: mirrors TrendDataPoint fields
  - `CoverageResponse`: `data: Vec<CoverageGroupResponse>`
  - `CoverageGroupResponse`: mirrors CoverageGroup fields

- [ ] 7. Write unit tests for `ReportService` --
       `requirements.md#US-1` through `US-3`
  - Test `get_project_summary`: maps repository result correctly, computes
    pass_rate
  - Test `get_project_summary`: zero denominator returns None pass_rate
  - Test `get_project_trend`: validates date range (from > to returns error)
  - Test `get_project_trend`: validates max range (exceeding 365 days returns
    error)
  - Test `get_project_trend`: zero-fills gaps between returned data points
  - Test `get_project_trend`: all buckets returned in chronological order
  - Test `get_project_coverage`: maps repository result correctly
  - Mock `ReportRepository` for all tests

---

## Layer 3 -- Adapters (HTTP)

- [ ] 8. Implement `ReportHandler` --
        `design.md#API Contract`, `design.md#Components`
  - Three handler methods: `get_summary`, `get_trend`, `get_coverage`
  - Deserialize path params (projectId) and query params (from_date, to_date,
    granularity, dimension, test_run_id)
  - Validate project exists and is non-deleted
  - Check project membership (or System Admin bypass) via AuthorizationService
  - Call `ReportService` methods
  - Serialize responses with proper status codes

- [ ] 9. Register report routes in HTTP router --
        `design.md#Components`
  - `GET /api/v1/reports/project/{projectId}/summary`   -> `get_summary`
  - `GET /api/v1/reports/project/{projectId}/trend`     -> `get_trend`
  - `GET /api/v1/reports/project/{projectId}/coverage`  -> `get_coverage`
  - All routes require session auth middleware
  - Route order: static before parameterized

- [ ] 10. Write integration tests for report HTTP handlers --
         `design.md#API Contract`
  - Test `GET .../summary`: returns correct counts and pass_rate
  - Test `GET .../summary`: returns 404 for non-existent project
  - Test `GET .../summary`: returns 403 for non-member
  - Test `GET .../trend`: returns chronological data points with optional
    filters
  - Test `GET .../trend`: returns 422 for from_date > to_date
  - Test `GET .../trend`: returns 422 for invalid granularity
  - Test `GET .../coverage`: returns coverage breakdown by category (default)
  - Test `GET .../coverage`: returns coverage breakdown by priority
  - Test `GET .../coverage`: returns 422 for invalid dimension
  - Test `GET .../coverage`: includes zero-count categories
  - Test `GET .../coverage`: scoped to test_run_id when provided
  - Test auth: all endpoints return 401 without session cookie
  - Test auth: all endpoints return 403 for non-member

---

## Layer 4 -- Infrastructure

- [ ] 11. Create performance indexes migration --
         `design.md#Data Model`
  - Add to `test_case_results`:
    - `idx_test_case_results_tc_updated` on (test_case_id, updated_at DESC)
    - `idx_test_case_results_updated_status` on (updated_at, status)
    - `idx_test_case_results_test_run` on (test_run_id, test_case_id)
  - Add to `test_cases`:
    - `idx_test_cases_category` partial on (category_id) WHERE deleted_at IS
      NULL
    - `idx_test_cases_priority` partial on (priority_id) WHERE deleted_at IS
      NULL
  - Rollback migration: `DROP INDEX IF EXISTS` for each index
  - Note: indexes are added to existing tables; no new tables are created

- [ ] 12. Implement `SqlReportRepository` --
         `design.md#Components`, `design.md#Sequence`
  - `get_summary`: single aggregation query using a subquery for the most
    recent result per test case (DISTINCT ON or ROW_NUMBER window function),
    then GROUP BY status
  - `get_trend`: generate date bucket series with `generate_series()`, LEFT
    JOIN to most-recent result per test case per bucket, GROUP BY bucket +
    status
  - `get_coverage`: LEFT JOIN from categories/priorities to test cases to
    most recent results, GROUP BY dimension name + status, include zero-count
    rows for unused dimensions
  - All queries use parameterized placeholders (`$1`, `$2`, ...)
  - Apply `REPORT_QUERY_TIMEOUT_SECONDS` via `statement_timeout` or
    equivalent
  - Write unit tests with a test database and seeded test data

- [ ] 13. Write integration tests for `SqlReportRepository` --
         `design.md#Data Model`, `requirements.md#US-1` through `US-3`
  - Seed test data: project with test cases, categories, priorities, multiple
    test runs with results of varying statuses
  - Test `get_summary`: correct counts for each status, correct deduplication
    (only most recent result per test case)
  - Test `get_trend`: correct bucketing by day/week/month, zero-fill gaps
  - Test `get_trend`: test_run_id filter works correctly
  - Test `get_coverage`: correct grouping by category and priority
  - Test `get_coverage`: zero-count categories/priorities included
  - Test `get_coverage`: test_run_id filter works correctly
  - Test query timeout: artificially slow query returns error within
    configured timeout

---

## Verification & Cleanup

- [ ] 14. End-to-end verification --
         `requirements.md#US-1` through `US-3`
  - Create a project with test cases, categories, and priorities
  - Add test results across multiple test runs with various statuses
  - Verify summary returns correct counts and pass_rate
  - Verify trend returns chronological data with correct bucketing by day,
    week, month
  - Verify coverage returns correct breakdown by category and priority
  - Verify all endpoints return 403 for non-members
  - Verify all endpoints return 404 for non-existent projects

- [ ] 15. Update `specs/README.md` -- `specs/README.md`
  - Mark `reporting-charts` as having completed specs (requirements.md,
    design.md, tasks.md)
