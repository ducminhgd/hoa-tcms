# Tasks: Project Members

> **Dependency note:** This feature depends on the `project-crud` feature for the
> `PROJECTS` table and on `iam-users` for the `USERS` table schema and the
> `UserRepository` base implementation. The auth middleware from `iam-auth` must
> also be in place.

---

## Domain Layer

- [ ] 1. **Define `ProjectMember` entity** — design.md#data-model,
      design.md#components
      - Fields: `user_id` (i64), `project_id` (i64), `role` (enum or enum-like
        string), `created_at` (DateTime<Utc>), `updated_at` (DateTime<Utc>),
        `updated_by` (Option<i64>).
      - Role enum: `Owner`, `Editor`, `Contributor`, `Viewer` with `TryFrom<&str>`
        for deserialization and `Display`/`AsRef<str>` for serialization.
      - Convenience methods: `is_owner()`, `is_editor()`, `is_contributor()`,
        `is_viewer()`.
      - Constructor validates role against allowed set; returns `Result` for
        invalid role strings.
      - **No ORM or HTTP framework imports** in this file.

## Application Layer

- [ ] 2. **Define `ProjectMemberRepository` interface (port)** —
      design.md#components
      - Methods:
        - `find_by_project(project_id: i64) -> Result<Vec<ProjectMember>>` —
          returns members ordered by `created_at` ASC.
        - `find_by_id(project_id: i64, user_id: i64) -> Result<Option<ProjectMember>>`.
        - `save(project_id: i64, user_id: i64, role: MemberRole, created_by: i64) -> Result<ProjectMember>`.
        - `delete(project_id: i64, user_id: i64) -> Result<()>`.
        - `update_role(project_id: i64, user_id: i64, new_role: MemberRole, updated_by: i64) -> Result<ProjectMember>`.
        - `count_owners(project_id: i64) -> Result<u64>`.
        - `exists(project_id: i64, user_id: i64) -> Result<bool>`.

- [ ] 3. **Implement `ListProjectMembersUseCase`** — design.md#api-contract,
      design.md#sequence, requirements.md#US-01
      - Accept `project_id`.
      - Call `ProjectMemberRepository::find_by_project()`.
      - For each member, fetch user details (username, fullname) via
        `UserRepository::find_by_id()` — or use a join query in the repository
        (preferred for efficiency).
      - Return a list of member details including user info.

- [ ] 4. **Implement `ManageProjectMembersUseCase`** — design.md#api-contract,
      design.md#sequence, requirements.md#US-02, requirements.md#US-03,
      requirements.md#US-06
      - Accept `project_id`, `caller_id`, `add_list: Vec<(i64, String)>`,
        `remove_list: Vec<i64>`.
      - **Validate additions:**
        - For each `(user_id, role)` pair:
          - Verify user exists via `UserRepository::find_by_id()`.
          - Verify user is ACTIVE and not deleted.
          - Verify user is not already a member via
            `ProjectMemberRepository::exists()`.
        - Collect all validation errors. If any exist, return
          `422 Unprocessable Entity` with aggregated field-level details.
      - **Validate removals:**
        - For each `user_id` in `remove_list`: if not a member, skip
          (idempotent — no error).
        - Check last-Owner constraint: count current Owners, subtract
          Owners in `remove_list`. If result < 1, return
          `409 Conflict LAST_OWNER_REMOVAL`.
      - **Execute in a database transaction:**
        - First: delete all rows for user_ids in `remove_list`.
        - Second: insert all rows for entries in `add_list`.
      - Return summary of `added` and `removed`.

- [ ] 5. **Implement `UpdateMemberRoleUseCase`** — design.md#api-contract,
      design.md#sequence, requirements.md#US-04
      - Accept `project_id`, `target_user_id`, `new_role`, `caller_id`.
      - Fetch current membership via `ProjectMemberRepository::find_by_id()`.
        - If not found, return "not found" error.
      - If the current role is `Owner` and the new role is not `Owner`:
        - Count total Owners via `ProjectMemberRepository::count_owners()`.
        - If count is 1, return `409 Conflict LAST_OWNER_DOWNGRADE`.
      - Call `ProjectMemberRepository::update_role()` to persist the change.
      - Return the updated `ProjectMember` with user details.

## Infrastructure Layer

- [ ] 6. **Create the `PROJECT_MEMBERS` table migration** — design.md#data-model
      - Migration SQL (forward):
        ```sql
        CREATE TABLE project_members (
            user_id    BIGINT       NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
            project_id BIGINT       NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
            role       VARCHAR(20)  NOT NULL CHECK (role IN ('Owner', 'Editor', 'Contributor', 'Viewer')),
            created_by BIGINT       NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
            created_at TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            updated_by BIGINT       REFERENCES users(id) ON DELETE RESTRICT,
            updated_at TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            CONSTRAINT pk_project_members PRIMARY KEY (user_id, project_id)
        );

        CREATE INDEX idx_project_members_project_id ON project_members(project_id);
        CREATE INDEX idx_project_members_user_id ON project_members(user_id);
        CREATE INDEX idx_project_members_active ON project_members(project_id, user_id) INCLUDE (role);
        ```
      - Rollback SQL: `DROP TABLE IF EXISTS project_members;`
      - Add trigger for `updated_at` auto-maintenance (See audit trigger convention
        from `audit-triggers` feature).
      - Migration file follows the project's migration naming convention (Alembic
        or `golang-migrate` format — use what `project-crud` established).

- [ ] 7. **Implement `SqlProjectMemberRepository`** — design.md#components,
      design.md#data-model
      - Use the project's SQL client (e.g., `sqlx` for Rust / Actix-Web).
      - All queries use parameterized placeholders (`$1`, `$2`, ...).
      - `find_by_project(project_id)`:
        ```sql
        SELECT pm.user_id, pm.project_id, pm.role, pm.created_by, pm.created_at,
               pm.updated_at, pm.updated_by, u.username, u.fullname
        FROM project_members pm
        JOIN users u ON u.id = pm.user_id
        WHERE pm.project_id = $1
        ORDER BY pm.created_at ASC
        ```
      - `find_by_id(project_id, user_id)`:
        ```sql
        SELECT user_id, project_id, role, created_by, created_at, updated_at, updated_by
        FROM project_members
        WHERE project_id = $1 AND user_id = $2
        ```
      - `save(project_id, user_id, role, created_by)`:
        ```sql
        INSERT INTO project_members (user_id, project_id, role, created_by, updated_by)
        VALUES ($1, $2, $3, $4, $4)
        RETURNING user_id, project_id, role, created_by, created_at, updated_at, updated_by
        ```
      - `delete(project_id, user_id)`:
        ```sql
        DELETE FROM project_members
        WHERE project_id = $1 AND user_id = $2
        ```
      - `update_role(project_id, user_id, new_role, updated_by)`:
        ```sql
        UPDATE project_members
        SET role = $3, updated_at = NOW(), updated_by = $4
        WHERE project_id = $1 AND user_id = $2
        RETURNING user_id, project_id, role, created_by, created_at, updated_at, updated_by
        ```
      - `count_owners(project_id)`:
        ```sql
        SELECT COUNT(*) FROM project_members
        WHERE project_id = $1 AND role = 'Owner'
        ```
      - `exists(project_id, user_id)`:
        ```sql
        SELECT 1 FROM project_members
        WHERE project_id = $1 AND user_id = $2
        ```
      - Compile-time interface check: Rust's trait system verifies this automatically
        when you write `impl ProjectMemberRepository for SqlProjectMemberRepository`.
        The compiler checks every method signature matches the trait definition.
        No explicit assertion statement is needed; add one for documentation only:
        ```rust
        // Compile-time verification
        const _: () = {
            fn assert_impl<T: ProjectMemberRepository>() {}
            // This line fails to compile if SqlProjectMemberRepository does not
            // implement ProjectMemberRepository
            assert_impl::<SqlProjectMemberRepository>();
        };
        ```

## Adapters Layer

- [ ] 8. **Define request/response DTOs** — design.md#api-contract
      - `MemberResponse`: `user_id: i64`, `username: String`, `fullname: String`,
        `role: String`, `created_at: DateTime<Utc>`.
      - `ManageMembersRequest`: `add: Option<Vec<AddMemberEntry>>`,
        `remove: Option<Vec<i64>>`.
        - `AddMemberEntry`: `user_id: i64`, `role: String`.
        - Validation: at least one of `add` or `remove` must be non-empty.
      - `ManageMembersResponse`: `added: Vec<AddedEntry>`, `removed: Vec<i64>`.
        - `AddedEntry`: `user_id: i64`, `role: String`.
      - `UpdateRoleRequest`: `role: String`.
      - Standard error response structs matching `{ "error": { "code", "message", "details" } }`
        format, consistent with `iam-auth` spec.

- [ ] 9. **Implement `ListMembersHandler` (GET)** — design.md#api-contract,
       requirements.md#US-01
        - Extract `project_id` from path.
        - Verify project exists (call `ProjectRepository::find_by_id()` or use
          a lightweight existence check).
        - Verify caller has `project:read` system permission.
        - Verify the caller is a member of this project (scope guard call).
        - Call `ListProjectMembersUseCase::execute(project_id)`.
        - Return `200 OK` with the member list.

- [ ] 10. **Implement `ManageMembersHandler` (POST)** — design.md#api-contract,
       requirements.md#US-02, requirements.md#US-03, requirements.md#US-06
        - Extract `project_id` from path.
        - Verify project exists.
        - Verify caller has `project:update` system permission.
        - Verify the caller is an Owner of this project (or has `project:update`
          system permission).
        - Deserialize request body.
        - Validate: at least one of `add` or `remove` is non-empty; role values
          are valid; no duplicate user_ids across `add` and `remove`.
        - Call `ManageProjectMembersUseCase::execute()`.
        - Map use case result to `200 OK` with `ManageMembersResponse`.
        - Map validation errors to `422` with field-level details.
        - Map last-Owner error to `409 Conflict`.
        - Map duplicate member error to `409 Conflict`.

- [ ] 11. **Implement `UpdateMemberRoleHandler` (PATCH)** — design.md#api-contract,
       requirements.md#US-04
        - Extract `project_id` and `user_id` from path.
        - Verify project exists.
        - Verify caller has `project:update` system permission.
        - Verify the caller is an Owner of this project (or has `project:update`
          system permission).
        - Deserialize request body; validate `role` is present and valid.
        - Call `UpdateMemberRoleUseCase::execute()`.
        - Return `200 OK` with the updated member record.
        - Map last-Owner downgrade to `409 Conflict`.

- [ ] 12. **Implement `ProjectScopeGuard`** — design.md#architecture,
       design.md#components, requirements.md#US-05
        - A guard/validator that given `(user_id, project_id)`:
          - Calls `ProjectMemberRepository::find_by_id(project_id, user_id)`.
          - If a record exists, returns `Ok(role)`.
          - If no record exists, returns `Err(Forbidden)`.
        - This guard is consumed by the `auth-project-scope` authorization
          middleware. It is defined in this feature so that the auth middleware
          feature depends on the interface without knowing the implementation.
        - The guard does NOT check system permissions — that is handled by
          the authorization middleware above it.

## Wiring

- [ ] 13. **Register project member routes in the HTTP server** —
       design.md#components
        - `GET /api/v1/projects/{project_id}/members` → `ListMembersHandler`
          (requires `project:read` session + project membership).
        - `POST /api/v1/projects/{project_id}/members` → `ManageMembersHandler`
          (requires `project:update` session + member Owner).
        - `PATCH /api/v1/projects/{project_id}/members/{user_id}` →
          `UpdateMemberRoleHandler` (requires `project:update` session + member Owner).
        - Wire `SqlProjectMemberRepository`, `ListProjectMembersUseCase`,
          `ManageProjectMembersUseCase`, `UpdateMemberRoleUseCase` into the
          dependency injection container.

## Testing

- [ ] 14. **Unit test: `ProjectMember` entity** — design.md#data-model
        - Valid role strings create instances.
        - Invalid role strings return errors.
        - `is_owner()`, `is_editor()`, `is_contributor()`, `is_viewer()` return
          correct values for each role.
        - Role round-trip serialization (Display, AsRef<str>, TryFrom<&str>).

- [ ] 15. **Unit test: `ListProjectMembersUseCase`** — requirements.md#US-01
        - Returns member list for valid project.
        - Returns empty list when project has no members.
        - Repository error propagates to caller.

- [ ] 16. **Unit test: `ManageProjectMembersUseCase`** — requirements.md#US-02,
       requirements.md#US-03, requirements.md#US-06
        - Successfully adds new members with valid data.
        - Successfully removes existing members.
        - Successfully performs add + remove in one operation.
        - Rejects adding a user that does not exist.
        - Rejects adding a deactivated (INACTIVE) user.
        - Rejects adding a user that is already a member (`409 Conflict`).
        - Rejects adding with an invalid role value (`422`).
        - Rejects removing the last Owner (`409 Conflict`).
        - Idempotently skips removing a non-member (no error).
        - Self-removal succeeds (caller removes themselves).
        - Both `add` and `remove` empty returns validation error.
        - All database operations within a transaction (rollback on failure).

- [ ] 17. **Unit test: `UpdateMemberRoleUseCase`** — requirements.md#US-04
        - Successfully changes a member's role.
        - Returns `404` when target user is not a member.
        - Rejects downgrading the last Owner away from Owner (`409 Conflict`).
        - Allows downgrading an Owner when other Owners exist.
        - Allows changing role of a non-Owner to any valid role.

- [ ] 18. **Unit test: `ProjectScopeGuard`** — requirements.md#US-05
        - Returns role when user is a member of the project.
        - Returns error when user is not a member.
        - Uses the partial index query path (verifiable via repository mock).

- [ ] 19. **Integration test: List members endpoint** — requirements.md#US-01,
       design.md#api-contract
        - Authenticated Owner can list members: returns `200`.
        - Authenticated non-member with `project:read` cannot list members:
          returns `403`.
        - Unauthenticated request: returns `401`.
        - Project does not exist: returns `404`.
        - List is ordered by `created_at` ASC.
        - Response includes username and fullname for each member.

- [ ] 20. **Integration test: Add/remove members endpoint** —
       requirements.md#US-02, requirements.md#US-03, requirements.md#US-06,
       design.md#api-contract
        - Owner adds a new member: `200` with member in response.
        - Owner removes an existing member: `200` with member in response.
        - Owner performs add + remove in one request: `200` with summary.
        - Non-Owner tries to add member: `403`.
        - System Admin (with `project:update` permission) can add member
          without being project Owner.
        - Adding non-existent user returns `422` with field-level error.
        - Adding INACTIVE user returns `422`.
        - Adding duplicate member returns `409`.
        - Removing last Owner returns `409`.
        - Removing non-member is silently skipped (idempotent).
        - After removal, the removed user cannot list project members.

- [ ] 21. **Integration test: Change member role endpoint** —
       requirements.md#US-04, design.md#api-contract
        - Owner changes a member's role: `200` with updated record.
        - Non-Owner tries to change role: `403`.
        - Changing non-member's role returns `404`.
        - Last Owner downgrade returns `409`.
        - Owner changes role of another Owner when 3 Owners exist: succeeds.
        - Invalid role value returns `422`.

- [ ] 22. **Integration test: Project membership scope check** —
       requirements.md#US-05
        - After adding a user as Viewer, that user can access project-scoped
          read endpoints.
        - After removing a user, that user loses access to project-scoped
          endpoints (`403`).
        - System Admin bypasses membership check and accesses project endpoints.
        - Deleted project (soft-delete) hides members from scope check lookups.

## Security & Hardening

- [ ] 23. **Implement last-Owner race condition protection** ---
       design.md#security-requirements
        - When counting Owners for the last-Owner check, use `SELECT ... FOR UPDATE`
          on `project_members` rows for the target project to prevent concurrent
          requests from both passing the check and leaving the project Owner-less.
        - Both `ManageProjectMembersUseCase` and `UpdateMemberRoleUseCase` must
          acquire the same row-level lock.
        - Unit test: two concurrent requests to remove different Owners from a
          two-Owner project --- exactly one succeeds, the other gets `409 Conflict`.

- [ ] 24. **Verify XSS sanitization coverage** ---
       requirements.md#security-considerations
        - Confirm that input sanitization (HTML tag stripping) is applied at the
          user creation/update boundary for `username` and `fullname` fields.
        - Confirm that output encoding is applied at the presentation layer for
          all user-controlled fields in the member list response.
        - Integration test: create a user with `<script>alert('xss')</script>` in
          `fullname`, add as member, verify the member list response shows the
          sanitized version (HTML tags stripped).

- [ ] 25. **CSRF protection verification** ---
       requirements.md#security-considerations
        - Confirm that `SameSite=Lax` (or stricter) is set on the session cookie.
        - Confirm that `POST` and `PATCH` endpoints verify the `Content-Type` header.
        - Integration test: submit a POST without `Content-Type: application/json`
          and verify `403` or `422` response (blocked by CSRF protection).
        - Integration test: submit a PATCH without valid `Origin`/`Referer` header
          and verify rejection.
