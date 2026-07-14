# Tasks: IAM Roles

> **Dependency chain:** Permission catalog must be seeded before roles can reference permissions.
> Database migrations must exist before repository code compiles.

## Migration & Seeding

- [ ] 1. Create migration: `ROLES` table with all audit columns (created_by, updated_by nullable
  — the System Admin role is seeded by migration before any user exists, so both must be NULLable)
  and case-insensitive unique index on `name`. — design.md#Data-Model
- [ ] 2. Create migration: `ROLE_PERMISSIONS` junction table with composite PK and FKs. —
  design.md#Data-Model
- [ ] 3. Create migration: `USER_ROLES` junction table with composite PK and FKs. —
  design.md#Data-Model
- [ ] 4. Create migration: `GROUP_ROLES` junction table with composite PK and FKs. —
  design.md#Data-Model
- [ ] 5. Create seed migration: Insert the "System Admin" role with `created_by = NULL`,
  `updated_by = NULL` (no user exists yet at migration time) and grant it all seeded
  permissions via `ROLE_PERMISSIONS`. — design.md#Data-Model, requirements.md#US-5
- [ ] 6. Write rollback migration for every forward migration (tasks 1-5). —
  design.md#Data-Model

## Domain Layer

- [ ] 7. Implement `Role` domain entity with fields: `id` (i64), `name` (String),
  `created_by` (Option<i64>), `created_at` (DateTime<Utc>), `updated_by` (Option<i64>),
  `updated_at` (DateTime<Utc>), `deleted_by` (Option<i64>), `deleted_at` (Option<DateTime<Utc>>).
  Add `is_protected()` method that returns `true` when `name == "System Admin"`.
  — design.md#Data-Model, requirements.md#US-5
- [ ] 8. Implement `RoleError` typed domain errors: `RoleNotFoundError`,
  `DuplicateRoleNameError`, `ProtectedRoleError`, `InvalidPermissionIdsError`. —
  design.md#Error-Handling

## Application Layer

- [ ] 9. Define `RoleRepository` interface with methods: `find_all(pagination)`, `find_by_id(id)`,
  `find_by_name(name)`, `save(role)`, `update(role)`, `set_permissions(role_id, permission_ids)`,
  `get_permissions(role_id)`. — design.md#Architecture
- [ ] 10. Define `RoleService` with methods: `list(ctx, pagination)`, `create(ctx, cmd)`,
  `get_by_id(ctx, id)`, `update(ctx, id, cmd)`. — design.md#Architecture
- [ ] 11. Implement `RoleService::create()`: validate name uniqueness, validate permission IDs
  exist, sanitise `name` (strip HTML tags to prevent stored XSS), construct domain entity,
  save role + permissions in transaction. — design.md#Sequence, requirements.md#US-2
- [ ] 12. Implement `RoleService::update()`: validate role exists and is not System Admin,
  validate new name uniqueness if changing, validate permission IDs if changing, sanitise
  `name` (strip HTML tags for XSS prevention), update in transaction. —
  design.md#Sequence, requirements.md#US-4, requirements.md#US-5
- [ ] 13. Implement `RoleService::get_by_id()`: load role + its permissions, return combined
  detail. — design.md#Sequence, requirements.md#US-3
- [ ] 14. Implement `RoleService::list()`: paginated query with total count. —
  design.md#Sequence, requirements.md#US-1

## Infrastructure Layer

- [ ] 15. Implement `SqlRoleRepository`: PostgreSQL queries for all `RoleRepository` methods
  using SQLx or Diesel. — design.md#Architecture
- [ ] 16. Implement `set_permissions()` as a transaction: `DELETE FROM role_permissions WHERE
  role_id = $1` followed by `INSERT INTO role_permissions` for each provided permission ID. —
  design.md#Sequence, design.md#Data-Model

## Adapters Layer (HTTP)

- [ ] 17. Implement `RoleListSchema`, `CreateRoleRequest`, `UpdateRoleRequest`,
  `RoleDetailResponse`, `RoleListResponse` request/response types. —
  design.md#API-Contract
- [ ] 18. Implement `ListRolesHandler`: extract pagination params, call `RoleService::list()`,
  return paginated response. — design.md#API-Contract, requirements.md#US-1
- [ ] 19. Implement `CreateRoleHandler`: deserialize body, validate, call
  `RoleService::create()`, return `201 Created` with `Location` header. —
  design.md#API-Contract, requirements.md#US-2
- [ ] 20. Implement `GetRoleHandler`: extract path param, call `RoleService::get_by_id()`,
  return detail response. — design.md#API-Contract, requirements.md#US-3
- [ ] 21. Implement `UpdateRoleHandler`: deserialize body, validate at least one field present,
  call `RoleService::update()`, return updated detail. — design.md#API-Contract,
  requirements.md#US-4
- [ ] 22. Register four role routes (`GET /api/v1/roles`, `POST /api/v1/roles`,
  `GET /api/v1/roles/{id}`, `PATCH /api/v1/roles/{id}`) in the HTTP router with permission
  middleware guards. — design.md#API-Contract

## Testing

- [ ] 23. Write unit tests for `RoleService::create()`: success, duplicate name, reserved name,
  invalid permission IDs. — requirements.md#US-2
- [ ] 24. Write unit tests for `RoleService::update()`: success (name only, permissions only,
  both), duplicate name, System Admin protection, not-found. — requirements.md#US-4,
  requirements.md#US-5
- [ ] 25. Write unit tests for `RoleService::get_by_id()`: found with permissions, not-found,
  soft-deleted. — requirements.md#US-3
- [ ] 26. Write unit tests for `RoleService::list()`: pagination boundaries, empty list,
  System Admin visibility. — requirements.md#US-1
- [ ] 27. Write integration tests: create role via API → verify DB state → read back via API. —
  requirements.md#US-2, requirements.md#US-3
- [ ] 28. Write integration test: create role with duplicate name returns `409 Conflict`. —
  requirements.md#US-2
- [ ] 29. Write integration test: update System Admin role returns `403 Forbidden`. —
  requirements.md#US-5
- [ ] 30. Write integration test: permission inheritance query (direct + group-inherited =
  deduplicated union). — requirements.md#US-6
- [ ] 31. Write integration test: role name XSS sanitisation (create a role with HTML in the name
  and verify the stored name is stripped/escaped). — design.md#Error-Handling
- [ ] 32. Implement System Admin action audit logging: log all mutations performed by System Admin
  users at WARN level with admin user ID, action, and target resource. —
  design.md#Error-Handling
