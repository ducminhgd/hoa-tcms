# Tasks: Metadata Templates

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `TestCaseTemplate` entity -- `requirements.md#US-1`, `design.md#Components`
  - Fields: `id`, `project_id`, `name`, `description`, `created_by`, `created_at`,
    `updated_by`, `updated_at`, `deleted_by`, `deleted_at`
  - Factory method `TestCaseTemplate::create(project_id, name, description, created_by)` with
    domain validation: name trimmed and must contain at least one non-whitespace character,
    max 255 chars; description <= 10000 chars; empty string `""` stored as-is, `null` becomes `NULL`
  - No framework imports; pure language struct + impl
  - Implement `is_soft_deleted()` convenience method
  - Implement a method to produce an update with changed fields:
    `apply_update(name: Option<String>, description: Option<String>)`
    -- validates new values and returns a new (or modified) entity

- [ ] 2. Define domain exceptions for templates -- `design.md#Error Handling`
  - `TemplateNotFoundError`
  - `DuplicateTemplateNameError` (carries the conflicting name)
  - `TemplatePermissionDenied`
  - `TemplateValidationError` (carries field-level details)

  Note: Unlike categories, templates have no `TemplateInUseError` because templates are not
  referenced by FK from any table. Template deletion is always allowed (assuming the template
  exists and is not already soft-deleted).

---

## Layer 2 -- Application

- [ ] 3. Define `TemplateRepository` interface (port) -- `design.md#Components`, `design.md#Data Model`
  - Methods:
    - `find_by_id(project_id, template_id) -> Option<TestCaseTemplate>`
    - `find_by_name_in_project(project_id, name) -> Option<TestCaseTemplate>`
    - `find_by_project(project_id, page, limit, search, sort) -> (Vec<TestCaseTemplate>, u64 total)`
    - `find_all_active_by_project(project_id) -> Vec<TemplateSelectItem>`
    - `save(template) -> TestCaseTemplate`
    - `update(template) -> TestCaseTemplate`
    - `soft_delete(template_id, deleted_by)`
  - All methods accept a transaction context for transactional composition
  - Return domain entities (not raw DB rows)
  - `TemplateSelectItem` is a lightweight DTO with `id`, `name`, and `description`
  - No `count_referencing_test_cases` method (unlike categories; templates have no FK refs)

- [ ] 4. Define command/query/response DTOs -- `design.md#API Contract`
  - `CreateTemplateCommand` (name, description)
  - `UpdateTemplateCommand` (name?, description?)
  - `ListTemplatesQuery` (project_id, page, limit, search?, sort?)
  - `TemplateListResponse` (id, name, description, created_by, created_at, updated_at)
    -- used in list endpoints; excludes `project_id` (known from URL) and `updated_by`
  - `TemplateDetailResponse` (id, name, description, project_id, created_by, created_at,
    updated_by, updated_at) -- full detail view
  - `TemplateSelectItem` (id, name, description)
    -- includes description so UI can show preview/snippet on hover or selection
  - `PaginationMeta` (total, page, limit) -- reuse from shared utilities if available

- [ ] 5. Implement `TemplateService` -- `requirements.md#US-1` through `US-6`,
      `design.md#Sequence`
  - `create_template(project_id, cmd, current_user_id)`:
    validates `template:create` permission, checks Owner/Editor role, begins DB
    transaction, checks duplicate name within transaction, constructs entity, saves via
    repository, commits transaction, returns `TemplateDetailResponse`
  - `list_templates(project_id, query, current_user_id)`:
    validates `template:read_list` permission, checks project membership (any role),
    delegates to repository with pagination/search/sort, returns paginated response
  - `get_template(project_id, template_id, current_user_id)`:
    validates `template:read` permission, checks project membership, fetches by ID,
    verifies belongs to project, returns `TemplateDetailResponse`
  - `update_template(project_id, template_id, cmd, current_user_id)`:
    validates `template:update` permission, checks Owner/Editor role, fetches template,
    begins DB transaction, checks duplicate name within transaction if name is being
    changed, applies updates, saves via repository, commits transaction,
    returns `TemplateDetailResponse`
  - `delete_template(project_id, template_id, current_user_id)`:
    validates `template:delete` permission, checks Owner/Editor role, fetches template,
    begins transaction, soft-deletes (no referential-integrity check needed),
    commits transaction
  - `select_templates(project_id, current_user_id)`:
    validates `template:select` permission, checks project membership (any role),
    delegates to `find_all_active_by_project`, returns flat list of `TemplateSelectItem`
  - All methods check system permission via `AuthorizationService`
  - All methods check project membership/role via `ProjectMemberRepository`

- [ ] 6. Write unit tests for `TemplateService` -- `requirements.md#US-1` through `US-6`
  - Table-driven tests with mock `TemplateRepository` and mock `ProjectMemberRepository`
  - Happy path: create, list (paginated), get, update, delete, select
  - Duplicate name rejection (create and update)
  - Permission denial for each permission code
  - Project role denial: Contributor/Viewer cannot create/update/delete
  - Project role acceptance: any member can read/select
  - Template not found returns error
  - Update on soft-deleted template returns error
  - Soft-delete already-deleted template returns error
  - Soft-delete succeeds without referential-integrity check (no "in use" gate)
  - Search and sort parameters passed through to repository correctly
  - System Admin bypasses project membership checks
  - Boundary values: name at exactly 255 chars (passes), 256 chars (rejects), empty string,
    single char, whitespace-only string (rejects), leading/trailing whitespace trimmed
  - Description: exactly 10000 chars (passes), 10001 chars (rejects), "" (stored as empty
    string), null (clears/sets NULL), omitted (preserves current on update)
  - Unicode: template name with multi-byte UTF-8 characters (Japanese, emoji) --
    LOWER() comparison and length checks work correctly
  - Transaction rollback: duplicate name after concurrent insert returns clean 409

---

## Layer 3 -- Adapters (HTTP)

- [ ] 7. Implement `TemplateHandler` -- `design.md#API Contract`, `design.md#Components`
  - Six handler methods: `list`, `create`, `get`, `update`, `delete`, `select`
  - Deserialize request bodies and query params into DTOs
  - Validate pagination: `page` in [1, 1000], `limit` in [1, 100]; return `422` otherwise
  - Validate `sort` against whitelist (`name`, `-name`, `created_at`, `-created_at`);
    return `422` for invalid values
  - Validate `search` does not exceed 255 characters; return `422` otherwise
  - Validate project exists and is not soft-deleted before any template operation
    (call `ProjectRepository::find_by_id` or a shared project validation guard)
  - Strip HTML tags from `name` and `description` fields before passing to service
    (XSS input sanitization at the boundary)
  - Call `TemplateService` methods
  - Serialize responses with proper status codes:
    - `201 Created` with `Location` header for create
    - `200 OK` for list, get, update, select
    - `204 No Content` for delete
  - Map domain/service errors to HTTP status codes and standard error body format
  - Reject unrecognised fields in request bodies with `422` (strict mode)

- [ ] 8. Register template routes in HTTP router -- `design.md#Route Registration`
  - `GET    /api/v1/projects/{projectId}/templates`          → `list`
  - `POST   /api/v1/projects/{projectId}/templates`          → `create`
  - `GET    /api/v1/projects/{projectId}/templates/select`   → `select`  (static path)
  - `GET    /api/v1/projects/{projectId}/templates/{id}`     → `get`     (dynamic path)
  - `PATCH  /api/v1/projects/{projectId}/templates/{id}`     → `update`
  - `DELETE /api/v1/projects/{projectId}/templates/{id}`     → `delete`
  - All routes require session auth middleware
  - Route order: `/select` **must** be registered before `/{id}` to prevent the
    word "select" from being interpreted as a template ID

- [ ] 9. Write integration tests for template HTTP handlers -- `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back)
  - Test `201 Created` with `Location` header
  - Test `200 OK` for list with pagination metadata
  - Test search filter (query parameter applied correctly)
  - Test sort parameter (`name`, `-name`, `created_at`, `-created_at`)
  - Test `200 OK` for select (flat list with id, name, description)
  - Test `409 Conflict` on duplicate name (create and update)
  - Test `422 Unprocessable Entity` on validation errors
  - Test `403 Forbidden` on missing permission (each endpoint)
  - Test `403 Forbidden` on insufficient project role (Contributor trying to create)
  - Test `404 Not Found` on non-existent project
  - Test `404 Not Found` on non-existent/soft-deleted template
  - Test `404 Not Found` on template belonging to a different project
  - Test `204 No Content` on successful soft-delete
  - Test `404 Not Found` on repeated soft-delete
  - Test that soft-delete always succeeds (no "in use" rejection -- unlike categories)
  - Test that System Admin can perform all operations on any project
  - Test that `/select` route is not shadowed by `/{id}` (GET with "select" as ID
    hits the select handler, not the get handler)
  - Test edge cases: name with only whitespace rejected, name exactly 255 chars accepted,
    name at 256 chars rejected, description exactly 10000 chars accepted, description at
    10001 chars rejected
  - Test PATCH semantics: omit description (preserves), send "" (stores empty string),
    send null (clears to NULL)
  - Test `search` with ILIKE wildcards `%` and `_` (treated as literals, not wildcards)
  - Test invalid `sort` values return `422`
  - Test `search` exceeding 255 characters returns `422`
  - Test `page` > 1000 returns `422`
  - Test unrecognised fields in request body return `422`
  - Test all six endpoints return `401 NOT_AUTHENTICATED` when no session cookie is present
  - Test all six endpoints return `401` when session cookie is expired or invalid
  - Test pagination boundaries: `page=0` returns `422`, `page=-1` returns `422`,
    `limit=0` returns `422`, `limit=-1` returns `422`
  - Test `search` with literal backslash (e.g., `C:\path\to\file`) -- treated as literal,
    no SQL error
  - Test Unicode template name (e.g., Japanese, emoji) round-trips correctly
  - Test case-only name change on update (e.g., "Login Test" to "LOGIN TEST") succeeds
  - Test that `403` response bodies use identical messages for "missing permission" and
    "wrong project role" (no information leakage)
  - Test Unicode template name (e.g., Japanese text) round-trips correctly
  - Test that `403` response bodies do not distinguish between "missing permission" and
    "wrong project role" (identical message for both failure modes)
  - Test that select response includes `description` field (for dropdown preview)

---

## Layer 4 -- Infrastructure

- [x] 10. Create `TEST_CASE_TEMPLATES` database migration -- `design.md#Data Model`
  - Table definition with all columns, PK, FKs, constraints
  - `CHECK` constraint on `description`: `description IS NULL OR char_length(description) <= 10000`
  - Partial unique index `uq_test_case_templates_name_project` on `(project_id, LOWER(name))`
    with `WHERE deleted_at IS NULL`
  - Foreign key indexes on `project_id`, `created_by`, `updated_by`, `deleted_by`
  - Partial index `idx_test_case_templates_active` on `(project_id, LOWER(name))`
    with `WHERE deleted_at IS NULL`
  - `BEFORE UPDATE` trigger `trg_test_case_templates_updated_at` that sets `NEW.updated_at = NOW()`
  - Rollback migration: `DROP TRIGGER IF EXISTS`, `DROP FUNCTION IF EXISTS`, `DROP TABLE IF EXISTS`

- [ ] 11. Add template permission codes to the permissions seed migration --
       `design.md#New Permission Codes`
  - Add 6 new rows to the permissions seed data with `INSERT ... ON CONFLICT (code) DO NOTHING`:
    `('template:create', 'Create Template')`,
    `('template:read', 'Read Template')`,
    `('template:read_list', 'Read Template List')`,
    `('template:update', 'Update Template')`,
    `('template:delete', 'Delete Template')`,
    `('template:select', 'Select Template')`
  - Let the database auto-assign IDs (do NOT hardcode numeric IDs -- they will collide
    with other features' permission seeds)
  - If permissions are seeded in a single migration file, append to that file
  - Verify idempotency: `ON CONFLICT (code) DO NOTHING` so the migration is safe to re-run

- [ ] 12. Implement `SqlTemplateRepository` -- `design.md#Components`
  - All methods from `TemplateRepository` interface
  - Every SELECT includes `WHERE deleted_at IS NULL` for active record filtering
  - `find_by_name_in_project` uses `LOWER(name) = LOWER($1) AND project_id = $2 AND deleted_at IS NULL`
  - `find_by_project` builds a dynamic query for search and sort. Escape `\`, `%`, and
    `_` in the search value in that order (double backslashes first, then escape `%` and
    `_` with backslash prefix) before building the ILIKE pattern. Validate `sort` against
    a whitelist of allowed column names (`name`, `created_at`) before building the
    `ORDER BY` clause -- never interpolate user input directly into SQL
  - `find_all_active_by_project` selects `id`, `name`, and `description`, ordered by `LOWER(name)`
  - `save` inserts and returns the new row with generated `id` and timestamps
  - `update` updates `name`, `description`, `updated_by`, `updated_at` (via trigger)
    for the given `id`, but only if `deleted_at IS NULL`
  - `soft_delete` sets `deleted_at = NOW()`, `deleted_by = $2` WHERE `id = $1 AND deleted_at IS NULL`
  - No `count_referencing_test_cases` method needed (unlike categories)
  - Use parameterized queries exclusively -- never string interpolation
  - Write unit tests with a test database (one test transaction per case)

- [ ] 13. Implement template seeding in `ConfigFileSeeder` -- `requirements.md#overview`,
       `design.md#YAML Config Seed Format`
  - Add `seed_templates(project_id, created_by, tx)` method
  - Read the `templates` list from the parsed YAML config
  - For each entry with a non-empty `name`, insert into `TEST_CASE_TEMPLATES` within the
    caller's transaction
  - Skip entries with missing or empty `name` with a warning log
  - Skip duplicate names within the same config file (first occurrence wins) with a
    warning log
  - Handle missing `templates` section gracefully (no-op, not an error)
  - Handle empty `templates` list gracefully (no-op)
  - Write integration test: seed templates into a test database and verify all rows
    are present with correct `project_id` and `created_by`

- [ ] 14. Wire authorization for template permissions -- `design.md#Components`
  - Register the 6 new `template:*` permission codes in the permission registry
  - Extend the project-scope authorization guard to accept required project roles:
    - `template:create`  → Owner or Editor
    - `template:read`    → any project member (all 4 roles)
    - `template:read_list` → any project member
    - `template:update`  → Owner or Editor
    - `template:delete`  → Owner or Editor
    - `template:select`  → any project member
  - System Admin bypasses all role checks
  - Write unit tests: verify each role is correctly accepted/rejected for each
    permission code

---

## Verification and Cleanup

- [ ] 15. End-to-end verification -- `requirements.md#US-1` through `US-6`
  - Manual or automated walkthrough of all six user stories
  - Verify create template with valid data returns `201` with correct fields
  - Verify duplicate name (case-insensitive) returns `409` within same project
  - Verify same name in different projects is allowed
  - Verify same name as soft-deleted template is allowed (create succeeds)
  - Verify list returns only non-deleted templates for the project
  - Verify pagination: page 1 with limit 5 returns correct slice and total count
  - Verify search filters correctly on name and description
  - Verify sort: `name`, `-name`, `created_at`, `-created_at`
  - Verify get returns full detail including audit timestamps and full description body
  - Verify update changes name and/or description, updates `updated_at`/`updated_by`
  - Verify update on non-existent/soft-deleted template returns `404`
  - Verify soft-delete sets `deleted_at`/`deleted_by` and returns `204`
  - Verify soft-delete succeeds unconditionally (no in-use gate)
  - Verify soft-delete on already-deleted template returns `404`
  - Verify select returns flat list of `{id, name, description}` sorted by name
  - Verify select excludes soft-deleted templates
  - Verify `403` for each endpoint with insufficient permission
  - Verify `403` for Contributor on create/update/delete
  - Verify Contributor can read/select
  - Verify Viewer can read/select (cannot modify)
  - Verify System Admin can perform all operations on any project
  - Verify `Location` header on `201 Created`
  - Verify templates seeded from YAML config on project creation (end-to-end with
    project creation flow)

- [ ] 16. Update `specs/README.md` -- mark `metadata-templates` as having completed
       specs (requirements.md, design.md, tasks.md)

---

## Security and Hardening

- [ ] 17. **Implement XSS input sanitization** --
       `requirements.md#security-considerations`
  - Strip disallowed HTML tags from `name` and `description` fields before storage,
    at the HTTP handler boundary (in both `create` and `update` handlers)
  - Apply consistent sanitization rules across all string fields
  - Integration test: create a template with `<script>alert('xss')</script>` in the
    name, verify the stored value has HTML tags stripped

- [ ] 18. **Implement CSRF protection** --
       `requirements.md#security-considerations`
  - Ensure session cookie carries `SameSite=Lax` (or stricter)
  - Ensure `POST`/`PATCH`/`DELETE` endpoints reject requests without
    `Content-Type: application/json` (to block simple form-based CSRF attacks)
  - Integration test: verify `POST /api/v1/projects/{pid}/templates` without JSON
    content type is rejected

- [ ] 19. **Add authorization integration tests** --
       `design.md#security-requirements`
  - Test that `POST /api/v1/projects/{pid}/templates` returns `403` for
    authenticated non-member with `template:create` permission
  - Test that `PATCH /api/v1/projects/{pid}/templates/{id}` returns `403` for
    project member with Viewer role
  - Test that `DELETE /api/v1/projects/{pid}/templates/{id}` returns `403` for
    project member with Contributor role
  - Test that System Admin can create/read/update/delete templates on any project
    regardless of membership
  - Test that `403` responses use a generic message (no distinction between
    "missing permission" and "wrong project role")

- [ ] 20. **Verify SQL injection resistance** --
       `design.md#Components`
  - Code review: confirm all SQL in `SqlTemplateRepository` uses parameterized
    queries (`$1`, `$2`, ...), never string interpolation
  - Code review: confirm the `sort` parameter is validated against a whitelist
    before being used in `ORDER BY` (since PostgreSQL does not support parameterized
    `ORDER BY`)
  - Integration test: attempt SQL injection via the `search` query parameter
    (e.g., `' OR '1'='1`) and verify no data leakage
  - Integration test: attempt SQL injection via the `sort` query parameter and
    verify the input is rejected or sanitized
