# Tasks: Test Run Executions

> **Dependency note:** This feature depends on `test-run-crud` for the `test_runs`
> table and the `test_run:read` permission, and on `test-execution-crud` for the
> `test_executions` table with the `test_run_id` FK and on `execution_testers` and
> `execution_test_cases` junction tables for the count aggregations. The auth
> middleware from `iam-auth` and project membership from `project-members` must
> also be in place.

---

## Application Layer

- [ ] 1. **Define `ExecutionSummary` DTO** -- design.md#components
  - Fields: `id` (i64), `name` (String), `status` (String), `tester_count` (u32),
    `case_count` (u32), `created_at` (DateTime<Utc>), `updated_at` (DateTime<Utc>).
  - Serialize all fields in `camelCase` for the JSON response.
  - This is a flat data object with no behaviour -- a pure projection DTO.

- [ ] 2. **Define `TestRunExecutionRepository` interface (port)** --
    design.md#components
  - Single method:
    - `find_by_run_id(run_id: i64, page: u32, limit: u32) -> Result<(Vec<ExecutionSummary>, u64)>`
      -- returns a tuple of (execution list, total count for pagination).
  - Interface lives in `application/` layer. No infrastructure imports.

- [ ] 3. **Implement `ListTestRunExecutionsUseCase`** -- design.md#sequence,
    requirements.md#US-01
  - Accept `project_id: i64`, `run_id: i64`, `page: u32`, `limit: u32`,
    `caller_id: i64`, `is_admin: bool`.
  - Check the caller has `test_run:read` system permission via
    `AuthorizationService::has_permission(caller_id, "test_run:read")`.
    If not, and not admin -> `403 Forbidden`.
  - Check the caller is a member of the project via
    `ProjectMemberRepository::exists(project_id, caller_id)`.
    If not, and not admin -> `403 Forbidden`.
  - Call `TestRunExecutionRepository::find_by_run_id(run_id, page, limit)`.
  - If the repository returns an error, propagate it.
  - Construct and return a paginated response: `{ data: Vec<ExecutionSummary>, meta: { total, page, limit } }`.

## Infrastructure Layer

- [ ] 4. **Implement `SqlTestRunExecutionRepository`** -- design.md#data-model,
    design.md#components
  - Use the project's SQL client (e.g., `sqlx`).
  - `find_by_run_id(run_id, page, limit)`:
    - Compute `offset = (page - 1) * limit`.
    - Execute the parameterized query from design.md#data-model:
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
    - Execute a companion count query:
      ```sql
      SELECT COUNT(*) AS total
      FROM test_executions te
      JOIN test_runs tr ON tr.id = te.test_run_id
      WHERE tr.id = $1
        AND tr.project_id = $2
        AND tr.deleted_at IS NULL
        AND te.deleted_at IS NULL;
      ```
    - Map rows to `ExecutionSummary` structs.
    - Return the tuple `(summaries, total)`.
  - Compile-time interface verification:
    ```rust
    const _: () = {
        fn assert_impl<T: TestRunExecutionRepository>() {}
        assert_impl::<SqlTestRunExecutionRepository>();
    };
    ```

## Adapters Layer

- [ ] 5. **Implement `ListTestRunExecutionsHandler`** -- design.md#api-contract,
    requirements.md#US-01, requirements.md#US-02
  - Extract `projectId` and `id` from the URL path.
  - Validate both are positive integers. Invalid -> `404 Not Found`.
  - Extract `page` and `limit` from query parameters (defaults: 1 and 25).
  - Validate `page` >= 1, `limit` between 1 and 100, both are integers.
    Invalid -> `422 Validation Error`.
  - Verify the Project exists and is not soft-deleted via
    `ProjectRepository::exists_and_active(projectId)`. If not -> `404 Not Found`.
  - Verify the Test Run exists, belongs to the project, and is not soft-deleted
    via `TestRunRepository::find_by_id(projectId, id)`. If not found,
    soft-deleted, or wrong project -> `404 Not Found`.
  - Extract `caller_id` and `is_admin` from the request context (set by
    AuthMiddleware).
  - Call `ListTestRunExecutionsUseCase::execute(projectId, id, page, limit, callerId, isAdmin)`.
  - On success: return `200 OK` with `{ data, meta }`.
  - Map `Forbidden` to `403`.
  - Map any unexpected errors to `500 Internal Server Error`.

- [ ] 6. **Register route and write integration tests** --
    design.md#route-registration
  - Register the route in the test-run-crud router group:
    `GET /api/v1/projects/{projectId}/test-runs/{id}/executions`
  - Wire `SqlTestRunExecutionRepository`, `ListTestRunExecutionsUseCase`, and
    `ListTestRunExecutionsHandler` into the dependency injection container.
  - Integration tests:
    - Authenticated project member can list linked executions: seed a Test Run
      with 3 linked Test Executions, request list, assert `200 OK` with 3 items.
    - Empty list: Test Run with no linked executions returns `200 OK` with
      `data: []` and `meta.total: 0`.
    - Soft-deleted executions are excluded: seed 2 active + 1 soft-deleted
      execution linked to the same run, assert `200 OK` with 2 items.
    - Pagination: seed 30 executions, request `page=2&limit=10`, assert correct
      slice and `meta.total = 30`.
    - Project does not exist: assert `404 Not Found`.
    - Test Run does not exist: assert `404 Not Found`.
    - Test Run belongs to a different project: assert `404 Not Found`
      (same message as non-existent).
    - Soft-deleted Test Run: assert `404 Not Found`.
    - Unauthenticated request: assert `401 Not Authenticated`.
    - User lacks `test_run:read` permission: assert `403 Forbidden`.
    - User is not a project member: assert `403 Forbidden` (generic message,
      indistinguishable from missing permission).
    - System Admin can access without project membership: assert `200 OK`.
    - Invalid `page` (0 or negative): assert `422 Validation Error`.
    - Invalid `limit` (0 or 101): assert `422 Validation Error`.
    - Non-integer `page` or `limit`: assert `422 Validation Error`.
    - Response `data` includes `tester_count` and `case_count`: seed an execution
      with 2 testers and 5 cases, assert count fields match.
