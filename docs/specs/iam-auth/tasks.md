# Tasks: IAM Auth

> **Dependency note:** This feature depends on the `iam-users` feature for the `USERS` table
> schema and the `UserRepository` base implementation. Those must be at least partially
> complete before tasks 4–8 can begin.

---

## Domain Layer

- [ ] 1. **Define `PasswordHash` value object** — design.md#data-model
      - Parse the `pbkdf2$<algorithm>$<salt>$<iterations>$<hash>` format string.
      - Provide `algorithm()`, `salt()`, `iterations()`, `hash_value()` accessors.
      - Validate format on construction (return `Result` / `Option` for malformed strings).
      - **Do not** implement cryptographic verification here — `PasswordHash` is a pure
        format parser/validator. Verification is handled by `PasswordHasher` (Application).

- [ ] 2. **Define `Session` entity** — design.md#data-model
      - Fields: `session_id` (UUID), `user_id` (i64), `created_at` (DateTime<Utc>),
        `fingerprint` (Option<String> — SHA-256 of User-Agent for hijacking detection).
      - Constructor: `Session::new(user_id: i64, fingerprint: Option<String>) -> Self`.

## Application Layer

- [ ] 3. **Define `PasswordHasher` interface (port)** — design.md#components
      - Methods: `hash(password: &str) -> Result<String>`, `verify(password: &str, hash: &str) -> bool`.
      - The `verify` method uses constant-time comparison.

- [ ] 4. **Define `SessionVerifier` interface (port)** — design.md#components
      - Methods:
        - `create_session(user_id: i64, fingerprint: Option<String>) -> Result<Session>`
        - `get_session(session_id: &str) -> Result<Option<Session>>`
        - `delete_session(session_id: &str) -> Result<()>`
        - `delete_all_user_sessions(user_id: i64) -> Result<()>` — delete **all** sessions
          for a user. Called on password change, user deactivation, or user soft-delete
          to forcibly terminate all active sessions.

- [ ] 5. **Implement `LoginUseCase`** — design.md#sequence, requirements.md#US-01
      - Receive either `username` or `email` + `password`.
      - Call `UserRepository::find_by_username()` or `UserRepository::find_by_email()` (case-insensitive).
      - If user not found, return `AuthenticationFailed` error.
      - If user is INACTIVE or deleted, return `AuthenticationFailed` error (same error).
      - Call `PasswordHasher::verify()`; on failure return `AuthenticationFailed`.
      - On success, compute fingerprint SHA-256 hash from `User-Agent` header (passed in
        from handler) and call `SessionVerifier::create_session()` with fingerprint.
      - After successful verification, check if PBKDF2 iterations have increased; if so,
        re-hash password and call `UserRepository::update_password_hash()`.

- [ ] 6. **Implement `LogoutUseCase`** — design.md#sequence, requirements.md#US-02
      - Receive `session_id`.
      - Call `SessionVerifier::delete_session()`.

## Infrastructure Layer

- [ ] 7. **Add auth-related methods to `UserRepository`** — design.md#components, requirements.md#US-01
      - `find_by_username(username: &str) -> Result<Option<User>>` — case-insensitive lookup.
      - `find_by_email(email: &str) -> Result<Option<User>>` — case-insensitive lookup.
      - `update_password_hash(user_id: i64, new_hash: &str) -> Result<()>`.
      - Use parameterized queries (`$1`, `$2` placeholders) — never format strings into SQL.

- [ ] 8. **Implement `Pbkdf2Hasher`** — design.md#components, requirements.md#US-04
      - Use the Rust `pbkdf2` crate (with `sha2` for SHA-256).
      - `hash(password)`: generate random salt (16+ bytes via `crypto::rand`), hash with
        configured iterations, format as `pbkdf2$<algorithm>$<salt>$<iterations>$<hash>`.
      - `verify(password, hash)`: parse the stored hash string, extract parameters,
        re-derive key with the same salt/iterations, compare with constant-time function.

- [ ] 9. **Implement `RedisSessionStore`** — design.md#components, design.md#redis-session-schema
      - Use the Rust `redis` crate (or `bb8-redis` for connection pooling).
      - `create_session(user_id, fingerprint)`: generate UUID v4 for session ID, compute
        fingerprint hash, store JSON payload in Redis at `session:<uuid>` with `SETEX`
        (set + TTL).
      - `get_session(session_id)`: `GET` the key, deserialize JSON. Return `None` if key
        does not exist. Include fingerprint validation in the deserialized session.
      - `delete_session(session_id)`: `DEL` the key.
      - `delete_all_user_sessions(user_id)`: `SCAN` for `session:*` keys, filter by
        `user_id` in the JSON payload, `DEL` matching keys. Use `UNLINK` for non-blocking
        deletion in production.
      - TTL is read from `SESSION_TTL_SECONDS` environment variable.
      - Redis connection uses TLS and password authentication as configured via environment
        variables (`REDIS_TLS_ENABLED`, `REDIS_PASSWORD`).

## Adapters Layer

- [ ] 10. **Define login/logout request/response DTOs** — design.md#api-contract
       - `LoginRequest`: `username: Option<String>`, `email: Option<String>`, `password: String`.
       - `LoginResponse`: `user_id, username, fullname`.
       - `LogoutResponse`: `message`.
       - Error response structs matching the `{ "error": { "code", "message", "details" } }` format.

- [ ] 11. **Implement `LoginHandler` (POST /api/v1/auth/login)** — design.md#api-contract, requirements.md#US-01
       - Validate Content-Type is `application/json`.
       - Deserialize request body into `LoginRequest`.
       - Validate: exactly one of `username`/`email` must be provided; `password` >= 8 chars.
       - Call `LoginUseCase::execute()`.
       - On success: construct `Set-Cookie` header from session config, return `200 OK`
         with `LoginResponse` payload.
       - On `AuthenticationFailed`: return `401` with generic error.
       - On validation error: return `422` with field-level details.
       - On session store unavailable: return `503`.
       - This handler is **unauthenticated** (no session required).

- [ ] 12. **Implement `LogoutHandler` (POST /api/v1/auth/logout)** — design.md#api-contract, requirements.md#US-02
       - Call `LogoutUseCase::execute(session_id)`.
       - On success: set `Set-Cookie` with `Max-Age=0` to clear the cookie, return `200 OK`.
       - Requires a valid session (auth middleware runs before this handler).

- [ ] 13. **Implement `AuthMiddleware`** — design.md#sequence, requirements.md#US-03
       - Extract session cookie by configured name.
       - If missing/malformed: return `401 NOT_AUTHENTICATED`.
       - Call `SessionVerifier::get_session()` with session ID.
       - If session not found/expired: return `401`.
       - **Fingerprint check:** Compute SHA-256 of request `User-Agent` header. If session
         has a fingerprint stored and it does not match the computed value, delete the session
         and return `401` (stolen cookie).
       - Load user by ID from `UserRepository::find_by_id()`.
       - If user not found or deleted: delete session from Redis, return `401`.
       - If user status is INACTIVE: delete session from Redis, return `403 USER_INACTIVE`.
       - If all checks pass: insert `AuthUser { user_id, username }` into the Actix-Web
         request extensions for downstream handlers and middleware.

## Wiring

- [ ] 14. **Register auth routes and middleware in the HTTP server** — design.md#components
       - Add `POST /api/v1/auth/login` route (unauthenticated).
       - Add `POST /api/v1/auth/logout` route (authenticated — behind `AuthMiddleware`).
       - Register `AuthMiddleware` as a global middleware (applied to all routes except
         `/api/v1/auth/login`).
       - Wire `LoginUseCase`, `LogoutUseCase`, `RedisSessionStore`, `Pbkdf2Hasher`,
         `UserRepository` into the dependency injection container.

## Testing

- [ ] 15. **Unit test: `PasswordHash` parsing and format validation** — design.md#data-model
       - Valid format strings parse correctly.
       - Malformed strings (missing fields, wrong separators) return errors.
       - Edge: empty string, extra fields, invalid base64, non-numeric iterations.

- [ ] 16. **Unit test: `Pbkdf2Hasher` hash and verify round-trip** — design.md#components, requirements.md#US-04
       - `hash()` produces a string matching the `pbkdf2$...` format.
       - `hash()` produces different salts on each call (same password, different hashes).
       - `verify()` returns `true` for a correctly hashed password.
       - `verify()` returns `false` for a wrong password.
       - `verify()` returns `false` for a malformed hash string.

- [ ] 17. **Unit test: `LoginUseCase`** — requirements.md#US-01
       - Successful login with username returns session.
       - Successful login with email returns session.
       - Case-insensitive username lookup succeeds.
       - Case-insensitive email lookup succeeds.
       - Wrong password returns `AuthenticationFailed`.
       - Non-existent username returns `AuthenticationFailed`.
       - Non-existent email returns `AuthenticationFailed`.
       - INACTIVE user returns `AuthenticationFailed` (same error as wrong password).
       - Deleted user (`deleted_at IS NOT NULL`) returns `AuthenticationFailed`.
       - Password re-hash triggered when configured iterations exceed stored iterations.
       - Session store failure propagates as error.
       - All `UserRepository` calls use parameterized queries (tested via integration).

- [ ] 18. **Unit test: `LogoutUseCase`** — requirements.md#US-02
       - Successful logout calls `delete_session` and returns success.
       - Error from session store propagates.

- [ ] 19. **Unit test: `AuthMiddleware` (with mock session store)** — requirements.md#US-03
       - Missing cookie returns `401`.
       - Invalid session ID (not in Redis) returns `401`.
       - Valid session but user not found in DB returns `401`.
       - Valid session but user INACTIVE returns `403`.
       - Valid session and active user passes request through with `AuthUser` in context.

- [ ] 20. **Integration test: Login endpoint with real Redis** — requirements.md#US-01, design.md#api-contract
       - Successful request returns `200` with `Set-Cookie` header.
       - Request with both `username` and `email` returns `422`.
       - Request with neither `username` nor `email` returns `422`.
       - Request with short password returns `422`.
       - Request with wrong password returns `401` with `"Invalid credentials"`.
       - Request with inactive user returns `401` with `"Invalid credentials"`.
       - Request for non-existent user returns `401` with `"Invalid credentials"`.
       - Set-Cookie header contains `HttpOnly`, `Secure`, `SameSite=Lax`.
       - Login with email (case-insensitive) succeeds.

- [ ] 21. **Integration test: Logout endpoint with real Redis** — requirements.md#US-02, design.md#api-contract
       - Successful logout returns `200` and clears the cookie (`Max-Age=0`).
       - Logout without session cookie returns `401`.
       - Logout with expired session returns `401`.
       - After logout, the session key is removed from Redis.

- [ ] 22. **Integration test: Auth middleware end-to-end** — requirements.md#US-03
       - Request to protected endpoint without cookie returns `401`.
       - Request with valid session cookie succeeds.
       - Request with expired session cookie returns `401`.
       - Request with valid session but inactive user returns `403`.
