# Tasks: IAM Groups

> **Dependency note:** This feature depends on the `iam-users` feature for the `USERS` table
> schema and `UserRepository`, and on the `iam-roles` feature for the `ROLES` table.
> The `iam-auth` auth middleware and the cross-cutting authorization middleware must be
> at least partially complete before tasks 11+ can begin.
>
> The `groups`, `user_groups`, and `group_roles` migrations (tasks 1-3) can be created
> independently of other features.

---

## Database Migrations

- [ ] 1. **Create `groups` table migration** — design.md#data-model
      - Create the `groups` table with all columns: `id` (BIGINT GENERATED ALWAYS AS IDENTITY
        PK), `name` (VARCHAR(255) NOT NULL UNIQUE), `description` (TEXT), `status` (VARCHAR(20)
        NOT NULL DEFAULT 'ACTIVE' with CHECK constraint for ACTIVE/INACTIVE), `created_by`,
        `created_at`, `updated_by`, `updated_at`, `deleted_by`, `deleted_at`.
      - Foreign keys: `created_by`, `updated_by`, `deleted_by` reference `users(id)`.
      - `deleted_by` is nullable; `created_by` and `updated_by` are NOT NULL.
      - Add partial index: `idx_groups_status ON groups(status) WHERE deleted_at IS NULL`.
      - Add index: `idx_groups_deleted_at ON groups(deleted_at)`.

- [ ] 2. **Create `user_groups` junction table migration** — design.md#data-model
      - Create the `user_groups` table with composite PK `(user_id, group_id)`.
      - Columns: `user_id` (BIGINT NOT NULL FK → users(id)), `group_id` (BIGINT NOT NULL
        FK → groups(id)), `created_at` (TIMESTAMPTZ NOT NULL DEFAULT NOW()).
      - Add index: `idx_user_groups_group_id ON user_groups(group_id)`.
      - No soft-delete columns (per junction table rules — hard delete only).

- [ ] 3. **GROUP_ROLES junction table** — owned by `iam-roles` feature. No migration
      needed here. See `iam-roles/tasks.md` for the `group_roles` table migration.
      The `PostgresGroupRoleRepository` (task 13) queries this table.

## Domain Layer

- [ ] 4. **Define `GroupStatus` value object** — design.md#architecture
      - Enum with variants: `Active`, `Inactive`.
      - Implement `Display` (→ `"ACTIVE"` / `"INACTIVE"`).
      - Implement `FromStr` for parsing from string.
      - Implement `Serialize`/`Deserialize` for JSON serialization.
      - Implement validation: reject any value other than `ACTIVE`/`INACTIVE`
        (case-insensitive).

- [ ] 5. **Define `Group` entity** — design.md#components
      - Fields: `id` (i64, Option for new groups), `name` (String), `description`
        (Option<String>), `status` (GroupStatus), `created_by` (i64), `created_at`
        (DateTime<Utc>), `updated_by` (i64), `updated_at` (DateTime<Utc>),
        `deleted_by` (Option<i64>), `deleted_at` (Option<DateTime<Utc>>).
      - Constructor: `Group::new(name, description, created_by) -> Self` — sets status to
        `Active`, `updated_at = created_at = NOW()`, `updated_by = created_by`.
      - Method: `update(name, description, status) -> &mut Self` — validates status value.
      - Method: `soft_delete(deleted_by: i64)` — sets `deleted_at` and `deleted_by`.
      - Method: `is_deleted() -> bool` — checks `deleted_at.is_some()`.
      - Method: `is_active() -> bool` — checks `status == GroupStatus::Active`.

## Application Layer

- [ ] 6. **Define `GroupRepository` interface (port)** — design.md#components
      - `find_by_id(id: i64) -> Result<Option<Group>>`
      - `find_by_name(name: &str) -> Result<Option<Group>>` — case-insensitive lookup
      - `save(group: &Group) -> Result<Group>` — INSERT, return with generated id
      - `update(group: &Group) -> Result<Group>` — UPDATE, rely on DB trigger for
        `updated_at`/`updated_by`
      - `soft_delete(id: i64, deleted_by: i64) -> Result<()>` — sets `deleted_at`/`deleted_by`
      - `find_all(pagination: Pagination, status_filter: Option<GroupStatus>) -> Result<PaginatedResult<Group>>` — paginated list, exclude soft-deleted, optional status filter

- [ ] 7. **Define `MemberRepository` interface (port)** — design.md#components
      - `add(group_id: i64, user_id: i64) -> Result<()>` — INSERT INTO user_groups,
        skip on conflict (ON CONFLICT DO NOTHING)
      - `remove(group_id: i64, user_id: i64) -> Result<()>` — DELETE FROM user_groups
      - `find_by_group(group_id: i64) -> Result<Vec<MemberInfo>>` — list members with
        user details (user_id, username, fullname) for a group
      - `count_by_group(group_id: i64) -> Result<i64>` — count members in a group
      - `validate_users_exist(user_ids: &[i64]) -> Result<Vec<i64>>` — returns the subset
        of user IDs that actually exist (to validate add request)

- [ ] 8. **Define `GroupRoleRepository` interface (port)** — design.md#components
      - `add(group_id: i64, role_id: i64) -> Result<()>` — INSERT INTO group_roles,
        skip on conflict (ON CONFLICT DO NOTHING)
      - `remove(group_id: i64, role_id: i64) -> Result<()>` — DELETE FROM group_roles
      - `find_by_group(group_id: i64) -> Result<Vec<RoleInfo>>` — list roles with
        role details (role_id, name) for a group
      - `count_by_group(group_id: i64) -> Result<i64>` — count roles in a group
      - `validate_roles_exist(role_ids: &[i64]) -> Result<Vec<i64>>` — returns the subset
        of role IDs that actually exist

- [ ] 9. **Define `Pagination` and `PaginatedResult` DTOs** — design.md#api-contract
      - `Pagination`: `page` (i64), `limit` (i64, max 100), `sort` (String),
        `order` (SortOrder enum)
      - `PaginatedResult<T>`: `items: Vec<T>`, `total: i64`, `page: i64`, `limit: i64`
      - `SortOrder`: enum `Asc`/`Desc` with `FromStr` and `Display`

- [ ] 10. **Implement `GroupService`** — design.md#sequence, requirements.md#US-01-08
       - `list_groups(pagination, status_filter) -> Result<PaginatedResult<GroupListItemDTO>>`
         — US-01: calls `GroupRepository::find_all()`, maps to list DTOs with member_count
       - `create_group(name, description, current_user_id) -> Result<GroupDTO>`
         — US-02: checks name uniqueness, creates Group entity, sanitises `name` and
         `description` (strip HTML tags for XSS prevention), calls `GroupRepository::save()`
       - `get_group(id) -> Result<GroupDetailDTO>` — US-03: loads group, members, roles,
         assembles detail DTO
       - `update_group(id, name, description, status, current_user_id) -> Result<GroupDTO>`
         — US-04: loads group, validates not deleted, checks name uniqueness if name changed,
         calls `GroupRepository::update()`
       - `delete_group(id, current_user_id) -> Result<()>` — US-07: loads group, validates
         not already deleted, calls `GroupRepository::soft_delete()`
       - `manage_members(group_id, add_ids, remove_ids) -> Result<MemberChangeResult>`
         — US-05: validates group exists and not deleted, validates user IDs exist,
         calls `MemberRepository::add()` for each add and `MemberRepository::remove()`
         for each remove
       - `manage_roles(group_id, add_ids, remove_ids) -> Result<RoleChangeResult>`
         — US-06: validates group exists and not deleted, validates role IDs exist,
         calls `GroupRoleRepository::add()` for each add and `GroupRoleRepository::remove()`
         for each remove

## Infrastructure Layer

- [ ] 11. **Implement `PostgresGroupRepository`** — design.md#components, design.md#data-model
       - Implement `GroupRepository` trait using `sqlx` or `diesel` against PostgreSQL.
       - `find_by_id`: `SELECT id, name, description, status, created_by, created_at,
         updated_by, updated_at, deleted_by, deleted_at FROM groups WHERE id = $1`
       - `find_by_name`: case-insensitive comparison using `LOWER(name) = LOWER($1)`
       - `save`: `INSERT INTO groups (name, description, status, created_by, created_at,
         updated_by, updated_at) VALUES ($1, $2, $3, $4, NOW(), $4, NOW()) RETURNING id, created_at`
       - `update`: `UPDATE groups SET name = $1, description = $2, status = $3 WHERE id = $4
         AND deleted_at IS NULL` — exclude soft-deleted rows (FR-54c)
       - `soft_delete`: `UPDATE groups SET deleted_at = NOW(), deleted_by = $1 WHERE id = $2
         AND deleted_at IS NULL`
       - `find_all`: dynamic SQL with pagination, sorting, and optional status filter.
         Exclude `deleted_at IS NOT NULL` rows. Use `LIMIT + OFFSET`.
       - Compile-time check: Rust's trait system verifies implementation when
         `impl GroupRepository for PostgresGroupRepository {}` is written.
       - All queries use parameterized placeholders (`$1`, `$2`, etc.) — never format strings.

- [ ] 12. **Implement `PostgresMemberRepository`** — design.md#components, design.md#data-model
       - `add`: `INSERT INTO user_groups (user_id, group_id, created_at) VALUES ($1, $2, NOW())
         ON CONFLICT DO NOTHING`
       - `remove`: `DELETE FROM user_groups WHERE user_id = $1 AND group_id = $2`
       - `find_by_group`: `SELECT ug.user_id, u.username, u.fullname FROM user_groups ug
         JOIN users u ON u.id = ug.user_id WHERE ug.group_id = $1 AND u.deleted_at IS NULL`
       - `count_by_group`: `SELECT COUNT(*) FROM user_groups WHERE group_id = $1`
       - `validate_users_exist`: `SELECT id FROM users WHERE id = ANY($1) AND deleted_at IS NULL`

- [ ] 13. **Implement `PostgresGroupRoleRepository`** — design.md#components, design.md#data-model
       - `add`: `INSERT INTO group_roles (group_id, role_id, created_at) VALUES ($1, $2, NOW())
         ON CONFLICT DO NOTHING`
       - `remove`: `DELETE FROM group_roles WHERE group_id = $1 AND role_id = $2`
       - `find_by_group`: `SELECT gr.role_id, r.name FROM group_roles gr
         JOIN roles r ON r.id = gr.role_id WHERE gr.group_id = $1 AND r.deleted_at IS NULL`
       - `count_by_group`: `SELECT COUNT(*) FROM group_roles WHERE group_id = $1`
       - `validate_roles_exist`: `SELECT id FROM roles WHERE id = ANY($1) AND deleted_at IS NULL`

## Adapters Layer

- [ ] 14. **Define group request/response DTOs** — design.md#api-contract
       - `CreateGroupRequest`: `name: String`, `description: Option<String>`
       - `UpdateGroupRequest`: `name: Option<String>`, `description: Option<String>`,
         `status: Option<String>` — all optional for partial update
       - `GroupResponse`: `id, name, description, status, member_count, created_at,
         created_by, updated_at, updated_by`
       - `GroupDetailResponse`: extends `GroupResponse` with `members: Vec<MemberInfo>`
         and `roles: Vec<RoleInfo>` and `role_count`
       - `MemberInfo`: `user_id, username, fullname`
       - `RoleInfo`: `role_id, name`
       - `ManageMembersRequest`: `add: Option<Vec<i64>>`, `remove: Option<Vec<i64>>`
       - `ManageMembersResponse`: `added: i64, removed: i64, member_count: i64`
       - `ManageRolesRequest`: `add: Option<Vec<i64>>`, `remove: Option<Vec<i64>>`
       - `ManageRolesResponse`: `added: i64, removed: i64, role_count: i64`
       - `PaginatedResponse<T>`: wrapping `data: Vec<T>` and `meta: PaginationMeta`
       - `PaginationMeta`: `total, page, limit`
       - `GroupListItem`: id, name, description, status, member_count, created_at, created_by,
         updated_at, updated_by (lighter than full GroupDetailResponse — excludes role_count,
         members, and roles arrays)
       - Error response structs matching the `{ "error": { "code", "message", "details" } }`
         format (reuse from `iam-auth` or shared error module).

- [ ] 15. **Implement `ListGroupsHandler` (GET /api/v1/groups)** — design.md#api-contract, requirements.md#US-01
       - Extract query parameters: `page` (default 1), `limit` (default 25, max 100),
         `sort` (default "name"), `order` (default "asc"), `status` (optional).
       - Validate parameters: reject invalid `sort` fields, invalid `order`, out-of-range
         `page`/`limit`, invalid `status` value.
       - Call `GroupService::list_groups()`.
       - Return `200 OK` with `PaginatedResponse<GroupListItem>`.
       - On validation error: return `422`.

- [ ] 16. **Implement `CreateGroupHandler` (POST /api/v1/groups)** — design.md#api-contract, requirements.md#US-02
       - Validate Content-Type is `application/json`.
       - Deserialize request body into `CreateGroupRequest`.
       - Validate: `name` is not empty and ≤ 255 chars.
       - Call `GroupService::create_group()`.
       - On success: return `201 Created` with `Location` header and `GroupResponse` body.
       - On `DuplicateName`: return `409`.
       - On validation error: return `422`.

- [ ] 17. **Implement `GetGroupHandler` (GET /api/v1/groups/{id})** — design.md#api-contract, requirements.md#US-03
       - Extract `id` from path.
       - Call `GroupService::get_group(id)`.
       - On success: return `200 OK` with `GroupDetailResponse`.
       - On `NotFound`: return `404`.

- [ ] 18. **Implement `UpdateGroupHandler` (PATCH /api/v1/groups/{id})** — design.md#api-contract, requirements.md#US-04
       - Extract `id` from path.
       - Deserialize request body into `UpdateGroupRequest` (all fields optional).
       - Call `GroupService::update_group()`.
       - On success: return `200 OK` with `GroupResponse`.
       - On `NotFound`: return `404`.
       - On `DuplicateName`: return `409`.
       - On validation error (invalid status): return `422`.

- [ ] 19. **Implement `DeleteGroupHandler` (DELETE /api/v1/groups/{id})** — design.md#api-contract, requirements.md#US-07
       - Extract `id` from path.
       - Call `GroupService::delete_group()`.
       - On success: return `204 No Content`.
       - On `NotFound`: return `404`.

- [ ] 20. **Implement `ManageMembersHandler` (POST /api/v1/groups/{id}/members)** — design.md#api-contract, requirements.md#US-05
       - Extract `group_id` from path.
       - Deserialize request body into `ManageMembersRequest`.
       - Validate: at least one of `add` or `remove` is non-empty.
       - Call `GroupService::manage_members()`.
       - On success: return `200 OK` with `ManageMembersResponse`.
       - On `NotFound`: return `404`.
       - On `UserNotFound`: return `422`.
       - On validation error (empty add and remove): return `422`.

- [ ] 21. **Implement `ManageRolesHandler` (POST /api/v1/groups/{id}/roles)** — design.md#api-contract, requirements.md#US-06
       - Extract `group_id` from path.
       - Deserialize request body into `ManageRolesRequest`.
       - Validate: at least one of `add` or `remove` is non-empty.
       - Call `GroupService::manage_roles()`.
       - On success: return `200 OK` with `ManageRolesResponse`.
       - On `NotFound`: return `404`.
       - On `RoleNotFound`: return `422`.
       - On validation error (empty add and remove): return `422`.

## Wiring

- [ ] 22. **Seed group permission codes** — design.md#components
       - Add the following permission codes to the `PERMISSIONS` seed data:
         `group:read_list`, `group:create`, `group:read`, `group:update`, `group:delete`.
       - If the `iam-roles` feature already handles permission seeding, add these to the
         existing seed migration.

- [ ] 23. **Register group routes in HTTP server** — design.md#components
       - Register all 7 group routes behind auth middleware:
         - `GET    /api/v1/groups` — `ListGroupsHandler` (`group:read_list`)
         - `POST   /api/v1/groups` — `CreateGroupHandler` (`group:create`)
         - `GET    /api/v1/groups/{id}` — `GetGroupHandler` (`group:read`)
         - `PATCH  /api/v1/groups/{id}` — `UpdateGroupHandler` (`group:update`)
         - `DELETE /api/v1/groups/{id}` — `DeleteGroupHandler` (`group:delete`)
         - `POST   /api/v1/groups/{id}/members` — `ManageMembersHandler` (`group:update`)
         - `POST   /api/v1/groups/{id}/roles` — `ManageRolesHandler` (`group:update`)
       - Wire `GroupService`, `PostgresGroupRepository`, `PostgresMemberRepository`,
         `PostgresGroupRoleRepository` into the dependency injection container.

- [ ] 24. **Update authorization query for group-inherited permissions** — design.md#data-model, requirements.md#US-08
       - In the authorization middleware or permission-loader component, add the UNION
         subquery that loads permissions from roles assigned via groups:
         ```sql
         SELECT DISTINCT p.code FROM role_permissions rp
         JOIN permissions p ON p.id = rp.permission_id
         JOIN group_roles gr ON gr.role_id = rp.role_id
         JOIN user_groups ug ON ug.group_id = gr.group_id
         WHERE ug.user_id = $1
         ```
       - The full permission set for a user is `direct_role_permissions UNION group_role_permissions`.
       - Ensure the authorization middleware loads group-inherited permissions on every
         authenticated request and merges them with directly assigned role permissions.

## Testing

- [ ] 25. **Unit test: `GroupStatus` parsing and validation** — design.md#data-model
       - `"ACTIVE"` and `"active"` parse to `Active`.
       - `"INACTIVE"` and `"inactive"` parse to `Inactive`.
       - Invalid values return error.
       - `Display` round-trip produces uppercase.

- [ ] 26. **Unit test: `Group` entity invariants** — design.md#components
       - `Group::new()` sets status to `Active`, sets updated_at = created_at, updated_by = created_by.
       - `soft_delete()` sets deleted_at and deleted_by.
       - `is_deleted()` returns true after soft_delete.
       - `is_active()` returns false when status is Inactive.
       - `update()` changes fields correctly.

- [ ] 27. **Unit test: `GroupService::create_group`** — requirements.md#US-02, design.md#sequence
       - Successful creation returns group DTO with generated id.
       - Duplicate name returns `DuplicateName` error.
       - `GroupRepository::save()` is called with correct values.

- [ ] 28. **Unit test: `GroupService::update_group`** — requirements.md#US-04, design.md#sequence
       - Partial update only modifies provided fields.
       - Updating name to a duplicate returns `DuplicateName`.
       - Updating a soft-deleted group returns `NotFound`.
       - Invalid status value propagates as validation error.

- [ ] 29. **Unit test: `GroupService::delete_group`** — requirements.md#US-07, design.md#sequence
       - Soft-delete sets `deleted_at` and `deleted_by`.
       - Deleting an already-deleted group returns `NotFound`.
       - Deleting a non-existent group returns `NotFound`.

- [ ] 30. **Unit test: `GroupService::manage_members`** — requirements.md#US-05, design.md#sequence
       - Adding users calls `MemberRepository::add()` for each.
       - Removing users calls `MemberRepository::remove()` for each.
       - Adding an already-member user is idempotent (ON CONFLICT DO NOTHING).
       - Removing a non-member user is idempotent (no-op).
       - Non-existent user IDs return `UserNotFound` error.
       - Modifying a soft-deleted group returns `NotFound`.

- [ ] 31. **Unit test: `GroupService::manage_roles`** — requirements.md#US-06, design.md#sequence
       - Assigning roles calls `GroupRoleRepository::add()` for each.
       - Removing roles calls `GroupRoleRepository::remove()` for each.
       - Assigning an already-assigned role is idempotent.
       - Removing a non-assigned role is idempotent.
       - Non-existent role IDs return `RoleNotFound` error.

- [ ] 32. **Unit test: `GroupService::list_groups`** — requirements.md#US-01
       - Paginated list returns correct page.
       - Sorting by name, id, created_at.
       - Filtering by status returns only matching groups.
       - Soft-deleted groups are excluded.

- [ ] 33. **Unit test: `GroupService::get_group`** — requirements.md#US-03
       - Returns group detail with members and roles.
       - Soft-deleted group returns `NotFound`.
       - Non-existent group returns `NotFound`.

- [ ] 34. **Integration test: Group CRUD endpoints with real PostgreSQL** — requirements.md#US-01-04, US-07, design.md#api-contract
       - `POST /api/v1/groups` creates a group and returns `201` with `Location` header.
       - `GET /api/v1/groups` returns paginated list.
       - `GET /api/v1/groups/{id}` returns group detail with members and roles.
       - `PATCH /api/v1/groups/{id}` updates name/description/status.
       - `DELETE /api/v1/groups/{id}` soft-deletes and subsequent `GET` returns `404`.
       - `POST /api/v1/groups` with duplicate name returns `409`.
       - `PATCH /api/v1/groups/{id}` with duplicate name returns `409`.
       - `PATCH /api/v1/groups/{id}` with invalid status returns `422`.
       - Unauthenticated requests return `401`.
       - Requests without required permission return `403`.

- [ ] 35. **Integration test: Member management endpoints** — requirements.md#US-05, design.md#api-contract
       - `POST /api/v1/groups/{id}/members` adds members and returns correct count.
       - `POST /api/v1/groups/{id}/members` removes members and returns correct count.
       - Adding an already-member user is idempotent.
       - Removing a non-member user is idempotent.
       - Adding non-existent user IDs returns `422`.
       - Both `add` and `remove` in same request work correctly.
       - `GET /api/v1/groups/{id}` reflects updated member list.

- [ ] 36. **Integration test: Role management endpoints** — requirements.md#US-06, design.md#api-contract
       - `POST /api/v1/groups/{id}/roles` assigns roles and returns correct count.
       - `POST /api/v1/groups/{id}/roles` removes roles and returns correct count.
       - Adding an already-assigned role is idempotent.
       - Assigning non-existent role IDs returns `422`.
       - `GET /api/v1/groups/{id}` reflects updated role list.

- [ ] 37. **Integration test: Permission inheritance from groups** — requirements.md#US-08, design.md#data-model
       - User who is a member of a group inherits permissions from roles assigned to that group.
       - User who belongs to multiple groups inherits union of all group role permissions.
       - Removing a user from a group removes inherited permissions on next request.
       - Removing a role from a group removes inherited permissions from group members.
       - Direct role permissions and group role permissions are merged without duplication (union).
