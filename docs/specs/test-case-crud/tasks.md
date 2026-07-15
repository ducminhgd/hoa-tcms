# Tasks: Test Case CRUD

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `TestCase` entity -- `requirements.md#US-1`, `design.md#Components`
  - Fields: `id`, `project_id`, `category_id`, `priority_id`, `summary`, `automated`,
    `description`, `notes`, `created_by`, `created_at`, `updated_by`, `updated_at`,
    `deleted_by`, `deleted_at`
  - Factory method `TestCase::create(project_id, summary, category_id, priority_id,
    automated, description, notes, created_by)` with domain validation:
    - `summary` trimmed and must contain at least one non-whitespace character, max 500
      chars
    - `automated` must be a boolean (no coercion from other types)
    - `description` <= 10000 chars; empty string `""` stored as-is, `null` becomes `NULL`
    - `notes` <= 5000 chars; empty string `""` stored as-is, `null` becomes `NULL`
    - `category_id` and `priority_id` are validated at the application layer (FK checks
      require DB access), so the domain entity accepts any nullable integer
  - No framework imports; pure language struct + impl
  - Implement `is_soft_deleted()` convenience method
  - Implement a method to produce an update with changed fields:
    `apply_update(cmd: UpdateTestCaseCommand)` -- validates new values and returns a new
    (or modified) entity. Fields: `summary` (Option<String>), `category_id`
    (Option<Option<i64>> -- outer Option for "provided", inner Option for "nullable"),
    `priority_id` (same nested Option pattern), `automated` (Option<bool>), `description`
    (Option<Option<String>>), `notes` (Option<Option<String>>)

- [ ] 2. Define domain exceptions for test cases -- `design.md#Error Handling`
  - `TestCaseNotFoundError`
  - `DuplicateTestCaseSummaryError` (carries the conflicting summary)
  - `TestCasePermissionDenied` (carries the reason: missing system permission, wrong
    project role, or contributor ownership restriction)
  - `TestCaseValidationError` (carries field-level details)
  - `InvalidCategoryError` (carries the invalid `category_id`)
  - `InvalidPriorityError` (carries the invalid `priority_id`)

- [ ] 3. Define value objects and enums -- `design.md#Components`
  - `TestCaseSummary` value object: wraps a `String`, validates non-empty after trim,
    max 500 chars, stores trimmed value. Implements `Display` and equality traits.
  - `AutomationStatus` enum: `Automated` and `Manual`. Converts to/from `bool` for
    storage.

---

## Layer 2 -- Application

- [ ] 4. Define `TestCaseRepository` interface (port) -- `design.md#Components`,
      `design.md#Data Model`
  - Methods:
    - `find_by_id(project_id, test_case_id) -> Option<TestCaseDetail>` -- includes
      resolved `category_name` and `priority_name` via LEFT JOINs
    - `find_by_summary_in_project(project_id, summary) -> Option<TestCase>` -- uses
      `LOWER(summary) = LOWER($1)`
    - `find_by_project(project_id, page, limit, filters, search, sort) ->
      (Vec<TestCaseListItem>, u64 total)` -- `filters` includes optional
      `category_id`, `priority_id`, `automated`
    - `find_all_active_by_project(project_id) -> Vec<TestCaseSelectItem>`
    - `save(test_case) -> TestCaseDetail` -- inserts and returns with generated `id`,
      timestamps, and resolved category/priority names
    - `update(test_case) -> TestCaseDetail` -- updates and returns with resolved names
    - `soft_delete(test_case_id, deleted_by)` -- sets `deleted_at` and `deleted_by`
    - `validate_category_in_project(category_id, project_id) -> bool` -- returns true if
      a non-deleted category with that ID exists in the given project
    - `validate_priority_in_project(priority_id, project_id) -> bool` -- returns true if
      a non-deleted priority with that ID exists in the given project
  - All methods accept a transaction context for transactional composition
  - Return domain entities (not raw DB rows)
  - `TestCaseSelectItem` is a lightweight DTO with only `id` and `summary`
  - `TestCaseDetail` includes all fields plus resolved `category_name` and `priority_name`

- [ ] 5. Define command/query/response DTOs -- `design.md#API Contract`
  - `CreateTestCaseCommand` (summary, category_id?, priority_id?, automated?,
    description?, notes?)
  - `UpdateTestCaseCommand` (summary?, category_id?, priority_id?, automated?,
    description?, notes?) -- all fields are `Option`; for nullable FKs and text fields,
    use a nested Option pattern: `None` = not provided (preserve),
    `Some(None)` = explicitly set to null, `Some(Some(value))` = set to value
  - `ListTestCasesQuery` (project_id, page, limit, category_id?, priority_id?,
    automated?, search?, sort?)
  - `TestCaseListFilters` (category_id?, priority_id?, automated?) -- extracted from
    query for cleaner repository interface
  - `TestCaseListItem` (id, summary, category_id, priority_id, automated, created_by,
    created_at, updated_at) -- used in list endpoints; excludes description, notes,
    updated_by, and resolved names to keep list payload compact
  - `TestCaseDetailResponse` (id, summary, description, notes, automated, project_id,
    category_id, category_name, priority_id, priority_name, created_by, created_at,
    updated_by, updated_at) -- full detail view
  - `TestCaseSelectItem` (id, summary)
  - `PaginationMeta` (total, page, limit) -- reuse from shared utilities if available

- [ ] 6. Implement `TestCaseService` -- `requirements.md#US-1` through `US-6`,
      `design.md#Sequence`
  - `create_test_case(project_id, cmd, current_user_id)`:
    validates `test_case:create` permission, checks Contributor/Editor/Owner role
    (Viewer rejected), begins DB transaction, checks duplicate summary within
    transaction, validates `category_id` FK (if non-null) within transaction, validates
    `priority_id` FK (if non-null) within transaction, constructs entity, saves via
    repository, commits transaction, returns `TestCaseDetailResponse`
  - `list_test_cases(project_id, query, current_user_id)`:
    validates `test_case:read_list` permission, checks project membership (any role),
    delegates to repository with pagination/filters/search/sort, returns paginated
    response
  - `get_test_case(project_id, test_case_id, current_user_id)`:
    validates `test_case:read` permission, checks project membership, fetches by ID,
    verifies belongs to project, returns `TestCaseDetailResponse`
  - `update_test_case(project_id, test_case_id, cmd, current_user_id)`:
    validates `test_case:update` permission, checks the user is at least a Contributor
    (Viewer rejected). If Contributor: fetches test case and verifies
    `created_by == current_user_id`; if not owner -> `403` (ownership restriction).
    Owners and Editors skip ownership check. Begins DB transaction. If summary is
    changing: checks duplicate summary within transaction. If `category_id` is provided:
    validates FK within transaction. If `priority_id` is provided: validates FK within
    transaction. Applies updates, saves via repository, commits transaction, returns
    `TestCaseDetailResponse`
  - `delete_test_case(project_id, test_case_id, current_user_id)`:
    validates `test_case:delete` permission, checks the user is at least a Contributor.
    If Contributor: verifies `created_by == current_user_id` (ownership restriction).
    Owners and Editors skip ownership check. Fetches test case to verify existence and
    project scope. Calls `soft_delete(test_case_id, current_user_id)`. No referential
    integrity check needed (test cases are leaf entities).
  - `select_test_cases(project_id, current_user_id)`:
    validates `test_case:select` permission, checks project membership (any role),
    delegates to `find_all_active_by_project`, returns flat list of
    `TestCaseSelectItem`
  - All methods check system permission via `AuthorizationService`
  - All methods check project membership/role via `ProjectMemberRepository`
  - System Admin bypasses all permission, membership, and ownership checks

- [ ] 7. Write unit tests for `TestCaseService` -- `requirements.md#US-1` through `US-6`
  - Table-driven tests with mock `TestCaseRepository`, mock `ProjectMemberRepository`,
    and mock `AuthorizationService`
  - Happy path: create, list (paginated), get, update, delete, select
  - Duplicate summary rejection (create and update)
  - Invalid category FK rejection (non-existent, soft-deleted, wrong project)
  - Invalid priority FK rejection (non-existent, soft-deleted, wrong project)
  - Null category_id/priority_id acceptance (clears reference on create, preserves on
    update when omitted, clears when explicitly set to null)
  - Permission denial for each permission code
  - Project role denial: Viewer cannot create/update/delete; Viewer can read/select
  - Project role acceptance: Contributor can create; Contributor can update/delete own
  - Contributor ownership restriction: cannot update/delete another user's test case
  - Owner/Editor can update/delete any test case regardless of `created_by`
  - Test case not found returns error
  - Update on soft-deleted test case returns error
  - Soft-delete already-deleted test case returns error
  - No referential integrity check on soft-delete (leaf entity)
  - Search and sort parameters passed through to repository correctly
  - Filter parameters (category_id, priority_id, automated) passed through correctly
  - System Admin bypasses project membership and ownership checks
  - Boundary values: summary at exactly 500 chars (passes), 501 chars (rejects), empty
    string, single char, whitespace-only string (rejects), leading/trailing whitespace
    trimmed
  - Description: exactly 10000 chars (passes), 10001 chars (rejects), "" (stored as empty
    string), null (clears/sets NULL), omitted (preserves current on update)
  - Notes: exactly 5000 chars (passes), 5001 chars (rejects), "" (stored as empty
    string), null (clears/sets NULL), omitted (preserves current on update)
  - Automated: true/false accepted, non-boolean values rejected, omitted defaults to false
    on create
  - Unicode: summary with multi-byte UTF-8 characters (Japanese, emoji) -- LOWER()
    comparison and length checks work correctly
  - Transaction rollback: duplicate summary after concurrent insert returns clean 409
  - Category/priority FK validation failure within transaction: rollback returns clean 422
  - Nested Option semantics for update: `null` in JSON clears FK, omitted field preserves,
    `null` for description/notes clears text, omitted preserves

---

## Layer 3 -- Adapters (HTTP)

- [ ] 8. Implement `TestCaseHandler` -- `design.md#API Contract`, `design.md#Components`
  - Six handler methods: `list`, `create`, `get`, `update`, `delete`, `select`
  - Deserialize request bodies and query params into DTOs
  - Validate pagination: `page` in [1, 1000], `limit` in [1, 100]; return `422` otherwise
  - Validate `sort` against whitelist (`id`, `-id`, `summary`, `-summary`, `created_at`,
    `-created_at`); return `422` for invalid values
  - Validate `search` does not exceed 255 characters; return `422` otherwise
  - Validate `automated` filter is a valid boolean string (`"true"` or `"false"`) if
    provided; return `422` otherwise
  - Validate project exists and is not soft-deleted before any test case operation
    (call `ProjectRepository::find_by_id` or a shared project validation guard)
  - Strip HTML tags from `summary`, `description`, and `notes` fields before passing to
    service (XSS input sanitization at the boundary)
  - For `create` and `update`: implement strict mode -- reject request bodies that contain
    unrecognised fields (compare against the allowed field set for each endpoint)
  - For `update`: handle nested Option deserialization for nullable FKs and text fields
    (distinguish between "field absent", "field set to null", and "field set to value")
  - Call `TestCaseService` methods
  - Serialize responses with proper status codes:
    - `201 Created` with `Location` header for create
    - `200 OK` for list, get, update, select
    - `204 No Content` for delete
  - Map domain/service errors to HTTP status codes and standard error body format:
    - `TestCasePermissionDenied` -> `403 Forbidden` (generic message for system perm /
      project role failures; distinct message for Contributor ownership failure)
    - `TestCaseNotFoundError` -> `404 Not Found`
    - `DuplicateTestCaseSummaryError` -> `409 Conflict`
    - `InvalidCategoryError` -> `422 Unprocessable Entity` with `INVALID_CATEGORY`
    - `InvalidPriorityError` -> `422 Unprocessable Entity` with `INVALID_PRIORITY`
    - `TestCaseValidationError` -> `422 Unprocessable Entity` with field-level details

- [ ] 9. Register test case routes in HTTP router -- `design.md#Route Registration`
  - `GET    /api/v1/projects/{projectId}/test-cases`          -> `list`
  - `POST   /api/v1/projects/{projectId}/test-cases`          -> `create`
  - `GET    /api/v1/projects/{projectId}/test-cases/select`   -> `select`  (static path)
  - `GET    /api/v1/projects/{projectId}/test-cases/{id}`     -> `get`     (dynamic path)
  - `PATCH  /api/v1/projects/{projectId}/test-cases/{id}`     -> `update`
  - `DELETE /api/v1/projects/{projectId}/test-cases/{id}`     -> `delete`
  - All routes require session auth middleware
  - Route order: `/select` **must** be registered before `/{id}` to prevent the
    word "select" from being interpreted as a test case ID

- [ ] 10. Write integration tests for test case HTTP handlers -- `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back)
  - Test `201 Created` with `Location` header
  - Test `200 OK` for list with pagination metadata
  - Test `category_id` filter (query parameter applied correctly; non-existent category
    produces empty result set, not error)
  - Test `priority_id` filter (query parameter applied correctly; non-existent priority
    produces empty result set, not error)
  - Test `automated` filter (`true`/`false`; invalid value returns `422`)
  - Test search filter (query parameter applied correctly to summary and description)
  - Test sort parameter (`id`, `-id`, `summary`, `-summary`, `created_at`, `-created_at`)
  - Test `200 OK` for select (flat list, only id and summary)
  - Test `200 OK` for detail with resolved `category_name` and `priority_name`
  - Test detail with null category_id returns `category_name: null`
  - Test detail with null priority_id returns `priority_name: null`
  - Test detail with soft-deleted category still returns `category_name: null`
  - Test `409 Conflict` on duplicate summary (create and update)
  - Test `422` with `INVALID_CATEGORY` on invalid category_id (non-existent, wrong project)
  - Test `422` with `INVALID_PRIORITY` on invalid priority_id (non-existent, wrong project)
  - Test `422 Unprocessable Entity` on validation errors
  - Test `403 Forbidden` on missing permission (each endpoint)
  - Test `403 Forbidden` on Viewer trying to create/update/delete
  - Test `403 Forbidden` on Contributor trying to update/delete another user's test case
  - Test Contributor can create test cases
  - Test Contributor can update/delete their own test cases
  - Test Owner/Editor can update/delete any test case
  - Test `404 Not Found` on non-existent project
  - Test `404 Not Found` on non-existent/soft-deleted test case
  - Test `404 Not Found` on test case belonging to a different project
  - Test `204 No Content` on successful soft-delete
  - Test `404 Not Found` on repeated soft-delete
  - Test that System Admin can perform all operations on any project regardless of
    membership or ownership
  - Test that `/select` route is not shadowed by `/{id}` (GET with "select" as ID
    hits the select handler, not the get handler)
  - Test edge cases: summary with only whitespace rejected, summary exactly 500 chars
    accepted, summary at 501 chars rejected, description exactly 10000 chars accepted,
    description at 10001 chars rejected, notes exactly 5000 chars accepted, notes at
    5001 chars rejected
  - Test PATCH semantics for nullable FKs:
    - omit `category_id` (preserves current value)
    - send `"category_id": null` (clears to NULL)
    - send `"category_id": <valid_id>` (updates reference)
  - Test PATCH semantics for text fields:
    - omit `description` (preserves current value)
    - send `"description": null` (clears to NULL)
    - send `"description": ""` (stores empty string)
    - same for `notes`
  - Test PATCH semantics for `automated`: omit preserves, send true/false updates
  - Test automated defaults to `false` on create when omitted
  - Test automated rejects non-boolean values on create and update
  - Test `search` with ILIKE wildcards `%` and `_` (treated as literals, not wildcards)
  - Test invalid `sort` values return `422`
  - Test `search` exceeding 255 characters returns `422`
  - Test `page` > 1000 returns `422`
  - Test unrecognised fields in request body return `422` (strict mode)
  - Test Unicode summary (e.g., Japanese text) round-trips correctly
  - Test that `403` response bodies for system permission / project role failures use
    identical generic message (do not distinguish between failure modes)
  - Test that `403` response body for Contributor ownership failure uses a distinct
    message ("Contributors can only update/delete their own test cases")

---

## Layer 4 -- Infrastructure

- [ ] 11. Create `TEST_CASES` database migration -- `design.md#Data Model`
  - Table definition with all columns, PK, FKs, constraints
  - `CHECK` constraint on `description`:
    `description IS NULL OR char_length(description) <= 10000`
  - `CHECK` constraint on `notes`:
    `notes IS NULL OR char_length(notes) <= 5000`
  - `DEFAULT false` on `automated` column
  - Partial unique index `uq_test_cases_summary_project` on `(project_id, LOWER(summary))`
    with `WHERE deleted_at IS NULL`
  - Foreign key indexes on `project_id`, `category_id`, `priority_id`, `created_by`,
    `updated_by`, `deleted_by`
  - Composite partial indexes:
    - `idx_test_cases_project_category` on `(project_id, category_id)
      WHERE deleted_at IS NULL`
    - `idx_test_cases_project_priority` on `(project_id, priority_id)
      WHERE deleted_at IS NULL`
    - `idx_test_cases_active` on `(project_id, id DESC) WHERE deleted_at IS NULL`
  - `BEFORE UPDATE` trigger `trg_test_cases_updated_at` that sets `NEW.updated_at = NOW()`
  - Rollback migration: `DROP TRIGGER IF EXISTS`, `DROP FUNCTION IF EXISTS`,
    `DROP TABLE IF EXISTS`

- [ ] 12. Add test case permission codes to the permissions seed migration --
       `design.md#New Permission Codes`
  - Add 6 new rows to the permissions seed data with
    `INSERT ... ON CONFLICT (code) DO NOTHING`:
    `('test_case:create', 'Create Test Case')`,
    `('test_case:read', 'Read Test Case')`,
    `('test_case:read_list', 'Read Test Case List')`,
    `('test_case:update', 'Update Test Case')`,
    `('test_case:delete', 'Delete Test Case')`,
    `('test_case:select', 'Select Test Case')`
  - Let the database auto-assign IDs (do NOT hardcode numeric IDs -- they will collide
    with other features' permission seeds)
  - If permissions are seeded in a single migration file, append to that file
  - Verify idempotency: `ON CONFLICT (code) DO NOTHING` so the migration is safe to re-run

- [ ] 13. Implement `SqlTestCaseRepository` -- `design.md#Components`
  - All methods from `TestCaseRepository` interface
  - Every SELECT includes `WHERE deleted_at IS NULL` for active record filtering
  - `find_by_id` executes a query with LEFT JOINs on `test_categories` and
    `test_priorities` to resolve `category_name` and `priority_name`. The JOINs are LEFT
    JOINs and include `AND <joined_table>.deleted_at IS NULL` so soft-deleted
    categories/priorities yield null names.
    ```sql
    SELECT tc.*,
           cat.name AS category_name,
           pri.name AS priority_name
    FROM test_cases tc
    LEFT JOIN test_categories cat
      ON tc.category_id = cat.id AND cat.deleted_at IS NULL
    LEFT JOIN test_priorities pri
      ON tc.priority_id = pri.id AND pri.deleted_at IS NULL
    WHERE tc.id = $1 AND tc.project_id = $2 AND tc.deleted_at IS NULL
    ```
  - `find_by_summary_in_project` uses
    `LOWER(summary) = LOWER($1) AND project_id = $2 AND deleted_at IS NULL`
  - `find_by_project` builds a dynamic query for filters, search, and sort:
    - Base: `WHERE project_id = $1 AND deleted_at IS NULL`
    - `category_id` filter: add `AND category_id = $N` if provided
    - `priority_id` filter: add `AND priority_id = $N` if provided
    - `automated` filter: add `AND automated = $N` if provided
    - `search` filter: add `AND (summary ILIKE $N OR description ILIKE $N)` if provided.
      Escape `%`, `_`, and `\` in the search value before building the ILIKE pattern
      (treat as literals). Wrap in `%...%` for substring match.
    - `sort`: validate against a whitelist of allowed columns
      (`id`, `summary`, `created_at`) and directions (`ASC`, `DESC`) before building the
      `ORDER BY` clause -- never interpolate user input directly into SQL. Default:
      `ORDER BY id DESC`
    - Execute `COUNT(*) OVER()` for total count in the same query, or a separate
      `COUNT(*)` query with the same WHERE clause
  - `find_all_active_by_project` selects only `id` and `summary`, ordered by
    `LOWER(summary)` ASC, limited to 1000 rows
  - `save` inserts and returns the new row with generated `id` and timestamps. After
    insert, execute a second query with LEFT JOINs (same as `find_by_id`) to resolve
    `category_name` and `priority_name` for the response.
  - `update` updates `summary`, `category_id`, `priority_id`, `automated`, `description`,
    `notes`, `updated_by` for the given `id`, but only if `deleted_at IS NULL`. After
    update, re-fetch with JOINs (same as `find_by_id`) for the response.
  - `soft_delete` sets `deleted_at = NOW()`, `deleted_by = $2` WHERE
    `id = $1 AND deleted_at IS NULL`
  - `validate_category_in_project` executes
    `SELECT EXISTS(SELECT 1 FROM test_categories WHERE id = $1 AND project_id = $2 AND deleted_at IS NULL)`
  - `validate_priority_in_project` executes
    `SELECT EXISTS(SELECT 1 FROM test_priorities WHERE id = $1 AND project_id = $2 AND deleted_at IS NULL)`
  - Use parameterized queries exclusively -- never string interpolation
  - Write unit tests with a test database (one test transaction per case):
    - Insert and retrieve a test case with category and priority (verify resolved names)
    - Insert and retrieve a test case with null category and null priority
    - Duplicate summary detection (case-insensitive)
    - Soft-deleted test case excluded from all queries
    - Category FK validation (valid, non-existent, soft-deleted, wrong project)
    - Priority FK validation (valid, non-existent, soft-deleted, wrong project)
    - List with filters (category_id, priority_id, automated) -- each individually and
      combined
    - List with search (matches summary, matches description, no match)
    - List with sort (each valid sort option; invalid sort rejected at app layer)
    - List pagination (page 1, page 2, empty page, limit boundaries)
    - Select (ordered by summary, only id and summary, capped at 1000)
    - Update with partial fields (only summary, only description, only notes, only
      category_id, only priority_id, only automated)
    - Update clears category_id to null
    - Update clears priority_id to null
    - Update on soft-deleted row returns 0 rows affected
    - Soft-delete sets both deleted_at and deleted_by
    - Repeated soft-delete returns 0 rows affected
    - Search escaping: `%`, `_`, `\` in search value treated as literals
    - Unicode summary round-trip (multi-byte UTF-8)
    - BEFORE UPDATE trigger sets updated_at correctly

- [ ] 14. Wire authorization for test case permissions -- `design.md#Components`
  - Register the 6 new `test_case:*` permission codes in the permission registry
  - Extend the project-scope authorization guard to accept required project roles:
    - `test_case:create`  -> Contributor, Editor, or Owner (Viewer excluded)
    - `test_case:read`    -> any project member (all 4 roles)
    - `test_case:read_list` -> any project member
    - `test_case:update`  -> Contributor (own only), Editor, or Owner (Viewer excluded)
    - `test_case:delete`  -> Contributor (own only), Editor, or Owner (Viewer excluded)
    - `test_case:select`  -> any project member
  - Implement the Contributor ownership check as a separate guard that is only invoked
    for `test_case:update` and `test_case:delete` when the user's project role is
    Contributor. The guard compares the test case's `created_by` with the authenticated
    user's ID.
  - System Admin bypasses all role checks and the ownership check
  - Write unit tests: verify each role is correctly accepted/rejected for each
    permission code; verify Contributor ownership check is enforced for update/delete
    and bypassed for create/read; verify Owner/Editor bypass ownership check

---

## Cross-Cutting Tasks

- [ ] 15. Add `category_id` FK to TEST_CASES migration (if categories migration already
       exists) -- `design.md#Data Model`
  - If the `TEST_CATEGORIES` table has been created in a prior migration, the
    `TEST_CASES` migration must reference it. Ensure the migration order is correct:
    `TEST_CATEGORIES` and `TEST_PRIORITIES` migrations must run before `TEST_CASES`.
  - Verify `REFERENCES test_categories(id) ON DELETE RESTRICT` on `category_id`
  - Verify `REFERENCES test_priorities(id) ON DELETE RESTRICT` on `priority_id`
  - Verify `REFERENCES projects(id) ON DELETE RESTRICT` on `project_id`

- [ ] 16. Wire dependency injection for test case components -- `design.md#Components`
  - Register `SqlTestCaseRepository` as the implementation of `TestCaseRepository`
  - Register `TestCaseService` with its dependencies (`TestCaseRepository`,
    `AuthorizationService`, `ProjectMemberRepository`)
  - Register `TestCaseHandler` with `TestCaseService`
  - If using a DI container, ensure all lifetimes/scopes are correct (e.g., repository
    scoped to request, service transient/singleton)
  - If using manual wiring in `main`, add the wiring code in the correct order

- [ ] 17. Add `X-Request-ID` correlation logging to test case endpoints --
       `design.md#Route Registration`
  - If a global middleware already handles `X-Request-ID` / `X-Correlation-ID`, no
    additional work is needed. If not, add it to the test case route group.
  - Accept from the client and echo back; generate a UUID v4 if absent

- [ ] 18. Verify rate limit configuration covers test case endpoints --
       `requirements.md#Security Considerations`
  - If rate limiting is configured at the route group level with different limits for
    read vs write endpoints, ensure test case routes are in the correct groups:
    - `GET .../test-cases` (list) -- 60 req/min group
    - `GET .../test-cases/{id}` (detail) -- 60 req/min group
    - `GET .../test-cases/select` -- 120 req/min group
    - `POST .../test-cases` -- 30 req/min group
    - `PATCH .../test-cases/{id}` -- 30 req/min group
    - `DELETE .../test-cases/{id}` -- 30 req/min group
  - If rate limiting is per-endpoint, add the configuration for each of the 6 endpoints
  - Verify `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` headers are
    present on responses
  - Verify `Retry-After` header is present on `429` responses

- [ ] 19. Add `description` and `notes` length checks at the domain entity level --
       `requirements.md#US-1`, `design.md#Components`
  - This is a defence-in-depth complement to the CHECK constraints in the migration (task
    11). The primary enforcement is at the domain layer (user gets a clear validation
    error). The CHECK constraint is a safety net against direct DB manipulation.

- [ ] 20. Write API documentation for test case endpoints -- `design.md#API Contract`
  - Generate or write OpenAPI 3.x spec for all 6 endpoints
  - Document: path, method, parameters, request body schema, all possible response codes
    and bodies, authentication requirements, permission requirements
  - Include examples for request bodies and success/error responses
  - Document the Contributor ownership restriction on update and delete

- [ ] 21. Manual QA checklist for test case CRUD -- `requirements.md#US-1` through `US-6`
  - [ ] Create a test case with all fields populated (verify 201 + Location header)
  - [ ] Create a test case with only summary (verify defaults: automated=false,
    category_id=null, priority_id=null, description=null, notes=null)
  - [ ] Attempt to create with duplicate summary (verify 409)
  - [ ] Attempt to create with invalid category_id (wrong project) (verify 422)
  - [ ] Attempt to create with invalid priority_id (wrong project) (verify 422)
  - [ ] Create as Contributor (verify success)
  - [ ] Attempt to create as Viewer (verify 403)
  - [ ] List test cases with pagination (verify meta)
  - [ ] List with category_id filter (verify only matching)
  - [ ] List with priority_id filter (verify only matching)
  - [ ] List with automated filter (verify only matching)
  - [ ] List with search on summary (verify results)
  - [ ] List with search on description (verify results)
  - [ ] List with sort by summary ascending, descending
  - [ ] List with sort by created_at ascending, descending
  - [ ] View detail with resolved category_name and priority_name
  - [ ] View detail with null category/priority (verify names are null)
  - [ ] Update summary as Contributor on own test case (verify 200)
  - [ ] Attempt to update as Contributor on another's test case (verify 403)
  - [ ] Update any test case as Owner (verify 200)
  - [ ] Update category_id to null (verify cleared)
  - [ ] Update priority_id to valid value (verify changed)
  - [ ] Attempt to update with duplicate summary (verify 409)
  - [ ] Attempt to update with invalid category_id (verify 422)
  - [ ] Attempt to update with invalid priority_id (verify 422)
  - [ ] Delete as Contributor on own test case (verify 204)
  - [ ] Attempt to delete as Contributor on another's test case (verify 403)
  - [ ] Delete any test case as Owner (verify 204)
  - [ ] Attempt to re-delete soft-deleted test case (verify 404)
  - [ ] Select test cases (verify flat list of id+summary)
  - [ ] Verify soft-deleted test cases excluded from list, detail, select
  - [ ] Verify System Admin can perform all operations
  - [ ] Verify rate limit headers present on all endpoints
  - [ ] Verify strict mode rejects unrecognised fields
  - [ ] Verify XSS sanitization on summary, description, notes
