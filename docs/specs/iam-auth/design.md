# Design: IAM Auth

## Architecture

The IAM Auth feature follows the Clean Architecture layering defined in the project layout
rules. It spans all four layers:

```
┌─────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                 │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                               │   │
│  │  - login_handler    POST /api/v1/auth/login                  │   │
│  │  - logout_handler   POST /api/v1/auth/logout                 │   │
│  │                                                              │   │
│  │  Auth Middleware:                                             │   │
│  │  - auth_middleware   Session cookie → user context           │   │
│  └──────────┬───────────────────────────────────────────────────┘   │
│             │ calls                                                 │
│             ▼                                                       │
│  Application (Layer 2)                         ┌─────────────────┐ │
│  ┌─────────────────────────────────────────┐   │  Domain (L1)   │ │
│  │  LoginUseCase    LogoutUseCase          │   │  - Session     │ │
│  │  SessionVerifier (interface)            │   │  - PasswordHash│ │
│  │  PasswordHasher  (interface)            │   └─────────────────┘ │
│  └──────────┬─────────────────────────────┘                       │
│             │ delegates to                                        │
│             ▼                                                     │
│  Infrastructure (Layer 4)                                         │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │  - RedisSessionStore  (implements SessionVerifier)           │ │
│  │  - Pbkdf2Hasher       (implements PasswordHasher)            │ │
│  │  - UserRepository     (lookup user by username/email)        │ │
│  └──────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. A login request arrives at the HTTP handler, which calls the `LoginUseCase`.
2. `LoginUseCase` calls `UserRepository` to look up the user by username or email
   (case-insensitive).
3. If found, `LoginUseCase` calls `PasswordHasher` to verify the password against the
   stored hash.
4. On success, `LoginUseCase` creates a session in `RedisSessionStore` (Redis with TTL).
5. The handler sets the session cookie on the HTTP response.
6. Subsequent requests pass through `AuthMiddleware`, which reads the cookie and calls
   `RedisSessionStore` to verify the session. The authenticated user ID is attached to the
   request context for downstream authorization.

**Sessions are stored exclusively in Redis** (not in PostgreSQL). The PostgreSQL `USERS`
table stores the password hash and user status. The application is stateless with respect
to sessions — session state lives entirely in Redis.

---

## API Contract

### POST `/api/v1/auth/login`

Authenticate a user and create a session.

**Request Headers:**
| Header | Value |
|--------|-------|
| Content-Type | `application/json` |

**Request Body** (exactly one of `username` or `email` is required):
```json
{
  "username": "jdoe",
  "email": "jdoe@example.com",
  "password": "P@ssword123"
}
```

**Success Response:** `200 OK`

The session cookie is set via the `Set-Cookie` response header. The JSON body is minimal:
```json
{
  "data": {
    "user_id": 42,
    "username": "jdoe",
    "fullname": "John Doe"
  }
}
```

Set-Cookie: `hoa-tcms-session=<session_id>; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=3600`

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `400 Bad Request` | `INVALID_CONTENT_TYPE` | Content-Type is not `application/json` |
| `422 Unprocessable Entity` | `VALIDATION_ERROR` | Missing both `username` and `email`, or provided both; or `password` shorter than 8 characters |
| `401 Unauthorized` | `AUTHENTICATION_FAILED` | Credentials do not match any active user, or user is INACTIVE (same message for both — `"Invalid credentials"`) |
| `503 Service Unavailable` | `SESSION_STORE_UNAVAILABLE` | Redis is unreachable |

**Error body format:**
```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid request body",
    "details": [
      { "field": "password", "message": "must be at least 8 characters" }
    ]
  }
}
```

For `AUTHENTICATION_FAILED`:
```json
{
  "error": {
    "code": "AUTHENTICATION_FAILED",
    "message": "Invalid credentials"
  }
}
```

**Notes:**
- The `username` and `email` fields are mutually exclusive. The handler must reject
  a request that supplies both or neither.
- Lookups are case-insensitive (`ILIKE` in PostgreSQL or `LOWER()` comparison) for both
  `username` and `email`.
- The password hash comparison must use a constant-time function to prevent timing attacks.
- The session ID is a cryptographically random UUID (v4) generated server-side.

---

### POST `/api/v1/auth/logout`

Destroy the current session.

**Request Headers:**
| Header | Value |
|--------|-------|
| Cookie | `hoa-tcms-session=<session_id>` (required) |

**Request Body:** None

**Success Response:** `200 OK`
```json
{
  "data": {
    "message": "Logged out successfully"
  }
}
```

The response also sets `Set-Cookie: hoa-tcms-session=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0`
to clear the cookie on the client side.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401 Unauthorized` | `NOT_AUTHENTICATED` | Session cookie is missing, invalid, or expired |
| `503 Service Unavailable` | `SESSION_STORE_UNAVAILABLE` | Redis is unreachable |

---

## Data Model

### No new tables

This feature introduces **no new database tables**. It reuses the `USERS` table and stores
session data in Redis.

### USERS table (reused columns)

| Column | Type | Used by auth for |
|--------|------|-----------------|
| `id` | `BIGINT PK` | Session payload identifies user |
| `username` | `VARCHAR` | Login lookup (case-insensitive unique) |
| `email` | `VARCHAR` | Login lookup (case-insensitive unique) |
| `password_hash` | `VARCHAR` | PBKDF2 hash string for verification |
| `fullname` | `VARCHAR` | Displayed in login success response and attached to request context |
| `status` | `VARCHAR` | Must be `ACTIVE` for login to succeed |
| `deleted_at` | `TIMESTAMPTZ` | Deleted users are treated as non-existent |

### Password hash format

Stored in the `password_hash` column as a single string:

```
pbkdf2$<algorithm>$<salt>$<iterations>$<hash>
```

| Component | Description | Example |
|-----------|-------------|---------|
| `algorithm` | Hash algorithm used by PBKDF2 | `sha256` |
| `salt` | Per-user cryptographically random salt, base64-encoded | `a1b2c3d4e5f6...` |
| `iterations` | Iteration count (configurable) | `600000` |
| `hash` | Derived key bytes, base64-encoded | `f7e8...` |

Example:
```
pbkdf2$sha256$c2FsdHlzYWx0eXNhbHQ$600000$kNfF3H...==
```

### Redis session schema

| Key | Value | TTL | Example |
|-----|-------|-----|---------|
| `session:<session_id>` | JSON: `{"user_id": 42, "created_at": "2026-07-14T10:00:00Z", "fingerprint": "a1b2c3d4..."}` | Configured TTL (default 3600s) | `session:a1b2c3d4-...` → `{"user_id":42,"created_at":"2026-07-14T10:00:00Z","fingerprint":"a1b2c3d4..."}` |

The session ID is a UUID v4 generated on login. Redis TTL governs session lifetime; there
is no sliding expiration (the TTL is not refreshed on each request).

**Redis security note:** Session data stored in Redis is in plaintext JSON. Redis must be
deployed on a trusted network, protected with a strong password (`REQUIREPASS`), and
configured with TLS encryption (`tls-port`). In environments where Redis is shared or
exposed to non-trusted networks, consider encrypting session payloads before storage
or using a dedicated Redis instance for sessions.

### Session fingerprint

A SHA-256 hash of the `User-Agent` header is stored in the session payload as `fingerprint`.
On every authenticated request, the middleware computes the hash of the request's `User-Agent`
and compares it against the stored value. A mismatch triggers session re-authentication
(returns `401 Unauthorized` and deletes the stale session). This mitigates session hijacking
via cookie theft — an attacker with a stolen cookie cannot use it from a different browser.

The fingerprint is an additional security layer, not a substitute for cookie security
attributes. It is not available in environments where the User-Agent varies legitimately
(e.g., API clients) — in such cases the fingerprint check can be relaxed for specific
integration endpoints (deferred to Phase 2 when API tokens are introduced).

### Configuration (environment variables)

| Variable | Default | Description |
|----------|---------|-------------|
| `SESSION_TTL_SECONDS` | `3600` | Session lifetime in seconds |
| `SESSION_COOKIE_NAME` | `hoa-tcms-session` | Cookie name for the session ID |
| `SESSION_COOKIE_PATH` | `/` | Cookie path |
| `SESSION_SECURE` | `true` | Set `Secure` flag on cookie |
| `PBKDF2_ITERATIONS` | `600000` | PBKDF2 iteration count |
| `PBKDF2_ALGORITHM` | `sha256` | PBKDF2 hash algorithm |

---

## Sequence

### Login Flow

1. Client sends `POST /api/v1/auth/login` with `{"email": "...", "password": "..."}`
   (or `{"username": "...", "password": "..."}`).
2. HTTP handler deserializes the request body and validates the input (mutual exclusivity
   of username/email, min password length).
3. Handler calls `LoginUseCase::execute(email_or_username, password)`.
4. `LoginUseCase` calls `UserRepository::find_by_email()` or `UserRepository::find_by_username()`
   (case-insensitive lookup).
5. If no user found, `LoginUseCase` returns `AuthenticationFailed` error (generic message).
6. If user found but `status != ACTIVE` or `deleted_at IS NOT NULL`, `LoginUseCase` returns
   `AuthenticationFailed` error (same generic message).
7. If user found and active, `LoginUseCase` calls `PasswordHasher::verify(password, stored_hash)`.
8. If password does not match, `LoginUseCase` returns `AuthenticationFailed` error.
9. If password matches, `LoginUseCase` calls `SessionVerifier::create_session(user_id)`:
   - Generates UUID v4 as session ID
   - Stores `{"user_id": user_id, "created_at": now}` in Redis at key `session:<uuid>`
   - Sets TTL on the Redis key
10. Return `Session` object to handler.
11. Handler constructs `Set-Cookie` header from session ID and config.
12. Handler returns `200 OK` response with the cookie and a minimal user payload.
13. If iterations have increased since the user's password was last hashed, `PasswordHasher`
    re-hashes the password in the background and updates the `password_hash` column.

### Logout Flow

1. Client sends `POST /api/v1/auth/logout` with the session cookie.
2. Auth middleware validates the session (see middleware flow below).
3. Handler calls `LogoutUseCase::execute(session_id)`.
4. `LogoutUseCase` calls `SessionVerifier::delete_session(session_id)` to remove the key
   from Redis.
5. Handler constructs a `Set-Cookie` header with `Max-Age=0` to clear the cookie.
6. Handler returns `200 OK` response.

### Auth Middleware Flow (per-request)

1. Extract the `SESSION_COOKIE_NAME` cookie value from the request.
2. If cookie is missing or malformed, return `401 Unauthorized` immediately.
3. Call `SessionVerifier::get_session(session_id)` to look up the session in Redis.
4. If session not found or expired (Redis returns nil), return `401 Unauthorized`.
5. Deserialize the session payload to get `user_id`.
6. **Fingerprint check:** Compute SHA-256 of the request's `User-Agent` header and compare
   against the `fingerprint` field in the session payload. If the fingerprint is present in
   the session and does not match, delete the session and return `401 Unauthorized` (stolen
   cookie detected). This check applies to browser-based sessions; API clients may opt out
   via a header (deferred to Phase 2 with API tokens).
7. Call `UserRepository::find_by_id(user_id)` to load the user.
7. If user not found or `deleted_at IS NOT NULL`, return `401 Unauthorized`. **Delete the session from Redis** (`delete_session`) — a deleted user's session must not persist, even if the user is later restored.
8. If user `status != ACTIVE`, return `403 Forbidden` (the user had a valid session but
   has been deactivated). **Delete the session from Redis** (`delete_session`) — a
   deactivated user's session must not persist, even if the user is later reactivated.
9. Attach `(user_id, username)` to the request context for downstream use by handlers
   and authorization middleware.
10. Pass control to the next handler/middleware.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `PasswordHash` | Domain (1) | Value object that parses and validates the `pbkdf2$...` format string. Provides accessors: `algorithm()`, `salt()`, `iterations()`, `hash_value()`. Does **not** perform cryptographic verification — that is the responsibility of `PasswordHasher` (Application layer). |
| `Session` | Domain (1) | Entity representing an authenticated session: `session_id`, `user_id`, `created_at`, `fingerprint` (optional SHA-256 hash of User-Agent for session hijacking detection). |
| `LoginUseCase` | Application (2) | Orchestrates user lookup, password verification, and session creation. |
| `LogoutUseCase` | Application (2) | Orchestrates session deletion on logout. |
| `SessionVerifier` | Application (2) | Interface (port) for session store operations: `create_session`, `get_session`, `delete_session`. |
| `PasswordHasher` | Application (2) | Interface (port) for password hashing and verification: `hash(password) -> str`, `verify(password, hash) -> bool`. |
| `LoginHandler` | Adapters (3) | HTTP handler for `POST /api/v1/auth/login`. Validates input, calls `LoginUseCase`, sets cookie. |
| `LogoutHandler` | Adapters (3) | HTTP handler for `POST /api/v1/auth/logout`. Calls `LogoutUseCase`, clears cookie. |
| `AuthMiddleware` | Adapters (3) | Actix-Web middleware that intercepts every request. Validates session cookie and populates request context with authenticated user. |
| `RedisSessionStore` | Infrastructure (4) | Implements `SessionVerifier` using Redis commands (`SETEX`, `GET`, `DEL`). |
| `Pbkdf2Hasher` | Infrastructure (4) | Implements `PasswordHasher` using the Rust `pbkdf2` crate. Handles hash format: `pbkdf2$<algorithm>$<salt>$<iterations>$<hash>`. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `UserRepository` (infrastructure) | Add `find_by_username(username: &str)` and `find_by_email(email: &str)` methods with case-insensitive lookup. Add `update_password_hash(user_id, hash)` for re-hashing on iteration upgrade. |
| HTTP router registration | Register new auth routes (`/api/v1/auth/login`, `/api/v1/auth/logout`) — login is unauthenticated, logout requires session. |

### Compile-time interface checks (Rust convention)

```rust
// Rust's trait system verifies these implementations at compile time automatically.
// When you write `impl SessionVerifier for RedisSessionStore`, the compiler checks
// every method signature matches the trait definition. No explicit statement needed.
//
// For documentation purposes:
// - `RedisSessionStore` implements `SessionVerifier`
// - `Pbkdf2Hasher` implements `PasswordHasher`
```

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| No credentials or wrong credentials | `401` | `AUTHENTICATION_FAILED` | INFO | Generic message: `"Invalid credentials"`. Same message for wrong password, wrong username/email, inactive user, deleted user. |
| Inactive user (during middleware check) | `403` | `USER_INACTIVE` | WARN | User had a valid session but was deactivated since. Already authenticated, so 403, not 401. |
| Missing or invalid session cookie | `401` | `NOT_AUTHENTICATED` | INFO | Request to protected endpoint without session. |
| Validation error (bad request body) | `422` | `VALIDATION_ERROR` | INFO | Includes field-level detail list. |
| Redis unreachable (login) | `503` | `SESSION_STORE_UNAVAILABLE` | ERROR | Login cannot proceed without session storage. |
| Redis unreachable (session check) | `503` | `SESSION_STORE_UNAVAILABLE` | ERROR | All authenticated requests fail if Redis is down. Fallback: reject all requests until Redis recovers. |
| Internal hashing error (e.g., bad algorithm) | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation, indicates misconfiguration. |

**Anti-patterns explicitly avoided:**

- **Do not return different messages** for "wrong username" vs "wrong password" vs
  "inactive user" — all produce `AUTHENTICATION_FAILED` to prevent user enumeration.
- **Do not return 404** for authentication failures (404 implies the resource does not
  exist, which leaks information).
- **Do not log passwords** — even in error scenarios. Log only the failure reason at
  INFO level without including the credential value.
- **Do not store sensitive data in session cookies** — the cookie contains only the
  session ID (a random UUID). User data is looked up from Redis on each request.
- **CSRF protection** is provided by `SameSite=Lax` on the session cookie, which prevents
  the browser from sending the cookie on cross-site non-safe requests. No additional CSRF
  token is required for Phase 1 (the UI is server-rendered from the same origin).
- **Do not implement sliding expiration** in Phase 1 — TTL is fixed at session creation.
