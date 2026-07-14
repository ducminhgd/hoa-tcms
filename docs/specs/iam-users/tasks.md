# Tasks: IAM Users

> **Dependency note:** This feature is a **dependency** of `iam-auth`. The USERS table,
> `UserRepository` base interface, and `SqlUserRepository` must exist before `iam-auth` can
> add auth-specific methods. Within this feature, database migration must come before the
> infrastructure repository implementation, which must come before use cases and handlers.

---

## Database

- [ ] 1. **Create `V1__create_users_table` migration** — design.md#data-model
      - Create `USERS` table with all columns: `id` (BIGINT PK, GENERATED ALWAYS AS IDENTITY),
        `username` (VARCHAR(100), NOT NULL), `email` (VARCHAR(255), NOT NULL),
        `password_hash` (VARCHAR(255), NOT NULL), `fullname` (VARCHAR(255), NOT NULL),
        `status` (VARCHAR(20), NOT NULL, DEFAULT 'ACTIVE'),
        `created_by` (BIGINT, nullable), `created_at` (TIMESTAMPTZ, NOT NULL, DEFAULT NOW()),
        `updated_by` (BIGINT, nullable), `updated_at` (TIMESTAMPTZ, NOT NULL, DEFAULT NOW()),
        `deleted_by` (BIGINT, nullable), `deleted_at` (TIMESTAMPTZ, nullable).
      - Add `CHECK (status IN ('ACTIVE', 'INACTIVE'))` constraint.
      - Add FK constraints: `created_by`, `updated_by`, `deleted_by` → `users(id)` ON DELETE RESTRICT.
      - Create unique indexes: `uq_users_username` on `LOWER(username)`,
        `uq_users_email` on `LOWER(email)`.
      - Create `idx_users_deleted_at` on `deleted_at`.

- [ ] 2. **Create `update_at_updated_by` DB trigger function and trigger** — design.md#data-model, requirements.md#US-03
      - Create a `BEFORE UPDATE` trigger function on `USERS` that sets
        `updated_at = NOW()` and reads `updated_by` from a custom session variable
        (`SET app.current_user_id = ...`).
      - Create the trigger on the `USERS` table.
      - Rollback migration drops the trigger and function.

## Domain Layer

- [ ] 3. **Define `UserStatus` enum** — design.md#data-model
      - Enum with `Active` and `Inactive` variants.
      - Implement `Display` (writes `"ACTIVE"` / `"INACTIVE"`) and `FromStr` (parses
        `"ACTIVE"` / `"INACTIVE"` case-insensitively).
      - Implement `serde::Serialize` and `serde::Deserialize`.

- [ ] 4. **Define `User` entity** — design.md#data-model
      - Struct with fields matching the USERS table schema.
      - Factory method `User::create(username, email, password_hash, fullname, created_by) -> Self`
        that sets status to `Active`, timestamps to `Utc::now()`, `updated_by = created_by`.
      - Method `is_active() -> bool` that checks status == Active and `deleted_at.is_none()`.
      - Method `is_deleted() -> bool` that checks `deleted_at.is_some()`.
      - No framework imports — pure domain type.

## Application Layer

- [ ] 5. **Define `UserRepository` interface (port)** — design.md#components
      - `find_by_id(id: i64) -> Result<Option<User>>`
      - `find_by_username(username: &str) -> Result<Option<User>>` (case-insensitive)
      - `find_by_email(email: &str) -> Result<Option<User>>` (case-insensitive)
      - `create(user: &User) -> Result<User>` (inserts row, returns with DB-assigned `id`)
      - `update(user: &User) -> Result<User>` (rejects if `deleted_at IS NOT NULL`)
      - `soft_delete(id: i64, deleted_by: i64) -> Result<User>`
      - `list_paginated(page: i64, limit: i64) -> Result<(Vec<User>, i64)>` (excludes soft-deleted)
      - `update_password_hash(id: i64, new_hash: &str) -> Result<()>`

- [ ] 6. **Implement `ListUsersUseCase`** — design.md#sequence, requirements.md#US-01
      - Accept `page`, `limit` parameters.
      - Validate pagination bounds (page >= 1, limit capped at 100).
      - Call `UserRepository::list_paginated(page, limit)`.
      - Return `(users, total_count)`.

- [ ] 7. **Implement `CreateUserUseCase`** — design.md#sequence, requirements.md#US-03
      - Accept `current_user_id, username, email, password, fullname`.
      - Check `UserRepository::find_by_username(username)` — if exists, return
        `DuplicateUsername` error.
      - Check `UserRepository::find_by_email(email)` — if exists, return
        `DuplicateEmail` error.
      - Call `PasswordHasher::hash(password)` (external dependency from `iam-auth`).
      - Build `User` domain object via factory method.
      - **Sanitize `fullname` on input** — strip HTML tags to prevent stored XSS.
        The frontend (Leptos SSR) provides output encoding, but defence-in-depth requires
        input sanitization as well.
      - Call `UserRepository::create(user)`.
      - Return created user.

- [ ] 8. **Implement `GetUserUseCase`** — design.md#sequence, requirements.md#US-02
      - Accept `user_id`.
      - Call `UserRepository::find_by_id(user_id)`.
      - If `None` or user is soft-deleted: return `NotFound` error.
      - Return user.

- [ ] 9. **Implement `UpdateUserUseCase`** — design.md#sequence, requirements.md#US-04
      - Accept `admin_user_id, target_user_id, username?, email?, fullname?, status?`.
      - Call `UserRepository::find_by_id(target_user_id)`.
      - If not found or soft-deleted: return `NotFound` or `ResourceDeleted` error.
      - If `username` changed: check uniqueness via `UserRepository::find_by_username()`,
        excluding current user's ID.
      - If `email` changed: check uniqueness via `UserRepository::find_by_email()`,
        excluding current user's ID.
      - If `status` provided: validate it is `ACTIVE` or `INACTIVE`.
      - Apply changes to the user object. Set `updated_at` and `updated_by` (trigger
        handles the DB side, but the domain object should reflect the change).
      - Call `UserRepository::update(user)`.
      - **If status changed to `INACTIVE`**: call
        `SessionVerifier::delete_all_user_sessions(target_user_id)` to invalidate all
        active sessions immediately.
      - Return updated user.

- [ ] 10. **Implement `DeleteUserUseCase`** — design.md#sequence, requirements.md#US-05
      - Accept `admin_user_id, target_user_id`.
      - Call `UserRepository::find_by_id(target_user_id)`.
      - If not found or already soft-deleted: return `NotFound` error.
      - Call `UserRepository::soft_delete(target_user_id, admin_user_id)`.
      - **Call `SessionVerifier::delete_all_user_sessions(target_user_id)`** to invalidate
        all active sessions of the deleted user.
      - Return updated user with `deleted_at` and `deleted_by` populated.

- [ ] 11. **Implement `GetSelfProfileUseCase`** — design.md#sequence, requirements.md#US-06
      - Accept `user_id` from session.
      - Call `UserRepository::find_by_id(user_id)`.
      - If not found or soft-deleted or INACTIVE: return error (should not happen if
        `AuthMiddleware` already validated the user, but defensive check).
      - Return user (excluding `password_hash`).

- [ ] 12. **Implement `UpdateSelfProfileUseCase`** — design.md#sequence, requirements.md#US-07
      - Accept `user_id, fullname?, email?`.
      - Call `UserRepository::find_by_id(user_id)`.
      - If not found or soft-deleted or INACTIVE: return error.
      - If `email` changed: check uniqueness via `UserRepository::find_by_email()`,
        excluding current user's ID.
      - Apply changes. `updated_by` set to `user_id`.
      - Call `UserRepository::update(user)`.
      - Return updated user.

- [ ] 13. **Implement `ChangePasswordUseCase`** — design.md#sequence, requirements.md#US-08
      - Accept `user_id, current_password, new_password`.
      - Validate `new_password` >= 8 characters.
      - Validate `current_password != new_password`.
      - Call `UserRepository::find_by_id(user_id)` to get current `password_hash`.
      - Call `PasswordHasher::verify(current_password, stored_hash)`.
      - If verification fails: return `InvalidCurrentPassword` error.
      - Call `PasswordHasher::hash(new_password)`.
      - Call `UserRepository::update_password_hash(user_id, new_hash)`.
      - **Call `SessionVerifier::delete_all_user_sessions(user_id)`** to invalidate ALL
        existing sessions. This is the **CRITICAL** security measure — without it, an
        attacker with a stolen session retains access even after the password changes.
      - Return success (no user payload needed, or return updated user).

## Infrastructure Layer

- [ ] 14. **Implement `SqlUserRepository`** — design.md#components, design.md#data-model
      - Implement `UserRepository` trait using `sqlx` (or `diesel`) with PostgreSQL.
      - `find_by_id`: `SELECT ... FROM users WHERE id = $1`.
      - `find_by_username`: `SELECT ... FROM users WHERE LOWER(username) = LOWER($1)`.
      - `find_by_email`: `SELECT ... FROM users WHERE LOWER(email) = LOWER($1)`.
      - `create`: `INSERT INTO users (...) VALUES (...) RETURNING *`.
      - `update`: `UPDATE users SET ... WHERE id = $1 AND deleted_at IS NULL RETURNING *`.
        Verify update was applied (check rows_affected > 0).
      - `soft_delete`: `UPDATE users SET deleted_at = NOW(), deleted_by = $1 WHERE id = $2 AND deleted_at IS NULL RETURNING *`.
      - `list_paginated`: `SELECT ... FROM users WHERE deleted_at IS NULL ORDER BY id ASC LIMIT $1 OFFSET $2`.
      - `count_active`: `SELECT COUNT(*) FROM users WHERE deleted_at IS NULL`.
      - `update_password_hash`: `UPDATE users SET password_hash = $1 WHERE id = $2`.
      - All queries use parameterized placeholders — never format strings into SQL.
      - Compile-time interface check: Rust's trait system verifies that `SqlUserRepository`
        implements `UserRepository` when `impl UserRepository for SqlUserRepository {}` is
        written. The compiler checks every method signature matches the trait definition.

## Adapters Layer

- [ ] 15. **Define request/response DTOs** — design.md#api-contract
      - `UserResponse`: all user fields except `password_hash`. Implements `Serialize`.
      - `PaginatedUserResponse`: `data: Vec<UserResponse>`, `meta: { total, page, limit }`.
      - `CreateUserRequest`: `username, email, password, fullname`. Implements `Deserialize`.
      - `UpdateAdminRequest`: optional `username, email, fullname, status`. Implements `Deserialize`.
      - `UpdateSelfRequest`: optional `fullname, email, current_password, new_password`. Implements `Deserialize`.
      - Error response structs matching the `{ error: { code, message, details } }` format.

- [ ] 16. **Implement `UserHandlers` — list and detail** — design.md#api-contract, requirements.md#US-01, requirements.md#US-02
      - `list_users_handler`: extract `page`/`limit` from query params, call `ListUsersUseCase`,
        return `200 OK` with `PaginatedUserResponse`. Requires `user:read_list` permission.
      - `get_user_handler`: extract `{id}` from path, call `GetUserUseCase`,
        return `200 OK` with `UserResponse`. Requires `user:read` permission.

- [ ] 17. **Implement `UserHandlers` — create and update and delete** — design.md#api-contract, requirements.md#US-03, requirements.md#US-04, requirements.md#US-05
      - `create_user_handler`: deserialize `CreateUserRequest`, validate fields, call
        `CreateUserUseCase`, return `201 Created` with `Location` header and `UserResponse`.
        Requires `user:create` permission.
      - `update_user_handler`: deserialize `UpdateAdminRequest`, call `UpdateUserUseCase`,
        return `200 OK` with `UserResponse`. Requires `user:update` permission.
      - `delete_user_handler`: call `DeleteUserUseCase`, return `200 OK` with `UserResponse`
        (showing `deleted_at`/`deleted_by`). Requires `user:delete` permission.

- [ ] 18. **Implement `UserHandlers` — self profile and self update** — design.md#api-contract, requirements.md#US-06, requirements.md#US-07, requirements.md#US-08
      - `self_profile_handler`: extract authenticated user ID from request context,
        call `GetSelfProfileUseCase`, return `200 OK` with `UserResponse`.
        Requires session (no additional permission — it's the user's own profile).
      - `self_update_handler`: deserialize `UpdateSelfRequest`, route fields to
        `UpdateSelfProfileUseCase` and/or `ChangePasswordUseCase` as appropriate.
        Return `200 OK` with `UserResponse`. Requires session.

## Wiring

- [ ] 19. **Register user routes in the HTTP server** — design.md#components
      - Add all 7 user routes to the Actix-Web router under `/api/v1/users`.
      - Wire `UserHandlers`, `SqlUserRepository`, all use cases into the dependency
        injection container.
      - Ensure auth middleware is applied to all user routes (all require session).

## Testing

- [ ] 20. **Unit test: `UserStatus` parsing and display** — design.md#data-model
      - `UserStatus::Active` displays as `"ACTIVE"`.
      - `UserStatus::Inactive` displays as `"INACTIVE"`.
      - `"ACTIVE"` parses to `UserStatus::Active` (case-insensitive).
      - `"active"` parses to `UserStatus::Active`.
      - Invalid string returns `Err`.

- [ ] 21. **Unit test: `User::create` factory** — requirements.md#US-03, design.md#data-model
      - Created user has `status = Active`, `deleted_at = None`.
      - `created_at == updated_at` and `created_by == updated_by` on first creation.
      - `id` is 0 (to be assigned by DB).

- [ ] 22. **Unit test: `CreateUserUseCase` (with mock repository)** — requirements.md#US-03
      - Successful creation returns user with correct fields.
      - Duplicate username returns `DuplicateUsername` error.
      - Duplicate email returns `DuplicateEmail` error.
      - `PasswordHasher::hash` is called with the raw password.
      - Created user audit fields are set correctly.

- [ ] 23. **Unit test: `UpdateUserUseCase` (with mock repository)** — requirements.md#US-04
      - Successful update returns updated user.
      - Duplicate username on update returns `DuplicateUsername`.
      - Duplicate email on update returns `DuplicateEmail`.
      - Update on soft-deleted user returns `ResourceDeleted`.
      - Self-rename is allowed (uniqueness check excludes target user).

- [ ] 24. **Unit test: `DeleteUserUseCase` (with mock repository)** — requirements.md#US-05
      - Successful soft-delete sets `deleted_at` and `deleted_by`.
      - Delete on already-deleted user returns `NotFound`.
      - Delete on non-existent user returns `NotFound`.

- [ ] 25. **Unit test: `ChangePasswordUseCase` (with mock repository + mock hasher)** — requirements.md#US-08
      - Successful password change calls `verify` with correct current password
        and `hash` with new password.
      - Incorrect current password returns error.
      - `current_password == new_password` returns validation error.
      - New password < 8 chars returns validation error.

- [ ] 26. **Unit test: `ListUsersUseCase` (with mock repository)** — requirements.md#US-01
      - Returns paginated results from repository.
      - Invalid page values are clamped (page < 1 becomes 1, limit > 100 becomes 100).
      - Default values are applied when page/limit are omitted.

- [ ] 27. **Integration test: Create user endpoint** — requirements.md#US-03, design.md#api-contract
      - Authenticated admin creates a user → `201 Created` with `Location` header.
      - Missing `username` returns `422`.
      - Password shorter than 8 chars returns `422`.
      - Duplicate username returns `409 DUPLICATE_USERNAME`.
      - Duplicate email returns `409 DUPLICATE_EMAIL`.
      - Unauthenticated request returns `401`.
      - Request without `user:create` permission returns `403`.

- [ ] 28. **Integration test: List users endpoint** — requirements.md#US-01, design.md#api-contract
      - Returns `200` with paginated list.
      - Defaults to `page=1, limit=25` when no query params provided.
      - Soft-deleted users are excluded from results.
      - `password_hash` is never present in response items.
      - Exceeding `limit=100` is capped.
      - Request without `user:read_list` permission returns `403`.

- [ ] 29. **Integration test: Get user detail endpoint** — requirements.md#US-02, design.md#api-contract
      - Returns `200` with user object for existing user.
      - Returns `404` for non-existent user ID.
      - Returns `404` for soft-deleted user ID.
      - Request without `user:read` permission returns `403`.

- [ ] 30. **Integration test: Update user (admin) endpoint** — requirements.md#US-04, design.md#api-contract
      - Successfully updates `fullname`, `email`, `status`.
      - Successfully updates `username`.
      - Duplicate email returns `409`.
      - Invalid status value returns `422`.
      - Update on soft-deleted user returns `409 RESOURCE_DELETED`.
      - `updated_at` is incremented and `updated_by` is set to the requesting admin.

- [ ] 31. **Integration test: Soft-delete user endpoint** — requirements.md#US-05, design.md#api-contract
      - Returns `200` with `deleted_at` and `deleted_by` set.
      - Request with non-existent ID returns `404`.
      - Already-deleted user returns `404`.
      - Soft-deleted user is excluded from list results.
      - Soft-deleted user returns `404` on detail endpoint.

- [ ] 32. **Integration test: Self profile endpoint** — requirements.md#US-06, design.md#api-contract
      - Returns `200` with the authenticated user's profile.
      - `password_hash` is never present in response.
      - Unauthenticated request returns `401`.
      - Deactivated user (INACTIVE) returns `403`.

- [ ] 33. **Integration test: Self profile update endpoint** — requirements.md#US-07, design.md#api-contract
      - Successfully updates `fullname`.
      - Successfully updates `email`.
      - Duplicate email returns `409`.
      - Empty body returns `422`.
      - Username is NOT updated (self-service cannot change username).

- [ ] 34. **Integration test: Password change endpoint** — requirements.md#US-08, design.md#api-contract
      - Successful password change with correct current password.
      - Incorrect current password returns `422 VALIDATION_ERROR` with field-level detail.
      - `current_password == new_password` returns `422`.
      - New password < 8 chars returns `422`.
      - After password change, login with new password succeeds (interop with `iam-auth`).

- [ ] 35. **Integration test: Audit column immutability** — requirements.md#US-03, requirements.md#US-04, design.md#data-model
      - After creation, `created_at` and `created_by` remain unchanged after an update.
      - After creation, `updated_at` and `updated_by` change on every update.
      - `updated_at` = `created_at` on a freshly created row.
      - `updated_by` = `created_by` on a freshly created row.
