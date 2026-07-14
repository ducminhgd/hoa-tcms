# Design: IAM Users

## Architecture

The IAM Users feature follows the Clean Architecture layering. The domain entity (`User`) and
value objects (`UserStatus`) live in Layer 1. Use cases and the `UserRepository` port live in
Layer 2. HTTP handlers and DTOs are in Layer 3 (Adapters). The SQL implementation of
`UserRepository` and database migrations live in Layer 4 (Infrastructure).

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                      │
│  ┌───────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                    │   │
│  │  - list_users_handler      GET /api/v1/users                     │   │
│  │  - create_user_handler     POST /api/v1/users                    │   │
│  │  - get_user_handler        GET /api/v1/users/{id}                │   │
│  │  - update_user_handler     PATCH /api/v1/users/{id}              │   │
│  │  - delete_user_handler     DELETE /api/v1/users/{id}             │   │
│  │  - self_profile_handler    GET /api/v1/users/me                  │   │
│  │  - self_update_handler     PATCH /api/v1/users/me                │   │
│  │                                                                       │
│  │  DTOs:                                                                │
│  │  - CreateUserRequest / UserResponse / PaginatedUserResponse           │
│  │  - UpdateSelfRequest (profile + password fields)                      │
│  └──────────┬────────────────────────────────────────────────────────┘   │
│             │ calls                                                       │
│             ▼                                                             │
│  Application (Layer 2)                           ┌─────────────────────┐ │
│  ┌───────────────────────────────────────────┐   │  Domain (Layer 1)   │ │
│  │  Use Cases:                                │   │                     │ │
│  │  - ListUsersUseCase                        │   │  - User entity      │ │
│  │  - CreateUserUseCase                       │   │  - UserStatus enum  │ │
│  │  - GetUserUseCase                          │   │                     │ │
│  │  - UpdateUserUseCase                       │   └─────────────────────┘ │
│  │  - DeleteUserUseCase                       │                           │
│  │  - GetSelfProfileUseCase                   │                           │
│  │  - UpdateSelfProfileUseCase                │                           │
│  │  - ChangePasswordUseCase                   │                           │
│  │                                             │                           │
│  │  Ports:                                     │                           │
│  │  - UserRepository (interface)               │                           │
│  │  - PasswordHasher (interface, from iam-auth)│                           │
│  └──────────┬─────────────────────────────────┘                           │
│             │ delegates to                                                │
│             ▼                                                             │
│  Infrastructure (Layer 4)                                                 │
│  ┌───────────────────────────────────────────────────────────────────┐   │
│  │  - SqlUserRepository   (implements UserRepository)                │   │
│  │  - V1__create_users_table.sql   (migration)                       │   │
│  └───────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. HTTP handlers validate the request and extract the authenticated user from the request
   context (set by `AuthMiddleware` from `iam-auth`).
2. Handlers call the appropriate use case, which encapsulates all business logic
   (uniqueness checks, status validation, password hashing).
3. Use cases call `UserRepository` methods to persist or retrieve data.
4. On self password change, the use case calls `PasswordHasher` (from `iam-auth`) to
   verify the current password and hash the new password.
5. The use case returns a result to the handler, which formats it as an HTTP response.

---

## API Contract

All endpoints are prefixed with `/api/v1`. Error responses follow the format established
by `iam-auth`:

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable message",
    "details": [
      { "field": "field_name", "message": "Per-field error message" }
    ]
  }
}
```

---

### GET `/api/v1/users/me`

Retrieve the authenticated user's own profile. Requires a valid session cookie.

**Request Headers:**
| Header | Value |
|--------|-------|
| Cookie | `hoa-tcms-session=<session_id>` (required) |

**Query Parameters:** None.

**Success Response:** `200 OK`
```json
{
  "data": {
    "id": 42,
    "username": "jdoe",
    "email": "jdoe@example.com",
    "fullname": "John Doe",
    "status": "ACTIVE",
    "created_by": 1,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 1,
    "updated_at": "2026-07-14T10:00:00Z",
    "deleted_by": null,
    "deleted_at": null
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | Session cookie is missing, invalid, or expired |
| `403 Forbidden` | `USER_INACTIVE` | User was deactivated after session creation |
| `503 Service Unavailable` | `SESSION_STORE_UNAVAILABLE` | Redis is unreachable |

---

### PATCH `/api/v1/users/me`

Update the authenticated user's own profile fields and/or change password. Requires a
valid session cookie.

**Request Headers:**
| Header | Value |
|--------|-------|
| Content-Type | `application/json` |
| Cookie | `hoa-tcms-session=<session_id>` (required) |

**Request Body:**
```json
{
  "fullname": "John Updated",
  "email": "jupdated@example.com",
  "current_password": "OldP@ss123",
  "new_password": "NewP@ss456"
}
```

All fields are optional, but at least one field must be provided. Profile fields
(`fullname`, `email`) and password fields (`current_password`, `new_password`) can be
combined in the same request.

- To update profile only: provide `fullname` and/or `email`, omit password fields.
- To change password only: provide `current_password` and `new_password`, omit profile fields.
- To update both: provide all four fields.

**Success Response:** `200 OK` — returns the updated user object (same shape as self profile).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | Session cookie is missing, invalid, or expired |
| `403 Forbidden` | `USER_INACTIVE` | User was deactivated after session creation |
| `409 Conflict` | `DUPLICATE_EMAIL` | New email already exists for another user (case-insensitive) |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | Empty body, invalid email format, `new_password` < 8 chars, `current_password` does not match stored hash, `current_password` equals `new_password` |
| `503 Service Unavailable` | `SESSION_STORE_UNAVAILABLE` | Redis is unreachable |

**Notes:**
- Password change and profile update are **atomic**. If both are requested, the entire
  request succeeds or fails together. If the password verification fails, none of the
  profile fields are updated.
- Email uniqueness check excludes the current user's own record (self-rename allowed).

---

### GET `/api/v1/users`

Retrieve a paginated list of all non-deleted users. Requires `user:read_list` permission.

**Request Headers:**
| Header | Value |
|--------|-------|
| Cookie | `hoa-tcms-session=<session_id>` (required) |

**Query Parameters:**
| Parameter | Type | Default | Max | Description |
|-----------|------|---------|-----|-------------|
| `page` | integer | 1 | — | Page number (1-indexed) |
| `limit` | integer | 25 | 100 | Items per page |

**Success Response:** `200 OK`
```json
{
  "data": [
    {
      "id": 42,
      "username": "jdoe",
      "email": "jdoe@example.com",
      "fullname": "John Doe",
      "status": "ACTIVE",
      "created_by": 1,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_by": 1,
      "updated_at": "2026-07-14T10:00:00Z",
      "deleted_by": null,
      "deleted_at": null
    }
  ],
  "meta": {
    "total": 100,
    "page": 1,
    "limit": 25
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | Session cookie is missing, invalid, or expired |
| `403 Forbidden` | `FORBIDDEN` | User does not hold `user:read_list` permission |
| `503 Service Unavailable` | `SESSION_STORE_UNAVAILABLE` | Redis is unreachable |

**Notes:**
- Soft-deleted users (`deleted_at IS NOT NULL`) are excluded from results.
- `password_hash` is never included in the response.
- Results are ordered by `id` ascending.

---

### POST `/api/v1/users`

Create a new user. Requires `user:create` permission.

**Request Headers:**
| Header | Value |
|--------|-------|
| Content-Type | `application/json` |
| Cookie | `hoa-tcms-session=<session_id>` (required) |

**Request Body:**
```json
{
  "username": "jdoe",
  "email": "jdoe@example.com",
  "password": "P@ssword123",
  "fullname": "John Doe"
}
```

All four fields are required.

**Success Response:** `201 Created`

`Location: /api/v1/users/42`
```json
{
  "data": {
    "id": 42,
    "username": "jdoe",
    "email": "jdoe@example.com",
    "fullname": "John Doe",
    "status": "ACTIVE",
    "created_by": 1,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 1,
    "updated_at": "2026-07-14T10:00:00Z",
    "deleted_by": null,
    "deleted_at": null
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | Session cookie is missing, invalid, or expired |
| `403 Forbidden` | `FORBIDDEN` | User does not hold `user:create` permission |
| `409 Conflict` | `DUPLICATE_USERNAME` | Username already exists (case-insensitive) |
| `409 Conflict` | `DUPLICATE_EMAIL` | Email already exists (case-insensitive) |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | Missing required fields, `password` < 8 chars, invalid `username` format (allowed: letters, digits, underscore, hyphen), invalid `email` format |
| `503 Service Unavailable` | `SESSION_STORE_UNAVAILABLE` | Redis is unreachable |

**Notes:**
- Password is hashed using PBKDF2 before storage. The raw password is never stored.
- `status` is set to `ACTIVE` by default.
- `created_by` is set to the requesting admin's user ID. For the CLI bootstrap admin
  (created via `iam-cli-init`), `created_by` is NULL.
- `updated_at` = `created_at` and `updated_by` = `created_by` on first insert.

---

### GET `/api/v1/users/{id}`

Retrieve the full profile of a specific user. Requires `user:read` permission.

**Request Headers:**
| Header | Value |
|--------|-------|
| Cookie | `hoa-tcms-session=<session_id>` (required) |

**Path Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | integer (i64) | User ID |

**Success Response:** `200 OK` — same shape as the user object returned by GET `/api/v1/users/me`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | Session cookie is missing, invalid, or expired |
| `403 Forbidden` | `FORBIDDEN` | User does not hold `user:read` permission |
| `404 Not Found` | `NOT_FOUND` | User with given `id` does not exist or is soft-deleted |
| `503 Service Unavailable` | `SESSION_STORE_UNAVAILABLE` | Redis is unreachable |

---

### PATCH `/api/v1/users/{id}`

Update a user's profile as a System Admin. Requires `user:update` permission.

**Request Headers:**
| Header | Value |
|--------|-------|
| Content-Type | `application/json` |
| Cookie | `hoa-tcms-session=<session_id>` (required) |

**Path Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | integer (i64) | User ID |

**Request Body:**
```json
{
  "username": "jdoe_new",
  "email": "jdoe_new@example.com",
  "fullname": "John Updated",
  "status": "INACTIVE"
}
```

All fields are optional — only provided fields are updated. At least one field must be
provided.

**Success Response:** `200 OK` — returns the updated user object (same shape as user detail).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | Session cookie is missing, invalid, or expired |
| `403 Forbidden` | `FORBIDDEN` | User does not hold `user:update` permission |
| `404 Not Found` | `NOT_FOUND` | User with given `id` does not exist |
| `409 Conflict` | `DUPLICATE_USERNAME` | New username already exists (case-insensitive) |
| `409 Conflict` | `DUPLICATE_EMAIL` | New email already exists (case-insensitive) |
| `409 Conflict` | `RESOURCE_DELETED` | Target user is soft-deleted (only restore or delete operations allowed) |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | Empty body, invalid status value (must be `ACTIVE` or `INACTIVE`), invalid email/username format |
| `503 Service Unavailable` | `SESSION_STORE_UNAVAILABLE` | Redis is unreachable |

**Notes:**
- Admin **cannot** update `password_hash` through this endpoint. Password changes are
  self-service only (see `PATCH /api/v1/users/me`).
- Uniqueness checks exclude the target user's own record (self-rename allowed).
- Update is rejected for soft-deleted rows (`deleted_at IS NOT NULL`) with
  `409 RESOURCE_DELETED`, unless the only change is clearing `deleted_at` and
  `deleted_by` (restore, reserved for future use).

---

### DELETE `/api/v1/users/{id}`

Soft-delete a user. Requires `user:delete` permission.

**Request Headers:**
| Header | Value |
|--------|-------|
| Cookie | `hoa-tcms-session=<session_id>` (required) |

**Path Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | integer (i64) | User ID |

**Request Body:** None.

**Success Response:** `200 OK` — returns the updated user object with `deleted_at` and
`deleted_by` populated. The `deleted_by` field is set to the requesting admin's user ID.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | Session cookie is missing, invalid, or expired |
| `403 Forbidden` | `FORBIDDEN` | User does not hold `user:delete` permission |
| `404 Not Found` | `NOT_FOUND` | User with given `id` does not exist or is already soft-deleted |
| `503 Service Unavailable` | `SESSION_STORE_UNAVAILABLE` | Redis is unreachable |

**Notes:**
- This is a **soft delete**: the row is not removed from the database. Only `deleted_at`
  and `deleted_by` are set. All other data is preserved.
- No cascade delete occurs. The user's group memberships, role assignments, project
  memberships, and references (as `created_by`, `updated_by`, `deleted_by` in other
  tables) are preserved.
- A soft-deleted user is excluded from list views and cannot log in (enforced by
  `iam-auth`'s `AuthMiddleware`).
- A soft-deleted user cannot be updated (returns `409 RESOURCE_DELETED`) except via a
  restore operation (future feature).

---

## Data Model

### USERS table

One new table. No other schema changes.

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate key, auto-increment |
| `username` | `VARCHAR(100)` | `NOT NULL` | Letters, digits, underscore, hyphen |
| `email` | `VARCHAR(255)` | `NOT NULL` | Validated format |
| `password_hash` | `VARCHAR(255)` | `NOT NULL` | PBKDF2 format string: `pbkdf2$alg$salt$iterations$hash` |
| `fullname` | `VARCHAR(255)` | `NOT NULL` | Display name |
| `status` | `VARCHAR(20)` | `NOT NULL`, `DEFAULT 'ACTIVE'` | `CHECK (status IN ('ACTIVE', 'INACTIVE'))` |
| `created_by` | `BIGINT` | `FK -> users(id)` | Nullable — CLI bootstrap admin has no creator |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_by` | `BIGINT` | `FK -> users(id)` | Nullable — bootstrap admin has no prior user to reference; all other rows are set by the application and the DB trigger on every mutation |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated on every mutation |
| `deleted_by` | `BIGINT` | `FK -> users(id)` | Nullable — populated on soft-delete |
| `deleted_at` | `TIMESTAMPTZ` | `NULL` | Null = active; set on soft-delete |

**Indexes:**

| Name | Columns / Expression | Type |
|------|---------------------|------|
| `pk_users` | `id` | Primary key (auto) |
| `uq_users_username` | `LOWER(username)` | Unique index (case-insensitive) |
| `uq_users_email` | `LOWER(email)` | Unique index (case-insensitive) |
| `idx_users_deleted_at` | `deleted_at` | B-tree (for filtering list views) |

**FK constraint on `created_by`, `updated_by`, `deleted_by`:**
- All three columns use `REFERENCES users(id) ON DELETE RESTRICT` — prevents deleting a user
  who is referenced as a creator/updater/deleter of other records.
- `created_by` and `updated_by` are nullable (bootstrap admin). The application layer sets
  these for all non-bootstrap rows. The DB trigger auto-maintains `updated_at` and `updated_by`
  on subsequent updates for rows where `updated_by` is non-NULL.

### Audit column rules (per FR-54a, FR-54b, FR-54c)

| Rule | Enforcement Layer | Details |
|------|------------------|---------|
| `updated_at` = `created_at`, `updated_by` = `created_by` on insert | Application | Use case sets both pairs to the same values |
| `created_at` and `created_by` immutable after insert | Application | Repository rejects any UPDATE that modifies `created_at` or `created_by` with a clear error |
| `updated_at` and `updated_by` set to `NOW()` and current user on update | DB Trigger | `BEFORE UPDATE` trigger sets `updated_at = NOW()` and `updated_by = current_user_id()` |
| UPDATE on soft-deleted rows rejected | Application | Repository checks `deleted_at IS NOT NULL` before applying UPDATE; returns error |
| `deleted_by` always populated on application soft-deletes | Application | Use case sets `deleted_by` to current user ID |

### Domain entity

```rust
// domain/user.rs — No framework imports
struct User {
    id: i64,
    username: String,
    email: String,
    password_hash: String,
    fullname: String,
    status: UserStatus,
    created_by: Option<i64>,
    created_at: DateTime<Utc>,
    updated_by: Option<i64>,
    updated_at: DateTime<Utc>,
    deleted_by: Option<i64>,
    deleted_at: Option<DateTime<Utc>>,
}

enum UserStatus {
    Active,
    Inactive,
}
```

---

## Sequence

### Create User Flow

1. Admin sends `POST /api/v1/users` with `{ username, email, password, fullname }`.
2. Handler deserializes and validates the request body (all fields required, password >= 8
   chars, username format allowed chars, email format).
3. Admin permission is checked: does the current user hold `user:create`? If not, `403`.
4. Handler calls `CreateUserUseCase::execute(current_user_id, username, email, password, fullname)`.
5. Use case calls `UserRepository::find_by_username(username)` (case-insensitive).
   - If found (including soft-deleted): return `DuplicateUsername` error → handler returns `409`.
6. Use case calls `UserRepository::find_by_email(email)` (case-insensitive).
   - If found (including soft-deleted): return `DuplicateEmail` error → handler returns `409`.
7. Use case calls `PasswordHasher::hash(password)` to generate the PBKDF2 hash string.
8. Use case constructs a `User` domain object with:
   - `id`: 0 (to be assigned by DB)
   - `status`: `Active`
   - `created_by`: `Some(current_user_id)`
   - `created_at`: `Utc::now()`
   - `updated_by`: `current_user_id`
   - `updated_at`: `created_at`
   - `deleted_by`: `None`, `deleted_at`: `None`
9. Use case calls `UserRepository::create(user)` to insert the row.
10. Repository returns the created user with the DB-assigned `id`.
11. Handler returns `201 Created` with `Location` header and the user payload (no `password_hash`).

### Self Profile Update / Password Change Flow

1. User sends `PATCH /api/v1/users/me` with optional `{ fullname, email, current_password, new_password }`.
2. Handler deserializes and validates the body (at least one field required, email format,
   `new_password` >= 8 chars if provided).
3. Session middleware has already verified the user and attached `AuthUser { user_id, ... }`
   to the request context.
4. Handler calls `UpdateSelfProfileUseCase::execute(user_id, fullname, email)` and/or
   `ChangePasswordUseCase::execute(user_id, current_password, new_password)` depending on
   which fields are present.
5. If profile update requested:
   - Use case calls `UserRepository::find_by_id(user_id)`.
   - If user is soft-deleted or INACTIVE: return error.
   - If `email` changed: call `UserRepository::find_by_email(new_email)` to check uniqueness
     (exclude current user's own ID).
   - If duplicate found: return `DuplicateEmail` error.
   - Call `UserRepository::update(user)`.
6. If password change requested:
   - Use case calls `UserRepository::find_by_id(user_id)` to get current `password_hash`.
   - Use case calls `PasswordHasher::verify(current_password, stored_hash)`.
   - If verification fails: return password mismatch error.
   - If `current_password == new_password`: return validation error.
   - If verification succeeds: call `PasswordHasher::hash(new_password)`.
   - Call `UserRepository::update_password_hash(user_id, new_hash)`.
   - **Invalidate all existing sessions for this user** by calling
     `SessionVerifier::delete_all_user_sessions(user_id)`. This forces the user to
     re-authenticate and ensures any attacker holding a stolen session is locked out
     immediately after the password change.
7. If both profile and password change are requested, steps 5 and 6 happen **within the
   same transaction**. If either fails, both are rolled back.
8. Handler returns `200 OK` with the updated user object.

### Soft-Delete Flow

1. Admin sends `DELETE /api/v1/users/{id}`.
2. Admin permission is checked: does the current user hold `user:delete`? If not, `403`.
3. Handler calls `DeleteUserUseCase::execute(admin_user_id, target_user_id)`.
4. Use case calls `UserRepository::find_by_id(target_user_id)`.
   - If not found or `deleted_at IS NOT NULL`: return `NotFound` error → handler returns `404`.
5. Use case calls `UserRepository::soft_delete(target_user_id, admin_user_id)`.
   - Sets `deleted_at = NOW()`, `deleted_by = admin_user_id`.
   - Does **not** delete any related records.
6. Use case calls `SessionVerifier::delete_all_user_sessions(target_user_id)` to invalidate
   all active sessions for the deleted user. This ensures the user cannot continue using
   existing session cookies and that reactivation does not carry forward stale sessions.
7. Handler returns `200 OK` with the updated user object showing `deleted_at` and `deleted_by`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `User` | Domain (1) | Entity representing a system user. Contains fields matching the USERS table. Provides factory method `create(...)` for new users. |
| `UserStatus` | Domain (1) | Enum: `Active`, `Inactive`. Parses from/into database string. |
| `UserRepository` | Application (2) | Interface (port) for user data access. Methods: `find_by_id`, `find_by_username`, `find_by_email`, `create`, `update`, `soft_delete`, `list_paginated`, `count_active`. |
| `ListUsersUseCase` | Application (2) | Paginated list. Applies soft-delete filter, returns `(users, total_count)`. |
| `CreateUserUseCase` | Application (2) | Uniqueness checks (username + email), password hashing, user creation. |
| `GetUserUseCase` | Application (2) | Fetch single user by ID. Returns `NotFound` for soft-deleted users. |
| `UpdateUserUseCase` | Application (2) | Admin update of user fields (username, email, fullname, status). Uniqueness checks, soft-delete rejection. |
| `DeleteUserUseCase` | Application (2) | Soft-delete: sets `deleted_at` and `deleted_by`. No cascade. |
| `GetSelfProfileUseCase` | Application (2) | Fetch own user profile by session user ID. |
| `UpdateSelfProfileUseCase` | Application (2) | Update own profile (fullname, email). Uniqueness check on email. Immutability check on username (self-service cannot change username). |
| `ChangePasswordUseCase` | Application (2) | Verify current password via `PasswordHasher`, hash new password, store new hash. |
| `CreateUserRequest` / `UserResponse` / `PaginatedUserResponse` / `UpdateSelfRequest` / `UpdateAdminRequest` | Adapters (3) | Request/Response DTOs with serde serialization. |
| `UserHandlers` | Adapters (3) | Actix-Web handler functions for all 7 user endpoints. |
| `SqlUserRepository` | Infrastructure (4) | PostgreSQL implementation of `UserRepository` using `sqlx` or `diesel`. |
| `V1__create_users_table` | Infrastructure (4) | SQL migration to create the USERS table with all columns, constraints, indexes, and the `updated_at`/`updated_by` trigger. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| HTTP router registration | Register 7 new user routes (`/api/v1/users/me`, `/api/v1/users/{id}`, etc.). All require session (behind `AuthMiddleware`). Admin routes additionally require permission checks. |
| `PasswordHasher` (from `iam-auth`) | Reused directly — no changes needed. Called by `CreateUserUseCase` and `ChangePasswordUseCase`. |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing/invalid/expired session | `401` | `NOT_AUTHENTICATED` | INFO | From auth middleware |
| User deactivated (session exists) | `403` | `USER_INACTIVE` | WARN | From auth middleware |
| Missing required permission | `403` | `FORBIDDEN` | INFO | Specific permission code (e.g., `user:create`) |
| User not found (or soft-deleted) | `404` | `NOT_FOUND` | INFO | Same message for non-existent and deleted |
| Duplicate username (create/update) | `409` | `DUPLICATE_USERNAME` | INFO | Case-insensitive comparison |
| Duplicate email (create/update) | `409` | `DUPLICATE_EMAIL` | INFO | Case-insensitive comparison |
| Update on soft-deleted row | `409` | `RESOURCE_DELETED` | WARN | Application-layer rejection per FR-54c |
| Validation error (missing/invalid fields) | `422` | `VALIDATION_ERROR` | INFO | Field-level details in the response |
| Redis unreachable | `503` | `SESSION_STORE_UNAVAILABLE` | ERROR | From auth middleware |
| Internal error (hashing, DB) | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not expose `password_hash`** in any API response — never.
- **Do not reveal whether a username/email is taken** via different HTTP status codes.
  Both `404 Not Found` and `409 Conflict` are used appropriately: `409` when the user
  explicitly tried to use a duplicate value (create/update). `404` when querying a
  non-existent ID.
- **Do not cascade-delete** user references when a user is soft-deleted. FK references
  are preserved.
- **Do not allow updating soft-deleted users** — reject with `409 RESOURCE_DELETED`.
- **Do not allow self-service username changes** — only email and fullname can be
  changed via `PATCH /api/v1/users/me`. Username changes require admin privileges.
- **Do not log passwords or password hashes** in any log output.
- **Sanitize user-controlled display fields (`fullname`, `email`) against XSS** — all
  user-controlled string fields shown in the UI must be output-encoded at the presentation
  layer (Leptos server-side rendering handles this by default, but raw interpolation in
  HTML attributes requires explicit escaping). Strip or reject HTML tags on input for
  `fullname` to provide defence-in-depth against stored XSS.
- **Cap all text input lengths server-side** — enforce maximum lengths for `fullname`
  (255 chars), `email` (255 chars), `username` (100 chars) at the application layer
  and reject oversized inputs with `422 Unprocessable Entity`.

### The DB Trigger for `updated_at` / `updated_by`

A PostgreSQL `BEFORE UPDATE` trigger automatically sets `updated_at = NOW()` and
`updated_by` to the current user ID. The application sets the initial `updated_at` /
`updated_by` on insert, and the trigger handles subsequent updates. This ensures
`updated_at` / `updated_by` are always correct even if a row is updated outside
the application layer.

The trigger function receives the current user ID via a custom session variable
(`SET app.current_user_id = ...`) set at the beginning of each request by the
auth middleware.
