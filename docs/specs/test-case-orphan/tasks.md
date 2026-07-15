# Tasks: Orphan Test Case

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Update `TestCase` entity to support nullable `project_id` -- `requirements.md#US-1`, `design.md#Components`
  - Allow `project_id` to be `None`/`null` in the entity.
  - Factory method `create()` skips `category_id` and `priority_id` validation when
    `project_id` is `None` (orphan test cases cannot have project-scoped FKs).
  - `apply_update(cmd)` rejects `category_id` and `priority_id` changes when
    `project_id` is `None` and `project_id` is not being set in the same update
    (i.e., regular orphan updates cannot set these fields).
  - `apply_update(cmd)` accepts `project_id` as a settable field (for the
    assign-to-project operation). When `project_id` transitions from `None` to
    `Some(value)`, `category_id` and `priority_id` validation is enabled.
  - No framework imports; pure language struct + impl.
  - Ensure existing project-scoped test case logic is unaffected: all project-scoped
    creation and update paths continue to work identically.

- [ ] 2. Define `TestCaseShare` entity and domain exceptions for orphan test cases --
       `design.md#Data Model`, `design.md#Error Handling`
  - `TestCaseShare` entity: `test_case_id`, `user_id`, `shared_by`, `created_at`.
    No domain behaviour beyond construction (a simple value object).
  - Domain exceptions:
    - `OrphanTestCaseNotFoundError`
    - `DuplicateOrphanTestCaseSummaryError` (carries the conflicting summary)
    - `OrphanTestCasePermissionDenied` (carries the reason: missing system permission,
      not the creator, or not a member of target project)
    - `OrphanTestCaseValidationError` (carries field-level details)
    - `InvalidShareUserError` (carries the list of invalid user IDs and reasons)
    - `OrphanAlreadyAssignedError` (test case already has a project; cannot share)
    - `InvalidCategoryError` and `InvalidPriorityError` (reused from test-case-crud)

---

## Layer 2 -- Application

- [ ] 3. Define `OrphanTestCaseRepository` interface (port) -- `design.md#Components`,
      `design.md#Data Model`
  - Methods:
    - `find_visible_by_id(test_case_id, user_id, is_admin) -> Option<TestCaseDetail>`
    - `find_visible_to_user(user_id, page, limit, filters, search, sort, ownership,
      is_admin) -> (Vec<OrphanTestCaseListItem>, u64 total)`
    - `find_by_summary_for_creator(created_by, summary) -> Option<TestCase>` --
      uses `LOWER(summary) = LOWER($1) AND created_by = $2 AND project_id IS NULL
      AND deleted_at IS NULL`
    - `save(test_case) -> TestCaseDetail` -- inserts with `project_id = NULL`
    - `update(test_case) -> TestCaseDetail` -- updates only if `deleted_at IS NULL`
      and `project_id IS NULL`
    - `soft_delete(test_case_id, deleted_by)` -- sets `deleted_at` and `deleted_by`
    - `assign_to_project(test_case_id, project_id, updates) -> TestCaseDetail` --
      updates `project_id` and optional `category_id`/`priority_id`; deletes all
      shares
    - `find_shares(test_case_id) -> Vec<ShareUser>` -- returns users shared with,
      including `id`, `username`, `email`, `shared_at`
    - `add_shares(test_case_id, user_ids, shared_by)` -- inserts into
      `TEST_CASE_SHARES` with `ON CONFLICT DO NOTHING`
    - `remove_share(test_case_id, user_id)` -- deletes from `TEST_CASE_SHARES`
    - `delete_all_shares(test_case_id)` -- deletes all shares for a test case (used
      during assign-to-project)
    - `validate_category_in_project(category_id, project_id) -> bool` -- reused
      signature from `TestCaseRepository`; delegates to or copies the same logic
    - `validate_priority_in_project(priority_id, project_id) -> bool` -- same as above
  - All methods accept a transaction context for transactional composition.
  - `OrphanTestCaseListItem` includes `id`, `summary`, `automated`, `created_by`,
    `created_at`, `updated_at`, and `shared` (bool).

- [ ] 4. Define command/query/response DTOs -- `design.md#API Contract`
  - `CreateOrphanTestCaseCommand` (summary, automated?, description?, notes?)
  - `UpdateOrphanTestCaseCommand` (summary?, automated?, description?, notes?) --
    all fields `Option`; `project_id` is NOT included (handled separately for
    assign-to-project)
  - `AssignToProjectCommand` (project_id, category_id?, priority_id?, summary?,
    automated?, description?, notes?) -- used when `project_id` is present in PATCH
  - `ListOrphanTestCasesQuery` (page, limit, automated?, search?, sort?, ownership?)
  - `OrphanTestCaseFilters` (automated?, ownership?)
  - `OrphanTestCaseListItem` (id, summary, automated, created_by, created_at,
    updated_at, shared)
  - `OrphanTestCaseDetailResponse` (id, summary, description, notes, automated,
    project_id, category_id, category_name, priority_id, priority_name, created_by,
    created_at, updated_by, updated_at, shared)
  - `ShareUserResponse` (id, username, email, shared_at)
  - `AddSharesCommand` (user_ids: Vec<i64>)
  - `PaginationMeta` (total, page, limit) -- reuse from shared utilities

- [ ] 5. Implement `OrphanTestCaseService` -- `requirements.md#US-1` through `US-4`,
      `design.md#Sequence`
  - `create_orphaned(cmd, current_user_id)`:
    validates `test_case:create` permission, checks `category_id` and `priority_id`
    are not present in cmd, begins DB transaction, checks duplicate summary within
    transaction, constructs entity with `project_id = None`, saves via repository,
    commits transaction, returns `OrphanTestCaseDetailResponse`
  - `list_orphaned(query, current_user_id)`:
    validates `test_case:read_orphaned` permission, delegates to repository with
    user_id, pagination, filters, search, sort, and ownership. System Admin flag
    checked via `AuthorizationService::is_admin()`. Returns paginated response.
  - `get_orphaned(test_case_id, current_user_id)`:
    validates `test_case:read_orphaned` permission, fetches by ID with visibility
    check (creator OR shared OR admin). If no row found -> `OrphanTestCaseNotFoundError`
    (same error regardless of cause). Returns `OrphanTestCaseDetailResponse`.
  - `update_orphaned(test_case_id, cmd, current_user_id)`:
    validates `test_case:update` permission, fetches to verify exists, is orphan,
    is non-deleted, and user is creator (not shared). If not creator and not admin
    -> `403`. Rejects `category_id`/`priority_id` in cmd (not allowed for orphans).
    Begins DB transaction. If summary is changing, checks duplicate among user's
    orphans. Applies updates, saves via repository, commits transaction, returns
    response.
  - `assign_to_project(test_case_id, project_id, cmd, current_user_id)`:
    validates `test_case:update` permission, fetches to verify exists, is orphan,
    is non-deleted, and user is creator (or admin). Validates target project exists
    and is non-deleted. Validates user is Contributor/Editor/Owner of target project
    (or admin). Begins DB transaction. If `category_id` provided: validates FK.
    If `priority_id` provided: validates FK. If `summary` provided and differs:
    checks duplicate in target project. Calls `assign_to_project` on repository
    (which updates `project_id` and deletes all shares within the same transaction).
    Commits transaction. Returns response with new `Location`.
  - `delete_orphaned(test_case_id, current_user_id)`:
    validates `test_case:delete` permission, fetches to verify exists, is orphan,
    is non-deleted, and user is creator (or admin). Calls
    `soft_delete(test_case_id, current_user_id)`. No referential integrity check.
  - `list_shares(test_case_id, current_user_id)`:
    validates `test_case:read_orphaned` permission, fetches test case to verify
    visibility (creator OR shared OR admin). Returns share list.
  - `add_shares(test_case_id, user_ids, current_user_id)`:
    validates `test_case:share` permission, fetches to verify exists, is orphan,
    is non-deleted, and user is creator (or admin). If test case has `project_id`
    set -> `OrphanAlreadyAssignedError`. Validates each user_id exists and is ACTIVE.
    Filters out creator's own ID and already-shared users. If no valid new users ->
    returns current share list (no-op). Otherwise, begins transaction, inserts shares,
    commits, returns updated share list.
  - `remove_share(test_case_id, user_id, current_user_id)`:
    validates `test_case:share` permission, fetches to verify exists, is orphan,
    is non-deleted, and user is creator (or admin). Calls
    `remove_share(test_case_id, user_id)`. If no row deleted -> `404`.
  - All methods check system permission via `AuthorizationService`.
  - System Admin bypasses creator ownership and sharing visibility checks.
  - System Admin bypasses target project membership check for assign-to-project.

- [ ] 6. Write unit tests for `OrphanTestCaseService` -- `requirements.md#US-1` through `US-4`
  - Table-driven tests with mock `OrphanTestCaseRepository`, mock `ProjectMemberRepository`,
    mock `UserRepository`, and mock `AuthorizationService`.
  - Happy path: create orphan, list (paginated), get detail, update, delete, list/add/remove
    shares, assign to project.
  - Create:
    - Rejects `category_id` and `priority_id` in request body.
    - Duplicate summary among user's orphans returns `409`.
    - Soft-deleted same-summary orphan does not block creation.
    - Permission denial for missing `test_case:create`.
    - Boundary values: summary at exactly 500 chars (passes), 501 chars (rejects), empty
      string, single char, whitespace-only string (rejects), leading/trailing whitespace
      trimmed.
    - Description: exactly 10000 chars (passes), 10001 chars (rejects), "" (stored as empty
      string), null (clears), omitted (defaults to null).
    - Notes: exactly 5000 chars (passes), 5001 chars (rejects), "" (stored as empty
      string), null (clears), omitted (defaults to null).
    - Automated: true/false accepted, non-boolean rejected, omitted defaults to false.
    - Unrecognised fields rejected (strict mode).
  - List:
    - Returns only orphan test cases (`project_id IS NULL`).
    - Returns test cases where user is creator.
    - Returns test cases where user is shared.
    - Returns both when no `ownership` filter.
    - `ownership=mine` returns only creator's test cases.
    - `ownership=shared` returns only shared test cases.
    - Invalid ownership value returns `422`.
    - Soft-deleted test cases excluded.
    - Search and sort parameters passed through correctly.
    - Filter parameter (automated) passed through correctly.
    - `shared` flag computed correctly: `false` for created_by, `true` for shared.
    - System Admin sees all orphan test cases.
  - Get detail:
    - Returns full detail with resolved category/priority names.
    - `shared` flag correct for creator vs shared user.
    - Visibility denied: user is neither creator nor shared -> `404`.
    - Test case is soft-deleted -> `404`.
    - Test case is not an orphan (has project_id) -> `404`.
    - System Admin can view any orphan test case.
  - Update:
    - Regular update: Creator can update summary, automated, description, notes.
    - Regular update: Rejects `category_id` and `priority_id`.
    - Regular update: Shared user cannot update -> `403`.
    - Regular update: Non-creator, non-admin cannot update -> `403`.
    - Regular update: Permission denial for missing `test_case:update`.
    - Regular update: Duplicate summary among user's orphans -> `409`.
    - Regular update: Empty body -> `422`.
    - Regular update: Update on soft-deleted -> `404`.
    - Regular update: Unrecognised fields -> `422`.
  - Assign to project:
    - Creator assigns to project with `project_id`.
    - Category and priority validated against target project within transaction.
    - Summary uniqueness checked against target project.
    - Shares deleted after assignment.
    - `Location` header reflects new project-scoped URL.
    - User is not creator -> `403`.
    - User is not Contributor/Editor/Owner of target project -> `403`.
    - Target project not found or soft-deleted -> `404`.
    - Category invalid (non-existent, soft-deleted, wrong project) -> `422`.
    - Priority invalid -> `422`.
    - Duplicate summary in target project -> `409`.
    - System Admin can assign any orphan to any project.
  - Delete:
    - Creator can soft-delete own orphan.
    - Shared user cannot delete -> `403`.
    - Non-creator, non-admin cannot delete -> `403`.
    - Repeated delete on soft-deleted -> `404`.
    - Shares are NOT deleted on soft-delete.
  - Shares:
    - Creator can list shares.
    - Shared user can list shares (but not add/remove).
    - Non-visible user gets `404` on list shares.
    - Creator can add shares for ACTIVE users.
    - INACTIVE users rejected with `422 INVALID_USER`.
    - Non-existent users rejected with `422 INVALID_USER`.
    - Creator's own ID silently skipped.
    - Already shared users silently skipped (idempotent).
    - Sharing on already-assigned test case returns `422 ALREADY_ASSIGNED`.
    - Creator can remove a share; non-existent share returns `404`.
    - Shared user cannot add or remove shares -> `403`.
    - Permission denial for missing `test_case:share`.
  - Unicode: summary with multi-byte UTF-8 characters -- LOWER() comparison and length
    checks work correctly.

---

## Layer 3 -- Adapters (HTTP)

- [ ] 7. Implement `OrphanTestCaseHandler` -- `design.md#API Contract`, `design.md#Components`
  - Eight handler methods: `create`, `list`, `get`, `update`, `delete`, `list_shares`,
    `add_shares`, `remove_share`.
  - Deserialize request bodies and query params into DTOs.
  - Validate pagination: `page` in [1, 1000], `limit` in [1, 100]; return `422` otherwise.
  - Validate `sort` against whitelist (`id`, `-id`, `summary`, `-summary`, `created_at`,
    `-created_at`); return `422` for invalid values.
  - Validate `search` does not exceed 255 characters; return `422` otherwise.
  - Validate `automated` filter is a valid boolean string (`"true"` or `"false"`) if
    provided; return `422` otherwise.
  - Validate `ownership` filter against whitelist (`mine`, `shared`); return `422` for
    invalid values.
  - Strip HTML tags from `summary`, `description`, and `notes` fields before passing to
    service (XSS input sanitization at the boundary).
  - For `create`: implement strict mode -- reject request bodies that contain unrecognised
    fields, including `category_id`, `priority_id`, and `project_id`.
  - For `update`: detect `project_id` in the request body to route to the assign-to-project
    flow (calling `OrphanTestCaseService::assign_to_project`) vs the regular update flow.
  - For `update` (regular): implement strict mode and reject `category_id` and `priority_id`.
  - For `update` (assign-to-project): validate `project_id` is a positive integer.
  - Call `OrphanTestCaseService` methods.
  - Serialize responses with proper status codes:
    - `201 Created` with `Location: /api/v1/test-cases/orphaned/{id}` header for create.
    - `200 OK` for list, get, update, assign-to-project, list shares, add shares.
    - `200 OK` with `Location: /api/v1/projects/{pid}/test-cases/{id}` for assign-to-project.
    - `204 No Content` for delete and remove share.
  - Map domain/service errors to HTTP status codes and standard error body format:
    - `OrphanTestCasePermissionDenied` -> `403 Forbidden` (generic message).
    - `OrphanTestCaseNotFoundError` -> `404 Not Found` (same message for non-existent,
      soft-deleted, or not-visible).
    - `DuplicateOrphanTestCaseSummaryError` -> `409 Conflict` with
      `DUPLICATE_TEST_CASE_SUMMARY`.
    - `InvalidCategoryError` -> `422 Unprocessable Entity` with `INVALID_CATEGORY`.
    - `InvalidPriorityError` -> `422 Unprocessable Entity` with `INVALID_PRIORITY`.
    - `InvalidShareUserError` -> `422 Unprocessable Entity` with `INVALID_USER`.
    - `OrphanAlreadyAssignedError` -> `422 Unprocessable Entity` with `ALREADY_ASSIGNED`.
    - `OrphanTestCaseValidationError` -> `422 Unprocessable Entity` with field-level details.

- [ ] 8. Register orphan test case routes in HTTP router -- `design.md#Route Registration`
  - `POST   /api/v1/test-cases/orphaned`                     -> `create`
  - `GET    /api/v1/test-cases/orphaned`                     -> `list`
  - `GET    /api/v1/test-cases/orphaned/{id}`                -> `get`
  - `PATCH  /api/v1/test-cases/orphaned/{id}`                -> `update`
  - `DELETE /api/v1/test-cases/orphaned/{id}`                -> `delete`
  - `GET    /api/v1/test-cases/orphaned/{id}/shares`         -> `list_shares`
  - `POST   /api/v1/test-cases/orphaned/{id}/shares`         -> `add_shares`
  - `DELETE /api/v1/test-cases/orphaned/{id}/shares/{userId}` -> `remove_share`
  - All routes require session auth middleware.
  - Route prefix does NOT include `{projectId}` (orphan endpoints are project-independent).
  - Ensure orphan routes do not shadow existing project-scoped test case routes
    (`/api/v1/projects/{projectId}/test-cases/...`).

- [ ] 9. Write integration tests for orphan test case HTTP handlers --
       `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back).
  - Create orphan:
    - `201 Created` with `Location` header.
    - `422` when `category_id` or `priority_id` provided.
    - `422` on validation errors (summary empty, description too long, etc.).
    - `409` on duplicate summary for same creator.
    - Different creators can use same summary without conflict.
    - Soft-deleted same-summary orphan does not block creation.
  - List orphan:
    - `200 OK` with pagination metadata.
    - Returns own orphan test cases.
    - Returns shared orphan test cases.
    - `ownership=mine` returns only own.
    - `ownership=shared` returns only shared.
    - `automated` filter works correctly.
    - `search` filter works correctly (summary and description).
    - `sort` parameter works correctly.
    - `shared` field is correct in response.
    - Soft-deleted excluded.
    - System Admin sees all orphans.
  - Get orphan detail:
    - `200 OK` with full detail and `shared` flag.
    - `404` when user is neither creator nor shared.
    - `404` on soft-deleted.
    - `404` on non-existent.
    - Category and priority names resolved (expect null for orphans but verify
      defensive resolution).
  - Update orphan (regular):
    - `200 OK` with updated fields.
    - `403` when user is shared (not creator).
    - `403` when user is neither creator nor shared.
    - `409` on duplicate summary.
    - `422` when `category_id` or `priority_id` provided.
    - `422` on empty body.
    - `422` on unrecognised fields.
    - PATCH semantics: omit field preserves, null clears, "" stores empty string.
  - Assign to project:
    - `200 OK` with `Location` header pointing to project-scoped URL.
    - Test case no longer appears in orphan list.
    - Test case appears in project-scoped list.
    - Shares are deleted after assignment.
    - Category and priority set during assignment and validated.
    - `403` when user is not a member of target project.
    - `403` when user is a Viewer of target project.
    - `404` when target project does not exist or is soft-deleted.
    - `422` on invalid category or priority.
    - `409` on duplicate summary in target project.
    - Summary can be updated in the same assign-to-project request.
  - Delete orphan:
    - `204 No Content` on successful soft-delete.
    - `403` when user is shared (not creator).
    - `404` on repeated soft-delete.
    - After soft-delete: excluded from list and detail.
    - Shares are NOT deleted (still exist in TEST_CASE_SHARES but naturally
      excluded by deleted_at IS NULL filter).
  - Shares:
    - List shares: `200 OK` with user list.
    - List shares: `404` when user has no visibility.
    - List shares: Shared user can view share list.
    - Add shares: `200 OK` with updated share list.
    - Add shares: Creator's own ID silently skipped.
    - Add shares: Already-shared user silently skipped (idempotent).
    - Add shares: INACTIVE users rejected with `422 INVALID_USER`.
    - Add shares: Non-existent users rejected with `422 INVALID_USER`.
    - Add shares: `403` when user is shared (not creator).
    - Add shares: `422` when test case already assigned to project.
    - Remove share: `204 No Content`.
    - Remove share: `404` when share does not exist.
    - Remove share: `403` when user is shared (not creator).
  - Test that `404` responses use the same message for non-existent, soft-deleted, and
    not-visible cases (information leakage prevention).
  - Test that System Admin can perform all operations on any orphan test case.
  - Test strict mode: unrecognised fields in request body return `422`.
  - Test XSS sanitization on summary, description, notes.

---

## Layer 4 -- Infrastructure

- [ ] 10. Create migrations for orphan test case support -- `design.md#Data Model`
  - Migration 1: Alter `TEST_CASES.project_id` to be nullable:
    ```sql
    ALTER TABLE test_cases ALTER COLUMN project_id DROP NOT NULL;
    ```
  - Migration 2: Create `TEST_CASE_SHARES` table:
    - Columns: `test_case_id` (BIGINT, NOT NULL, FK -> test_cases(id) ON DELETE CASCADE),
      `user_id` (BIGINT, NOT NULL, FK -> users(id) ON DELETE CASCADE),
      `shared_by` (BIGINT, NOT NULL, FK -> users(id) ON DELETE RESTRICT),
      `created_at` (TIMESTAMPTZ, NOT NULL, DEFAULT NOW())
    - Composite PK on `(test_case_id, user_id)`
    - Index `idx_test_case_shares_test_case` on `(test_case_id)`
    - Index `idx_test_case_shares_user` on `(user_id)`
  - Migration 3: Add partial unique index for orphan test case summaries:
    ```sql
    CREATE UNIQUE INDEX uq_orphan_test_cases_summary_creator
      ON test_cases (created_by, LOWER(summary))
      WHERE deleted_at IS NULL AND project_id IS NULL;
    ```
  - Migration 4: Add indexes for orphan queries:
    ```sql
    CREATE INDEX idx_test_cases_orphan_creator
      ON test_cases (created_by, id DESC)
      WHERE deleted_at IS NULL AND project_id IS NULL;

    CREATE INDEX idx_test_cases_orphan_active
      ON test_cases (id DESC)
      WHERE deleted_at IS NULL AND project_id IS NULL;
    ```
  - Rollback migrations:
    - `DROP INDEX IF EXISTS idx_test_cases_orphan_active`
    - `DROP INDEX IF EXISTS idx_test_cases_orphan_creator`
    - `DROP INDEX IF EXISTS uq_orphan_test_cases_summary_creator`
    - `DROP TABLE IF EXISTS test_case_shares`
    - `ALTER TABLE test_cases ALTER COLUMN project_id SET NOT NULL`
      (only safe if no orphan test cases exist; document this as a manual
      prerequisite: delete all orphan test cases before rolling back)
  - Ensure migration order: the `TEST_CASES` table must exist before these migrations.
  - Ensure the `USERS` table exists before `TEST_CASE_SHARES`.
  - Verify the existing partial unique index `uq_test_cases_summary_project` does not
    conflict with the new `uq_orphan_test_cases_summary_creator` (they operate on
    disjoint WHERE clauses: `project_id IS NULL` vs the implicit `project_id IS NOT NULL`
    in the original index's key).

- [ ] 11. Implement `SqlOrphanTestCaseRepository` -- `design.md#Components`
  - All methods from `OrphanTestCaseRepository` interface.
  - Every SELECT includes `WHERE deleted_at IS NULL` for active record filtering.
  - `find_visible_by_id`:
    ```sql
    SELECT tc.*,
           cat.name AS category_name,
           pri.name AS priority_name
    FROM test_cases tc
    LEFT JOIN test_categories cat
      ON tc.category_id = cat.id AND cat.deleted_at IS NULL
    LEFT JOIN test_priorities pri
      ON tc.priority_id = pri.id AND pri.deleted_at IS NULL
    WHERE tc.id = $1
      AND tc.project_id IS NULL
      AND tc.deleted_at IS NULL
      AND (tc.created_by = $2 OR EXISTS(
        SELECT 1 FROM test_case_shares tcs
        WHERE tcs.test_case_id = tc.id AND tcs.user_id = $2
      ))
    ```
    For System Admin, omit the `(tc.created_by = $2 OR EXISTS(...))` clause.
  - `find_visible_to_user` builds a dynamic query:
    - Base: `WHERE tc.project_id IS NULL AND tc.deleted_at IS NULL`
    - Visibility (non-admin):
      `AND (tc.created_by = $N OR EXISTS(SELECT 1 FROM test_case_shares tcs WHERE tcs.test_case_id = tc.id AND tcs.user_id = $N))`
    - `ownership=mine`: keep only `AND tc.created_by = $N`
    - `ownership=shared`: `AND tc.created_by != $N AND EXISTS(SELECT 1 FROM test_case_shares tcs WHERE tcs.test_case_id = tc.id AND tcs.user_id = $N)`
    - `automated` filter: `AND tc.automated = $N` if provided
    - `search` filter: `AND (tc.summary ILIKE $N OR tc.description ILIKE $N)` if provided
    - `sort`: validate against whitelist and build ORDER BY. Default: `ORDER BY tc.id DESC`
    - Select computed `shared` field: `tc.created_by != $N AS shared`
    - `COUNT(*) OVER()` for total count
  - `find_by_summary_for_creator`:
    `SELECT * FROM test_cases WHERE LOWER(summary) = LOWER($1) AND created_by = $2 AND project_id IS NULL AND deleted_at IS NULL`
  - `save`: inserts with `project_id = NULL`, `category_id = NULL`, `priority_id = NULL`.
    After insert, re-fetch with LEFT JOINs (same query as `find_visible_by_id`) for the
    response.
  - `update`: updates `summary`, `automated`, `description`, `notes`, `updated_by` for the
    given `id`, but only if `project_id IS NULL AND deleted_at IS NULL`. After update,
    re-fetch with JOINs for the response.
  - `assign_to_project`: within a single transaction:
    1. Update `project_id`, optionally `category_id`, `priority_id`, `summary`,
       `automated`, `description`, `notes`, `updated_by` for the given `id`, only if
       `project_id IS NULL AND deleted_at IS NULL`.
    2. `DELETE FROM test_case_shares WHERE test_case_id = $1`
    3. Re-fetch the updated test case with regular project-scoped query (including resolved
       names) for the response.
  - `soft_delete`: sets `deleted_at = NOW()`, `deleted_by = $2` WHERE
    `id = $1 AND project_id IS NULL AND deleted_at IS NULL`
  - `find_shares`:
    ```sql
    SELECT u.id, u.username, u.email, tcs.created_at AS shared_at
    FROM test_case_shares tcs
    JOIN users u ON tcs.user_id = u.id
    WHERE tcs.test_case_id = $1
    ORDER BY tcs.created_at ASC
    ```
  - `add_shares`: batch insert with
    `INSERT INTO test_case_shares (test_case_id, user_id, shared_by) VALUES ($1, $2, $3), ($1, $4, $3), ... ON CONFLICT (test_case_id, user_id) DO NOTHING`
  - `remove_share`: `DELETE FROM test_case_shares WHERE test_case_id = $1 AND user_id = $2`
  - `delete_all_shares`: `DELETE FROM test_case_shares WHERE test_case_id = $1`
  - `validate_category_in_project` and `validate_priority_in_project`: reuse the same logic
    as `SqlTestCaseRepository` (or call through to it if both repositories share a DB
    connection).
  - Use parameterized queries exclusively -- never string interpolation.
  - Write unit tests with a test database (one test transaction per case):
    - Insert and retrieve an orphan test case (verify project_id is null, category_id null,
      priority_id null).
    - Duplicate summary detection for same creator.
    - Different creators with same summary (both allowed).
    - Visibility: creator can see own orphan.
    - Visibility: shared user can see orphan.
    - Visibility: non-creator, non-shared user cannot see orphan.
    - List: ownership=mine returns only creator's.
    - List: ownership=shared returns only shared.
    - List: admin sees all.
    - Soft-deleted orphan excluded from all queries.
    - Update orphan fields.
    - Update on soft-deleted returns 0 rows affected.
    - Assign to project: project_id updated, shares deleted.
    - Add shares: idempotent (duplicate share does not error).
    - Remove share: row deleted.
    - Delete all shares: all rows removed.
    - Search escaping: `%`, `_`, `\` in search value treated as literals.
    - Unicode summary round-trip.
    - Share list: includes username, email, shared_at.
    - Category/priority resolution: null for orphans, non-null after assignment.

- [ ] 12. Wire authorization and dependency injection for orphan test case components --
       `design.md#Components`, `design.md#New Permission Codes`
  - Add 2 new `test_case:*` permission codes to the permission registry:
    - `test_case:read_orphaned` -- "Read Orphan Test Case"
    - `test_case:share` -- "Share Orphan Test Case"
  - Add these to the permissions seed migration with
    `INSERT ... ON CONFLICT (code) DO NOTHING` (let the database auto-assign IDs).
  - The existing `test_case:create`, `test_case:update`, `test_case:delete` permissions
    are reused without modification.
  - Register `SqlOrphanTestCaseRepository` as the implementation of
    `OrphanTestCaseRepository`.
  - Register `OrphanTestCaseService` with its dependencies (`OrphanTestCaseRepository`,
    `AuthorizationService`, `ProjectMemberRepository`, `UserRepository`).
  - Register `OrphanTestCaseHandler` with `OrphanTestCaseService`.
  - Wire rate limit configuration for orphan endpoints:
    - `GET .../test-cases/orphaned` (list) -- 60 req/min
    - `GET .../test-cases/orphaned/{id}` (detail) -- 60 req/min
    - `POST .../test-cases/orphaned` (create) -- 30 req/min
    - `PATCH .../test-cases/orphaned/{id}` (update/assign) -- 30 req/min
    - `DELETE .../test-cases/orphaned/{id}` (delete) -- 30 req/min
    - `GET .../test-cases/orphaned/{id}/shares` (list shares) -- 60 req/min
    - `POST .../test-cases/orphaned/{id}/shares` (add shares) -- 30 req/min
    - `DELETE .../test-cases/orphaned/{id}/shares/{userId}` (remove share) -- 30 req/min
  - Write unit tests: verify each system permission is correctly checked for each endpoint;
    verify creator ownership check is enforced for update/delete/share; verify shared users
    have read-only access; verify System Admin bypasses all checks.
