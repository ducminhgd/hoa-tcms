# Tasks: Project CRUD

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 — Domain

- [ ] 1. Implement `Project` entity — `requirements.md#US-1`, `design.md#Components`
  - Fields: `id`, `name`, `description`, `status`, audit columns
  - Factory method `Project::create(name, description, status, created_by)` with
    domain validation (name not empty, valid status enum)
  - Implement `Status` enum with `Active` and `Inactive` variants
  - No framework imports; pure Rust struct + impl

- [ ] 2. Implement `ProjectMember` value object — `requirements.md#US-1`, `design.md#Components`
  - Fields: `user_id`, `project_id`, `role`
  - Implement `MemberRole` enum with `Owner`, `Editor`, `Contributor`, `Viewer`
  - No ORM / framework imports

- [ ] 3. Define domain exceptions for projects — `design.md#Error Handling`
  - `ProjectNotFoundError`
  - `DuplicateProjectNameError`
  - `ProjectPermissionDenied`
  - `ProjectValidationError` (carries field-level details)

---

## Layer 2 — Application

- [ ] 4. Define `ProjectRepository` interface (port) — `design.md#Components`, `design.md#Data Model`
  - Methods: `find_by_id`, `find_by_name` (case-insensitive), `save`, `update`,
    `soft_delete`, `find_by_user_id` (paginated, scoped to active projects where
    user is a member), `add_member`, `get_members`, `get_member_count`
  - All methods accept `&self` and `&mut Transaction` or equivalent for
    transactional composition
  - Return domain entities (not DTOs)

- [ ] 5. Define `MetadataSeeder` interface (port) — `design.md#Components`, `requirements.md#US-1`
  - Method: `async fn seed(&self, project_id: i64, tx: &mut Transaction) -> Result<()>`
  - Called within the create-project transaction
  - Implementation responsibility is in Infrastructure

- [ ] 6. Define `ProjectMemberRepository` interface (port) — `design.md#Components`
  - Methods: `add_member`, `get_members_by_project`
  - Initially minimal; will be expanded in `project-members` spec

- [ ] 7. Implement `ProjectService` — `requirements.md#US-1` through `US-5`,
      `design.md#Sequence`
  - `create_project(cmd: CreateProjectCommand)`: validates permission, checks
    duplicate name, runs transactional create + seed + member insert
  - `list_projects(query: ListProjectsQuery)`: paginated, filtered, scoped to user
  - `get_project(project_id, current_user_id)`: returns project + members
  - `update_project(project_id, cmd: UpdateProjectCommand)`: validates permission,
    checks duplicate name on change, updates fields
  - `delete_project(project_id, current_user_id)`: soft-delete with `deleted_by`
  - All methods check the required system permission via `AuthorizationService`

- [ ] 8. Define command/query DTOs — `design.md#API Contract`
  - `CreateProjectCommand` (name, description, status)
  - `UpdateProjectCommand` (name?, description?, status?)
  - `ListProjectsQuery` (page, limit, status?)
  - `ProjectResponse` (id, name, description, status, member_count, timestamps)
  - `ProjectDetailResponse` (includes members list)
  - `PaginationMeta` (total, page, limit)

- [ ] 9. Write unit tests for `ProjectService` — `requirements.md#US-1` through `US-5`
  - Table-driven tests: happy path for each use case
  - Duplicate name rejection
  - Permission denial (each permission code)
  - Soft-delete already-deleted project returns 404
  - Update on soft-deleted project returns 404 (FR-54c)
  - Inactive status hides children (verify flag is set on entity)

---

## Layer 3 — Adapters (HTTP)

- [ ] 10. Implement `ProjectHandler` — `design.md#API Contract`, `design.md#Components`
  - Five handler methods: `list`, `create`, `get`, `update`, `delete`
  - Deserialize request bodies and query params into DTOs
  - Call `ProjectService` methods
  - Serialize responses with proper status codes
  - Set `Location` header on `201 Created`

- [ ] 11. Register project routes in HTTP router — `design.md#Components`
  - `GET    /api/v1/projects`          → `list`
  - `POST   /api/v1/projects`          → `create`
  - `GET    /api/v1/projects/{id}`     → `get`
  - `PATCH  /api/v1/projects/{id}`     → `update`
  - `DELETE /api/v1/projects/{id}`     → `delete`
  - All routes require session auth middleware
  - Route order: static before dynamic to avoid conflicts

- [ ] 12. Write integration tests for project HTTP handlers — `design.md#API Contract`
  - Full request/response cycle with a test database
  - Test `201 Created` with Location header
  - Test `409 Conflict` on duplicate name
  - Test `422 Unprocessable Entity` on validation errors
  - Test `403 Forbidden` on missing permission
  - Test `404 Not Found` on non-existent/soft-deleted project
  - Test `204 No Content` on soft-delete
  - Test pagination query parameters
  - Test status filter on list endpoint

---

## Layer 4 — Infrastructure

> **Note:** The `PROJECT_MEMBERS` table migration is owned by the
> `project-members` feature. This feature only references the table;
> the migration is defined in `specs/project-members/tasks.md`.

- [ ] 13. Create `PROJECTS` database migration — `design.md#Data Model`
  - Table definition with all columns, PK, FKs, check constraints
  - Case-insensitive unique index on `LOWER(name)`
  - Partial index for active projects: `WHERE deleted_at IS NULL`
  - FK indexes on `created_by`, `updated_by`, `deleted_by`
  - Rollback migration: `DROP TABLE IF EXISTS projects`

- [ ] 14. Implement `SqlProjectRepository` — `design.md#Components`
  - All methods from `ProjectRepository` interface
  - Every SELECT includes `WHERE deleted_at IS NULL` for active record filtering
  - `save`, `update`, `soft_delete` execute within the passed transaction
  - `find_by_name` uses `LOWER(name) = LOWER($1)` for case-insensitive matching
  - `find_by_user_id` joins through `project_members` and applies pagination
  - Write unit tests with a mock DB or test transaction

- [ ] 15. Implement `ConfigFileSeeder` — `requirements.md#US-1`, `design.md#Components`
  - Load YAML config file at application startup (path from env var)
  - Parse config sections: categories, templates, priorities, plan_types
  - `seed(project_id, tx)` inserts default records into per-project metadata tables
    (test_categories, test_case_templates, priorities, plan_types)
  - Handle missing config file gracefully (log warning, seed nothing)
  - Handle malformed YAML with a clear error

- [ ] 16. Write integration tests for `ConfigFileSeeder` — `design.md#Components`
  - Test seeding with a valid YAML config
  - Test seeding with missing config file (should warn, not fail)
  - Test seeding with malformed YAML (should return error)
  - Verify all four metadata types are inserted with correct project_id
  - Verify transactional rollback on failure

- [ ] 17. Wire `AuthorizationService` project-permission check — `design.md#Components`
  - Implement `check_project_permission(user_id, project_id, permission_code, required_role)`
  - Verifies: user has the system permission AND user is a project member with the
    required role (Owner for delete, Owner/Editor for update)
  - System Admin bypasses the check
  - Write unit tests for each role/permission combination

---

## Verification & Cleanup

- [ ] 18. End-to-end verification — `requirements.md#US-1` through `US-5`
  - Manual or automated walkthrough of all five user stories
  - Verify auto-seeding creates all four metadata types
  - Verify soft-delete hides project from list/detail but preserves data
  - Verify child objects are hidden when parent is soft-deleted (no cascade)
  - Verify deactivated project hides children but preserves references
  - Verify duplicate name rejection (case-insensitive)
  - Verify permission enforcement (each endpoint with insufficient permission)
  - Verify `Location` header on `201 Created`

- [ ] 19. Update `specs/README.md` — `specs/README.md`
  - Mark `project-crud` as having completed specs (requirements.md, design.md, tasks.md)

---

## Security & Hardening

- [ ] 20. **Implement XSS input sanitization** —
       requirements.md#security-considerations
        - Strip disallowed HTML tags from `name` and `description` fields before
          storage, at the HTTP handler boundary.
        - Apply consistent sanitization in both `create_project` and
          `update_project` handlers.
        - Integration test: create a project with `<script>alert('xss')</script>`
          in the name, verify the stored value has HTML tags stripped.

- [ ] 21. **Implement CSRF protection** —
       requirements.md#security-considerations
        - Ensure session cookie carries `SameSite=Lax` (or stricter).
        - Ensure `POST`/`PATCH`/`DELETE` endpoints reject requests without
          `Content-Type: application/json` (to block simple form-based CSRF).
        - Integration test: verify `POST /api/v1/projects` without JSON content
          type is rejected.

- [ ] 22. **Add authorization integration tests** —
       design.md#security-requirements
        - Test that `GET /api/v1/projects/{id}` returns `403` for authenticated
          non-member with `project:read` permission.
        - Test that `PATCH /api/v1/projects/{id}` returns `403` for project
          member with Viewer role (not Owner/Editor).
        - Test that `DELETE /api/v1/projects/{id}` returns `403` for project
          Editor (not Owner).
        - Test that System Admin can read/update/delete any project regardless
          of membership.
