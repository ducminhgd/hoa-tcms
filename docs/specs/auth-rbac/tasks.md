# Tasks: Auth RBAC

Tasks ordered by dependency. All tasks reference their source section in either
`requirements.md` or `design.md`.

---

- [ ] **1. Create the API contract mock for integration testing** -- design.md#Components
  - Define `AuthorizationService` struct with the public method signature:
    `fn check_permission(&self, user_id: i64, permission_code: &str) -> bool`.
  - Create a mock/stub implementation that returns `true` for all checks by default.
    This enables downstream features (project-crud, test-case-crud, etc.) to integrate
    with the authorization layer immediately, without waiting for the database-backed
    implementation.
  - The mock is replaced when task 6 is complete.
  - References: requirements.md#US-1, design.md#Components

- [ ] **2. Define the `PermissionResolver` trait** -- design.md#Components
  - Create the `PermissionResolver` trait in the application layer:
    `fn resolve_effective_permissions(&self, user_id: i64) -> Result<HashSet<i64>>`.
  - Document the contract: returns the union of permission IDs from direct roles and
    group-inherited roles, excluding permissions from soft-deleted roles and groups.
  - References: design.md#Components, design.md#Data-Model

- [ ] **3. Implement `SqlPermissionResolver`** -- design.md#Components, design.md#Data-Model
  - Implement `PermissionResolver` trait for `SqlPermissionResolver` using `sqlx` or
    `diesel`.
  - Execute the complete production query (direct roles UNION group-inherited roles)
    with soft-delete filtering on both `roles` and `groups` tables.
  - Collect results into a `HashSet<i64>`.
  - Add compile-time trait implementation check.
  - Write a query plan test: `EXPLAIN ANALYZE` the query with realistic data to verify
    it uses the composite primary key indexes on `user_roles`, `user_groups`,
    `role_permissions`, and `group_roles`.
  - References: design.md#Data-Model, design.md#Components

- [ ] **4. Implement `AuthorizationService` with admin bypass integration** -- design.md#Components
  - Create `AuthorizationService` struct with dependencies:
    - `AdminBypassService` (from `auth-admin-bypass`, may be a mock until that feature
      is implemented)
    - `PermissionRepository` (from `iam-permissions`)
    - `PermissionResolver` (from task 2/3)
  - Implement `check_permission(user_id, permission_code)`:
    - Step 1: call `admin_bypass.is_system_admin(user_id)`. If `true`, return `true`.
    - Step 2: resolve the permission code to its ID via `permission_repo.find_by_code(code)`.
      If `None`, log a WARN and return `false`.
    - Step 3: call `permission_resolver.resolve_effective_permissions(user_id)`.
    - Step 4: return `permissions.contains(permission_id)`.
  - On `false` return, log `[INFO] Authorization denied: user_id=X, permission="code"`.
  - References: requirements.md#US-1, requirements.md#US-2, design.md#Sequence

- [ ] **5. Create the authorization middleware/guard** -- design.md#Components
  - Implement an authorization middleware or route guard that:
    - Runs after `AuthMiddleware` (requires `user_id` in request context).
    - Reads the required permission code from route metadata (e.g., a custom attribute
      or annotation on the handler function).
    - Calls `AuthorizationService::check_permission(user_id, permission_code)`.
    - On `true`: passes control to the next handler.
    - On `false`: returns `403 Forbidden` with `{"error": {"code": "FORBIDDEN",
      "message": "Insufficient permissions"}}`.
  - If no permission code is defined on a route (unprotected endpoint), the middleware
    must allow the request through (this is a deliberate choice: some routes like
    `/api/v1/auth/login` are unprotected).
  - References: requirements.md#US-3, design.md#Sequence

- [ ] **6. Wire the authorization middleware into the route registration** -- design.md#Components
  - Register the authorization middleware in the middleware chain after `AuthMiddleware`.
  - Annotate every existing protected route with its required permission code. For each
    existing feature, add the required permission annotation to every handler:
    - `iam-users`: `user:read_list`, `user:create`, `user:read`, `user:update`,
      `user:delete`, `user:select`
    - `iam-groups`: `group:read_list`, `group:create`, `group:read`, `group:update`,
      `group:delete`, `group:select`
    - `iam-roles`: `role:read_list`, `role:create`, `role:read`, `role:update`
    - `iam-permissions`: `permission:read_list`
    - `project-crud`: `project:read_list`, `project:create`, `project:read`,
      `project:update`, `project:delete`, `project:select`
    - `project-members`: `project:read` (list), `project:update` (manage/change)
    - `metadata-categories`: `category:read_list`, `category:create`, `category:read`,
      `category:update`, `category:delete`, `category:select`
    - Others defined in their respective design.md docs
  - Verify that no protected route lacks a permission annotation (a lint check or
    compile-time assertion is desirable but not required for Phase 1).
  - References: design.md#Components

- [ ] **7. Write unit tests** -- requirements.md#US-1, requirements.md#US-2
  - Test `AuthorizationService::check_permission`:
    - Happy path: user has the permission via direct role → `true`.
    - Happy path: user has the permission via group-inherited role → `true`.
    - Happy path: user has the permission via both → `true`.
    - Happy path: user is System Admin → `true` (bypassed, no permission set loaded).
    - Edge case: user does not have the permission → `false`.
    - Edge case: unknown permission code → `false`.
    - Edge case: user has no roles at all → `false`.
    - Edge case: soft-deleted role is excluded from the effective set.
    - Edge case: soft-deleted group is excluded from the effective set.
  - Use mock implementations of `PermissionResolver`, `PermissionRepository`, and
    `AdminBypassService`.

- [ ] **8. Write integration tests** -- requirements.md#US-1, design.md#Data-Model
  - Test `SqlPermissionResolver` against a test database with seeded data:
    - Seed users, roles, permissions, role_permissions, user_roles, groups, user_groups,
      group_roles.
    - Verify the union query returns the correct permission IDs for a user with direct
      roles only.
    - Verify the union query returns the correct permission IDs for a user with group
      roles only.
    - Verify the union query deduplicates overlapping permissions.
    - Verify soft-deleted roles are excluded.
    - Verify soft-deleted groups are excluded.
  - Test end-to-end via HTTP:
    - A protected endpoint returns `200` when the user has the required permission.
    - A protected endpoint returns `403` when the user lacks the required permission.
    - The `403` response body is generic and does not include diagnostic information.
    - An unauthenticated request returns `401` (from auth middleware, not authorization).
