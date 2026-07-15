# Feature: Test Run Statistics

## Overview

Test Run Statistics provides a computed aggregation endpoint that returns the distribution
of test case result statuses within a test run. The endpoint computes counts by result
status (NOT_TESTED, IN_PROGRESS, PASS, FAIL, WARNING, IGNORE) plus a total, based on
the test cases linked to the run and their current results in linked test executions.

No new database tables are introduced — statistics are computed on each request via an
aggregation query against existing `TEST_CASE_RESULTS` (joined through `TEST_EXECUTIONS`
and `TEST_RUN_TEST_CASES`). The endpoint is read-only and available to any project member.

---

## User Stories

### US-1: View Test Run Statistics

As a project member, I want to view aggregated statistics of test case results in a test
run, so that I can quickly see the distribution of result statuses (pass/fail/pending/etc.)
and assess the overall execution progress at a glance.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_run:read_statistics` permission (or a System Admin) sends
  `GET /api/v1/projects/{projectId}/test-runs/{id}/statistics`, THE SYSTEM SHALL compute
  aggregated counts of test case results grouped by result status, plus a total, and return
  `200 OK` with the statistics payload.
- IF the user does not hold the `test_run:read_statistics` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `test_run:read_statistics` but is not a member of the project AND is
  not a System Admin, THE SYSTEM SHALL return `403 Forbidden` (same generic message).
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the test run does not exist, is soft-deleted, or belongs to a different project, THE
  SYSTEM SHALL return `404 Not Found` (same message to avoid information leakage).
- THE SYSTEM SHALL return counts for each result status:
  `not_tested`, `in_progress`, `pass`, `fail`, `warning`, `ignore`, plus `total`.
- THE SYSTEM SHALL compute statistics from all test cases linked to the test run (via
  `TEST_RUN_TEST_CASES`), looking up their current result status from the most recent test
  execution linked to the run. Test cases that have never been executed shall be counted
  under `not_tested`.
- THE SYSTEM SHALL exclude soft-deleted test case results and soft-deleted test executions
  from the aggregation.
- IF the test run has no linked test cases, THE SYSTEM SHALL return all counts as zero
  (including `total: 0`).
- THE SYSTEM SHALL compute statistics fresh from the database on every request (no caching
  at the database or application layer). The response reflects the current state of all
  test case results at request time.
- THE SYSTEM SHALL NOT return individual result details or test case identifiers in the
  response -- only aggregated counts.
- IF the user is a System Admin, THE SYSTEM SHALL return statistics for any test run
  regardless of the admin's project membership (admin bypass).

### US-2: Statistics Update on Result Change (No-Op from API Perspective)

As a tester updating test case results via the execution endpoints, I want the statistics
endpoint to always reflect the current state of results, so that I do not need to trigger
any recalculation or wait for a cache to expire.

**Acceptance Criteria (EARS)**

- WHEN a test case result status is created or updated through the Test Case Result Update
  endpoint (`test-case-result-update` feature), THE SYSTEM SHALL require no additional
  action by the caller -- the statistics endpoint computes fresh data on the next request.
- THE SYSTEM SHALL NOT use denormalized counters, materialized views, or cached aggregates
  for the statistics endpoint. Every request computes counts from the live result data.
- IF the underlying result data changes between two statistics requests, THE SYSTEM SHALL
  reflect the new state immediately in the next response without any synchronization delay.

---

## Security Considerations

### Authentication
The endpoint requires a valid authenticated session. Requests without a valid session
cookie return `401 Unauthorized` with error code `NOT_AUTHENTICATED`. This is enforced by
`AuthMiddleware` before any handler logic executes.

### Authorization Consistency
The system permission check is the first authorization gate; the project membership check
is the second gate. System Admin implicitly holds all permissions and bypasses both gates.
A `403 Forbidden` response must use a generic message without revealing which gate was
triggered, to prevent information leakage about project existence or membership.

Authorization checks are performed against live data on every request -- not cached in the
session. If a user's role or permissions are changed, the new authorization takes effect on
their next request.

### Data Exposure
The statistics endpoint exposes only aggregated counts per status. It does not expose
individual test case IDs, result details, or any data from test cases that the user could
not otherwise access (because the test run membership check already scopes access to the
project). The response does not leak information about soft-deleted test cases, test
executions, or test case results -- these are excluded from the aggregation query.

### Rate Limiting
The statistics endpoint is a read endpoint and should be in the same rate limit group as
other read endpoints:

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../test-runs/{id}/statistics` | 60 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### SQL Injection Prevention
The aggregation query must use parameterized inputs for the test run ID and project ID.
Never interpolate user input directly into the SQL query.

---

## Out of Scope

- **Statistics for multiple test runs** (bulk/comparative statistics -- only single run in
  Phase 1)
- **Trend/time-series data** (statistics over time -- deferred to Phase 3 reporting)
- **Statistics caching** (fresh computation on every request; caching may be introduced
  later if performance requires it)
- **Statistics by category/priority** (grouped breakdowns beyond result status -- separate
  feature)
- **Statistics export** (CSV, PDF -- deferred to Phase 3)
- **Denormalized counters or materialized views** (not needed for Phase 1 scale)
- **UI views** (statistics display on the test run detail page -- covered in `ui-*` specs)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; the endpoint requires an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission code `test_run:read_statistics` must be seeded in
  the `PERMISSIONS` table and assignable to roles.
- **Project CRUD** -- The `PROJECTS` table must exist; the test run is scoped to a project.
- **Project Members** -- Project membership (any role) is used for authorization scope
  checks.
- **Auth RBAC** -- System permission check for `test_run:read_statistics`.
- **Auth Project Scope** -- Project membership scope check on the endpoint.
- **Test Run CRUD** -- The `TEST_RUNS` table must exist; the statistics endpoint fetches a
  test run by ID and validates it belongs to the specified project.
- **Test Run Cases** -- The `TEST_RUN_TEST_CASES` junction table links test cases to test runs
  and is the base set for statistics aggregation.
- **Test Execution CRUD** -- The `TEST_EXECUTIONS` table must exist; executions link to
  test runs and carry test case result snapshots.
- **Test Case Result Update** -- The `TEST_CASE_RESULTS` table must exist with a `status`
  column whose values include NOT_TESTED, IN_PROGRESS, PASS, FAIL, WARNING, IGNORE.
