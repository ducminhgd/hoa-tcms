# Tasks: Public REST API

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable
unit of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `ApiKeyScope` value object --
       `requirements.md#US-1`, `design.md#Components`
  - Validate scope string format: `{resource}:{action}` pattern
  - Validate against known permission codes from the PERMISSIONS table (loaded
    at startup or passed as a reference set)
  - No framework imports

- [ ] 2. Implement `ApiKey` entity --
       `requirements.md#US-1`, `design.md#Components`
  - Fields: `id`, `user_id`, `key_hash`, `name`, `scopes` (Vec<ApiKeyScope>),
    `expires_at` (Option<DateTime<Utc>>), `last_used_at`, `revoked_at`,
    `created_at`
  - Factory method `ApiKey::create(user_id, name, scopes, expires_at,
    plaintext_key)` -- generates SHA-256 hash from plaintext, validates scopes
    not empty, validates name not empty
  - Methods: `is_revoked() -> bool`, `is_expired() -> bool`,
    `is_valid() -> bool` (not revoked AND not expired)
  - The plaintext key is passed to `create()` but ONLY the hash is stored in
    the entity; the plaintext is returned to the caller separately
  - No ORM / framework imports

- [ ] 3. Define domain exceptions for API keys --
       `design.md#Error Handling`
  - `ApiKeyNotFoundError`
  - `DuplicateApiKeyNameError`
  - `InvalidApiKeyScopeError`
  - `ApiKeyPermissionDeniedError`

---

## Layer 2 -- Application

- [ ] 4. Define `ApiKeyRepository` interface (port) --
       `design.md#Components`, `design.md#Data Model`
  - Methods: `save(api_key)`, `find_by_id(id)`, `find_by_hash(hash)`,
    `find_by_user_id(user_id, include_revoked)` (list for user),
    `revoke(id)`, `update_last_used(id)`
  - `save` stores the key_hash, never the plaintext key
  - `update_last_used` is best-effort; failures are swallowed

- [ ] 5. Define `ApiKeyHasher` interface (port) --
       `design.md#Components`
  - Method: `fn hash(key: &str) -> String`
  - SHA-256 single-round hash (keys have sufficient entropy; no KDF needed)
  - Constant-time for the lookup comparison (comparing hashes)

- [ ] 6. Define `ApiRateLimiter` interface (port) --
       `design.md#Components`, `requirements.md#US-4`
  - Method: `async fn check(key_hash: &str) -> bool`
  - Token bucket algorithm: returns `true` if request is allowed, `false` if
    rate limit exceeded
  - Method: `async fn clear(key_hash: &str)` -- removes rate limit state for
    a revoked key
  - Rate limit parameters (RPM, burst, concurrent) are injected via config

- [ ] 7. Implement `ApiKeyService` --
       `requirements.md#US-1` through `US-4`, `design.md#Sequence`
  - `create_api_key(user_id, cmd: CreateApiKeyCommand)`: validates scopes
    against user's current permissions, generates random key, hashes it,
    saves, returns DTO with plaintext key
  - `list_api_keys(user_id, include_revoked)`: lists keys for user, excludes
    revoked by default
  - `revoke_api_key(key_id, user_id)`: ownership check, sets revoked_at,
    clears rate limiter state
  - `authenticate_api_key(key_plaintext)`: hashes key, looks up by hash,
    validates (not revoked, not expired), loads user, computes effective
    scopes (key.scopes INTERSECT user.permissions), updates last_used_at
  - Generate random key material using cryptographically secure RNG
    (`/dev/urandom` or `getrandom` crate)

- [ ] 8. Define command/query/response DTOs --
       `design.md#API Contract`
  - `CreateApiKeyCommand` (name, scopes?, expires_in_days?, never_expires?)
  - `CreateApiKeyResponse` (id, name, key -- plaintext shown only here,
    scopes, expires_at, created_at)
  - `ApiKeyResponse` (id, name, scopes, expires_at, is_expired,
    last_used_at, created_at) -- NEVER includes key or key_hash
  - `RevokeApiKeyResponse` -- no body (just 204)
  - `ApiKeyAuthContext` -- attached to request: user_id, username, scopes
    (effective), auth_method, api_key_id

- [ ] 9. Write unit tests for `ApiKeyService` --
       `requirements.md#US-1` through `US-4`
  - Test `create_api_key`: generates valid key, stores hash only, returns
    plaintext once
  - Test `create_api_key`: validates scopes against PERMISSIONS
  - Test `create_api_key`: defaults scopes to user's permissions if omitted
  - Test `create_api_key`: rejects duplicate name (case-insensitive,
    excluding revoked)
  - Test `create_api_key`: rejects name longer than 255 chars
  - Test `create_api_key`: rejects expires_in_days outside 1--730 range
  - Test `create_api_key`: rejects both expires_in_days and never_expires
  - Test `list_api_keys`: excludes revoked by default
  - Test `list_api_keys`: includes revoked when include_revoked=true
  - Test `revoke_api_key`: sets revoked_at, clears rate limiter
  - Test `revoke_api_key`: rejects cross-user revocation
  - Test `revoke_api_key`: rejects already-revoked key
  - Test `authenticate_api_key`: valid key returns auth context
  - Test `authenticate_api_key`: wrong key returns error
  - Test `authenticate_api_key`: revoked key returns error
  - Test `authenticate_api_key`: expired key returns error
  - Test `authenticate_api_key`: effective scopes = intersection of key
    scopes and user permissions
  - Mock all three interfaces

---

## Layer 3 -- Adapters (HTTP)

- [ ] 10. Implement `ApiKeyHandler` --
         `design.md#API Contract`, `design.md#Components`
  - Three handler methods: `list`, `create`, `revoke`
  - Deserialize request body/query params into DTOs
  - Call `ApiKeyService` methods
  - `create`: set `Location` header on `201 Created`
  - `revoke`: return `204 No Content`
  - Serialize responses with proper status codes

- [ ] 11. Register API key management routes in HTTP router --
         `design.md#Components`
  - `GET    /api/v1/api-keys`      -> `list`
  - `POST   /api/v1/api-keys`      -> `create`
  - `DELETE /api/v1/api-keys/{id}` -> `revoke`
  - All routes require session auth middleware
  - Route order: static before dynamic

- [ ] 12. Implement `ApiKeyAuthMiddleware` --
         `requirements.md#US-4`, `design.md#Sequence`
  - Extract `Authorization` header; if missing or not `Bearer` scheme, pass
    through (fall to next auth method)
  - Extract token, check rate limiter
  - Hash token, look up in `ApiKeyRepository`
  - Validate: exists, not revoked, not expired, user exists and is active
  - Compute effective scopes: key.scopes INTERSECT user's current permissions
  - Attach `ApiKeyAuthContext` to request extensions
  - Update `last_used_at` on key (fire and forget -- spawn async task or
    use channel)
  - On failure: return `401 INVALID_API_KEY` (same message for all failure
    modes)
  - Ensure the middleware runs before session `AuthMiddleware` in the
    middleware stack

- [ ] 13. Implement `ApiRateLimitMiddleware` --
         `requirements.md#US-4`, `design.md#Components`
  - Runs before `ApiKeyAuthMiddleware` (rate limiting before hash computation
    prevents DoS)
  - Extract key hash from token (or compute hash if needed)
  - Call `ApiRateLimiter::check(key_hash)`
  - On limit exceeded: return `429 Too Many Requests` with `Retry-After`
    header and `X-RateLimit-*` headers
  - On success: add `X-RateLimit-*` response headers
  - Only applies to requests with `Authorization: Bearer` header; session
    requests are not affected

- [ ] 14. Implement OpenAPI / Swagger documentation --
         `requirements.md#overview`, `design.md#Components`
  - Generate OpenAPI 3.x spec from route annotations (e.g., `utoipa` crate
    for Rust)
  - Include all public endpoints with request/response schemas, auth
    requirements, error codes
  - Serve Swagger UI at `GET /api/v1/docs` (HTML page)
  - Serve raw spec at `GET /api/v1/docs/openapi.json`
  - Swagger UI endpoint is accessible without authentication
  - Mark session-only endpoints (like `/api/v1/api-keys`) as requiring session
    auth; mark dual-auth endpoints as accepting either session or Bearer token

- [ ] 15. Write integration tests for API key HTTP handlers --
         `design.md#API Contract`
  - Test `POST /api/v1/api-keys`: creates key, returns 201 with Location
    header and plaintext key
  - Test `POST /api/v1/api-keys`: duplicate name returns 409
  - Test `POST /api/v1/api-keys`: invalid scopes returns 422
  - Test `POST /api/v1/api-keys`: invalid expires_in_days returns 422
  - Test `GET /api/v1/api-keys`: lists keys without key field
  - Test `GET /api/v1/api-keys`: include_revoked filter
  - Test `DELETE /api/v1/api-keys/{id}`: revokes key, returns 204
  - Test `DELETE /api/v1/api-keys/{id}`: cross-user returns 403
  - Test `DELETE /api/v1/api-keys/{id}`: already-revoked returns 404
  - Test auth: all management endpoints return 401 without session cookie
  - Test end-to-end: create key, use it to call a protected endpoint, verify
    response, revoke key, verify subsequent call returns 401

- [ ] 16. Write integration tests for Bearer token authentication --
         `requirements.md#US-4`, `design.md#Sequence`
  - Test: valid API key authenticates successfully and returns data
  - Test: invalid API key returns 401 INVALID_API_KEY
  - Test: revoked API key returns 401 INVALID_API_KEY
  - Test: expired API key returns 401 INVALID_API_KEY
  - Test: API key for deleted/inactive user returns 401 INVALID_API_KEY
  - Test: effective scopes are intersection of key scopes and user permissions
  - Test: request without Bearer token uses session auth (fallback)
  - Test: rate limit exceeded returns 429 with Retry-After header
  - Test: rate limit headers (X-RateLimit-*) present on responses

---

## Layer 4 -- Infrastructure

- [ ] 17. Create `API_KEYS` database migration --
         `design.md#Data Model`
  - Table definition with all columns, PK, FK (user_id -> users(id) ON DELETE
    CASCADE)
  - Unique index: `uq_api_keys_user_name` on (user_id, LOWER(name)) WHERE
    revoked_at IS NULL
  - Unique index: `idx_api_keys_key_hash` on (key_hash) for auth lookup
  - Index: `idx_api_keys_user_active` on (user_id, created_at DESC) WHERE
    revoked_at IS NULL
  - CHECK constraint: `chk_api_keys_scopes_not_empty` -- scopes array must
    have at least 1 element
  - Rollback migration: `DROP TABLE IF EXISTS api_keys`

- [ ] 18. Implement `SqlApiKeyRepository` --
         `design.md#Components`, `design.md#Data Model`
  - All methods from `ApiKeyRepository` interface
  - `save`: INSERT with scopes as PostgreSQL TEXT[] array
  - `find_by_hash`: lookup by key_hash for auth (hot path -- use the unique
    index)
  - `find_by_user_id`: list keys for user, optional include_revoked filter
  - `revoke`: UPDATE revoked_at = NOW() WHERE id = $1 AND revoked_at IS NULL
  - `update_last_used`: UPDATE last_used_at = NOW() WHERE id = $1 (fire and
    forget -- execute in its own short-lived transaction)
  - Write unit tests with a test transaction

- [ ] 19. Implement `Sha256ApiKeyHasher` --
         `design.md#Components`
  - `hash(key)`: compute SHA-256 of key bytes, return hex-encoded string
  - Use the Rust `sha2` crate
  - Constant-time comparison for hash matching (use
    `constant_time_eq` crate or `subtle`)
  - Write unit test: round-trip hash comparison

- [ ] 20. Implement `RedisApiRateLimiter` --
         `design.md#Components`, `requirements.md#US-4`
  - Token bucket algorithm using Redis sorted sets or plain keys with
    expiration
  - `check(key_hash)`: increment counter, check against limit, set TTL
  - Use Redis pipeline or Lua script for atomicity
  - `clear(key_hash)`: delete the rate limit key(s) for a revoked key
  - On Redis failure: log ERROR but allow the request through (fail open --
    rate limiting is protection, not security enforcement)
  - Write unit test with a mock Redis connection or embedded Redis

- [ ] 21. Wire `ApiKeyAuthMiddleware` and `ApiRateLimitMiddleware` into
         the HTTP server stack --
         `design.md#Components`
  - Middleware order: ApiRateLimitMiddleware -> ApiKeyAuthMiddleware ->
    SessionAuthMiddleware
  - ApiRateLimitMiddleware applies only to Bearer-authenticated requests
  - ApiKeyAuthMiddleware falls through to SessionAuthMiddleware if no Bearer
    token
  - Wire ApiKeyService, ApiKeyRepository, ApiKeyHasher, ApiRateLimiter into
    the dependency injection container

---

## Verification & Cleanup

- [ ] 22. End-to-end verification --
         `requirements.md#US-1` through `US-4`
  - Generate an API key via POST /api/v1/api-keys; verify plaintext returned
  - List keys via GET /api/v1/api-keys; verify plaintext NOT returned
  - Use the key to authenticate to a test case endpoint; verify response
  - Verify effective scopes limit access (key with read-only scope cannot
    create)
  - Revoke the key; verify subsequent auth returns 401
  - Test rate limiting: send rapid requests, verify 429 response
  - Test Swagger UI accessible at /api/v1/docs
  - Test key expiry: create key with 1-day expiry, verify it expires

- [ ] 23. Update `specs/README.md` -- `specs/README.md`
  - Mark `public-api` as having completed specs (requirements.md, design.md,
    tasks.md)

---

## Security & Hardening

- [ ] 24. Implement scope intersection on auth --
         `requirements.md#US-4`, `design.md#Sequence`
  - Every API-key-authenticated request computes effective scopes as
    `key.scopes INTERSECT user.current_permissions`
  - If user's permissions have been downgraded since key creation, the key
    loses those permissions
  - Integration test: create key with broad scopes, downgrade user
    permissions, verify key's effective scopes are reduced

- [ ] 25. Implement API key hash constant-time comparison --
         `design.md#Components`
  - SHA-256 hash comparison must use constant-time equality check to prevent
    timing attacks on key lookup
  - Use `subtle::ConstantTimeEq` or equivalent
  - Unit test: verify constant-time comparison (or at minimum, verify
    comparison does not short-circuit)
