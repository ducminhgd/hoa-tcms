# Tasks: Test Run CRUD

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit of
work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `TestRun` entity -- `requirements.md#US-1`, `design.md#Components`
  - Fields: `id`, `project_id`, `plan_id`, `summary`, `report_to`, `default_tester`,
    `version`, `notes`, `planned_start_date`, `planned_end_date`, `created_by`,
    `created_at`, `updated_by`, `updated_at`, `deleted_by`, `deleted_at`
  - Factory method `TestRun::create(project_id, summary, report_to, plan_id, default_tester,
    version, notes, planned_start_date, planned_end_date, created_by)` with domain
    validation:
    - `summary` trimmed and must contain at least one non-whitespace character, max 500
      chars
    - `version` <= 100 chars (null allowed)
    - `notes` <= 10000 chars; empty string `""` stored as-is, `null` becomes `NULL`
    - `planned_start_date` and `planned_end_date`: if both non-null,
      `planned_end_date >= planned_start_date`
    - `plan_id`, `default_tester` are validated at the application layer (FK checks require
      DB access), so the domain entity accepts any nullable integer
  - No framework imports; pure language struct + impl
  - Implement `is_soft_deleted()` convenience method
  - Implement a method to produce an update with changed fields:
    `apply_update(cmd: UpdateTestRunCommand)` -- validates new values and returns a new
    entity. Fields: `summary` (Option<String>), `plan_id` (Option<Option<i64>>),
    `report_to` (Option<i64>), `default_tester` (Option<Option<i64>>), `version`
    (Option<Option<String>>), `notes` (Option<Option<String>>), `planned_start_date`
    (Option<Option<NaiveDate>>), `planned_end_date` (Option<Option<NaiveDate>>)

- [ ] 2. Define domain exceptions for test runs -- `design.md#Error Handling`
  - `TestRunNotFoundError`
  - `DuplicateTestRunSummaryError` (carries the conflicting summary)
  - `TestRunPermissionDenied` (carries the reason: missing system permission, wrong project
    role, or contributor ownership restriction)
  - `TestRunValidationError` (carries field-level details)
  - `InvalidPlanError` (carries the invalid `plan_id`)
  - `InvalidUserError` (carries the invalid user ID and the field name)
  - `InvalidDateRangeError` (carries start_date and end_date)

- [ ] 3. Define value objects -- `design.md#Components`
  - `TestRunSummary` value object: wraps a `String`, validates non-empty after trim, max
    500 chars, stores trimmed value. Implements `Display` and equality traits.
  - `PlannedDateRange` value object: wraps `Option<NaiveDate>` for start and end, validates
    `end >= start` when both present.

---

## Layer 2 -- Application

- [ ] 4. Define `TestRunRepository` interface (port) -- `design.md#Components`,
      `design.md#Data Model`
  - Methods:
    - `find_by_id(project_id, test_run_id) -> Option<TestRunDetail>` -- includes resolved
      `report_to_username`, `report_to_fullname`, `default_tester_username`,
      `default_tester_fullname` via LEFT JOINs on `users`
    - `find_by_summary_in_project(project_id, summary) -> Option<TestRun>` -- uses
      `LOWER(summary) = LOWER($1)`
    - `find_by_project(project_id, page, limit, filters, search, sort) ->
      (Vec<TestRunListItem>, u64 total)` -- `filters` includes optional `plan_id`
    - `find_all_active_by_project(project_id) -> Vec<TestRunSelectItem>`
    - `save(test_run) -> TestRunDetail` -- inserts and returns with generated `id`,
      timestamps, and resolved user names
    - `update(test_run) -> TestRunDetail` -- updates and returns with resolved user names
    - `soft_delete(test_run_id, deleted_by)` -- sets `deleted_at` and `deleted_by`
    - `validate_plan_in_project(plan_id, project_id) -> bool` -- returns true if a
      non-deleted plan with that ID exists in the given project
    - `validate_user_exists(user_id) -> bool` -- returns true if a non-deleted user with
      that ID exists
  - All methods accept a transaction context for transactional composition
  - Return domain entities (not raw DB rows)
  - `TestRunSelectItem` is a lightweight DTO with only `id` and `summary`
  - `TestRunDetail` includes all fields plus resolved user names
  - `TestRunListItem` includes a compact set of fields plus resolved user names (excludes
    `notes`, `updated_by`)

- [ ] 5. Define command/query/response DTOs -- `design.md#API Contract`
  - `CreateTestRunCommand` (summary, report_to, plan_id?, default_tester?, version?, notes?,
    planned_start_date?, planned_end_date?)
  - `UpdateTestRunCommand` (summary?, plan_id?, report_to?, default_tester?, version?,
    notes?, planned_start_date?, planned_end_date?) -- all fields are `Option`; for nullable
    FKs and text fields, use a nested Option pattern: `None` = not provided (preserve),
    `Some(None)` = explicitly set to null, `Some(Some(value))` = set to value
  - `ListTestRunsQuery` (project_id, page, limit, plan_id?, search?, sort?)
  - `TestRunListFilters` (plan_id?) -- extracted from query for cleaner repository interface
  - `TestRunListItem` (id, summary, version, plan_id, report_to, report_to_username,
    report_to_fullname, default_tester, default_tester_username, default_tester_fullname,
    planned_start_date, planned_end_date, created_by, created_at, updated_at) -- used in
    list endpoints; excludes notes, updated_by
  - `TestRunDetailResponse` (id, summary, version, project_id, plan_id, report_to,
    report_to_username, report_to_fullname, default_tester, default_tester_username,
    default_tester_fullname, notes, planned_start_date, planned_end_date, created_by,
    created_at, updated_by, updated_at) -- full detail view
  - `TestRunSelectItem` (id, summary)
  - `PaginationMeta` (total, page, limit) -- reuse from shared utilities if available

- [ ] 6. Implement `TestRunService` -- `requirements.md#US-1` through `US-6`,
      `design.md#Sequence`
  - `create_test_run(project_id, cmd, current_user_id)`:
    validates `test_run:create` permission, checks Contributor/Editor/Owner role (Viewer
    rejected), begins DB transaction, checks duplicate summary within transaction, validates
    `report_to` user FK within transaction, validates `default_tester` user FK (if non-null)
    within transaction, validates `plan_id` FK (if non-null) within transaction, validates
    date range, constructs entity, saves via repository, commits transaction, returns
    `TestRunDetailResponse`
  - `list_test_runs(project_id, query, current_user_id)`:
    validates `test_run:read_list` permission, checks project membership (any role),
    delegates to repository with pagination/filters/search/sort, returns paginated response
  - `get_test_run(project_id, test_run_id, current_user_id)`:
    validates `test_run:read` permission, checks project membership, fetches by ID, verifies
    belongs to project, returns `TestRunDetailResponse`
  - `update_test_run(project_id, test_run_id, cmd, current_user_id)`:
    validates `test_run:update` permission, checks the user is at least a Contributor
    (Viewer rejected). If Contributor: fetches test run and verifies
    `created_by == current_user_id`; if not owner -> `403` (ownership restriction). Owners
    and Editors skip ownership check. Begins DB transaction. If summary is changing: checks
    duplicate summary within transaction. If `plan_id` is provided: validates FK within
    transaction. If `report_to` is provided: validates user FK within transaction. If
    `default_tester` is provided: validates user FK within transaction. If date fields are
    provided: validates date range against merged values. Applies updates, saves via
    repository, commits transaction, returns `TestRunDetailResponse`
  - `delete_test_run(project_id, test_run_id, current_user_id)`:
    validates `test_run:delete` permission, checks the user is at least a Contributor. If
    Contributor: verifies `created_by == current_user_id` (ownership restriction). Owners
    and Editors skip ownership check. Fetches test run to verify existence and project
    scope. Calls `soft_delete(test_run_id, current_user_id)`. No cascade to children.
  - `select_test_runs(project_id, current_user_id)`:
    validates `test_run:select` permission, checks project membership (any role), delegates
    to `find_all_active_by_project`, returns flat list of `TestRunSelectItem`
  - All methods check system permission via `AuthorizationService`
  - All methods check project membership/role via `ProjectMemberRepository`
  - System Admin bypasses all permission, membership, and ownership checks

- [ ] 7. Write unit tests for `TestRunService` -- `requirements.md#US-1` through `US-6`
  - Table-driven tests with mock `TestRunRepository`, mock `ProjectMemberRepository`, and
    mock `AuthorizationService`
  - Happy path: create, list (paginated), get, update, delete, select
  - Duplicate summary rejection (create and update)
  - Invalid plan FK rejection (non-existent, soft-deleted, wrong project)
  - Invalid report_to user rejection (non-existent, soft-deleted)
  - Invalid default_tester user rejection (non-existent, soft-deleted)
  - Invalid date range rejection (end before start, on both create and update)
  - Null plan_id acceptance (no plan linked)
  - Null default_tester acceptance (no default tester assigned)
  - Permission denial for each permission code
  - Project role denial: Viewer cannot create/update/delete; Viewer can read/select
  - Project role acceptance: Contributor can create; Contributor can update/delete own
  - Contributor ownership restriction: cannot update/delete another user's test run
  - Owner/Editor can update/delete any test run regardless of `created_by`
  - Test run not found returns error
  - Update on soft-deleted test run returns error
  - Soft-delete already-deleted test run returns error
  - No cascade on soft-delete (parent entity, children remain)
  - Search and sort parameters passed through to repository correctly
  - Filter parameter (plan_id) passed through correctly
  - System Admin bypasses project membership and ownership checks
  - Boundary values: summary at exactly 500 chars (passes), 501 chars (rejects), empty
    string, single char, whitespace-only string (rejects), leading/trailing whitespace
    trimmed
  - Version: exactly 100 chars (passes), 101 chars (rejects)
  - Notes: exactly 10000 chars (passes), 10001 chars (rejects), "" (stored as empty string),
    null (clears/sets NULL), omitted (preserves current on update)
  - Date format: valid YYYY-MM-DD accepted, invalid formats rejected
  - Date range edge cases: same date for start and end (passes), end one day after start
    (passes), end one day before start (rejects)
  - Transaction rollback: duplicate summary after concurrent insert returns clean 409
  - Plan FK validation failure within transaction: rollback returns clean 422
  - User FK validation failure within transaction: rollback returns clean 422
  - Nested Option semantics for update: `null` in JSON clears FK, omitted field preserves,
    `null` for notes/version clears text, omitted preserves
  - Partial date update: providing only `planned_start_date` preserves existing
    `planned_end_date`; the combined pair is re-validated
  - Unicode: summary with multi-byte UTF-8 characters (Japanese, emoji) -- LOWER()
    comparison and length checks work correctly

---

## Layer 3 -- Adapters (HTTP)

- [ ] 8. Implement `TestRunHandler` -- `design.md#API Contract`, `design.md#Components`
  - Six handler methods: `list`, `create`, `get`, `update`, `delete`, `select`
  - Deserialize request bodies and query params into DTOs
  - Validate pagination: `page` in [1, 1000], `limit` in [1, 100]; return `422` otherwise
  - Validate `sort` against whitelist (`id`, `-id`, `summary`, `-summary`, `created_at`,
    `-created_at`); return `422` for invalid values
  - Validate `search` does not exceed 255 characters; return `422` otherwise
  - Validate project exists and is not soft-deleted before any test run operation
    (call `ProjectRepository::find_by_id` or a shared project validation guard)
  - Strip HTML tags from `summary`, `version`, and `notes` fields before passing to service
    (XSS input sanitization at the boundary)
  - Parse `planned_start_date` and `planned_end_date` as ISO 8601 dates (`YYYY-MM-DD`);
    return `422` for invalid formats
  - For `create` and `update`: implement strict mode -- reject request bodies that contain
    unrecognised fields (compare against the allowed field set for each endpoint)
  - For `update`: handle nested Option deserialization for nullable FKs and text fields
    (distinguish between "field absent", "field set to null", and "field set to value")
  - Call `TestRunService` methods
  - Serialize responses with proper status codes:
    - `201 Created` with `Location` header for create
    - `200 OK` for list, get, update, select
    - `204 No Content` for delete
  - Map domain/service errors to HTTP status codes and standard error body format:
    - `TestRunPermissionDenied` -> `403 Forbidden` (generic message for system perm /
      project role failures; distinct message for Contributor ownership failure)
    - `TestRunNotFoundError` -> `404 Not Found`
    - `DuplicateTestRunSummaryError` -> `409 Conflict`
    - `InvalidPlanError` -> `422 Unprocessable Entity` with `INVALID_PLAN`
    - `InvalidUserError` -> `422 Unprocessable Entity` with `INVALID_USER`
    - `InvalidDateRangeError` -> `422 Unprocessable Entity` with `INVALID_DATE_RANGE`
    - `TestRunValidationError` -> `422 Unprocessable Entity` with field-level details

- [ ] 9. Register test run routes in HTTP router -- `design.md#Route Registration`
  - `GET    /api/v1/projects/{projectId}/test-runs`          -> `list`
  - `POST   /api/v1/projects/{projectId}/test-runs`          -> `create`
  - `GET    /api/v1/projects/{projectId}/test-runs/select`   -> `select`  (static path)
  - `GET    /api/v1/projects/{projectId}/test-runs/{id}`     -> `get`     (dynamic path)
  - `PATCH  /api/v1/projects/{projectId}/test-runs/{id}`     -> `update`
  - `DELETE /api/v1/projects/{projectId}/test-runs/{id}`     -> `delete`
  - All routes require session auth middleware
  - Route order: `/select` **must** be registered before `/{id}` to prevent the word
    "select" from being interpreted as a test run ID

- [ ] 10. Write integration tests for test run HTTP handlers -- `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back)
  - Test `201 Created` with `Location` header
  - Test `200 OK` for list with pagination metadata
  - Test `plan_id` filter (query parameter applied correctly; non-existent plan produces
    empty result set, not error)
  - Test search filter (query parameter applied correctly to summary)
  - Test sort parameter (`id`, `-id`, `summary`, `-summary`, `created_at`, `-created_at`)
  - Test `200 OK` for select (flat list, only id and summary)
  - Test `200 OK` for detail with resolved `report_to_username`, `report_to_fullname`,
    `default_tester_username`, `default_tester_fullname`
  - Test detail with null default_tester returns null user name fields
  - Test detail with soft-deleted user still returns user names (audit integrity)
  - Test `409 Conflict` on duplicate summary (create and update)
  - Test `422` with `INVALID_PLAN` on invalid plan_id (non-existent, wrong project)
  - Test `422` with `INVALID_USER` on invalid report_to (non-existent, soft-deleted)
  - Test `422` with `INVALID_USER` on invalid default_tester (non-existent, soft-deleted)
  - Test `422` with `INVALID_DATE_RANGE` on end before start
  - Test `422 Unprocessable Entity` on validation errors
  - Test `422` on invalid date format (not YYYY-MM-DD)
  - Test `403 Forbidden` on missing permission (each endpoint)
  - Test `403 Forbidden` on Viewer trying to create/update/delete
  - Test `403 Forbidden` on Contributor trying to update/delete another user's test run
  - Test Contributor can create test runs
  - Test Contributor can update/delete their own test runs
  - Test Owner/Editor can update/delete any test run
  - Test `404 Not Found` on non-existent project
  - Test `404 Not Found` on non-existent/soft-deleted test run
  - Test `404 Not Found` on test run belonging to a different project
  - Test `204 No Content` on successful soft-delete
  - Test `404 Not Found` on repeated soft-delete
  - Test that System Admin can perform all operations on any project regardless of
    membership or ownership
  - Test that `/select` route is not shadowed by `/{id}` (GET with "select" as ID hits the
    select handler, not the get handler)
  - Test edge cases: summary with only whitespace rejected, summary exactly 500 chars
    accepted, summary at 501 chars rejected, version exactly 100 chars accepted, version at
    101 chars rejected, notes exactly 10000 chars accepted, notes at 10001 chars rejected
  - Test date edge cases: same start and end date accepted, end after start accepted, end
    before start rejected
  - Test PATCH semantics for nullable FKs:
    - omit `plan_id` (preserves current value)
    - send `"plan_id": null` (clears to NULL)
    - send `"plan_id": <valid_id>` (updates reference)
    - omit `default_tester` (preserves current value)
    - send `"default_tester": null` (clears to NULL)
    - send `"default_tester": <valid_id>` (updates reference)
  - Test PATCH semantics for text fields:
    - omit `notes` (preserves current value)
    - send `"notes": null` (clears to NULL)
    - send `"notes": ""` (stores empty string)
    - same for `version`
  - Test PATCH semantics for date fields:
    - omit `planned_start_date` (preserves current value)
    - send `"planned_start_date": null` (clears to NULL)
    - send `"planned_start_date": "2026-08-01"` (updates value)
    - provide only `planned_start_date` while `planned_end_date` is already set --
      re-validates the pair; if resulting range is invalid, return `422`
  - Test `search` with ILIKE wildcards `%` and `_` (treated as literals, not wildcards)
  - Test invalid `sort` values return `422`
  - Test `search` exceeding 255 characters returns `422`
  - Test `page` > 1000 returns `422`
  - Test unrecognised fields in request body return `422` (strict mode)
  - Test Unicode summary (e.g., Japanese text) round-trips correctly
  - Test that `403` response bodies for system permission / project role failures use
    identical generic message (do not distinguish between failure modes)
  - Test that `403` response body for Contributor ownership failure uses a distinct message
    ("Contributors can only update/delete their own test runs")

---

## Layer 4 -- Infrastructure

- [ ] 11. Create `TEST_RUNS` database migration -- `design.md#Data Model`
  - Table definition with all columns, PK, FKs, constraints
  - `CHECK` constraint on `notes`:
    `notes IS NULL OR char_length(notes) <= 10000`
  - `CHECK` constraint on `version`:
    `version IS NULL OR char_length(version) <= 100`
  - `CHECK` constraint on date range:
    `planned_start_date IS NULL OR planned_end_date IS NULL OR
     planned_end_date >= planned_start_date`
  - Partial unique index `uq_test_runs_summary_project` on `(project_id, LOWER(summary))`
    with `WHERE deleted_at IS NULL`
  - Foreign key indexes on `project_id`, `plan_id`, `report_to`, `default_tester`,
    `created_by`, `updated_by`, `deleted_by`
  - Composite partial indexes:
    - `idx_test_runs_project_plan` on `(project_id, plan_id) WHERE deleted_at IS NULL`
    - `idx_test_runs_active` on `(project_id, id DESC) WHERE deleted_at IS NULL`
  - `BEFORE UPDATE` trigger `trg_test_runs_updated_at` that sets `NEW.updated_at = NOW()`
  - Rollback migration: `DROP TRIGGER IF EXISTS`, `DROP FUNCTION IF EXISTS`,
    `DROP TABLE IF EXISTS`
  - Verify migration ordering: `test-plan-crud` migration must run before this migration
    (for the `plan_id` FK referencing `test_plans`)

- [ ] 12. Add test run permission codes to the permissions seed migration --
       `design.md#New Permission Codes`
  - Add 6 new rows to the permissions seed data with
    `INSERT ... ON CONFLICT (code) DO NOTHING`:
    `('test_run:create', 'Create Test Run')`,
    `('test_run:read', 'Read Test Run')`,
    `('test_run:read_list', 'Read Test Run List')`,
    `('test_run:update', 'Update Test Run')`,
    `('test_run:delete', 'Delete Test Run')`,
    `('test_run:select', 'Select Test Run')`
  - Let the database auto-assign IDs (do NOT hardcode numeric IDs)
  - Verify idempotency: `ON CONFLICT (code) DO NOTHING`

- [ ] 13. Implement `SqlTestRunRepository` -- `design.md#Components`
  - All methods from `TestRunRepository` interface
  - Every SELECT includes `WHERE deleted_at IS NULL` for active record filtering
  - `find_by_id` executes a query with LEFT JOINs on `users` to resolve `report_to` and
    `default_tester` names:
    ```sql
    SELECT tr.*,
           ru.username AS report_to_username,
           ru.fullname AS report_to_fullname,
           du.username AS default_tester_username,
           du.fullname AS default_tester_fullname
    FROM test_runs tr
    LEFT JOIN users ru ON tr.report_to = ru.id
    LEFT JOIN users du ON tr.default_tester = du.id
    WHERE tr.id = $1 AND tr.project_id = $2 AND tr.deleted_at IS NULL
    ```
  - `find_by_summary_in_project` uses
    `LOWER(summary) = LOWER($1) AND project_id = $2 AND deleted_at IS NULL`
  - `find_by_project` builds a dynamic query for filters, search, and sort:
    - Base: `WHERE project_id = $1 AND deleted_at IS NULL`
    - `plan_id` filter: add `AND plan_id = $N` if provided
    - `search` filter: add `AND (summary ILIKE $N)` if provided. Escape `%`, `_`, and `\`
      in the search value before building the ILIKE pattern (treat as literals). Wrap in
      `%...%` for substring match.
    - `sort`: validate against whitelist of allowed columns (`id`, `summary`, `created_at`)
      and directions (`ASC`, `DESC`) before building the `ORDER BY` clause. Default:
      `ORDER BY id DESC`
    - Execute `COUNT(*) OVER()` for total count in the same query, or a separate `COUNT(*)`
      query with the same WHERE clause
    - LEFT JOIN `users` on `report_to` and `default_tester` to resolve user names
  - `find_all_active_by_project` selects only `id` and `summary`, ordered by
    `LOWER(summary)` ASC, limited to 1000 rows
  - `save` inserts and returns the new row with generated `id` and timestamps. After insert,
    execute a second query with LEFT JOINs (same as `find_by_id`) to resolve user names for
    the response.
  - `update` updates all provided fields for the given `id`, but only if
    `deleted_at IS NULL`. After update, re-fetch with JOINs (same as `find_by_id`) for the
    response.
  - `soft_delete` sets `deleted_at = NOW()`, `deleted_by = $2` WHERE
    `id = $1 AND deleted_at IS NULL`
  - `validate_plan_in_project` executes
    `SELECT EXISTS(SELECT 1 FROM test_plans WHERE id = $1 AND project_id = $2 AND deleted_at IS NULL)`
  - `validate_user_exists` executes
    `SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND deleted_at IS NULL)`
  - Use parameterized queries exclusively -- never string interpolation
  - Write unit tests with a test database (one test transaction per case):
    - Insert and retrieve a test run with plan, report_to, and default_tester (verify
      resolved user names)
    - Insert and retrieve a test run with null plan and null default_tester
    - Duplicate summary detection (case-insensitive)
    - Soft-deleted test run excluded from all queries
    - Plan FK validation (valid, non-existent, soft-deleted, wrong project)
    - User FK validation for report_to (valid, non-existent, soft-deleted)
    - User FK validation for default_tester (valid, non-existent, soft-deleted)
    - List with plan_id filter
    - List with search (matches summary, no match)
    - List with sort (each valid sort option)
    - List pagination (page 1, page 2, empty page, limit boundaries)
    - Select (ordered by summary, only id and summary, capped at 1000)
    - Update with partial fields (only summary, only version, only notes, only plan_id,
      only report_to, only default_tester, only planned_start_date, only planned_end_date)
    - Update clears plan_id to null
    - Update clears default_tester to null
    - Update clears planned_start_date to null (and re-validates remaining date pair)
    - Update on soft-deleted row returns 0 rows affected
    - Soft-delete sets both deleted_at and deleted_by
    - Repeated soft-delete returns 0 rows affected
    - Search escaping: `%`, `_`, `\` in search value treated as literals
    - Unicode summary round-trip (multi-byte UTF-8)
    - BEFORE UPDATE trigger sets updated_at correctly
    - Date CHECK constraint rejects invalid range at DB level (defence in depth)

- [ ] 14. Wire authorization for test run permissions -- `design.md#Components`
  - Register the 6 new `test_run:*` permission codes in the permission registry
  - Extend the project-scope authorization guard to accept required project roles:
    - `test_run:create`  -> Contributor, Editor, or Owner (Viewer excluded)
    - `test_run:read`    -> any project member (all 4 roles)
    - `test_run:read_list` -> any project member
    - `test_run:update`  -> Contributor (own only), Editor, or Owner (Viewer excluded)
    - `test_run:delete`  -> Contributor (own only), Editor, or Owner (Viewer excluded)
    - `test_run:select`  -> any project member
  - Implement the Contributor ownership check as a separate guard that is only invoked for
    `test_run:update` and `test_run:delete` when the user's project role is Contributor.
    The guard compares the test run's `created_by` with the authenticated user's ID.
  - System Admin bypasses all role checks and the ownership check
  - Write unit tests: verify each role is correctly accepted/rejected for each permission
    code; verify Contributor ownership check is enforced for update/delete and bypassed for
    create/read; verify Owner/Editor bypass ownership check

---

## Cross-Cutting Tasks

- [ ] 15. Verify migration ordering for `plan_id` FK -- `design.md#Data Model`
  - The `TEST_RUNS` table references `test_plans(id)` via `plan_id` FK. Ensure the
    `test-plan-crud` migration runs before the `test-run-crud` migration.
  - Verify `REFERENCES test_plans(id) ON DELETE RESTRICT` on `plan_id`
  - Verify `REFERENCES projects(id) ON DELETE RESTRICT` on `project_id`
  - Verify `REFERENCES users(id) ON DELETE RESTRICT` on `report_to`, `default_tester`,
    `created_by`, `updated_by`, `deleted_by`

- [ ] 16. Wire dependency injection for test run components -- `design.md#Components`
  - Register `SqlTestRunRepository` as the implementation of `TestRunRepository`
  - Register `TestRunService` with its dependencies (`TestRunRepository`,
    `AuthorizationService`, `ProjectMemberRepository`)
  - Register `TestRunHandler` with `TestRunService`
  - If using a DI container, ensure all lifetimes/scopes are correct (e.g., repository
    scoped to request, service transient/singleton)

- [ ] 17. Add `X-Request-ID` correlation logging to test run endpoints --
       `design.md#Route Registration`
  - If a global middleware already handles `X-Request-ID` / `X-Correlation-ID`, no
    additional work is needed. If not, add it to the test run route group.
  - Accept from the client and echo back; generate a UUID v4 if absent

- [ ] 18. Verify rate limit configuration covers test run endpoints --
       `requirements.md#Security Considerations`
  - If rate limiting is configured at the route group level with different limits for read
    vs write endpoints, ensure test run routes are in the correct groups:
    - `GET .../test-runs` (list) -- 60 req/min group
    - `GET .../test-runs/{id}` (detail) -- 60 req/min group
    - `GET .../test-runs/select` -- 120 req/min group
    - `POST .../test-runs` -- 30 req/min group
    - `PATCH .../test-runs/{id}` -- 30 req/min group
    - `DELETE .../test-runs/{id}` -- 30 req/min group
  - Verify `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` headers are
    present on responses
  - Verify `Retry-After` header is present on `429` responses

- [ ] 19. Add `notes` and `version` length checks at the domain entity level --
       `requirements.md#US-1`, `design.md#Components`
  - This is a defence-in-depth complement to the CHECK constraints in the migration (task
    11). The primary enforcement is at the domain layer (user gets a clear validation
    error). The CHECK constraint is a safety net against direct DB manipulation.

- [ ] 20. Write API documentation for test run endpoints -- `design.md#API Contract`
  - Generate or write OpenAPI 3.x spec for all 6 endpoints
  - Document: path, method, parameters, request body schema, all possible response codes
    and bodies, authentication requirements, permission requirements
  - Include examples for request bodies and success/error responses
  - Document the Contributor ownership restriction on update and delete

- [ ] 21. Manual QA checklist for test run CRUD -- `requirements.md#US-1` through `US-6`
  - [ ] Create a test run with all fields populated (verify 201 + Location header)
  - [ ] Create a test run with only required fields (summary, report_to) (verify defaults:
    plan_id=null, default_tester=null, version=null, notes=null, dates=null)
  - [ ] Attempt to create with duplicate summary (verify 409)
  - [ ] Attempt to create with invalid plan_id (wrong project) (verify 422 INVALID_PLAN)
  - [ ] Attempt to create with invalid report_to (non-existent user) (verify 422
    INVALID_USER)
  - [ ] Attempt to create with invalid default_tester (soft-deleted user) (verify 422
    INVALID_USER)
  - [ ] Attempt to create with planned_end_date before planned_start_date (verify 422
    INVALID_DATE_RANGE)
  - [ ] Attempt to create with invalid date format (verify 422)
  - [ ] Create as Contributor (verify success)
  - [ ] Attempt to create as Viewer (verify 403)
  - [ ] List test runs with pagination (verify meta)
  - [ ] List with plan_id filter (verify only matching)
  - [ ] List with search on summary (verify results)
  - [ ] List with sort by summary ascending, descending
  - [ ] List with sort by created_at ascending, descending
  - [ ] List response includes resolved user names (report_to, default_tester)
  - [ ] View detail with resolved user names for report_to and default_tester
  - [ ] View detail with null default_tester (verify user name fields are null)
  - [ ] View detail with null plan_id (verify plan_id is null)
  - [ ] Update summary as Contributor on own test run (verify 200)
  - [ ] Attempt to update as Contributor on another's test run (verify 403)
  - [ ] Update any test run as Owner (verify 200)
  - [ ] Update plan_id to null (verify cleared)
  - [ ] Update default_tester to valid user (verify changed)
  - [ ] Update planned_start_date while planned_end_date is set (verify re-validated)
  - [ ] Attempt to update with duplicate summary (verify 409)
  - [ ] Attempt to update with invalid plan_id (verify 422)
  - [ ] Attempt to update with invalid report_to (verify 422)
  - [ ] Attempt to update with invalid date range (verify 422)
  - [ ] Delete as Contributor on own test run (verify 204)
  - [ ] Attempt to delete as Contributor on another's test run (verify 403)
  - [ ] Delete any test run as Owner (verify 204)
  - [ ] Attempt to re-delete soft-deleted test run (verify 404)
  - [ ] Select test runs (verify flat list of id+summary)
  - [ ] Verify soft-deleted test runs excluded from list, detail, select
  - [ ] Verify System Admin can perform all operations
  - [ ] Verify rate limit headers present on all endpoints
  - [ ] Verify strict mode rejects unrecognised fields
  - [ ] Verify XSS sanitization on summary, version, notes
