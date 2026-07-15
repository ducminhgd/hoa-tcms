# Tasks: Test Execution CRUD

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `TestExecution` entity -- `requirements.md#US-1`, `design.md#Components`
  - Fields: `id`, `name`, `test_run_id`, `created_by`, `created_at`, `updated_by`,
    `updated_at`, `deleted_by`, `deleted_at`
  - Factory method `TestExecution::create(name, test_run_id, tester_ids, created_by)`
    with domain validation:
    - `name` trimmed and must contain at least one non-whitespace character, max 500
      chars
    - `tester_ids` must be non-empty (at least one tester required)
    - Duplicate `tester_ids` are deduplicated
  - No framework imports; pure language struct + impl
  - Implement `is_soft_deleted()` convenience method
  - Implement a method to produce an update with changed fields:
    `apply_update(cmd: UpdateTestExecutionCommand)` -- validates new values and returns a
    new (or modified) entity. Fields: `name` (Option<String>), `tester_ids`
    (Option<Vec<i64>> -- None = not provided (preserve), Some = replace).

- [ ] 2. Define domain exceptions for test executions -- `design.md#Error Handling`
  - `TestExecutionNotFoundError`
  - `DuplicateTestExecutionNameError` (carries the conflicting name)
  - `TestExecutionPermissionDenied` (carries the reason: missing system permission,
    wrong project role, or contributor ownership restriction)
  - `TestExecutionValidationError` (carries field-level details)
  - `InvalidTesterError` (carries the invalid user IDs and reasons)

- [ ] 3. Define value objects -- `design.md#Components`
  - `TestExecutionName` value object: wraps a `String`, validates non-empty after trim,
    max 500 chars, stores trimmed value. Implements `Display` and equality traits.
  - `TesterAssignment` value object: `execution_id`, `user_id`. Simple struct with no
    domain logic beyond construction. Used for junction table operations.

---

## Layer 2 -- Application

- [ ] 4. Define `TestExecutionRepository` interface (port) -- `design.md#Components`,
      `design.md#Data Model`
  - Methods:
    - `find_by_id(execution_id, run_id) -> Option<TestExecutionDetail>` -- includes
      resolved tester list (`user_id` and `username` via JOINs on
      `TEST_EXECUTION_TESTERS` and `USERS`)
    - `find_by_name_in_run(run_id, name) -> Option<TestExecution>` -- uses
      `LOWER(name) = LOWER($1)`
    - `find_by_run(run_id, page, limit, search, sort) ->
      (Vec<TestExecutionListItem>, u64 total)` -- includes tester details in each item
    - `find_all_active_by_run(run_id) -> Vec<TestExecutionSelectItem>`
    - `save(execution, tester_ids) -> TestExecutionDetail` -- inserts execution row,
      then inserts junction rows for tester assignments, returns with generated `id`,
      timestamps, and resolved tester details
    - `update(execution, tester_ids?) -> TestExecutionDetail` -- updates execution row;
      if `tester_ids` is provided, deletes all existing junction rows and re-inserts
      within the same transaction. Returns with resolved tester details.
    - `soft_delete(execution_id, deleted_by)` -- sets `deleted_at` and `deleted_by`
    - `validate_testers_in_project(tester_ids, project_id) ->
      Vec<InvalidTesterDetail>` -- for each user ID, checks existence, ACTIVE status,
      and project membership. Returns empty vec if all valid; returns details for each
      invalid ID.
  - All methods accept a transaction context for transactional composition
  - Return domain entities (not raw DB rows)
  - `TestExecutionSelectItem` is a lightweight DTO with only `id` and `name`
  - `TestExecutionDetail` includes all fields plus resolved tester list
  - `TestExecutionListItem` includes all fields plus tester list, excluding
    `updated_by`, `deleted_at`, `deleted_by`

- [ ] 5. Define command/query/response DTOs -- `design.md#API Contract`
  - `CreateTestExecutionCommand` (name, tester_ids: Vec<i64>)
  - `UpdateTestExecutionCommand` (name?: Option<String>, tester_ids?:
    Option<Vec<i64>>) -- `None` = not provided (preserve), `Some` = set to value
  - `ListTestExecutionsQuery` (run_id, page, limit, search?, sort?)
  - `TestExecutionListItem` (id, name, test_run_id, testers: Vec<TesterDto>, created_by,
    created_at, updated_at) -- used in list endpoints; includes tester details for UI
  - `TestExecutionDetailResponse` (id, name, test_run_id, testers: Vec<TesterDto>,
    created_by, created_at, updated_by, updated_at) -- full detail view
  - `TestExecutionSelectItem` (id, name)
  - `TesterDto` (user_id: i64, username: String)
  - `PaginationMeta` (total, page, limit) -- reuse from shared utilities if available

- [ ] 6. Implement `TestExecutionService` -- `requirements.md#US-1` through `US-6`,
      `design.md#Sequence`
  - `create_execution(project_id, run_id, cmd, current_user_id)`:
    validates `test_execution:create` permission, checks Contributor/Editor/Owner role
    (Viewer rejected), begins DB transaction, validates Test Run existence and project
    scope within transaction (SELECT ... FOR UPDATE), checks duplicate name within
    transaction, validates tester IDs within transaction (user exists, ACTIVE, project
    member), constructs entity, saves via repository (inserts execution + junction rows),
    invokes `TestCaseImportService::import_from_run(execution_id, run_id)` to snapshot
    test cases from the Test Run, commits transaction, returns
    `TestExecutionDetailResponse`. If import fails, rolls back entire transaction (no
    execution created).
  - `list_executions(project_id, run_id, query, current_user_id)`:
    validates `test_execution:read_list` permission, checks project membership (any
    role), validates Test Run existence and project scope, delegates to repository with
    pagination/search/sort, returns paginated response
  - `get_execution(project_id, run_id, execution_id, current_user_id)`:
    validates `test_execution:read` permission, checks project membership, validates
    Test Run existence and project scope, fetches by ID, verifies execution belongs to
    the Test Run, returns `TestExecutionDetailResponse`
  - `update_execution(project_id, run_id, execution_id, cmd, current_user_id)`:
    validates `test_execution:update` permission, checks the user is at least a
    Contributor (Viewer rejected). If Contributor: fetches execution and verifies
    `created_by == current_user_id`; if not owner -> `403` (ownership restriction).
    Owners and Editors skip ownership check. Begins DB transaction. Validates Test Run
    scope. If name is changing: checks duplicate name within transaction. If
    `tester_ids` is provided: validates tester IDs within transaction, reconciles
    junction table (delete-all + re-insert). Applies updates, saves via repository,
    commits transaction, returns `TestExecutionDetailResponse`
  - `delete_execution(project_id, run_id, execution_id, current_user_id)`:
    validates `test_execution:delete` permission, checks the user is at least a
    Contributor. If Contributor: verifies `created_by == current_user_id` (ownership
    restriction). Owners and Editors skip ownership check. Fetches execution to verify
    existence and scope. Calls `soft_delete(execution_id, current_user_id)`. No
    referential integrity check needed (results are preserved and hidden by query
    filters).
  - `select_executions(project_id, run_id, current_user_id)`:
    validates `test_execution:select` permission, checks project membership (any role),
    validates Test Run existence and scope, delegates to `find_all_active_by_run`,
    returns flat list of `TestExecutionSelectItem`
  - All methods check system permission via `AuthorizationService`
  - All methods check project membership/role via `ProjectMemberRepository`
  - System Admin bypasses all permission, membership, and ownership checks

- [ ] 7. Write unit tests for `TestExecutionService` -- `requirements.md#US-1` through
      `US-6`
  - Table-driven tests with mock `TestExecutionRepository`, mock
    `ProjectMemberRepository`, mock `AuthorizationService`, and mock
    `TestCaseImportService`
  - Happy path: create (with test case import), list (paginated), get, update, delete,
    select
  - Duplicate name rejection (create and update)
  - Invalid tester rejection (non-existent user, inactive user, non-project-member)
  - Empty tester_ids rejection (create requires at least one; update rejects empty
    when provided; omitting preserves)
  - Tester deduplication (duplicate IDs in input produce single junction row)
  - Permission denial for each permission code
  - Project role denial: Viewer cannot create/update/delete; Viewer can read/select
  - Project role acceptance: Contributor can create; Contributor can update/delete own
  - Contributor ownership restriction: cannot update/delete another user's execution
  - Owner/Editor can update/delete any execution regardless of `created_by`
  - Execution not found returns error
  - Test Run not found, soft-deleted, or wrong project returns error
  - Update on soft-deleted execution returns error
  - Soft-delete already-deleted execution returns error
  - No referential integrity check on soft-delete
  - Test case import failure causes transaction rollback (execution not created)
  - Search and sort parameters passed through to repository correctly
  - System Admin bypasses project membership and ownership checks
  - Boundary values: name at exactly 500 chars (passes), 501 chars (rejects), empty
    string, single char, whitespace-only string (rejects), leading/trailing whitespace
    trimmed
  - Transaction rollback: duplicate name after concurrent insert returns clean 409
  - Tester reconciliation on update: old testers removed, new testers added within
    transaction
  - Omit `tester_ids` on update: preserves current tester set
  - Provide empty `tester_ids` on update: rejects with 422

---

## Layer 3 -- Adapters (HTTP)

- [ ] 8. Implement `TestExecutionHandler` -- `design.md#API Contract`,
      `design.md#Components`
  - Six handler methods: `list`, `create`, `get`, `update`, `delete`, `select`
  - Deserialize request bodies and query params into DTOs
  - Validate pagination: `page` in [1, 1000], `limit` in [1, 100]; return `422`
    otherwise
  - Validate `sort` against whitelist (`id`, `-id`, `name`, `-name`, `created_at`,
    `-created_at`); return `422` for invalid values
  - Validate `search` does not exceed 255 characters; return `422` otherwise
  - Validate project exists and is not soft-deleted before any execution operation
  - Validate Test Run exists, is not soft-deleted, and belongs to the project before
    any execution operation
  - Strip HTML tags from `name` field before passing to service (XSS input sanitization
    at the boundary)
  - For `create` and `update`: implement strict mode -- reject request bodies that
    contain unrecognised fields
  - For `create`: validate `tester_ids` is present, non-empty, and contains valid
    integers
  - For `update`: handle `tester_ids` semantics -- omitted preserves current, provided
    (non-empty) replaces, empty rejects with `422`
  - Call `TestExecutionService` methods
  - Serialize responses with proper status codes:
    - `201 Created` with `Location` header for create
    - `200 OK` for list, get, update, select
    - `204 No Content` for delete
  - Map domain/service errors to HTTP status codes and standard error body format:
    - `TestExecutionPermissionDenied` -> `403 Forbidden` (generic message for system
      perm / project role failures; distinct message for Contributor ownership failure)
    - `TestExecutionNotFoundError` -> `404 Not Found`
    - `DuplicateTestExecutionNameError` -> `409 Conflict`
    - `InvalidTesterError` -> `422 Unprocessable Entity` with `INVALID_TESTER`
    - `TestExecutionValidationError` -> `422 Unprocessable Entity` with field-level
      details

- [ ] 9. Register test execution routes in HTTP router --
      `design.md#Route Registration`
  - `GET    /api/v1/projects/{projectId}/test-runs/{runId}/executions`          -> `list`
  - `POST   /api/v1/projects/{projectId}/test-runs/{runId}/executions`          -> `create`
  - `GET    /api/v1/projects/{projectId}/test-runs/{runId}/executions/select`   -> `select`  (static path)
  - `GET    /api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}`     -> `get`     (dynamic path)
  - `PATCH  /api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}`     -> `update`
  - `DELETE /api/v1/projects/{projectId}/test-runs/{runId}/executions/{id}`     -> `delete`
  - All routes require session auth middleware
  - Route order: `/select` **must** be registered before `/{id}` to prevent the
    word "select" from being interpreted as an execution ID

- [ ] 10. Write integration tests for test execution HTTP handlers --
       `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back)
  - Test `201 Created` with `Location` header
  - Test `200 OK` for list with pagination metadata
  - Test list includes tester details (user_id, username) per execution
  - Test search filter (query parameter applied correctly to name)
  - Test sort parameter (`id`, `-id`, `name`, `-name`, `created_at`, `-created_at`)
  - Test `200 OK` for select (flat list, only id and name)
  - Test `200 OK` for detail with resolved tester details
  - Test `409 Conflict` on duplicate name (create and update)
  - Test `422` with `INVALID_TESTER` on invalid tester IDs (non-existent, inactive,
    not project member)
  - Test `422` with `VALIDATION_ERROR` on empty `tester_ids` on create
  - Test `422` with `VALIDATION_ERROR` on empty `tester_ids` on update (when provided)
  - Test that omitting `tester_ids` on update preserves current testers
  - Test that providing `tester_ids` on update replaces testers
  - Test `422 Unprocessable Entity` on validation errors
  - Test `403 Forbidden` on missing permission (each endpoint)
  - Test `403 Forbidden` on Viewer trying to create/update/delete
  - Test `403 Forbidden` on Contributor trying to update/delete another user's execution
  - Test Contributor can create executions
  - Test Contributor can update/delete their own executions
  - Test Owner/Editor can update/delete any execution
  - Test `404 Not Found` on non-existent project
  - Test `404 Not Found` on non-existent/soft-deleted Test Run
  - Test `404 Not Found` on Test Run belonging to a different project
  - Test `404 Not Found` on non-existent/soft-deleted execution
  - Test `404 Not Found` on execution belonging to a different Test Run
  - Test `204 No Content` on successful soft-delete
  - Test `404 Not Found` on repeated soft-delete
  - Test that System Admin can perform all operations on any project regardless of
    membership or ownership
  - Test that `/select` route is not shadowed by `/{id}` (GET with "select" as ID
    hits the select handler, not the get handler)
  - Test edge cases: name with only whitespace rejected, name exactly 500 chars
    accepted, name at 501 chars rejected
  - Test tester deduplication (duplicate IDs in input produce single tester entry)
  - Test unrecognised fields in request body return `422` (strict mode)
  - Test Unicode name (e.g., Japanese text) round-trips correctly
  - Test that `403` response bodies for system permission / project role failures use
    identical generic message
  - Test that `403` response body for Contributor ownership failure uses a distinct
    message ("Contributors can only update/delete their own executions")
  - Test that `X-Result-Truncated: true` header appears when select results hit the
    limit

---

## Layer 4 -- Infrastructure

- [ ] 11. Create database migrations for `TEST_EXECUTIONS` and
       `TEST_EXECUTION_TESTERS` -- `design.md#Data Model`

  **Migration 1: `TEST_EXECUTIONS` table**
  - Table definition with all columns, PK, FKs, constraints
  - `DEFAULT NOW()` on `created_at` and `updated_at`
  - Partial unique index `uq_test_executions_name_run` on `(test_run_id, LOWER(name))`
    with `WHERE deleted_at IS NULL`
  - Foreign key indexes on `test_run_id`, `created_by`, `updated_by`, `deleted_by`
  - Partial index `idx_test_executions_active` on `(test_run_id, id DESC)
    WHERE deleted_at IS NULL`
  - `BEFORE UPDATE` trigger `trg_test_executions_updated_at` that sets
    `NEW.updated_at = NOW()`
  - Verify migration order: `TEST_RUNS` and `USERS` migrations must run before
    `TEST_EXECUTIONS`

  **Migration 2: `TEST_EXECUTION_TESTERS` junction table**
  - Columns: `execution_id BIGINT NOT NULL`, `user_id BIGINT NOT NULL`
  - Composite `PRIMARY KEY (execution_id, user_id)`
  - FK on `execution_id` references `test_executions(id) ON DELETE RESTRICT`
  - FK on `user_id` references `users(id) ON DELETE RESTRICT`
  - Indexes on `execution_id` and `user_id`
  - Verify migration order: this table must run after `TEST_EXECUTIONS`

  **Rollback migrations:**
  - `DROP TABLE IF EXISTS test_execution_testers`
  - `DROP TRIGGER IF EXISTS trg_test_executions_updated_at ON test_executions`
  - `DROP FUNCTION IF EXISTS trg_test_executions_updated_at()`
  - `DROP TABLE IF EXISTS test_executions`

- [ ] 12. Add test execution permission codes to the permissions seed migration --
       `design.md#New Permission Codes`
  - Add 6 new rows to the permissions seed data with
    `INSERT ... ON CONFLICT (code) DO NOTHING`:
    `('test_execution:create', 'Create Test Execution')`,
    `('test_execution:read', 'Read Test Execution')`,
    `('test_execution:read_list', 'Read Test Execution List')`,
    `('test_execution:update', 'Update Test Execution')`,
    `('test_execution:delete', 'Delete Test Execution')`,
    `('test_execution:select', 'Select Test Execution')`
  - Let the database auto-assign IDs (do NOT hardcode numeric IDs -- they will collide
    with other features' permission seeds)
  - If permissions are seeded in a single migration file, append to that file
  - Verify idempotency: `ON CONFLICT (code) DO NOTHING` so the migration is safe to
    re-run

- [ ] 13. Implement `SqlTestExecutionRepository` -- `design.md#Components`
  - All methods from `TestExecutionRepository` interface
  - Every SELECT on `test_executions` includes `WHERE deleted_at IS NULL` for active
    record filtering
  - `find_by_id` executes a query with LEFT JOIN on `test_execution_testers` and
    `users` to resolve tester details:
    ```sql
    SELECT e.*,
           tet.user_id AS tester_user_id,
           u.username AS tester_username
    FROM test_executions e
    LEFT JOIN test_execution_testers tet ON e.id = tet.execution_id
    LEFT JOIN users u ON tet.user_id = u.id
    WHERE e.id = $1 AND e.test_run_id = $2 AND e.deleted_at IS NULL
    ```
    Aggregate tester rows into a `Vec<TesterDto>` in the repository mapping layer.
  - `find_by_name_in_run` uses
    `LOWER(name) = LOWER($1) AND test_run_id = $2 AND deleted_at IS NULL`
  - `find_by_run` builds a dynamic query for search and sort:
    - Base: `WHERE test_run_id = $1 AND deleted_at IS NULL`
    - `search` filter: add `AND name ILIKE $N` if provided. Escape `%`, `_`, and `\`
      in the search value before building the ILIKE pattern. Wrap in `%...%` for
      substring match.
    - `sort`: validate against whitelist (`id`, `name`, `created_at`) and directions
      before building `ORDER BY`. Default: `ORDER BY id DESC`.
    - JOIN `test_execution_testers` and `users` for tester details; aggregate in the
      mapping layer.
    - Execute `COUNT(*) OVER()` for total count in the same query, or a separate
      `COUNT(*)` query with the same WHERE clause
  - `find_all_active_by_run` selects only `id` and `name`, ordered by `LOWER(name)`
    ASC, limited to 500 rows
  - `save` inserts the execution row, then inserts junction rows for each (deduplicated)
    tester ID. After insert, re-fetch with JOINs (same as `find_by_id`) to resolve
    tester details for the response.
  - `update` updates `name` and `updated_by` for the given `id`, but only if
    `deleted_at IS NULL`. If `tester_ids` is provided: delete all existing junction
    rows (`DELETE FROM test_execution_testers WHERE execution_id = $1`), then insert
    new rows. Re-fetch with JOINs for the response.
  - `soft_delete` sets `deleted_at = NOW()`, `deleted_by = $2` WHERE
    `id = $1 AND deleted_at IS NULL`
  - `validate_testers_in_project` executes a single query to check all provided tester
    IDs at once:
    ```sql
    SELECT pm.user_id
    FROM project_members pm
    JOIN users u ON pm.user_id = u.id
    WHERE pm.user_id = ANY($1) AND pm.project_id = $2 AND u.status = 'ACTIVE'
    ```
    Any ID from the input that does not appear in the result set is invalid. The method
    returns details for each invalid ID with a specific reason (not found / inactive /
    not a member).
  - Use parameterized queries exclusively -- never string interpolation
  - Write unit tests with a test database (one test transaction per case):
    - Insert and retrieve an execution with testers (verify resolved usernames)
    - Insert and retrieve an execution with a single tester
    - Duplicate name detection (case-insensitive)
    - Soft-deleted execution excluded from all queries
    - Tester validation (valid, non-existent user, inactive user, non-project-member)
    - Tester deduplication (duplicate IDs produce single junction row)
    - List with search (matches name, no match)
    - List with sort (each valid sort option)
    - List pagination (page 1, page 2, empty page, limit boundaries)
    - Select (ordered by name, only id and name, capped at 500)
    - Update name only (testers preserved)
    - Update testers only (name preserved, junction rows reconciled)
    - Update both name and testers
    - Update with empty tester_ids is rejected at service layer (not repository)
    - Update on soft-deleted row returns 0 rows affected
    - Soft-delete sets both deleted_at and deleted_by
    - Repeated soft-delete returns 0 rows affected
    - Search escaping: `%`, `_`, `\` in search value treated as literals
    - Unicode name round-trip (multi-byte UTF-8)
    - BEFORE UPDATE trigger sets updated_at correctly

- [ ] 14. Wire authorization for test execution permissions --
       `design.md#Components`
  - Register the 6 new `test_execution:*` permission codes in the permission registry
  - Extend the project-scope authorization guard to accept required project roles:
    - `test_execution:create`  -> Contributor, Editor, or Owner (Viewer excluded)
    - `test_execution:read`    -> any project member (all 4 roles)
    - `test_execution:read_list` -> any project member
    - `test_execution:update`  -> Contributor (own only), Editor, or Owner (Viewer
      excluded)
    - `test_execution:delete`  -> Contributor (own only), Editor, or Owner (Viewer
      excluded)
    - `test_execution:select`  -> any project member
  - Implement the Contributor ownership check as a separate guard that is only invoked
    for `test_execution:update` and `test_execution:delete` when the user's project role
    is Contributor. The guard compares the execution's `created_by` with the
    authenticated user's ID.
  - System Admin bypasses all role checks and the ownership check
  - Write unit tests: verify each role is correctly accepted/rejected for each
    permission code; verify Contributor ownership check is enforced for update/delete
    and bypassed for create/read; verify Owner/Editor bypass ownership check

---

## Cross-Cutting Tasks

- [ ] 15. Wire dependency injection for test execution components --
       `design.md#Components`
  - Register `SqlTestExecutionRepository` as the implementation of
    `TestExecutionRepository`
  - Register `TestExecutionService` with its dependencies
    (`TestExecutionRepository`, `AuthorizationService`, `ProjectMemberRepository`,
    `TestCaseImportService`)
  - Register `TestExecutionHandler` with `TestExecutionService`
  - If using a DI container, ensure all lifetimes/scopes are correct (repository scoped
    to request, service transient/singleton)
  - If using manual wiring in `main`, add the wiring code in the correct order

- [ ] 16. Verify rate limit configuration covers test execution endpoints --
       `requirements.md#Security Considerations`
  - If rate limiting is configured at the route group level, ensure execution routes
    are in the correct groups:
    - `GET .../executions` (list) -- 60 req/min group
    - `GET .../executions/{id}` (detail) -- 60 req/min group
    - `GET .../executions/select` -- 120 req/min group
    - `POST .../executions` -- 30 req/min group
    - `PATCH .../executions/{id}` -- 30 req/min group
    - `DELETE .../executions/{id}` -- 30 req/min group
  - If rate limiting is per-endpoint, add the configuration for each of the 6 endpoints
  - Verify `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` headers
    are present on responses
  - Verify `Retry-After` header is present on `429` responses

- [ ] 17. Write API documentation for test execution endpoints --
       `design.md#API Contract`
  - Generate or write OpenAPI 3.x spec for all 6 endpoints
  - Document: path, method, parameters, request body schema, all possible response codes
    and bodies, authentication requirements, permission requirements
  - Include examples for request bodies and success/error responses
  - Document the Contributor ownership restriction on update and delete
  - Document the mandatory Test Run FK and the test case import that occurs on creation

- [ ] 18. Manual QA checklist for test execution CRUD --
       `requirements.md#US-1` through `US-6`
  - [ ] Create an execution with name and multiple testers (verify 201 + Location header)
  - [ ] Verify test case import ran on creation (check that test case results exist for
    the new execution)
  - [ ] Attempt to create with duplicate name in same Test Run (verify 409)
  - [ ] Attempt to create with invalid tester (non-existent user, inactive user,
    non-project-member) (verify 422 with INVALID_TESTER)
  - [ ] Attempt to create with empty `tester_ids` (verify 422)
  - [ ] Attempt to create with whitespace-only name (verify 422)
  - [ ] Create as Contributor (verify success)
  - [ ] Attempt to create as Viewer (verify 403)
  - [ ] List executions with pagination (verify meta)
  - [ ] List with search on name (verify results)
  - [ ] List with sort by name ascending, descending
  - [ ] Verify list items include tester details in response
  - [ ] View detail with resolved tester details (user_id + username)
  - [ ] Update name as Contributor on own execution (verify 200)
  - [ ] Attempt to update as Contributor on another's execution (verify 403)
  - [ ] Update testers as Owner (verify junction rows reconciled)
  - [ ] Update only testers (verify name unchanged; verify old testers removed, new
    added)
  - [ ] Omit `tester_ids` from update (verify testers preserved)
  - [ ] Attempt to update with duplicate name (verify 409)
  - [ ] Attempt to update with invalid tester (verify 422)
  - [ ] Delete as Contributor on own execution (verify 204)
  - [ ] Attempt to delete as Contributor on another's execution (verify 403)
  - [ ] Delete any execution as Owner (verify 204)
  - [ ] Attempt to re-delete soft-deleted execution (verify 404)
  - [ ] Select executions within a Test Run (verify flat list of id+name)
  - [ ] Verify soft-deleted executions excluded from list, detail, select
  - [ ] Verify System Admin can perform all operations
  - [ ] Verify rate limit headers present on all endpoints
  - [ ] Verify strict mode rejects unrecognised fields
  - [ ] Verify XSS sanitization on name
  - [ ] Verify Test Run scope: attempt to access execution via wrong Test Run returns
    404
  - [ ] Verify project scope: attempt to access Test Run via wrong project returns 404
