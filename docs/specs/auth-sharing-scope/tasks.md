# Tasks: Auth Sharing Scope

Tasks ordered by dependency. All tasks reference their source section in either
`requirements.md` or `design.md`.

---

- [ ] **1. Define the `SharingRole` domain enum** -- design.md#SharingRole-Domain-Enum
  - Create the `SharingRole` enum in the domain layer with variants: `Editor`,
    `Contributor`, `Viewer`.
  - If `SharingRole` is already defined in the `sharing-roles` feature, verify it
    matches this definition and reuse it. Do not duplicate.
  - Implement `Display` and `FromStr` traits for serialization/logging.
  - References: design.md#SharingRole-Domain-Enum

- [ ] **2. Define the `SharingRepository` interface** -- design.md#Components
  - Create the `SharingRepository` trait in the application layer if `sharing-override`
    has not yet defined one:
    `fn find_active_share(&self, object_type: &str, object_id: i64, user_id: i64) -> Result<Option<ShareRecord>>`
  - `ShareRecord` is a domain entity containing at minimum: `object_type`, `object_id`,
    `user_id`, `sharing_role` (`SharingRole`).
  - If `sharing-override` has already defined this repository interface, verify its
    signature matches and reuse it directly.
  - Coordinate with `sharing-override` feature: the concrete implementation
    (`SqlSharingRepository`) lives there. For unit testing, create a mock
    implementation.
  - References: design.md#Components

- [ ] **3. Extend `AuthorizationService` with `check_sharing_access`** -- design.md#Components
  - Add the method to `AuthorizationService` using the self-contained signature:
    `fn check_sharing_access(&self, user_id: i64, object_type: &str, object_id: i64, project_id: Option<i64>, required_sharing_roles: &[SharingRole], required_project_roles: &[ProjectRole]) -> bool`
  - Dependencies to inject:
    - `AdminBypassService` (from `auth-admin-bypass`)
    - `SharingRepository` (from `sharing-override` or task 2)
    - Existing `ProjectMemberRepository` and `ProjectRepository` (already injected
      for `check_project_membership`)
  - Implementation steps:
    - Step 1: Call `admin_bypass.is_system_admin(user_id)`. If `true`, return `true`.
    - Step 2: Call `sharing_repo.find_active_share(object_type, object_id, user_id)`.
    - Step 3: If a share record exists:
      - If `required_sharing_roles.contains(share_role)` → return `true`.
      - Else → log and return `false` (sharing role is the effective role; no fallback).
    - Step 4: If no share record and `project_id` is `Some`:
      - Map `required_sharing_roles` to equivalent `required_project_roles` using the
        role mapping table.
      - Delegate to `self.check_project_membership(user_id, project_id,
        &mapped_project_roles)`.
      - Return its result.
    - Step 5: If no share record and `project_id` is `None` (orphaned object):
      - Return `false` (no sharing and no project to fall back to).
      - Note: the caller should have already checked `created_by` for orphaned objects.
  - On `false`, log `[INFO] Sharing scope denied: user_id=X, object_type=..., object_id=Y,
    share_role=Z, required_roles=[...], reason="..."`.
  - References: requirements.md#US-1, design.md#Sequence, design.md#Role-Mapping

- [ ] **4. Integrate `check_sharing_access` into shareable object handlers** -- design.md#Components
  - For each shareable object type, replace `check_project_membership` calls with
    `check_sharing_access` calls that include the project membership fallback:
    - `test-case-crud`: All GET/POST/PATCH/DELETE handlers.
    - `test-case-files`: Upload/download/delete handlers.
    - `test-plan-crud`: All handlers.
    - `test-run-crud`: All handlers.
    - `test-execution-crud`: All handlers.
  - For each handler, define both `required_sharing_roles` and
    `required_project_roles`:
    - Read operations: sharing=Viewer, project=any member
    - Write operations: sharing=Editor+Contributor, project=Owner+Editor
    - Share operations: sharing=Editor, project=Owner+Editor
  - For orphaned test cases (project_id is None), the handler must check the user is
    the creator (`created_by`) before calling `check_sharing_access`. This creator
    check can be a dedicated method on `AuthorizationService` or inline in the handler.
  - References: requirements.md#US-1, design.md#Sharing-Enabled-Object-Types

- [ ] **5. Write unit tests** -- requirements.md#US-1, requirements.md#US-2
  - Test `AuthorizationService::check_sharing_access`:
    - Happy path: object is shared with sufficient role → `true`.
    - Happy path: object is not shared, but project role is sufficient → `true`
      (fallback works).
    - Happy path: user is System Admin → `true` (bypass).
    - Edge case: object is shared but role is insufficient → `false` (no fallback).
    - Edge case: object is not shared and project role is insufficient → `false`.
    - Edge case: orphaned object (project_id=None), not shared, user is not creator
      → `false`.
    - Edge case: shared role = Viewer, required_roles = [Editor] → `false`
      (sharing role is effective, overrides project Editor role).
    - Edge case: share record exists but is revoked/expired (not active) → treated as
      "not shared", fall through to project membership.
  - Use mock implementations of `AdminBypassService`, `SharingRepository`,
    `ProjectMemberRepository`, and `ProjectRepository`.

- [ ] **6. Write integration tests** -- requirements.md#US-1, design.md#Sequence
  - Test end-to-end via HTTP for a representative shareable object endpoint
    (e.g., `GET /api/v1/test-cases/{id}`):
    - User has system permission AND sharing access with Viewer+ role → `200 OK`.
    - User has system permission AND is shared with Viewer role, but attempts PATCH
      (requires Editor) → `403 Forbidden`.
    - User has system permission AND project role Viewer, but is shared with Editor
      role → can PATCH (sharing role overrides project role) → `200 OK`.
    - User has system permission AND project role Editor, but is shared with Viewer
      role → cannot PATCH (sharing role overrides project role) → `403 Forbidden`.
    - User has system permission, no sharing, but project role Owner → can PATCH
      (fallback works) → `200 OK`.
    - System Admin with no sharing and no project membership → `200 OK`.
    - Orphaned test case, user is creator → can access → `200 OK`.
    - Orphaned test case, user is not creator, not shared → `403 Forbidden`.
    - All `403` responses use the same generic body as `auth-rbac` and
      `auth-project-scope`.
