# Tasks: Auth Project Scope

Tasks ordered by dependency. All tasks reference their source section in either
`requirements.md` or `design.md`.

---

- [ ] **1. Define the `ProjectRole` domain enum** -- design.md#ProjectRole-Domain-Enum
  - Create the `ProjectRole` enum in the domain layer with variants: `Owner`, `Editor`,
    `Contributor`, `Viewer`.
  - If `ProjectRole` already exists in the `project-members` domain layer, verify it
    matches this definition and reuse it. Do not duplicate.
  - Implement `Display` and `FromStr` traits for serialization/logging.
  - References: design.md#ProjectRole-Domain-Enum

- [ ] **2. Extend `AuthorizationService` with `check_project_membership`** -- design.md#Components
  - Add the method to `AuthorizationService`:
    `fn check_project_membership(&self, user_id: i64, project_id: i64, required_roles: &[ProjectRole]) -> bool`
  - Dependencies to inject (add to the struct if not already present):
    - `AdminBypassService` (from `auth-admin-bypass`)
    - `ProjectRepository` (from `project-crud`)
    - `ProjectMemberRepository` (from `project-members`)
  - Implementation steps:
    - Step 1: Call `admin_bypass.is_system_admin(user_id)`. If `true`, return `true`.
    - Step 2: Call `project_repo.find_by_id(project_id)`. If `None` or `deleted_at` is
      set, log and return `false`.
    - Step 3: Call `member_repo.find_by_id(project_id, user_id)`.
    - Step 4: If `None`, log and return `false`.
    - Step 5: If `required_roles.is_empty()`, return `true` (any role is sufficient).
      Otherwise, return `required_roles.contains(member.role)`.
    - On `false`, log `[INFO] Project scope denied: user_id=X, project_id=Y,
      required_roles=[...], actual_role=Z`.
  - References: requirements.md#US-1, design.md#Sequence

- [ ] **3. Integrate `check_project_membership` into project-scoped handlers** -- design.md#Components
  - Add a project-scope authorization call to every project-scoped endpoint handler.
    For each existing feature's handlers:
    - `metadata-categories`: All handlers (list/select: any member; create/update/delete:
      Owner, Editor)
    - `metadata-templates`: All handlers (same pattern as categories)
    - `metadata-priorities`: All handlers (same pattern)
    - `metadata-plan-types`: All handlers (same pattern)
    - `test-case-crud`: All handlers (same pattern)
    - `test-case-files`: All handlers (same pattern)
    - `test-plan-crud`: All handlers (same pattern)
    - `test-run-crud`: All handlers (same pattern)
    - `test-execution-crud`: All handlers (same pattern)
    - `project-members`: List/GET (any member), POST/PATCH (Owner)
    - Sharing endpoints (defined in sharing features): Owner, Editor
  - The project ID is extracted from the URL path parameter (`{projectId}`).
  - The `required_roles` array is defined per-endpoint based on the operation type.
  - References: requirements.md#US-1, design.md#Required-Roles-per-Operation-Type

- [ ] **4. Write unit tests** -- requirements.md#US-1, requirements.md#US-2
  - Test `AuthorizationService::check_project_membership`:
    - Happy path: user is member with required role → `true`.
    - Happy path: user is System Admin → `true` (bypass, no DB queries).
    - Happy path: required_roles is empty, user is any member → `true`.
    - Edge case: user is member but wrong role → `false`.
    - Edge case: user is not a member → `false`.
    - Edge case: project does not exist → `false`.
    - Edge case: project is soft-deleted → `false`.
    - Edge case: empty required_roles, user not a member → `false`.
  - Use mock implementations of `ProjectRepository`, `ProjectMemberRepository`,
    and `AdminBypassService`.

- [ ] **5. Write integration tests** -- requirements.md#US-1, design.md#Sequence
  - Test end-to-end via HTTP for a representative project-scoped endpoint
    (e.g., `GET /api/v1/projects/{id}/categories`):
    - User with `category:read_list` permission AND project membership (any role)
      → `200 OK`.
    - User with `category:read_list` permission but NOT a project member → `403 Forbidden`
      (generic message).
    - User with `category:read_list` permission AND project membership as Viewer
      (reading is allowed) → `200 OK`.
    - User with `category:read_list` permission AND project membership as Viewer,
      but attempting `POST` (which requires Owner/Editor) → `403 Forbidden`.
    - System Admin with no project membership → `200 OK` (bypasses membership check).
    - Unauthenticated request → `401 Unauthorized`.
    - The `403` response for "not a member" is identical to the `403` response for
      "missing system permission" (same body, same error code).

- [ ] **6. Verify generic 403 responses across layers** -- requirements.md#US-2, design.md#Error-Handling
  - Audit that every `403` response from project-scope denial uses the exact same JSON
    body as system permission denial:
    `{"error": {"code": "FORBIDDEN", "message": "Insufficient permissions"}}`.
  - Verify that `404` is never returned for a project membership failure (including when
    the project does not exist -- return `403` not `404` for unauthorized users).
  - Verify that the response body does not include `project_id`, `user_role`, or
    `required_roles`.
  - Note: `404` may still be returned when a user passes both authorization gates but
    the requested resource itself does not exist (e.g., a non-existent category within
    a valid project). This is correct behavior.
