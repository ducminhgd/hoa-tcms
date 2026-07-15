# Tasks: Metadata Priorities

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `TestPriority` entity -- `requirements.md#US-1,US-3`, `design.md#Components`
  - Fields: `id`, `project_id`, `name`, `description`, `rank`, `created_by`, `created_at`,
    `updated_by`, `updated_at`
  - Factory method `TestPriority::create(project_id, name, description, rank, created_by)`
    with domain validation:
    - `rank` must be in 1..5 (inclusive)
    - `name` must contain at least one non-whitespace character; max 50 chars; leading/trailing
      whitespace trimmed
    - `description` <= 2000 chars; empty string `""` stored as-is, `null` becomes `NULL`
  - Method `apply_description_update(description: Option<String>)`:
    - Validates the new description value
    - Returns a new entity with the updated description (immutable `id`, `project_id`, `name`,
      `rank`, `created_by`, `created_at` preserved; `updated_by` and `updated_at` left for
      the service/repository layer)
  - No framework imports; pure language struct + impl

- [ ] 2. Define domain exceptions for priorities -- `design.md#Error Handling`
  - `PriorityNotFoundError`
  - `PriorityPermissionDenied`
  - `PriorityValidationError` (carries field-level details)
  - `PriorityImmutableFieldError` (raised when attempting to change `name` or `rank` via
    the entity method; caught at service/handler layer)

---

## Layer 2 -- Application

- [ ] 3. Define `PriorityRepository` interface (port) -- `design.md#Components`, `design.md#Data Model`
  - Methods:
    - `find_by_id(project_id, priority_id) -> Option<TestPriority>`
    - `find_by_project(project_id, page, limit, search, sort) -> (Vec<TestPriority>, u64 total)`
    - `find_all_by_project(project_id) -> Vec<PrioritySelectItem>`
    - `update(priority) -> TestPriority`
  - All methods accept a transaction context for transactional composition
  - Return domain entities (not raw DB rows)
  - `PrioritySelectItem` is a lightweight DTO with `id`, `name`, and `rank`
  - No `save` method (priorities are created only by the seeder during project creation)
  - No `soft_delete` method (priorities are permanent and never deleted)

- [ ] 4. Define command/query/response DTOs -- `design.md#API Contract`
  - `UpdatePriorityCommand` (description?) -- note: only `description` is accepted;
    `name` and `rank` are not in this struct. The command is constructed by the handler
    after validating that no immutable fields were sent.
  - `ListPrioritiesQuery` (project_id, page, limit, search?, sort?)
  - `PriorityListResponse` (id, name, description, rank, created_by, created_at, updated_at)
    -- used in list endpoints; excludes `project_id` (known from URL) and `updated_by`
  - `PriorityDetailResponse` (id, name, description, rank, project_id, created_by,
    created_at, updated_by, updated_at) -- full detail view
  - `PrioritySelectItem` (id, name, rank)
  - `PaginationMeta` (total, page, limit) -- reuse from shared utilities if available

- [ ] 5. Implement `PriorityService` -- `requirements.md#US-1` through `US-4`,
      `design.md#Sequence`
  - `list_priorities(project_id, query, current_user_id)`:
    validates `priority:read_list` permission, checks project membership (any role),
    delegates to repository with pagination/search/sort, returns paginated response
  - `get_priority(project_id, priority_id, current_user_id)`:
    validates `priority:read` permission, checks project membership (any role), fetches by
    ID, verifies belongs to project, returns `PriorityDetailResponse`
  - `update_priority(project_id, priority_id, cmd, current_user_id)`:
    validates `priority:update` permission, checks Owner/Editor role, fetches priority,
    begins DB transaction, applies description update via entity method
    `apply_description_update`, persists via repository, commits transaction,
    returns `PriorityDetailResponse`
  - `select_priorities(project_id, current_user_id)`:
    validates `priority:select` permission, checks project membership (any role),
    delegates to `find_all_by_project`, returns flat list of `PrioritySelectItem`
  - All methods check system permission via `AuthorizationService`
  - All methods check project membership/role via `ProjectMemberRepository`

- [ ] 6. Write unit tests for `PriorityService` -- `requirements.md#US-1` through `US-4`
  - Table-driven tests with mock `PriorityRepository` and mock `ProjectMemberRepository`
  - Happy path: list (paginated), get, update, select
  - Permission denial for each permission code (`priority:read`, `priority:read_list`,
    `priority:update`, `priority:select`)
  - Project role denial: Contributor/Viewer cannot update
  - Project role acceptance: any member can read/read_list/select
  - Priority not found returns error
  - Priority from wrong project returns error
  - Search and sort parameters passed through to repository correctly
  - System Admin bypasses project membership checks
  - Boundary values: description at exactly 2000 chars (passes), 2001 chars (rejects)
  - Rank boundaries: rank=0 and rank=6 rejected (`PriorityValidationError`), rank=1 and
    rank=5 accepted
  - Description: `""` (stored as empty string), `null` (clears/sets NULL)
  - Immutable field rejection: attempting to change `name` or `rank` via entity method
    raises `PriorityImmutableFieldError`

---

## Layer 3 -- Adapters (HTTP)

- [ ] 7. Implement `PriorityHandler` -- `design.md#API Contract`, `design.md#Components`
  - Four handler methods: `list`, `get`, `update`, `select`
  - Deserialize request bodies and query params into DTOs
  - Validate pagination: `page` in [1, 1000], `limit` in [1, 100]; return `422` otherwise
  - Validate `sort` against whitelist (`rank`, `-rank`, `name`, `-name`);
    return `422` for invalid values
  - Validate `search` does not exceed 255 characters; return `422` otherwise
  - Validate project exists and is not soft-deleted before any priority operation
    (call `ProjectRepository::find_by_id` or a shared project validation guard)
  - In `update` handler:
    - Reject empty body with `422`
    - Reject requests containing `name` field with `422` (immutable field)
    - Reject requests containing `rank` field with `422` (immutable field)
    - Reject requests containing unrecognised fields with `422` (strict mode)
    - Strip HTML tags from `description` before passing to service (XSS input sanitization
      at the boundary)
    - Construct `UpdatePriorityCommand` with only the `description` field
  - Call `PriorityService` methods
  - Serialize responses with proper status codes:
    - `200 OK` for list, get, update, select
  - Map domain/service errors to HTTP status codes and standard error body format

- [ ] 8. Register priority routes in HTTP router -- `design.md#Route Registration`
  - `GET    /api/v1/projects/{projectId}/priorities`          → `list`
  - `GET    /api/v1/projects/{projectId}/priorities/select`   → `select`  (static path)
  - `GET    /api/v1/projects/{projectId}/priorities/{id}`     → `get`     (dynamic path)
  - `PATCH  /api/v1/projects/{projectId}/priorities/{id}`     → `update`
  - All routes require session auth middleware
  - Route order: `/select` **must** be registered before `/{id}` to prevent the
    word "select" from being interpreted as a priority ID
  - No `POST` or `DELETE` routes (priorities are fixed and permanent)

- [ ] 9. Write integration tests for priority HTTP handlers -- `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back)
  - Test `200 OK` for list with pagination metadata
  - Test search filter (query parameter applied correctly)
  - Test sort parameter (`rank`, `-rank`, `name`, `-name`); verify default is `rank` ascending
  - Test `200 OK` for select (flat list with `id`, `name`, `rank`, ordered by `rank`)
  - Test `200 OK` for get (full detail including audit timestamps)
  - Test `200 OK` for update: description changes, `updated_at`/`updated_by` updated
  - Test update preserves `name` and `rank` (unchanged after PATCH)
  - Test `422 Unprocessable Entity` when PATCH body includes `name` field
  - Test `422 Unprocessable Entity` when PATCH body includes `rank` field
  - Test `422 Unprocessable Entity` on empty PATCH body
  - Test `403 Forbidden` on missing permission (each endpoint)
  - Test `403 Forbidden` on insufficient project role (Contributor trying to update)
  - Test that `403` response bodies do not distinguish between "missing permission" and
    "wrong project role" (identical message for both failure modes)
  - Test `404 Not Found` on non-existent project
  - Test `404 Not Found` on non-existent priority
  - Test `404 Not Found` on priority belonging to a different project
  - Test that System Admin can perform all operations on any project
  - Test that `/select` route is not shadowed by `/{id}` (GET with "select" as ID
    hits the select handler, not the get handler)
  - Test edge cases: description exactly 2000 chars accepted, description at 2001 chars
    rejected
  - Test PATCH semantics: send `""` (stores empty string), send `null` (clears to NULL),
    omit `description` (empty body → `422`)
  - Test `search` with ILIKE special chars `%`, `_`, and `\` (treated as literals)
  - Test invalid `sort` values return `422`
  - Test `search` exceeding 255 characters returns `422`
  - Test `page` > 1000 returns `422`, `page` < 1 returns `422`, `limit` < 1 returns `422`
  - Test all four endpoints return `401 Unauthorized` when no session cookie is present
  - Test unrecognised fields in request body return `422`
  - Verify only 4 routes are registered (no POST or DELETE)
  - Test XSS input sanitization: update with `<script>alert('xss')</script>` in
    description, verify stored value has HTML tags stripped
  - Authorization integration tests:
    - `PATCH` returns `403` for authenticated non-member with `priority:update` permission
    - `PATCH` returns `403` for project member with Viewer role
    - `PATCH` returns `403` for project member with Contributor role
    - System Admin can update priorities on any project regardless of membership

---

## Layer 4 -- Infrastructure

- [ ] 10. Create `TEST_PRIORITIES` database migration -- `design.md#Data Model`
  - Table definition with all columns, PK, FKs, constraints
  - `CHECK` constraint on `description`: `description IS NULL OR char_length(description) <= 2000`
  - `CHECK` constraint on `rank`: `rank >= 1 AND rank <= 5`
  - Unique index `uq_test_priorities_name_project` on `(project_id, LOWER(name))`
    (no `WHERE` clause -- priorities are never soft-deleted, so uniqueness is unconditional)
  - Unique index `uq_test_priorities_rank_project` on `(project_id, rank)`
    (no `WHERE` clause -- same reason)
  - Foreign key indexes on `project_id`, `created_by`, `updated_by`
  - Composite index `idx_test_priorities_project_rank` on `(project_id, rank)` for
    the most common query pattern
  - `BEFORE UPDATE` trigger `trg_test_priorities_before_update` that:
    - Rejects changes to `name`, `rank`, and `project_id` (raises exception)
    - Auto-sets `NEW.updated_at = NOW()`
  - Rollback migration: `DROP TRIGGER IF EXISTS`, `DROP FUNCTION IF EXISTS`, `DROP TABLE IF EXISTS`

- [ ] 11. Add priority permission codes and wire authorization --
       `design.md#New Permission Codes`, `design.md#Components`
  - Add 4 new rows to the permissions seed data with `INSERT ... ON CONFLICT (code) DO NOTHING`:
    `('priority:read', 'Read Priority')`,
    `('priority:read_list', 'Read Priority List')`,
    `('priority:update', 'Update Priority')`,
    `('priority:select', 'Select Priority')`
  - Let the database auto-assign IDs (do NOT hardcode numeric IDs)
  - Verify idempotency: `ON CONFLICT (code) DO NOTHING`
  - Register the 4 new `priority:*` permission codes in the permission registry
  - Extend the project-scope authorization guard to accept required project roles:
    - `priority:read`      → any project member (all 4 roles)
    - `priority:read_list` → any project member
    - `priority:update`    → Owner or Editor
    - `priority:select`    → any project member
  - System Admin bypasses all role checks
  - Write unit tests: verify each role is correctly accepted/rejected for each
    permission code

- [ ] 12. Implement `SqlPriorityRepository` -- `design.md#Components`
  - All methods from `PriorityRepository` interface
  - No `WHERE deleted_at IS NULL` clause needed (priorities are never soft-deleted)
  - `find_by_id` uses `WHERE id = $1 AND project_id = $2`
  - `find_by_project` builds a dynamic query for search and sort. Escape `\`, `%`, and
    `_` in the search value in that order (double backslashes first, then escape `%` and
    `_` with backslash prefix) before building the ILIKE pattern. Validate `sort` against
    a whitelist of allowed column names (`name`, `rank`) before building the
    `ORDER BY` clause -- never interpolate user input directly into SQL
  - `find_all_by_project` selects `id`, `name`, and `rank`, ordered by `rank` ASC
  - `update` updates `description`, `updated_by`, `updated_at` (via trigger)
    for the given `id` and `project_id`. Does NOT touch `name`, `rank`, or other fields.
    Uses `UPDATE test_priorities SET description = $1, updated_by = $2 WHERE id = $3 AND project_id = $4`
  - Use parameterized queries exclusively -- never string interpolation
  - Write unit tests with a test database (one test transaction per case)
    - Verify `update` does not modify `name` or `rank`
    - Verify `find_all_by_project` returns results in rank order
    - Verify `find_by_project` with `sort=rank` returns in rank order
    - Verify `find_by_project` with `sort=name` returns in name order

- [ ] 13. Implement priority seeding in `ConfigFileSeeder` -- `requirements.md#overview`,
       `design.md#YAML Config Seed Format`
  - Add `seed_priorities(project_id, created_by, tx)` method
  - Read the `priorities` list from the parsed YAML config
  - Validate exactly 5 entries exist; reject (roll back) with an error if not
  - Validate each entry has a non-empty `name` (string) and a valid `rank` (integer 1--5);
    reject with an error if not
  - Validate `rank` values are unique (1, 2, 3, 4, 5 -- no duplicates); reject if not
  - Sort entries by `rank` ascending before inserting (so they are always stored in rank
    order regardless of YAML order)
  - Insert each priority into `TEST_PRIORITIES` within the caller's transaction
  - Handle `description` gracefully: not a string → treat as `null`; > 2000 chars →
    truncate with warning log
  - Handle missing `priorities` section: reject (priorities are required for every project)
  - Write integration test:
    - Seed priorities into a test database and verify all 5 rows are present with correct
      `project_id`, `created_by`, `name`, `rank`
    - Verify seeding with only 4 entries in YAML causes rollback
    - Verify seeding with duplicate `rank` in YAML causes rollback
    - Verify seeding with `rank` outside 1--5 causes rollback
    - Verify seeding with missing `name` causes rollback
    - Verify `description` truncation at 2000 chars

---

## Verification and Cleanup

- [ ] 14. End-to-end verification and update specs/README.md --
       `requirements.md#US-1` through `US-4`
  - Manual or automated walkthrough of all four user stories
  - Verify list returns all 5 priorities ordered by rank ascending by default
  - Verify list supports `sort=-rank` (descending: LOWEST first)
  - Verify list supports `sort=name` (alphabetical)
  - Verify list supports `search` filtering on name and description
  - Verify pagination: page 1 with limit 2 returns correct slice and `total: 5`
  - Verify get returns full detail including `rank` and audit timestamps
  - Verify update changes `description`, updates `updated_at`/`updated_by`, and
    preserves `name` and `rank` unchanged
  - Verify update rejects `name` in request body with `422`
  - Verify update rejects `rank` in request body with `422`
  - Verify update on non-existent priority returns `404`
  - Verify select returns flat list of `{id, name, rank}` sorted by rank
  - Verify `403` for each endpoint with insufficient permission
  - Verify `403` for Contributor on PATCH
  - Verify Contributor can read/read_list/select
  - Verify Viewer can read/read_list/select (cannot PATCH)
  - Verify System Admin can perform all operations on any project
  - Verify priorities seeded from YAML config on project creation (end-to-end with
    project creation flow): all 5 rows present with correct names and ranks (1--5)
  - Verify no accidental POST or DELETE routes exist (confirm `405 Method Not Allowed`
    for those verbs)
  - Update `specs/README.md`: mark `metadata-priorities` as having completed specs
    (requirements.md, design.md, tasks.md)

---

## Security and Hardening

- [ ] 15. **Implement CSRF protection** --
       `requirements.md#security-considerations`, `design.md#security-requirements`
  - Ensure session cookie carries `SameSite=Lax` (or stricter)
  - Ensure `PATCH` endpoints reject requests without `Content-Type: application/json`
    (to block simple form-based CSRF attacks)
  - Integration test: verify `PATCH /api/v1/projects/{pid}/priorities/{id}` without JSON
    content type is rejected
  - Note: XSS sanitization is handled in the handler implementation (Task 7) -- confirm
    it is correctly wired in the update handler

- [ ] 16. **Verify SQL injection resistance** --
       `design.md#Components`
  - Code review: confirm all SQL in `SqlPriorityRepository` uses parameterized
    queries (`$1`, `$2`, ...), never string interpolation
  - Code review: confirm the `sort` parameter is validated against a whitelist
    (`rank`, `-rank`, `name`, `-name`) before being used in `ORDER BY`
  - Integration test: attempt SQL injection via the `search` query parameter
    (e.g., `' OR '1'='1`) and verify no data leakage
  - Integration test: attempt SQL injection via the `sort` query parameter and
    verify the input is rejected or sanitized
