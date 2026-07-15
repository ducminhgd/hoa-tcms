# Feature: Reporting & Charts

## Overview

The reporting and charts feature provides project-level dashboards and trend
analysis for test execution data. Users can view summary statistics (test case
status distribution), pass/fail trends over time, and coverage breakdowns by
category and priority. All data is derived from existing test case and test
execution result records through read-only aggregation queries -- no new mutable
tables are introduced.

---

## User Stories

### US-1: Project Execution Summary

As a project member, I want to view a summary dashboard for a project showing the
distribution of test case result statuses across all test runs, so that I can
quickly assess the current state of testing without manually counting results.

**Acceptance Criteria (EARS)**

- WHEN a user sends `GET /api/v1/reports/project/{projectId}/summary`, THE SYSTEM
  SHALL return aggregated counts of test case results grouped by status (PASS, FAIL,
  WARNING, IGNORE, IN PROGRESS, NOT TESTED), along with a total count of all test
  case results and a computed pass rate percentage.
- IF the user is not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.
- IF the user is not a member of the project AND is not a System Admin, THE SYSTEM
  SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL scope results to the given project only.
- THE SYSTEM SHALL count each test case result exactly once -- the most recent
  result for each test case within the project (deduplicated by test case ID across
  all test runs and executions).
- THE SYSTEM SHALL compute `pass_rate` as `(PASS count / total with results) * 100`
  rounded to one decimal place. Test cases with status NOT TESTED or IN PROGRESS are
  excluded from the pass rate denominator (they have no definitive outcome yet).
- The response SHALL include: `total`, `not_tested`, `in_progress`, `pass`, `fail`,
  `warning`, `ignore`, and `pass_rate`.

### US-2: Pass/Fail Trend Analysis

As a project member, I want to view pass/fail trends over time for a project, so
that I can see how test quality is evolving across test runs and identify
regressions early.

**Acceptance Criteria (EARS)**

- WHEN a user sends `GET /api/v1/reports/project/{projectId}/trend`, THE SYSTEM
  SHALL return a chronological series of data points, each representing a date
  and the pass/fail/other counts of test case results recorded on that date.
- IF the user is not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.
- IF the user is not a member of the project AND is not a System Admin, THE SYSTEM
  SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL support an optional `from_date` and `to_date` query parameter
  (ISO 8601 date, e.g., `2026-06-01`). Only results with `updated_at` within the
  inclusive range are returned. If omitted, the default range is the last 30 days.
- THE SYSTEM SHALL support an optional `granularity` query parameter with values
  `day` (default), `week`, or `month`. This controls how data points are bucketed.
- THE SYSTEM SHALL support an optional `test_run_id` query parameter that filters
  trend data to a specific test run. If omitted, all test runs in the project are
  aggregated.
- Each data point SHALL include: `date` (bucket start), `pass_count`, `fail_count`,
  `warning_count`, `ignore_count`, and `total_count`.
- THE SYSTEM SHALL deduplicate by counting each test case's most recent result as
  of the end of each date bucket (snapshot-based trend, not cumulative).
- Data points SHALL be returned in chronological order (oldest first).
- IF `granularity` is `week`, the date SHALL be the Monday of that ISO week. IF
  `granularity` is `month`, the date SHALL be the first day of that month.
- IF `from_date` is after `to_date`, THE SYSTEM SHALL return `422`.

### US-3: Coverage by Category and Priority

As a project member, I want to view test case result coverage broken down by
category and priority, so that I can identify which areas of the test suite need
attention (e.g., high-priority tests with high failure rates, or categories
with low coverage).

**Acceptance Criteria (EARS)**

- WHEN a user sends `GET /api/v1/reports/project/{projectId}/coverage`, THE SYSTEM
  SHALL return coverage data broken down by both `category` and `priority`
  dimensions, with pass/fail/other counts for each breakdown group.
- IF the user is not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.
- IF the user is not a member of the project AND is not a System Admin, THE SYSTEM
  SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL support an optional `dimension` query parameter with values
  `category` (default) or `priority`. When `category`, the breakdown is grouped by
  test case category. When `priority`, the breakdown is grouped by test case
  priority.
- THE SYSTEM SHALL support an optional `test_run_id` query parameter. If provided,
  coverage is scoped to that test run. If omitted, coverage spans all test runs in
  the project.
- Each breakdown group SHALL include: `dimension_name` (category name or priority
  name), `total`, `pass`, `fail`, `warning`, `ignore`, `in_progress`, `not_tested`,
  and `pass_rate`.
- THE SYSTEM SHALL use the most recent result for each test case (same
  deduplication as US-1).
- THE SYSTEM SHALL include all non-deleted categories/priorities in the project,
  even those with zero test cases (to show coverage gaps explicitly). Empty
  categories/priorities SHALL appear with counts of zero.
- Groups SHALL be ordered alphabetically by `dimension_name`.

---

## Security Considerations

### Authentication
All reporting endpoints require a valid authenticated session. Requests without a
valid session cookie return `401 Unauthorized` with error code
`NOT_AUTHENTICATED`. This is enforced by `AuthMiddleware` before any handler logic
executes.

### Authorization
Users must be members of the project to view its reports. A System Admin bypasses
the membership check and can view reports for any project. A `403 Forbidden` response
uses a generic message that does not distinguish "not a member" from "project does
not exist" to prevent project enumeration.

### Read-Only
All reporting endpoints are read-only (`GET` only). They perform aggregation queries
against existing data and never modify the database. No CSRF protection is needed
for read-only endpoints.

### Query Performance
Aggregation queries may be expensive on large datasets (projects with thousands
of test cases and results). The system must:
- Use database-level aggregation (`GROUP BY`, `COUNT`) rather than in-memory
  aggregation in the application layer.
- Include appropriate database indexes on test case result timestamp and status
  columns to support trend and coverage queries.
- Apply query timeouts (configurable, default 10s) and return `503 Service
  Unavailable` if a query exceeds the timeout.

### Rate Limiting

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../summary` | 30 requests | per minute |
| `GET .../trend` | 30 requests | per minute |
| `GET .../coverage` | 30 requests | per minute |

### Data Exposure
Reporting data is scoped to a single project. There is no cross-project aggregation.
Each endpoint requires the `projectId` path parameter and validates project
membership before returning any data. A user cannot infer data about projects they
do not belong to.

---

## Out of Scope

- **Saved / custom reports** (user-defined report configurations -- deferred to a
  future phase)
- **Report export** (PDF, CSV, Excel -- deferred to a future phase)
- **Cross-project dashboards** (aggregate view across multiple projects -- deferred)
- **Drill-down from charts to individual test results** (charts are aggregate only;
  drill-down to detail views is a UI feature covered in `ui-*` specs)
- **Scheduled / emailed reports** (automated report delivery on a schedule --
  deferred to a future phase)
- **Custom date range comparison** (comparing two time periods side by side --
  deferred)
- **Real-time dashboard updates** (polling-based in Phase 1; WebSocket real-time
  deferred)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an
  authenticated session via `AuthMiddleware`.
- **Project CRUD** -- The `PROJECTS` table must exist; reports are scoped to a
  project via `project_id`.
- **Project Members** -- Project membership is verified before returning report data.
- **Test Case CRUD** -- Test cases carry `category_id` and `priority_id` FK
  references used in coverage breakdowns.
- **Test Execution Management** -- Test case results are the primary data source
  for all report aggregations.
- **Metadata Categories** -- Category names are resolved via JOIN on
  `TEST_CATEGORIES` for coverage breakdowns.
- **Metadata Priorities** -- Priority names are resolved via JOIN on
  `TEST_PRIORITIES` for coverage breakdowns.
