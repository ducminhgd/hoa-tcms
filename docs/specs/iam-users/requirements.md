# Feature: IAM Users

## Overview

Provide user identity management for the HOA TCMS. System Administrators can create, read,
update, list, and soft-delete users. Authenticated users can view and update their own profile
and change their password. The system enforces case-insensitive uniqueness of username and
email, and user status (ACTIVE/INACTIVE) controls system access without deleting data.

## User Stories

### US-01: List Users

As a System Admin, I want to view a paginated list of all users, so that I can browse and
manage user accounts.

**Acceptance Criteria (EARS)**

- WHEN I submit a `GET /api/v1/users` request with valid `page` and `limit` query parameters
  and I hold the `user:read_list` permission, THE SYSTEM SHALL return `200 OK` with a paginated
  list of users ordered by `id` ascending.
- WHEN I submit a `GET /api/v1/users` request without `page` or `limit` parameters,
  THE SYSTEM SHALL default to `page=1` and `limit=25`.
- IF `limit` exceeds 100, THE SYSTEM SHALL cap it to 100 and include a warning in the response.
- IF the `page` parameter is less than 1, THE SYSTEM SHALL treat it as 1.
- IF the requesting user does not hold the `user:read_list` permission, THE SYSTEM SHALL
  return `403 Forbidden`.
- WHEN a user record has been soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL
  exclude it from the list results.
- WHEN listing users, THE SYSTEM SHALL exclude the `password_hash` field from every response
  object.

### US-02: View User Detail

As a System Admin, I want to view the full details of a specific user, so that I can inspect
their profile, status, and audit timestamps.

**Acceptance Criteria (EARS)**

- WHEN I submit a `GET /api/v1/users/{id}` request and I hold the `user:read` permission,
  THE SYSTEM SHALL return `200 OK` with the user's full profile (excluding `password_hash`).
- IF the user with the given `id` does not exist or has been soft-deleted,
  THE SYSTEM SHALL return `404 Not Found`.
- IF I do not hold the `user:read` permission, THE SYSTEM SHALL return `403 Forbidden`.

### US-03: Create User

As a System Admin, I want to create a new user with username, email, password, and full name,
so that the user can access the system.

**Acceptance Criteria (EARS)**

- WHEN I submit a `POST /api/v1/users` request with valid `username`, `email`, `password`,
  and `fullname` JSON fields, and I hold the `user:create` permission, THE SYSTEM SHALL:
  - Hash the password using PBKDF2.
  - Insert a new row into the `USERS` table with `status = ACTIVE`.
  - Set `created_at = NOW()`, `created_by = my user ID`, `updated_at = created_at`,
    `updated_by = created_by`.
  - Return `201 Created` with the created user object (excluding `password_hash`)
    and a `Location` header pointing to `/api/v1/users/{id}`.
- IF the provided `username` already exists (case-insensitive match), THE SYSTEM SHALL
  return `409 Conflict` with error code `DUPLICATE_USERNAME`.
- IF the provided `email` already exists (case-insensitive match), THE SYSTEM SHALL
  return `409 Conflict` with error code `DUPLICATE_EMAIL`.
- IF the `password` is shorter than 8 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a validation error.
- IF the `username` contains characters other than letters, digits, underscores, and hyphens,
  THE SYSTEM SHALL return `422 Unprocessable Entity` with a validation error.
- IF `username`, `email`, `password`, or `fullname` is missing or empty,
  THE SYSTEM SHALL return `422 Unprocessable Entity` with a validation error.
- IF `fullname` contains HTML tags or script content, THE SYSTEM SHALL sanitize it on input
  (strip HTML tags) OR reject the request with a validation error — user-controlled display
  fields must not introduce stored XSS vulnerabilities.
- IF I do not hold the `user:create` permission, THE SYSTEM SHALL return `403 Forbidden`.

### US-04: Update User (Admin)

As a System Admin, I want to update a user's profile fields (username, email, fullname,
status), so that I can manage user accounts.

**Acceptance Criteria (EARS)**

- WHEN I submit a `PATCH /api/v1/users/{id}` request with valid fields and I hold the
  `user:update` permission, THE SYSTEM SHALL update the specified fields, increment
  `updated_at`, set `updated_by` to my user ID, and return `200 OK` with the updated user
  object (excluding `password_hash`).
- IF the user with the given `id` does not exist or has been soft-deleted,
  THE SYSTEM SHALL return `404 Not Found`.
- IF the update changes `username` to a value that already exists (case-insensitive),
  THE SYSTEM SHALL return `409 Conflict` with error code `DUPLICATE_USERNAME`.
- IF the update changes `email` to a value that already exists (case-insensitive),
  THE SYSTEM SHALL return `409 Conflict` with error code `DUPLICATE_EMAIL`.
- IF I do not hold the `user:update` permission, THE SYSTEM SHALL return `403 Forbidden`.
- WHEN a PATCH request targets a soft-deleted row (`deleted_at IS NOT NULL`),
  THE SYSTEM SHALL reject the request and return `409 Conflict` with error code
  `RESOURCE_DELETED`, except when the only mutation is clearing `deleted_at` and `deleted_by`
  (restore operation, reserved for future use).

### US-05: Soft-Delete User

As a System Admin, I want to soft-delete a user, so that the user is removed from active
views without losing data or cascade-deleting related records.

**Acceptance Criteria (EARS)**

- WHEN I submit a `DELETE /api/v1/users/{id}` request and I hold the `user:delete`
  permission, THE SYSTEM SHALL set `deleted_at = NOW()`, set `deleted_by` to my user ID,
  and return `200 OK` with the updated user object showing the deleted timestamps.
- IF the user with the given `id` does not exist or is already soft-deleted,
  THE SYSTEM SHALL return `404 Not Found`.
- IF I do not hold the `user:delete` permission, THE SYSTEM SHALL return `403 Forbidden`.
- WHEN a user is soft-deleted, THE SYSTEM SHALL NOT cascade-delete their related records
  (groups, roles, project memberships, created/updated references in other tables).
- WHEN a soft-deleted user record is referenced as `created_by`, `updated_by`, or
  `deleted_by` in other tables, THE SYSTEM SHALL preserve those FK references intact.

### US-06: View Self Profile

As an authenticated user, I want to view my own profile, so that I can see my account
information.

**Acceptance Criteria (EARS)**

- WHEN I submit a `GET /api/v1/users/me` request with a valid session cookie,
  THE SYSTEM SHALL return `200 OK` with my user profile object (excluding `password_hash`).
- IF the session cookie is missing, invalid, or expired, THE SYSTEM SHALL return
  `401 Unauthorized`.
- IF my user status is INACTIVE, THE SYSTEM SHALL return `403 Forbidden` (the session
  exists but the user has been deactivated since).

### US-07: Update Self Profile

As an authenticated user, I want to update my own full name and email, so that my profile
reflects my current information.

**Acceptance Criteria (EARS)**

- WHEN I submit a `PATCH /api/v1/users/me` request with valid `fullname` and/or `email`
  fields and a valid session cookie, THE SYSTEM SHALL update only the provided fields,
  increment `updated_at`, set `updated_by` to my own user ID, and return `200 OK` with
  the updated user object (excluding `password_hash`).
- IF the provided `email` already exists for another user (case-insensitive),
  THE SYSTEM SHALL return `409 Conflict` with error code `DUPLICATE_EMAIL`.
- IF the session cookie is missing, invalid, or expired, THE SYSTEM SHALL return
  `401 Unauthorized`.
- IF my user status is INACTIVE, THE SYSTEM SHALL return `403 Forbidden`.
- WHEN the request body is empty (no fields to update), THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a validation error.
- WHEN the `email` field is provided but is not a valid email format,
  THE SYSTEM SHALL return `422 Unprocessable Entity` with a validation error.

### US-08: Change Password

As an authenticated user, I want to change my password by providing my current password
and a new password, so that I can maintain account security.

**Acceptance Criteria (EARS)**

- WHEN I submit a `PATCH /api/v1/users/me` request with `current_password` and `new_password`
  fields and a valid session cookie, THE SYSTEM SHALL:
  - Verify `current_password` matches the stored PBKDF2 hash using constant-time comparison.
  - Reject if `current_password` is incorrect (return `422 Unprocessable Entity`).
  - Hash `new_password` using PBKDF2 and store the new hash.
  - **Invalidate ALL existing sessions for this user** (including the current session) by
    calling `SessionVerifier::delete_all_user_sessions()`. The user must re-authenticate
    to obtain a new session. This prevents an attacker with a stolen session from maintaining
    access after the legitimate user changes their password.
  - Increment `updated_at`, set `updated_by` to my own user ID.
  - Return `200 OK` with the updated user object (excluding `password_hash`).
- IF `new_password` is shorter than 8 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a validation error.
- IF `current_password` and `new_password` are the same, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a validation error.
- IF the session cookie is missing, invalid, or expired, THE SYSTEM SHALL return
  `401 Unauthorized`.
- IF my user status is INACTIVE, THE SYSTEM SHALL return `403 Forbidden`.
- WHEN only `current_password` and `new_password` are provided (without `fullname` or
  `email`), THE SYSTEM SHALL perform only the password change and NOT modify profile fields.

### US-09: Uniqueness Enforcement

As a system, I want to enforce that `username` and `email` are unique across all users
using case-insensitive comparison, so that no two users share the same identity.

**Acceptance Criteria (EARS)**

- WHEN a new user is created with a `username` that matches an existing user's `username`
  regardless of letter casing (e.g., "JohnDoe" vs "johndoe"), THE SYSTEM SHALL reject the
  create with `409 Conflict` and error code `DUPLICATE_USERNAME`.
- WHEN a new user is created with an `email` that matches an existing user's `email`
  regardless of letter casing (e.g., "John@Example.com" vs "john@example.com"),
  THE SYSTEM SHALL reject the create with `409 Conflict` and error code `DUPLICATE_EMAIL`.
- WHEN a user record is updated to a `username` that matches another user's `username`
  (case-insensitive), THE SYSTEM SHALL reject the update with `409 Conflict` and error
  code `DUPLICATE_USERNAME`.
- WHEN a user record is updated to an `email` that matches another user's `email`
  (case-insensitive), THE SYSTEM SHALL reject the update with `409 Conflict` and error
  code `DUPLICATE_EMAIL`.
- WHEN checking uniqueness, THE SYSTEM SHALL exclude the current user's own record from
  the comparison (self-rename allowed).
- WHEN checking uniqueness, THE SYSTEM SHALL consider soft-deleted users in the uniqueness
  comparison — deleted usernames and emails remain reserved.

### US-10: Status Management

As a System Admin, I want to activate or deactivate a user via the status field, so that
I can grant or revoke system access without deleting data or removing role memberships.

**Acceptance Criteria (EARS)**

- WHEN a user is created, THE SYSTEM SHALL set `status = ACTIVE` by default.
- WHEN a user's status is changed to `INACTIVE` via `PATCH /api/v1/users/{id}`,
  THE SYSTEM SHALL:
  - Set `status = 'INACTIVE'`.
  - Increment `updated_at` and set `updated_by`.
  - Preserve all group memberships, role assignments, and project memberships intact.
  - Return `200 OK` with the updated user.
- WHEN a user's status is changed from `INACTIVE` to `ACTIVE` (reactivation),
  THE SYSTEM SHALL restore full access on next login without requiring any re-provisioning.
- IF a status value other than `ACTIVE` or `INACTIVE` is provided,
  THE SYSTEM SHALL return `422 Unprocessable Entity` with a validation error.
- WHILE a user is INACTIVE, any existing valid session SHALL be rejected by the
  auth middleware with `403 Forbidden` (enforced by `iam-auth` feature).

## Out of Scope

- User self-registration (accounts are created by System Admin only — see `iam-cli-init`
  for the bootstrap admin).
- Password reset or "forgot password" flow (deferred to a future phase; password changes
  require current password confirmation).
- Admin-initiated password reset for another user (only self-service password change is
  supported).
- Bulk user creation or import (CSV/JSON upload).
- User roles and group membership management on the user form (those are handled by
  separate `iam-groups` and `iam-roles` features).
- User search or advanced filtering on the list endpoint (Phase 1 supports pagination only;
  search is deferred).
- Rate limiting on password change attempts (deferred to a cross-cutting feature).

## Dependencies

- **USERS table** — will be created by this feature's database migration.
- **PBKDF2 password hashing** — the `Pbkdf2Hasher` component from `iam-auth` is used for
  password hashing during user creation and password change.
- **Session middleware** — the `AuthMiddleware` from `iam-auth` verifies the session cookie
  and populates the authenticated user context for all protected endpoints.
- **Permission checking** — each admin endpoint requires a system permission
  (`user:read_list`, `user:create`, `user:read`, `user:update`, `user:delete`).
  Permission enforcement is provided by the authorization middleware (separate
  `auth-rbac` feature).
- **Error response format** — the `{ "error": { "code", "message", "details" } }` format
  established by `iam-auth` is reused for all error responses.
