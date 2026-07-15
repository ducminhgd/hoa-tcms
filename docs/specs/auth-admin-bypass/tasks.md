# Tasks: Auth Admin Bypass

Tasks ordered by dependency. All tasks reference their source section in either
`requirements.md` or `design.md`.

---

- [ ] **1. Define the `AdminBypassRepository` trait** -- design.md#Components
  - Create the `AdminBypassRepository` trait in the application layer:
    `fn is_system_admin(&self, user_id: i64) -> Result<bool>`.
  - The trait encapsulates the admin check query, allowing mock implementations for
    unit tests.
  - References: design.md#Components

- [ ] **2. Implement `SqlAdminBypassRepository`** -- design.md#Components, design.md#Data-Model
  - Implement `AdminBypassRepository` for `SqlAdminBypassRepository` using `sqlx` or
    `diesel`.
  - Execute the EXISTS query:
    ```sql
    SELECT EXISTS (
        SELECT 1 FROM user_roles ur
        JOIN roles r ON r.id = ur.role_id
        WHERE ur.user_id = $1 AND r.name = 'System Admin' AND r.deleted_at IS NULL
    ) OR EXISTS (
        SELECT 1 FROM user_groups ug
        JOIN group_roles gr ON gr.group_id = ug.group_id
        JOIN roles r ON r.id = gr.role_id
        WHERE ug.user_id = $1 AND r.name = 'System Admin' AND r.deleted_at IS NULL
    );
    ```
  - The query returns a single boolean value.
  - Add compile-time trait implementation check.
  - References: design.md#Data-Model, design.md#Components

- [ ] **3. Create `AdminBypassService`** -- design.md#Components
  - Create `AdminBypassService` struct wrapping `AdminBypassRepository`:
    `fn is_system_admin(&self, user_id: i64) -> bool`
  - The service method returns `bool` (not `Result<bool>`) by logging database errors
    and returning `false` (fail closed: if the database is unreachable, no one is a
    System Admin).
  - On database error, log at ERROR level:
    `[ERROR] Admin bypass check failed for user_id=X: <error>`
  - Inject `AdminBypassService` into `AuthorizationService`.
  - References: requirements.md#US-1, design.md#Components

- [ ] **4. Wire the bypass into all three `AuthorizationService` methods** -- design.md#Sequence
  - In `check_permission`: add `if self.admin_bypass.is_system_admin(user_id) { return
    true; }` as the first line after signature.
  - In `check_project_membership`: add the same bypass check as the first line.
  - In `check_sharing_access`: add the same bypass check as the first line.
  - Verify that the bypass is the absolute first operation in each method (before any
    other database queries, permission resolution, or membership lookups).
  - References: requirements.md#US-1, design.md#Sequence

- [ ] **5. Write unit tests** -- requirements.md#US-1
  - Test `AdminBypassService::is_system_admin`:
    - Happy path: user has System Admin role directly → `true`.
    - Happy path: user has System Admin role via group → `true`.
    - Edge case: user does not have System Admin role → `false`.
    - Edge case: System Admin role is soft-deleted (should not happen in practice,
      but verify the query filters `r.deleted_at IS NULL`) → `false`.
    - Edge case: database error → `false` (fail closed).
  - Test that `AuthorizationService` methods respect the bypass:
    - Mock `AdminBypassService` to return `true`.
    - Verify `check_permission` returns `true` without calling `PermissionResolver`.
    - Verify `check_project_membership` returns `true` without calling
      `ProjectMemberRepository`.
    - Verify `check_sharing_access` returns `true` without calling `SharingRepository`.
  - Test that when `AdminBypassService` returns `false`, the normal authorization
    logic runs (i.e., mocks for other dependencies are called).

- [ ] **6. Write integration tests** -- requirements.md#US-1, design.md#Sequence
  - Test end-to-end via HTTP:
    - Seed a System Admin user (user with the "System Admin" role via `user_roles`).
    - System Admin accesses a protected endpoint without the specific permission
      (e.g., the System Admin is not assigned `category:create` directly, but the
      role seed gives them all permissions) → `200 OK`.
    - System Admin accesses a project-scoped endpoint without being a project member
      → `200 OK`.
    - System Admin accesses a shared endpoint without being shared on the object
      → `200 OK`.
    - Normal user without System Admin role → goes through normal authorization
      (covered by `auth-rbac`, `auth-project-scope`, `auth-sharing-scope`
      integration tests).
    - Add the System Admin role to a user via `user_roles`, then their next request
      bypasses authorization (test mid-session role grant).
    - Remove the System Admin role from a user via `user_roles`, then their next
      request is denied (test mid-session role revocation).
