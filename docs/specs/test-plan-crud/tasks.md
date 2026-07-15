# Tasks: Test Plan CRUD

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit of
work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `TestPlanStatus` enum and transition logic -- `design.md#Status Transitions`
  - Enum variants: `TODO`, `IN_PROGRESS`, `DONE`, `CANCEL`
  - Transition map: `TODO -> {IN_PROGRESS, CANCEL}`, `IN_PROGRESS -> {DONE, CANCEL}`,
    `DONE -> {IN_PROGRESS}`, `CANCEL -> {TODO}`
  - Method `can_transition_to(target: TestPlanStatus) -> bool`: returns `true` if the
    transition is allowed or is a self-transition (no-op)
  - Method `transition_to(target: TestPlanStatus) -> Result<TestPlanStatus, Error>`:
    validates and returns the new status
  - No framework imports; pure language enum/type

- [ ] 2. Implement `TestPlan` entity -- `requirements.md#US-1,US-4`, `design.md#Components`
  - Fields: `id`, `name`, `version`, `plan_type_id`, `description`, `status`,
    `created_by`, `created_at`, `updated_by`, `updated_at`, `deleted_by`, `deleted_at`
  - Factory method `TestPlan::create(name, version, plan_type_id, description, created_by)`
    with domain validation:
    - `name` trimmed and must contain at least one non-whitespace character, max 255 chars
    - `version` trimmed and max 50 chars; defaults to `"1.0"` if not provided or empty
    - `plan_type_id` must be a positive integer (existence validated at app layer)
    - `description` <= 10000 chars; empty string `""` stored as-is, `null` becomes `NULL`
  - Status always initialises to `TestPlanStatus::TODO` on create
  - Method `apply_update(cmd: UpdateTestPlanCommand)` -- validates new values and returns a
    new (or modified) entity. Fields: `name` (Option<String>), `version`
    (Option<Option<String>> -- outer for "provided", inner for "nullable"), `plan_type_id`
    (Option<i64>), `description` (Option<Option<String>>), `status` (Option<TestPlanStatus>)
  - Method `apply_status_transition(new_status: TestPlanStatus)` -- validates the transition
    via `TestPlanStatus::can_transition_to` and returns a new entity with updated status
  - No framework imports; pure language struct + impl
  - Implement `is_soft_deleted()` convenience method

- [ ] 3. Define domain exceptions for test plans -- `design.md#Error Handling`
  - `TestPlanNotFoundError`
  - `DuplicateTestPlanNameError` (carries the conflicting name)
  - `TestPlanPermissionDenied` (carries the reason: missing system permission or wrong
    project role)
  - `TestPlanValidationError` (carries field-level details)
  - `InvalidPlanTypeError` (carries the invalid `plan_type_id`)
  - `InvalidProjectError` (carries the invalid `project_id`)
  - `InvalidStatusTransitionError` (carries current and target statuses)
  - `PlanTypeProjectMismatchError` (carries plan_type_id and project_ids)
  - `EmptyProjectIdsError`

---

## Layer 2 -- Application

- [ ] 4. Define `TestPlanRepository` interface (port) -- `design.md#Components`,
      `design.md#Data Model`
  - Methods:
    - `find_by_id(test_plan_id) -> Option<TestPlanDetail>` -- includes resolved
      `plan_type_name` via JOIN on `test_plan_types` and aggregated `project_ids` from
      `test_plan_projects`
    - `find_by_name(name) -> Option<TestPlan>` -- uses `LOWER(name) = LOWER($1) AND
      deleted_at IS NULL` (global search, not project-scoped)
    - `find_accessible_by_user(user_id, page, limit, filters, search, sort) ->
      (Vec<TestPlanListItem>, u64 total)` -- filters to test plans where the user is a
      member of at least one linked project (via JOIN on `test_plan_projects` and
      `project_members`); `filters` includes optional `status`, `plan_type_id`, `project_id`
    - `find_all_accessible_by_user(user_id) -> Vec<TestPlanSelectItem>` -- returns all
      accessible, non-deleted test plans with `id` and `name` only, ordered by
      `LOWER(name)`, limited to 1000
    - `save(test_plan, project_ids) -> TestPlanDetail` -- inserts into `TEST_PLANS` and
      `TEST_PLAN_PROJECTS` within a transaction, returns with generated `id`, timestamps,
      resolved `plan_type_name`, and aggregated `project_ids`
    - `update(test_plan, project_ids?) -> TestPlanDetail` -- updates `TEST_PLANS` and
      optionally sync-replaces `TEST_PLAN_PROJECTS` (if `project_ids` is `Some`); returns
      full detail with resolved names and project IDs
    - `soft_delete(test_plan_id, deleted_by)` -- sets `deleted_at` and `deleted_by`; does
      NOT touch `TEST_PLAN_PROJECTS`
    - `validate_plan_type_in_projects(plan_type_id, project_ids) -> bool` -- returns true if
      a non-deleted plan type with that ID exists and its `project_id` is in the given set
    - `validate_projects_exist(project_ids) -> Vec<i64>` -- returns the subset of project IDs
      that exist and are non-deleted; used to detect invalid project IDs
  - All mutation methods accept a transaction context for transactional composition
  - Return domain entities (not raw DB rows)
  - `TestPlanSelectItem` is a lightweight DTO with only `id` and `name`
  - `TestPlanDetail` includes all fields plus resolved `plan_type_name` and aggregated
    `project_ids`
  - `TestPlanListItem` is a compact DTO for list endpoints (excludes `description`,
    `updated_by`)

- [ ] 5. Define command/query/response DTOs -- `design.md#API Contract`
  - `CreateTestPlanCommand` (name, project_ids, plan_type_id, version?, description?)
  - `UpdateTestPlanCommand` (name?, version?, plan_type_id?, description?, status?,
    project_ids?) -- all fields are `Option`; for nullable fields (`version`, `description`),
    use a nested Option pattern: `None` = not provided, `Some(None)` = explicitly set to
    null, `Some(Some(value))` = set to value
  - `ListTestPlansQuery` (page, limit, status?, plan_type_id?, project_id?, search?, sort?)
  - `TestPlanListFilters` (status?, plan_type_id?, project_id?) -- extracted from query for
    cleaner repository interface
  - `TestPlanListItem` (id, name, version, plan_type_id, plan_type_name, status, project_ids,
    created_by, created_at, updated_at) -- used in list endpoints; excludes description and
    updated_by to keep list payload compact
  - `TestPlanDetailResponse` (id, name, version, plan_type_id, plan_type_name, status,
    description, project_ids, created_by, created_at, updated_by, updated_at) -- full detail
    view
  - `TestPlanSelectItem` (id, name)
  - `PaginationMeta` (total, page, limit) -- reuse from shared utilities if available

- [ ] 6. Implement `TestPlanService` -- `requirements.md#US-1` through `US-6`,
      `design.md#Sequence`
  - `create_test_plan(cmd, current_user_id)`:
    validates `test_plan:create` permission; deduplicates `project_ids`; checks user is
    Owner/Editor in at least one specified project (or System Admin); validates all project
    IDs exist and are non-deleted via repository; checks duplicate name; validates
    `plan_type_id` belongs to a project in `project_ids`; begins DB transaction; constructs
    entity with `status = TODO`; saves via repository (including project associations);
    commits transaction; returns `TestPlanDetailResponse`
  - `list_test_plans(query, current_user_id)`:
    validates `test_plan:read_list` permission; delegates to
    `find_accessible_by_user(user_id, ...)` which inherently filters to test plans where the
    user is a member (or returns all for System Admin); returns paginated response
  - `get_test_plan(test_plan_id, current_user_id)`:
    validates `test_plan:read` permission; fetches by ID; verifies user is a member of at
    least one linked project (or System Admin); if not a member of any linked project ->
    `404 Not Found`; returns `TestPlanDetailResponse`
  - `update_test_plan(test_plan_id, cmd, current_user_id)`:
    validates `test_plan:update` permission; fetches test plan to verify existence and
    non-deleted; checks user is Owner/Editor in at least one of the CURRENT linked projects;
    if `project_ids` is being changed: additionally checks user is Owner/Editor in at least
    one project in the NEW set; begins DB transaction; if name changing: checks duplicate;
    if `plan_type_id` provided: validates against current or new project set; if `status`
    provided: validates transition via `TestPlanStatus::can_transition_to`; if `project_ids`
    provided: validates all IDs exist, then sync-replaces associations; after sync, verifies
    plan type's project is still in the set; applies remaining updates; saves via
    repository; commits transaction; returns `TestPlanDetailResponse`
  - `delete_test_plan(test_plan_id, current_user_id)`:
    validates `test_plan:delete` permission; fetches test plan to verify existence and
    non-deleted; checks user is Owner/Editor in at least one linked project; calls
    `soft_delete(test_plan_id, current_user_id)`; project associations are preserved
  - `select_test_plans(current_user_id)`:
    validates `test_plan:select` permission; delegates to `find_all_accessible_by_user`;
    returns flat list of `TestPlanSelectItem`
  - All methods check system permission via `AuthorizationService`
  - All methods check project membership via `ProjectMemberRepository`
  - System Admin bypasses all permission and membership checks; for list/select, System Admin
    sees all non-deleted test plans

- [ ] 7. Write unit tests for `TestPlanService` -- `requirements.md#US-1` through `US-6`
  - Table-driven tests with mock `TestPlanRepository`, mock `ProjectMemberRepository`, and
    mock `AuthorizationService`
  - Happy path: create, list (paginated), get, update, delete, select
  - Create: status defaults to TODO; version defaults to "1.0"; project_ids deduplicated
    silently
  - Duplicate name rejection (create and update)
  - Invalid plan type FK rejection (non-existent, soft-deleted, plan type from non-linked
    project)
  - Invalid project ID rejection (non-existent project in project_ids)
  - Empty project_ids rejection (create and update)
  - Permission denial for each permission code
  - Project role denial: Viewer/Contributor cannot create/update/delete; any member can read
  - User not Owner/Editor in any specified project on create -> `403`
  - User not Owner/Editor in any linked project on update/delete -> `403`
  - User not a member of any linked project on get -> `404`
  - Update with new project_ids: user must be Owner/Editor in both current and new sets
  - Status transitions: TODO -> IN_PROGRESS (passes), TODO -> CANCEL (passes),
    IN_PROGRESS -> DONE (passes), IN_PROGRESS -> CANCEL (passes),
    DONE -> IN_PROGRESS (passes), CANCEL -> TODO (passes)
  - Invalid status transitions: TODO -> DONE (rejects), IN_PROGRESS -> TODO (rejects),
    DONE -> TODO (rejects), DONE -> CANCEL (rejects), CANCEL -> DONE (rejects),
    CANCEL -> IN_PROGRESS (rejects)
  - Self-transition: TODO -> TODO (silently accepted, no-op)
  - Update project_ids and plan_type_id together: plan type validated against new set
  - Plan type project mismatch after project_ids update
  - Search and sort parameters passed through to repository correctly
  - Filter parameters (status, plan_type_id, project_id) passed through correctly
  - System Admin bypasses project membership and role checks
  - System Admin sees all test plans in list/select regardless of membership
  - Boundary values: name at exactly 255 chars (passes), 256 chars (rejects), empty string,
    single char, whitespace-only string (rejects), leading/trailing whitespace trimmed
  - Version: 50 chars (passes), 51 chars (rejects), "" (stored as empty string),
    null (clears to default "1.0" on create, sets NULL on update), omitted (defaults to
    "1.0" on create, preserves on update)
  - Description: exactly 10000 chars (passes), 10001 chars (rejects), "" (stored as empty
    string), null (clears/sets NULL), omitted (preserves current on update, defaults to null
    on create)
  - Transaction rollback: duplicate name after concurrent insert returns clean 409
  - Project validation failure within transaction: rollback returns clean 422
  - Nested Option semantics for update: `null` in JSON for version/description clears field,
    omitted field preserves current value

---

## Layer 3 -- Adapters (HTTP)

- [ ] 8. Implement `TestPlanHandler` -- `design.md#API Contract`, `design.md#Components`
  - Six handler methods: `list`, `create`, `get`, `update`, `delete`, `select`
  - Deserialize request bodies and query params into DTOs
  - Validate pagination: `page` in [1, 1000], `limit` in [1, 100]; return `422` otherwise
  - Validate `sort` against whitelist (`-updated_at`, `updated_at`, `name`, `-name`,
    `status`, `-status`); return `422` for invalid values
  - Validate `status` filter against whitelist (`TODO`, `IN_PROGRESS`, `DONE`, `CANCEL`);
    return `422` for invalid values
  - Validate `search` does not exceed 255 characters; return `422` otherwise
  - Validate `project_ids` on create: must be present, non-empty array; return `422` with
    `EMPTY_PROJECT_IDS` if missing or empty
  - Validate `project_ids` elements are positive integers
  - Strip HTML tags from `name`, `version`, and `description` fields before passing to
    service (XSS input sanitization at the boundary)
  - For `create` and `update`: implement strict mode -- reject request bodies that contain
    unrecognised fields (compare against the allowed field set for each endpoint)
  - For `create`: reject `status` field if present (status is always TODO on create); reject
    with `422` and a message indicating status is not accepted on create
  - For `update`: handle nested Option deserialization for nullable text fields (`version`,
    `description`) -- distinguish between "field absent", "field set to null", and "field set
    to value"
  - Call `TestPlanService` methods
  - Serialize responses with proper status codes:
    - `201 Created` with `Location` header for create
    - `200 OK` for list, get, update, select
    - `204 No Content` for delete
  - Map domain/service errors to HTTP status codes and standard error body format:
    - `TestPlanPermissionDenied` -> `403 Forbidden` (generic message)
    - `TestPlanNotFoundError` -> `404 Not Found`
    - `DuplicateTestPlanNameError` -> `409 Conflict` with `DUPLICATE_TEST_PLAN_NAME`
    - `InvalidPlanTypeError` -> `422 Unprocessable Entity` with `INVALID_PLAN_TYPE`
    - `InvalidProjectError` -> `422 Unprocessable Entity` with `INVALID_PROJECT`
    - `EmptyProjectIdsError` -> `422 Unprocessable Entity` with `EMPTY_PROJECT_IDS`
    - `InvalidStatusTransitionError` -> `422 Unprocessable Entity` with
      `INVALID_STATUS_TRANSITION`
    - `PlanTypeProjectMismatchError` -> `422 Unprocessable Entity` with
      `PLAN_TYPE_PROJECT_MISMATCH`
    - `TestPlanValidationError` -> `422 Unprocessable Entity` with field-level details

- [ ] 9. Register test plan routes in HTTP router -- `design.md#Route Registration`
  - `GET    /api/v1/test-plans`          -> `list`
  - `POST   /api/v1/test-plans`          -> `create`
  - `GET    /api/v1/test-plans/select`   -> `select`  (static path)
  - `GET    /api/v1/test-plans/{id}`     -> `get`     (dynamic path)
  - `PATCH  /api/v1/test-plans/{id}`     -> `update`
  - `DELETE /api/v1/test-plans/{id}`     -> `delete`
  - All routes require session auth middleware
  - Route order: `/select` **must** be registered before `/{id}` to prevent the word
    "select" from being interpreted as a test plan ID
  - Note: routes are under `/api/v1/test-plans/` (not under a project path -- test plans are
    multi-project)

- [ ] 10. Write integration tests for test plan HTTP handlers -- `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back)
  - Test `201 Created` with `Location` header; verify status is `TODO` regardless of input
  - Test `200 OK` for list with pagination metadata
  - Test `status` filter (query parameter applied correctly; invalid value returns `422`)
  - Test `plan_type_id` filter
  - Test `project_id` filter (user sees only plans linked to that project where they are a
    member)
  - Test `search` filter (query parameter applied correctly to name and description)
  - Test sort parameter (all 6 allowed values)
  - Test `200 OK` for select (flat list, only id and name, ordered by name ascending)
  - Test `200 OK` for detail with resolved `plan_type_name` and `project_ids`
  - Test `409 Conflict` on duplicate name (create and update)
  - Test `422` with `EMPTY_PROJECT_IDS` on missing/empty project_ids
  - Test `422` with `INVALID_PROJECT` on non-existent project ID
  - Test `422` with `INVALID_PLAN_TYPE` on invalid plan_type_id (non-existent, wrong project)
  - Test `422` with `INVALID_STATUS_TRANSITION` on invalid status transition
  - Test `422` with `PLAN_TYPE_PROJECT_MISMATCH` when plan type's project is removed
  - Test `422 Unprocessable Entity` on validation errors (name too long, version too long,
    description too long)
  - Test `422 Unprocessable Entity` when `status` is included in create request
  - Test `403 Forbidden` on missing permission (each endpoint)
  - Test `403 Forbidden` on Viewer trying to create/update/delete
  - Test `403 Forbidden` on Contributor trying to create/update/delete
  - Test `403 Forbidden` on Owner/Editor not in any linked project trying to update/delete
  - Test `404 Not Found` on non-existent test plan
  - Test `404 Not Found` on soft-deleted test plan
  - Test `404 Not Found` on user not a member of any linked project (detail endpoint)
  - Test `204 No Content` on successful soft-delete
  - Test `404 Not Found` on repeated soft-delete
  - Test that project associations are preserved after soft-delete
  - Test that System Admin can perform all operations on any test plan
  - Test that System Admin sees all non-deleted test plans in list/select
  - Test that `/select` route is not shadowed by `/{id}` (GET with "select" as ID hits the
    select handler, not the get handler)
  - Test edge cases: name with only whitespace rejected, name exactly 255 chars accepted,
    name at 256 chars rejected, description exactly 10000 chars accepted, description at
    10001 chars rejected, version exactly 50 chars accepted, version at 51 chars rejected
  - Test PATCH semantics for nullable fields:
    - omit `version` (preserves current value)
    - send `"version": null` (clears to NULL)
    - send `"version": ""` (stores empty string)
    - omit `description` (preserves current value)
    - send `"description": null` (clears to NULL)
    - send `"description": ""` (stores empty string)
  - Test PATCH semantics for `project_ids`:
    - omit (preserves current associations)
    - send new set (sync-replaces associations)
    - send empty array (returns `422`)
  - Test PATCH semantics for `status`:
    - omit (preserves current)
    - valid transition (applies)
    - invalid transition (returns `422`)
    - self-transition (no-op, returns 200)
  - Test `search` with ILIKE wildcards `%` and `_` (treated as literals, not wildcards)
  - Test invalid `sort` values return `422`
  - Test `search` exceeding 255 characters returns `422`
  - Test `page` > 1000 returns `422`
  - Test unrecognised fields in request body return `422` (strict mode)
  - Test duplicate project_ids in create (deduplication)
  - Test that `403` response bodies use generic message (do not distinguish failure modes)
  - Test Unicode name (e.g., Japanese text) round-trips correctly
  - Test all status transitions end-to-end: TODO->IN_PROGRESS->DONE->IN_PROGRESS->CANCEL->TODO

---

## Layer 4 -- Infrastructure

- [ ] 11. Create `TEST_PLANS` and `TEST_PLAN_PROJECTS` database migration --
       `design.md#Data Model`
  - `TEST_PLANS` table definition with all columns, PK, FKs, constraints
  - `CHECK` constraint on `description`:
    `description IS NULL OR char_length(description) <= 10000`
  - `CHECK` constraint on `status`:
    `status IN ('TODO', 'IN_PROGRESS', 'DONE', 'CANCEL')`
  - `DEFAULT 'TODO'` on `status` column
  - `DEFAULT '1.0'` on `version` column
  - Partial unique index `uq_test_plans_name` on `(LOWER(name))` with
    `WHERE deleted_at IS NULL`
  - Foreign key indexes on `plan_type_id`, `created_by`, `updated_by`, `deleted_by`
  - Partial index `idx_test_plans_status` on `(status) WHERE deleted_at IS NULL`
  - Composite partial index `idx_test_plans_active_updated` on `(updated_at DESC) WHERE
    deleted_at IS NULL` for the default list sort
  - `TEST_PLAN_PROJECTS` junction table definition with:
    - `plan_id BIGINT NOT NULL REFERENCES test_plans(id) ON DELETE RESTRICT`
    - `project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT`
    - `PRIMARY KEY (plan_id, project_id)`
  - Index `idx_test_plan_projects_project_id` on `(project_id)`
  - `BEFORE UPDATE` trigger `trg_test_plans_updated_at` that sets `NEW.updated_at = NOW()`
  - Rollback migration: `DROP TRIGGER IF EXISTS`, `DROP FUNCTION IF EXISTS`,
    `DROP TABLE IF EXISTS test_plan_projects`, `DROP TABLE IF EXISTS test_plans`

- [ ] 12. Add test plan permission codes to the permissions seed migration --
       `design.md#New Permission Codes`
  - Add 6 new rows to the permissions seed data with
    `INSERT ... ON CONFLICT (code) DO NOTHING`:
    `('test_plan:create', 'Create Test Plan')`,
    `('test_plan:read', 'Read Test Plan')`,
    `('test_plan:read_list', 'Read Test Plan List')`,
    `('test_plan:update', 'Update Test Plan')`,
    `('test_plan:delete', 'Delete Test Plan')`,
    `('test_plan:select', 'Select Test Plan')`
  - Let the database auto-assign IDs (do NOT hardcode numeric IDs -- they will collide with
    other features' permission seeds)
  - Verify idempotency: `ON CONFLICT (code) DO NOTHING` so the migration is safe to re-run

- [ ] 13. Implement `SqlTestPlanRepository` -- `design.md#Components`
  - All methods from `TestPlanRepository` interface
  - Every SELECT on `TEST_PLANS` includes `WHERE tp.deleted_at IS NULL` for active record
    filtering
  - `find_by_id` executes a query with LEFT JOIN on `test_plan_types` for `plan_type_name`
    and array aggregation of `project_ids` from `test_plan_projects`:
    ```sql
    SELECT tp.*,
           tpt.name AS plan_type_name,
           COALESCE(array_agg(tpp.project_id) FILTER (WHERE tpp.project_id IS NOT NULL),
                    '{}') AS project_ids
    FROM test_plans tp
    LEFT JOIN test_plan_types tpt ON tp.plan_type_id = tpt.id
    LEFT JOIN test_plan_projects tpp ON tp.id = tpp.plan_id
    WHERE tp.id = $1 AND tp.deleted_at IS NULL
    GROUP BY tp.id, tpt.name
    ```
  - `find_by_name` uses `LOWER(name) = LOWER($1) AND deleted_at IS NULL`
  - `find_accessible_by_user` builds a dynamic query with:
    - Subquery filter: `WHERE EXISTS (SELECT 1 FROM test_plan_projects tpp JOIN
      project_members pm ON tpp.project_id = pm.project_id WHERE tpp.plan_id = tp.id AND
      pm.user_id = $1)` (skipped for System Admin)
    - `deleted_at IS NULL`
    - Optional filters: `status`, `plan_type_id`, `project_id` (additional membership check
      for project_id filter)
    - `search` filter: `AND (tp.name ILIKE $N OR tp.description ILIKE $N)`. Escape `%`,
      `_`, and `\` in the search value before building the ILIKE pattern
    - `sort`: validate against whitelist before building `ORDER BY` clause. Default:
      `ORDER BY tp.updated_at DESC`
    - JOIN on `test_plan_types` for `plan_type_name`
    - Array aggregation for `project_ids`
    - Execute `COUNT(*) OVER()` or a separate `COUNT(*)` query for total count
  - `find_all_accessible_by_user` selects only `tp.id` and `tp.name`, with the same
    membership subquery filter, ordered by `LOWER(tp.name)`, limited to 1000
  - `save` begins a transaction:
    - INSERT into `TEST_PLANS`
    - INSERT each project_id into `TEST_PLAN_PROJECTS`
    - Re-fetch with JOINs to build `TestPlanDetail` response
  - `update` begins a transaction if `project_ids` is provided:
    - UPDATE the `TEST_PLANS` row (only if `deleted_at IS NULL`)
    - If `project_ids` is `Some`: DELETE all existing rows from `TEST_PLAN_PROJECTS` for
      this plan, then INSERT new rows
    - Re-fetch with JOINs for the response
  - `soft_delete` sets `deleted_at = NOW()`, `deleted_by = $2` WHERE
    `id = $1 AND deleted_at IS NULL`
  - `validate_plan_type_in_projects` executes:
    `SELECT EXISTS(SELECT 1 FROM test_plan_types WHERE id = $1 AND project_id = ANY($2))`
  - `validate_projects_exist` executes:
    `SELECT id FROM projects WHERE id = ANY($1) AND deleted_at IS NULL`
  - Use parameterized queries exclusively -- never string interpolation
  - Write unit tests with a test database (one test transaction per case):
    - Insert and retrieve a test plan with plan type and project associations
    - Duplicate name detection (case-insensitive)
    - Soft-deleted test plan excluded from all queries
    - Plan type FK validation (valid, non-existent, wrong project)
    - Project existence validation (valid, non-existent, soft-deleted)
    - List with filters (status, plan_type_id, project_id) -- each individually and combined
    - List with search (matches name, matches description, no match)
    - List with sort (each valid sort option)
    - List pagination (page 1, page 2, empty page, limit boundaries)
    - Select (ordered by name, only id and name, capped at 1000)
    - Select filters to user-accessible plans (different user sees different results)
    - Update with partial fields (only name, only version, only description, only plan_type_id,
      only status, only project_ids)
    - Update sync-replaces project associations correctly
    - Update on soft-deleted row returns 0 rows affected
    - Soft-delete sets both deleted_at and deleted_by
    - Soft-delete preserves TEST_PLAN_PROJECTS rows
    - Repeated soft-delete returns 0 rows affected
    - Search escaping: `%`, `_`, `\` in search value treated as literals
    - Unicode name round-trip (multi-byte UTF-8)
    - BEFORE UPDATE trigger sets updated_at correctly
    - Plan type name resolved via JOIN even for non-deleted plan types
    - Project IDs aggregated correctly (empty array, single ID, multiple IDs)

- [ ] 14. Wire authorization for test plan permissions -- `design.md#Components`
  - Register the 6 new `test_plan:*` permission codes in the permission registry
  - Implement multi-project authorization checks:
    - `test_plan:create` -> user must be Owner or Editor in at least one of the projects
      specified in `project_ids`; System Admin bypasses
    - `test_plan:read` -> user must be a member of at least one linked project; System Admin
      bypasses
    - `test_plan:read_list` -> SQL query filters to test plans where user is a member of at
      least one linked project; System Admin sees all
    - `test_plan:update` -> user must be Owner or Editor in at least one linked project;
      System Admin bypasses
    - `test_plan:delete` -> user must be Owner or Editor in at least one linked project;
      System Admin bypasses
    - `test_plan:select` -> SQL query filters to test plans where user is a member of at
      least one linked project; System Admin sees all
  - Add `find_user_roles_in_projects(user_id, project_ids) -> Map<project_id, role>` to
    `ProjectMemberRepository` to efficiently check membership across multiple projects
  - System Admin bypasses all role checks
  - Write unit tests: verify each role is correctly accepted/rejected for each permission
    code; verify multi-project scenarios (user is Owner in project A but not in project B);

---

## Cross-Cutting Tasks

- [ ] 15. Ensure migration order -- `design.md#Data Model`
  - `TEST_PLAN_TYPES` must be created before `TEST_PLANS` (FK dependency)
  - `PROJECTS` must be created before `TEST_PLAN_PROJECTS` (FK dependency)
  - `USERS` must be created before `TEST_PLANS` (FK dependency for `created_by`,
    `updated_by`, `deleted_by`)
  - `PROJECT_MEMBERS` must be created before list/select queries can filter by membership
  - Verify FK constraints: `ON DELETE RESTRICT` on all FKs

- [ ] 16. Wire dependency injection for test plan components -- `design.md#Components`
  - Register `SqlTestPlanRepository` as the implementation of `TestPlanRepository`
  - Register `TestPlanService` with its dependencies (`TestPlanRepository`,
    `AuthorizationService`, `ProjectMemberRepository`)
  - Register `TestPlanHandler` with `TestPlanService`
  - Ensure repository scope is appropriate for transactional composition (request-scoped or
    method-injected transaction context)

- [ ] 17. Add `X-Request-ID` correlation logging to test plan endpoints
  - If a global middleware already handles `X-Request-ID` / `X-Correlation-ID`, no additional
    work is needed. If not, add it to the test plan route group.
  - Accept from the client and echo back; generate a UUID v4 if absent

- [ ] 18. Verify rate limit configuration covers test plan endpoints --
       `requirements.md#Security Considerations`
  - If rate limiting is configured at the route group level:
    - `GET .../test-plans` (list) -- 60 req/min group
    - `GET .../test-plans/{id}` (detail) -- 60 req/min group
    - `GET .../test-plans/select` -- 120 req/min group
    - `POST .../test-plans` -- 30 req/min group
    - `PATCH .../test-plans/{id}` -- 30 req/min group
    - `DELETE .../test-plans/{id}` -- 30 req/min group
  - If rate limiting is per-endpoint, add the configuration for each of the 6 endpoints
  - Verify `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` headers are
    present
  - Verify `Retry-After` header is present on `429` responses

- [ ] 19. Add field length checks at the domain entity level (defence-in-depth) --
       `requirements.md#US-1,US-4`, `design.md#Components`
  - `name`: max 255 chars, must contain at least one non-whitespace character
  - `version`: max 50 chars
  - `description`: max 10000 chars
  - These complement the `CHECK` constraints in the migration (task 11). Primary enforcement
    is at the domain layer (clear validation error). The `CHECK` constraint is a safety net
    against direct DB manipulation.

- [ ] 20. Write API documentation for test plan endpoints -- `design.md#API Contract`
  - Generate or write OpenAPI 3.x spec for all 6 endpoints
  - Document: path, method, parameters, request body schema, all possible response codes and
    bodies, authentication requirements, permission requirements
  - Include examples for request bodies and success/error responses
  - Document the status transition state machine (diagram or table)
  - Document the multi-project authorization model

- [ ] 21. Manual QA checklist for test plan CRUD -- `requirements.md#US-1` through `US-6`
  - [ ] Create a test plan with all fields populated (verify 201 + Location header; verify
    status is TODO)
  - [ ] Create a test plan with only required fields (name, project_ids, plan_type_id);
    verify version defaults to "1.0" and description is null
  - [ ] Create a test plan with multiple project_ids (verify multi-project associations)
  - [ ] Create as Owner of one project and member of another (verify success)
  - [ ] Attempt to create as Contributor (verify 403)
  - [ ] Attempt to create as Viewer (verify 403)
  - [ ] Attempt to create with duplicate name (verify 409)
  - [ ] Attempt to create with invalid plan_type_id (wrong project) (verify 422)
  - [ ] Attempt to create with empty project_ids (verify 422)
  - [ ] Attempt to create with non-existent project ID (verify 422)
  - [ ] Attempt to create with status field (verify 422 -- status not accepted on create)
  - [ ] List test plans with pagination (verify meta; verify only accessible plans returned)
  - [ ] List with status filter (verify only matching)
  - [ ] List with plan_type_id filter (verify only matching)
  - [ ] List with project_id filter (verify only plans linked to that project)
  - [ ] List with search on name (verify results)
  - [ ] List with search on description (verify results)
  - [ ] List with sort by name ascending, descending
  - [ ] List with sort by status ascending, descending
  - [ ] List with sort by updated_at ascending, descending
  - [ ] View detail with resolved plan_type_name and project_ids
  - [ ] View detail as non-member of any linked project (verify 404)
  - [ ] View detail as System Admin on any test plan (verify 200)
  - [ ] Update name as Owner in linked project (verify 200)
  - [ ] Update version to new value, null, and empty string
  - [ ] Update description to new value, null, and empty string
  - [ ] Update plan_type_id to another valid plan type in a linked project
  - [ ] Update status: TODO -> IN_PROGRESS -> DONE -> IN_PROGRESS -> CANCEL -> TODO
    (verify each valid transition)
  - [ ] Attempt invalid status transition: DONE -> CANCEL (verify 422)
  - [ ] Attempt invalid status transition: CANCEL -> DONE (verify 422)
  - [ ] Attempt to update as non-member of any linked project (verify 404)
  - [ ] Attempt to update as Contributor (verify 403)
  - [ ] Update project_ids to add a new project (verify sync-replace; user must be
    Owner/Editor in at least one new project)
  - [ ] Update project_ids and plan_type_id together: verify plan type validated against
    new project set
  - [ ] Update project_ids removing the plan type's project (verify 422
    PLAN_TYPE_PROJECT_MISMATCH)
  - [ ] Attempt to update with empty project_ids (verify 422)
  - [ ] Update project_ids where user is not Owner/Editor in any new project (verify 403)
  - [ ] Delete as Owner in linked project (verify 204)
  - [ ] Attempt to delete as Viewer (verify 403)
  - [ ] Attempt to re-delete soft-deleted test plan (verify 404)
  - [ ] Select test plans (verify flat list of id+name, only accessible plans)
  - [ ] Verify soft-deleted test plans excluded from list, detail, select
  - [ ] Verify project associations preserved after soft-delete (re-fetch via direct DB query)
  - [ ] Verify System Admin can perform all operations on any test plan
  - [ ] Verify System Admin sees all non-deleted test plans in list/select
  - [ ] Verify rate limit headers present on all endpoints
  - [ ] Verify strict mode rejects unrecognised fields
  - [ ] Verify XSS sanitization on name, version, description
