# Tasks: Test Plan Runs

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `TestPlanRun` entity -- `requirements.md#US-2`, `design.md#Components`
  - Fields: `plan_id`, `run_id`, `linked_by`, `linked_at`
  - Factory method `TestPlanRun::link(plan_id, run_id, linked_by)` with domain validation:
    - `plan_id` must be a positive integer (> 0)
    - `run_id` must be a positive integer (> 0)
    - `linked_by` must be a positive integer (> 0)
    - `linked_at` is set to the current UTC timestamp at creation time
  - No mutator methods (links are immutable; they either exist or are deleted)
  - No framework imports; pure language struct + impl
  - Implement `PartialEq` and `Debug` traits

- [ ] 2. Define domain exceptions for test plan runs -- `design.md#Error Handling`
  - `TestPlanRunNotFoundError` (carries `plan_id` and `run_id`)
  - `TestPlanRunPermissionDenied` (carries the reason: missing system permission or wrong
    project role)
  - `InvalidRunIdError` (carries the invalid run ID and reason, e.g., not found,
    soft-deleted)
  - `TestPlanRunValidationError` (carries field-level details for batch validation
    failures)

---

## Layer 2 -- Application

- [ ] 3. Define `TestPlanRunRepository` interface (port) -- `design.md#Components`,
      `design.md#Data Model`
  - Methods:
    - `find_by_plan(plan_id, page, limit) -> (Vec<LinkedRunItem>, u64 total)` --
      returns paginated linked runs with resolved Test Run fields (summary, status,
      project_id, project_name) via JOIN on `TEST_RUNS`. Filters out soft-deleted runs
      (`WHERE tr.deleted_at IS NULL`). Ordered by `linked_at DESC`.
    - `link(plan_id, run_id, linked_by)` -- inserts a single junction row. Returns
      `Ok(())` on success. On conflict (already linked), silently succeeds (no error).
    - `bulk_link(plan_id, run_ids, linked_by) -> Vec<i64>` -- inserts multiple rows.
      Uses `INSERT ... ON CONFLICT (plan_id, run_id) DO NOTHING`. Returns the list of
      newly linked run IDs (excludes already-linked duplicates).
    - `delete(plan_id, run_id) -> bool` -- hard-deletes the junction row. Returns
      `true` if a row was deleted, `false` if no link existed.
    - `exists(plan_id, run_id) -> bool` -- checks whether a link exists.
    - `validate_run_ids(run_ids: &[i64]) -> Vec<i64>` -- accepts a list of run IDs and
      returns the subset that exist and are not soft-deleted. Used to validate the
      entire batch before any inserts.
  - All methods accept a transaction context for transactional composition
  - Return domain entities (not raw DB rows)
  - `LinkedRunItem` is a DTO with `run_id`, `summary`, `status`, `project_id`,
    `project_name`, `linked_by`, `linked_at`

- [ ] 4. Define DTOs for request/response -- `design.md#API Contract`
  - `LinkRunsRequest` (run_ids: Vec<i64>) -- deserialized from POST body
  - `LinkRunsResponse` (linked: Vec<i64>) -- summary of newly linked run IDs
  - `LinkedRunItem` (run_id, summary, status, project_id, project_name, linked_by,
    linked_at) -- individual entry in list response
  - `ListLinkedRunsResponse` (data: Vec<LinkedRunItem>, meta: PaginationMeta) --
    paginated list response
  - `PaginationMeta` (total, page, limit) -- reuse from shared utilities if available

- [ ] 5. Implement `TestPlanRunService` -- `requirements.md#US-1` through `US-3`,
      `design.md#Sequence`
  - `list_linked_runs(plan_id, page, limit, current_user_id)`:
    validates `test_plan:read` permission, checks the user is a project member of at
    least one project linked to the Test Plan, calls `find_by_plan`, returns paginated
    response. System Admin bypasses project membership check.
  - `link_runs(plan_id, run_ids, current_user_id)`:
    validates `test_plan:update` permission, checks the user is an Owner or Editor of
    at least one project linked to the Test Plan. Begins DB transaction. Calls
    `validate_run_ids` to confirm all IDs are valid (exist + non-deleted). If any are
    invalid, collects errors, rolls back, and returns validation error with details for
    each invalid ID. Calls `bulk_link` for all valid IDs. Commits transaction. Returns
    list of newly linked run IDs. System Admin bypasses project membership check.
  - `unlink_run(plan_id, run_id, current_user_id)`:
    validates `test_plan:update` permission, checks Owner/Editor role. Calls
    `delete(plan_id, run_id)`. If returns `false` -> raises
    `TestPlanRunNotFoundError`. No transaction needed for single-row delete. System
    Admin bypasses project membership check.
  - All methods check system permission via `AuthorizationService`
  - All methods check project membership/role via `ProjectMemberRepository` scoped to
    the Test Plan's linked projects
  - Service does NOT deduplicate `run_ids` -- that responsibility is at the handler
    layer (input boundary)

- [ ] 6. Write unit tests for `TestPlanRunService` -- `requirements.md#US-1` through `US-3`
  - Table-driven tests with mock `TestPlanRunRepository`, mock `ProjectMemberRepository`,
    and mock `AuthorizationService`
  - Happy path: list linked runs (paginated, empty list), link runs (single, batch),
    unlink run
  - List: verify pagination parameters are passed through correctly
  - List: verify empty result when no runs are linked (`total: 0`, `data: []`)
  - Link: all run IDs valid -> all linked successfully
  - Link: some run IDs already linked -> silently skipped; response includes only newly
    linked
  - Link: all run IDs already linked -> response `linked: []`, still `200 OK`
  - Link: some run IDs invalid -> transaction rolled back, validation error returned
    with details per invalid ID
  - Link: all run IDs invalid -> transaction rolled back, validation error returned
  - Unlink: existing link removed -> success, repository returns `true`
  - Unlink: link does not exist -> `TestPlanRunNotFoundError`, repository returns `false`
  - Permission denial: missing `test_plan:read` -> `403`
  - Permission denial: missing `test_plan:update` -> `403`
  - Project role denial: Viewer/Contributor cannot link or unlink -> `403`
  - Project role acceptance: any member can list; Owner/Editor can link/unlink
  - System Admin bypasses all project membership checks
  - Service does NOT deduplicate `run_ids` internally (handler responsibility)
  - Boundary: single run ID linked, batch of 100 run IDs linked

---

## Layer 3 -- Adapters (HTTP)

- [ ] 7. Implement `TestPlanRunHandler` -- `design.md#API Contract`, `design.md#Components`
  - Three handler methods: `list`, `link`, `unlink`
  - Deserialize request bodies and query params into DTOs
  - Validate pagination: `page` in [1, 1000], `limit` in [1, 100]; return `422` otherwise
  - Validate `runId` path parameter in `unlink` is a positive integer; return `422` for
    non-integer, zero, or negative values
  - Validate Test Plan exists and is not soft-deleted before any operation
    (call `TestPlanRepository::find_by_id` or a shared validation guard)
  - In `link` handler:
    - Reject empty or missing `run_ids` array with `422`
    - Reject `run_ids` exceeding 100 entries with `422`
    - Reject any non-integer, zero, or negative values in `run_ids` with `422`
    - Deduplicate `run_ids` (remove duplicate values within the array) before passing
      to service
    - Reject requests containing unrecognised top-level fields with `422` (strict mode)
  - Call `TestPlanRunService` methods
  - Serialize responses with proper status codes:
    - `200 OK` for list and link
    - `204 No Content` for unlink
  - Map domain/service errors to HTTP status codes and standard error body format:
    - `TestPlanRunPermissionDenied` -> `403 Forbidden` (generic message)
    - `TestPlanRunNotFoundError` -> `404 Not Found`
    - `InvalidRunIdError` -> `422 Unprocessable Entity` with field-level details
    - `TestPlanRunValidationError` -> `422 Unprocessable Entity` with field-level details

- [ ] 8. Register test plan run routes in HTTP router -- `design.md#Route Registration`
  - `GET    /api/v1/test-plans/{id}/runs`         -> `list`
  - `POST   /api/v1/test-plans/{id}/runs`         -> `link`
  - `DELETE /api/v1/test-plans/{id}/runs/{runId}` -> `unlink`
  - All routes require session auth middleware
  - No route ordering conflict: `/runs` is a sub-resource and `/{runId}` is an integer
    directly under `/runs/`, so there is no static-path vs dynamic-path disambiguation
    issue
  - Note: these routes must be registered **after** the test-plan-crud routes are
    registered (the `test-plans` path prefix is defined by that spec)

- [ ] 9. Write integration tests for test plan run HTTP handlers --
       `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back)
  - Prerequisite: seed a Test Plan, a project linked to the plan, a user with project
    membership, and several Test Runs in the database
  - Test `200 OK` for list with pagination metadata
  - Test list returns empty `data` array with `total: 0` when no runs linked
  - Test list excludes soft-deleted Test Runs
  - Test list includes `summary`, `status`, `project_id`, `project_name` for each run
  - Test `200 OK` for link (single run)
  - Test `200 OK` for link (batch of multiple runs)
  - Test link silently skips already-linked runs (idempotent; only newly linked IDs in
    `linked` array of response)
  - Test `422` on empty `run_ids` array
  - Test `422` on missing `run_ids` field
  - Test `422` on `run_ids` exceeding 100 entries
  - Test `422` on invalid run IDs (non-existent, soft-deleted)
  - Test `422` rejects non-integer values in `run_ids` (e.g., `"abc"`, `1.5`)
  - Test `422` rejects zero and negative values in `run_ids`
  - Test `422` on unrecognised fields in POST body (strict mode)
  - Test `204 No Content` on successful unlink
  - Test `404 Not Found` on unlink when no link exists
  - Test `404 Not Found` on unlink when Test Plan is soft-deleted
  - Test `403 Forbidden` on missing `test_plan:read` permission (list)
  - Test `403 Forbidden` on missing `test_plan:update` permission (link)
  - Test `403 Forbidden` on missing `test_plan:update` permission (unlink)
  - Test `403 Forbidden` on insufficient project role (Viewer tries to link)
  - Test `403 Forbidden` on insufficient project role (Contributor tries to unlink)
  - Test that `403` response bodies use generic message (no distinction between
    missing permission and wrong role)
  - Test System Admin can perform all operations regardless of project membership
  - Test rate limit headers present on all endpoints
  - Test duplicate `run_ids` within array are deduplicated by handler before reaching
    service (verify only unique IDs in response)
  - Test `linked_by` and `linked_at` are set on link
  - Test link transaction rolls back on any invalid run ID (verify no partial links
    when batch contains one invalid ID)
  - Test `POST` rejects non-JSON content type
  - Test `runId` path parameter: non-integer, zero, and negative values return `422`
  - Verify pagination: `page` and `limit` validation (page < 1, page > 1000, limit < 1,
    limit > 100, non-integer values)

---

## Layer 4 -- Infrastructure

- [ ] 10. Create `TEST_PLAN_TEST_RUNS` database migration -- `design.md#Data Model`
  - Table definition with all columns, composite PK, FKs
  - `plan_id BIGINT NOT NULL REFERENCES test_plans(id) ON DELETE RESTRICT`
  - `run_id BIGINT NOT NULL REFERENCES test_runs(id) ON DELETE RESTRICT`
  - `linked_by BIGINT NOT NULL REFERENCES users(id) ON DELETE RESTRICT`
  - `linked_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
  - Composite primary key: `CONSTRAINT pk_test_plan_test_runs PRIMARY KEY (plan_id, run_id)`
  - Foreign key indexes:
    - `CREATE INDEX idx_test_plan_test_runs_plan_id ON test_plan_test_runs (plan_id)`
    - `CREATE INDEX idx_test_plan_test_runs_run_id ON test_plan_test_runs (run_id)`
    - `CREATE INDEX idx_test_plan_test_runs_linked_by ON test_plan_test_runs (linked_by)`
  - Composite index for common query pattern:
    - `CREATE INDEX idx_test_plan_test_runs_plan_linked ON test_plan_test_runs (plan_id, linked_at DESC)`
  - No trigger needed (no `updated_at` column -- links are immutable)
  - No soft-delete columns (junction table uses hard delete per FR-52 convention)
  - Rollback migration: `DROP TABLE IF EXISTS test_plan_test_runs`
  - Ensure migration runs after `TEST_PLANS` and `TEST_RUNS` migrations (FK dependencies)

- [ ] 11. Implement `SqlTestPlanRunRepository` -- `design.md#Components`
  - All methods from `TestPlanRunRepository` interface
  - `find_by_plan` executes a JOIN query:
    ```sql
    SELECT tr.id AS run_id,
           tr.summary,
           tr.status,
           tr.project_id,
           p.name AS project_name,
           tp.linked_by,
           tp.linked_at
    FROM test_plan_test_runs tp
    JOIN test_runs tr ON tp.run_id = tr.id AND tr.deleted_at IS NULL
    JOIN projects p ON tr.project_id = p.id AND p.deleted_at IS NULL
    WHERE tp.plan_id = $1
    ORDER BY tp.linked_at DESC
    LIMIT $2 OFFSET $3
    ```
    Also execute a `COUNT(*)` query with the same JOIN and WHERE clause for `total`.
  - `link` executes:
    ```sql
    INSERT INTO test_plan_test_runs (plan_id, run_id, linked_by, linked_at)
    VALUES ($1, $2, $3, NOW())
    ON CONFLICT (plan_id, run_id) DO NOTHING
    ```
  - `bulk_link` executes a single INSERT with multiple rows (use unnest or multiple
    VALUES clauses parameterized with array binding):
    ```sql
    INSERT INTO test_plan_test_runs (plan_id, run_id, linked_by, linked_at)
    SELECT $1, unnest($2::bigint[]), $3, NOW()
    ON CONFLICT (plan_id, run_id) DO NOTHING
    RETURNING run_id
    ```
    Returns the list of newly inserted run IDs.
  - `delete` executes:
    ```sql
    DELETE FROM test_plan_test_runs
    WHERE plan_id = $1 AND run_id = $2
    ```
    Returns `true` if `rows_affected > 0`.
  - `exists` executes:
    ```sql
    SELECT EXISTS(
      SELECT 1 FROM test_plan_test_runs
      WHERE plan_id = $1 AND run_id = $2
    )
    ```
  - `validate_run_ids` executes:
    ```sql
    SELECT id FROM test_runs
    WHERE id = ANY($1::bigint[]) AND deleted_at IS NULL
    ```
    Returns only the valid subset of IDs.
  - Use parameterized queries exclusively -- never string interpolation
  - Write unit tests with a test database (one test transaction per case):
    - Insert a link and verify it can be retrieved via `find_by_plan`
    - `find_by_plan` excludes soft-deleted Test Runs (link exists but run is deleted --> excluded from results)
    - `find_by_plan` excludes runs from soft-deleted projects
    - `find_by_plan` paginates correctly (page 1, page 2, empty page)
    - `find_by_plan` orders by `linked_at DESC`
    - `link` with duplicate (plan_id, run_id) silently succeeds (ON CONFLICT DO NOTHING)
    - `bulk_link` inserts only non-duplicate rows and returns their run IDs
    - `bulk_link` with all duplicate run IDs returns empty array
    - `delete` removes an existing link and returns `true`
    - `delete` on non-existent link returns `false`
    - `exists` returns `true` for existing link, `false` for non-existent
    - `validate_run_ids` returns only existing, non-deleted IDs
    - `validate_run_ids` excludes soft-deleted Test Runs
    - `validate_run_ids` with empty input returns empty array

- [ ] 12. Wire authorization and dependency injection for test plan run components --
       `design.md#Components`, `design.md#Modified Existing Components`
  - Register `SqlTestPlanRunRepository` as the implementation of `TestPlanRunRepository`
  - Register `TestPlanRunService` with its dependencies (`TestPlanRunRepository`,
    `AuthorizationService`, `ProjectMemberRepository`)
  - Register `TestPlanRunHandler` with `TestPlanRunService`
  - Extend the authorization guard to resolve project membership from the Test Plan's
    linked projects (not a single `project_id` path parameter):
    - For `test_plan:read`: verify the user is a member of at least one project linked
      to the Test Plan
    - For `test_plan:update`: verify the user is an Owner or Editor of at least one
      project linked to the Test Plan
    - System Admin bypasses all checks
  - If no new permission codes are needed (reuses `test_plan:read` and `test_plan:update`
    from `test-plan-crud`), verify those codes are correctly registered in the
    permission registry and seed data
  - If using a DI container, ensure all lifetimes/scopes are correct (e.g., repository
    scoped to request, service transient/singleton)
  - If using manual wiring in `main`, add the wiring code in the correct order
  - Write unit tests for authorization wiring:
    - Verify `test_plan:read` -> any project role in linked project passes
    - Verify `test_plan:update` -> Owner passes
    - Verify `test_plan:update` -> Editor passes
    - Verify `test_plan:update` -> Contributor rejected
    - Verify `test_plan:update` -> Viewer rejected
    - Verify non-member of any linked project rejected
    - Verify System Admin bypass for all operations

---

## Cross-Cutting Tasks

- [ ] 13. Coordinate with `test-plan-crud` spec for Test Plan detail view integration --
       `requirements.md#US-4`
  - Ensure the Test Plan detail response includes a way for the UI to access linked
    runs. Options (to be decided in `test-plan-crud` spec):
    - Include `runs_count` field in the Test Plan detail response body
    - Include `runs_url` (e.g., `"/api/v1/test-plans/42/runs"`) in the response body
    - Or rely on the UI to make a separate `GET /runs` call
  - Verify the Test Plan detail page renders the linked runs list using the data from
    `GET /api/v1/test-plans/{id}/runs`
  - Confirm that `test-plan-crud` spec references this spec as a dependency

- [ ] 14. Add `X-Request-ID` correlation logging to test plan run endpoints --
       `design.md#Route Registration`
  - If a global middleware already handles `X-Request-ID` / `X-Correlation-ID`, no
    additional work is needed. If not, add it to the route group.
  - Accept from the client and echo back; generate a UUID v4 if absent

- [ ] 15. Verify rate limit configuration covers test plan run endpoints --
       `requirements.md#Security Considerations`
  - If rate limiting is configured at the route group level, ensure routes are in the
    correct groups:
    - `GET .../test-plans/{id}/runs` -- 60 req/min group
    - `POST .../test-plans/{id}/runs` -- 30 req/min group
    - `DELETE .../test-plans/{id}/runs/{runId}` -- 30 req/min group
  - If rate limiting is per-endpoint, add the configuration for each of the 3 endpoints
  - Verify `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` headers are
    present on responses
  - Verify `Retry-After` header is present on `429` responses

- [ ] 16. Manual QA checklist for test plan runs -- `requirements.md#US-1` through `US-4`
  - [ ] Link a single Test Run to a Test Plan (verify `200 OK`, run appears in list)
  - [ ] Link multiple Test Runs in one request (verify all appear in list)
  - [ ] Attempt to link an already-linked run (verify idempotent -- `linked` array excludes
    it, run still appears once in list)
  - [ ] Attempt to link with empty `run_ids` (verify `422`)
  - [ ] Attempt to link with non-existent run ID (verify `422`)
  - [ ] Attempt to link with soft-deleted Test Run (verify `422`)
  - [ ] Attempt to link with > 100 run IDs (verify `422`)
  - [ ] Attempt to link with unrecognised field in body (verify `422` strict mode)
  - [ ] List linked runs with pagination (verify `meta` and correct ordering)
  - [ ] List linked runs for a plan with no runs (verify empty `data`, `total: 0`)
  - [ ] Unlink a run from a plan (verify `204 No Content`, run gone from list)
  - [ ] Attempt to unlink a run that is not linked (verify `404`)
  - [ ] Attempt to unlink from a non-existent Test Plan (verify `404`)
  - [ ] Verify soft-deleted runs are excluded from the list
  - [ ] Attempt to link as Viewer (verify `403`)
  - [ ] Attempt to link as Contributor (verify `403`)
  - [ ] Link as Editor (verify success)
  - [ ] Link as Owner (verify success)
  - [ ] List linked runs as any project member (verify success)
  - [ ] Verify System Admin can link, list, and unlink regardless of membership
  - [ ] Verify `linked_by` and `linked_at` are correctly set on new links
  - [ ] Verify rate limit headers present on all endpoints
  - [ ] Verify Test Plan detail view shows linked runs (when UI integration complete)
  - [ ] Verify `403` response bodies use generic message (no distinction between
    missing permission and wrong role)
