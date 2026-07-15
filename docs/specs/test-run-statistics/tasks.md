# Tasks: Test Run Statistics

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Define `TestRunStatistics` data object -- `requirements.md#US-1`, `design.md#Components`
  - Fields: `total`, `not_tested`, `in_progress`, `pass`, `fail`, `warning`, `ignore` (all
    non-negative integers)
  - Constructor validates the invariant: `total` == sum of all status counts. If the
    invariant is violated, panic/error (this indicates a bug in the aggregation query, not
    a user error).
  - No framework imports; pure language struct
  - No domain exceptions needed (the statistics endpoint has no business rule rejections
    beyond authorization -- field-level validation errors are impossible since there are no
    request body or query parameters)

---

## Layer 2 -- Application

- [ ] 2. Define `TestRunStatisticsRepository` interface and response DTOs --
       `design.md#Components`, `design.md#API Contract`
  - `TestRunStatisticsRepository` trait/interface with single method:
    `aggregate_by_status(test_run_id) -> TestRunStatistics`
    Accepts a transaction context for transactional composition. Returns a fully populated
    value (never null -- the aggregation always returns a result, even if all counts zero).
  - `TestRunStatisticsResponse` DTO wrapping `data: TestRunStatistics` for JSON
    serialization (camelCase field mapping for each status count)
  - No additional command/query DTOs (the endpoint has no request body or query params)

- [ ] 3. Implement `TestRunStatisticsService` -- `requirements.md#US-1`, `design.md#Sequence`
  - Single method: `get_statistics(project_id, test_run_id, current_user_id) ->
    TestRunStatistics`
  - Validates `test_run:read_statistics` system permission via `AuthorizationService`.
    If denied and not System Admin -> `403 Forbidden`.
  - Validates project membership (any role) via `ProjectMemberRepository`. If not a member
    and not System Admin -> `403 Forbidden`.
  - Delegates to `TestRunStatisticsRepository::aggregate_by_status(test_run_id)`.
  - System Admin bypasses all permission and membership checks.

- [ ] 4. Write unit tests for `TestRunStatisticsService` -- `requirements.md#US-1`
  - Table-driven tests with mock `TestRunStatisticsRepository`,
    mock `ProjectMemberRepository`, and mock `AuthorizationService`
  - Happy path: mixed statuses return correct counts
  - Empty run: no linked test cases -> all counts zero
  - All not_tested: test cases exist but no executions yet
  - All pass / all fail: homogeneous statuses
  - Permission denial: user lacks `test_run:read_statistics`
  - Project membership denial: user has permission but is not a project member
  - System Admin bypass: admin without project membership still gets results

---

## Layer 3 -- Adapters (HTTP)

- [ ] 5. Implement `TestRunStatisticsHandler` and register route --
       `design.md#API Contract`, `design.md#Route Registration`
  - Single handler method `get_statistics`: extract `project_id` and `test_run_id` from
    path params, validate both are positive integers
  - Validate project exists and is not soft-deleted via `ProjectRepository` ->
    `404 Not Found` if invalid
  - Validate test run exists, belongs to project, and is not soft-deleted via
    `TestRunRepository::find_by_id_in_project` -> `404 Not Found` if invalid (same message
    for missing, soft-deleted, or wrong-project cases)
  - Call `TestRunStatisticsService::get_statistics(...)`, serialize with `200 OK`
  - Map service errors: permission/membership denied -> `403 Forbidden` (generic message);
    not found -> `404 Not Found`; unexpected -> `500 Internal Server Error`
  - Register route: `GET /api/v1/projects/{projectId}/test-runs/{id}/statistics`
    under the test-runs route group with session auth middleware
  - No route ordering concerns (`statistics` is a static sub-path, not a parameter)

- [ ] 6. Write integration tests for the statistics HTTP handler --
       `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back per test)
  - Test `200 OK` with correct counts for a run with mixed statuses; verify invariant
  - Test `200 OK` for a run with no linked test cases (all zeros)
  - Test `200 OK` for a run with test cases but no executions (all not_tested)
  - Test `403 Forbidden` on missing permission; test `403 Forbidden` on non-member
    (identical generic response body for both failure modes)
  - Test `404 Not Found`: non-existent project, soft-deleted project, non-existent test
    run, soft-deleted test run, wrong-project test run (same message for all)
  - Test `401 Unauthorized` on missing/invalid session
  - Test System Admin can read statistics for any project regardless of membership
  - Test soft-deleted executions and soft-deleted results are excluded (fall back to
    not_tested)
  - Test real-time reflection: update a result status, re-request, verify counts updated

---

## Layer 4 -- Infrastructure

- [ ] 7. Implement `SqlTestRunStatisticsRepository` -- `design.md#Components`,
      `design.md#Data Model`
  - Single method `aggregate_by_status(test_run_id) -> TestRunStatistics`
  - Execute one aggregation query that:
    - Starts from `TEST_RUN_TEST_CASES` for the given `test_run_id` (exclude soft-deleted
      links if the junction table supports soft-delete)
    - For each linked test case, finds the latest result from non-deleted
      `TEST_EXECUTIONS` (by `created_at DESC`) linked to the same run
    - Excludes soft-deleted executions and soft-deleted result rows
    - Maps missing results to status `NOT_TESTED`
    - Groups by status, counts, and computes `total` as `SUM`
  - Constraints: use parameterized inputs exclusively (`$1` for `test_run_id`); no string
    interpolation; single database round-trip
  - Write unit tests with a test database (one transaction per case):
    - Run with no linked test cases -> all zeros
    - Run with test cases but no executions -> all not_tested
    - Run with one execution and mixed statuses -> correct counts
    - Run with multiple executions -> uses latest execution per test case
    - Soft-deleted execution excluded; soft-deleted result falls back to not_tested
    - All seven statuses represented; total equals sum invariant holds

---

## Cross-Cutting

- [ ] 8. Wire authorization, dependency injection, and rate limit --
       `design.md#New Permission Code`, `design.md#Components`,
       `requirements.md#Security Considerations`
  - Add permission code `test_run:read_statistics` to the permissions seed migration with
    `INSERT ... ON CONFLICT (code) DO NOTHING`; let DB auto-assign IDs
  - Register `SqlTestRunStatisticsRepository` as the implementation of
    `TestRunStatisticsRepository`
  - Register `TestRunStatisticsService` with its dependencies (`TestRunStatisticsRepository`,
    `AuthorizationService`, `ProjectMemberRepository`)
  - Register `TestRunStatisticsHandler` with `TestRunStatisticsService`,
    `ProjectRepository`, and `TestRunRepository`
  - Wire authorization: map `test_run:read_statistics` to require any project member role
    (Viewer, Contributor, Editor, or Owner); System Admin bypasses
  - Verify rate limit configuration: ensure the statistics endpoint is in the 60 req/min
    group; verify `X-RateLimit-*` headers present on responses and `Retry-After` on `429`
  - Write unit tests for the authorization wiring: verify each role is accepted; verify
    non-member is rejected; verify System Admin bypass
