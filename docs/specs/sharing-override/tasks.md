# Tasks: Sharing Override

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work.

---

## Layer 1 -- Domain

- [ ] 1. Define `EffectiveRole` enum -- `design.md#Components`
  - Five variants: `Admin`, `Editor`, `Contributor`, `Viewer`, `NoAccess`
  - Methods:
    - `can_read() -> bool` -- true for Admin, Editor, Contributor, Viewer
    - `can_write() -> bool` -- true for Admin, Editor, Contributor
    - `can_share() -> bool` -- true for Admin, Editor
    - `can_delete_any() -> bool` -- true for Admin, Editor
    - `satisfies(required: &Self) -> bool` -- true if self >= required using the
      hierarchy: Admin > Editor > Contributor > Viewer > NoAccess
  - Implement `From<SharingRole>` for conversion from sharing roles
  - Implement `From<ProjectRole>` for conversion from project membership roles
    (Owner maps to Editor for effective role purposes -- there is no sharing Owner)
  - Unit test: verify all method outputs for each variant; verify ordering

---

## Layer 2 -- Application

- [ ] 2. Extend `AuthorizationService` with `get_effective_role` --
      `design.md#Algorithm`, `design.md#Components`
  - Method signature: `get_effective_role(user_id, group_ids, object_type,
    object_id) -> Result<EffectiveRole, AuthError>`
  - Steps:
    1. Check if user is System Admin -> return `Admin`
    2. Query `SharingRepository::find_highest_role(object_type, object_id, user_id,
       group_ids)`:
       ```sql
       SELECT role FROM sharing_entries
       WHERE object_type = $1 AND object_id = $2
         AND (user_id = $3 OR group_id = ANY($4))
       ORDER BY CASE role WHEN 'editor' THEN 3 WHEN 'contributor' THEN 2
                WHEN 'viewer' THEN 1 END DESC
       LIMIT 1
       ```
    3. If a row is returned -> convert `SharingRole` to `EffectiveRole`
    4. If no row:
       a. Resolve project_id from the object's table (via a registry or direct query)
       b. Query `ProjectMemberRepository::find_role(user_id, project_id)`
       c. Convert project role to `EffectiveRole`
       d. If not a member -> return `NoAccess`
  - Validate `object_type` against a server-side allowlist before querying
  - Log DEBUG when sharing override activates (effective role != project role)
  - Write unit tests with mocks:
    - System Admin returns Admin regardless of sharing entries
    - Direct user sharing entry returns the sharing role
    - Group sharing entry returns the sharing role
    - Multiple entries (direct + group): highest role wins
    - No sharing entry, user is project member: returns project role
    - No sharing entry, user is not project member: returns NoAccess
    - Invalid object_type is rejected before any query
    - Owner project role maps to Editor effective role
    - Admin effective role satisfies all requirements
    - Viewer effective role only satisfies can_read

- [ ] 3. Define query method on `SharingRepository` for highest role lookup --
      `design.md#Database Query`
  - Add method `find_highest_role(object_type, object_id, user_id, group_ids) ->
    Option<SharingRole>` to the `SharingRepository` interface
  - The method executes the query with `ORDER BY ... LIMIT 1` to return only the
    highest role

- [ ] 4. Write integration tests for effective role resolution --
      `design.md#Sequence`
  - Full flow with a test database:
    - Create project, add user as Viewer
    - Create test case in project
    - Verify effective role is Viewer (no sharing entry)
    - Create sharing entry for user on test case with role=Editor
    - Verify effective role is now Editor (sharing overrides project)
    - Create a second sharing entry via group with role=Contributor
    - Add user to group, verify effective role is Editor (highest wins)
    - Remove direct sharing entry, verify effective role is Contributor (from group)
    - Remove group sharing entry, verify effective role reverts to Viewer (project)
    - Remove user from project, verify effective role is NoAccess (no sharing, not a
      member)
    - Re-add sharing entry with role=Viewer, verify effective role is Viewer (sharing
      works without project membership)
    - Test with non-existent object_type -> rejected
    - Test with non-existent object_id -> NoAccess

---

## Layer 3 -- Adapters (HTTP)

- [ ] 5. Update shareable object handlers to use effective role --
      `design.md#Components`
  - For each shareable object handler (test case, test plan, test run, test
    execution):
    - Replace direct `ProjectMemberRepository` role check with
      `AuthorizationService::get_effective_role` call
    - Pass `object_type` (string constant per handler) and `object_id` (from path
      or body) to the authorization call
    - The handler no longer needs to import or call `ProjectMemberRepository`
      directly for role checks -- `AuthorizationService` encapsulates this
  - Shared pattern (pseudocode):
    ```text
    let effective_role = auth_service.get_effective_role(
        user_id, groups, "test_case", test_case_id
    )?;
    if !effective_role.satisfies(RequiredRole::Contributor) {
        return 403;
    }
    ```
  - The `object_type` string must be a constant defined alongside the handler,
    not constructed dynamically from user input

- [ ] 6. Wire authorization middleware integration for sharing override --
      `design.md#Authorization Gate Integration`
  - Ensure `AuthMiddleware` populates `groups` (list of group IDs) in the request
    context alongside `user_id`
  - If groups are not already populated from the session, add a query to
    `GroupMemberRepository` to fetch group IDs for the authenticated user
  - Consider caching group memberships for the duration of the request (not across
    requests) to avoid repeated queries within a single request lifecycle
  - Write integration tests:
    - Handler calls `get_effective_role` and receives correct role
    - 403 returned when effective role is insufficient
    - 200 returned when effective role is sufficient
    - Sharing-only access (no project membership) works for read operations
    - Sharing-only access denied for create operations (project membership required)

---

## Cross-Cutting Tasks

- [ ] 7. Add `object_type` validation and allowlist -- `design.md#Shareable Object Types`
  - Define a constant or configuration listing all supported object types:
    `test_case`, `test_plan`, `test_run`, `test_execution`
  - In `get_effective_role`, validate the `object_type` parameter against this list
    before querying
  - If the type is unrecognised, return an error (fail closed)

- [ ] 8. Add integration test for ownership check with sharing role --
      `design.md#Authorization Gate Integration`
  - Scenario: User has sharing role=Contributor on a test case they did not create
  - User attempts PATCH on that test case
  - Ownership check (layer 3) applies because effective role is Contributor
  - User is denied with 403 (ownership mismatch)
  - Scenario: User has sharing role=Editor on a test case they did not create
  - User attempts PATCH on that test case
  - Ownership check is skipped because effective role is Editor
  - User is allowed (200)
