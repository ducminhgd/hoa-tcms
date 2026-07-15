# Design: Public REST API

## Architecture

The Public API feature adds API key management (CRUD for keys) and Bearer token
authentication that coexists with session-based auth. It introduces a new
middleware for API key validation and rate limiting, and new endpoints for key
management.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_api_keys   GET    /api/v1/api-keys                           │   │
│  │  - create_api_key  POST   /api/v1/api-keys                           │   │
│  │  - revoke_api_key  DELETE /api/v1/api-keys/{id}                      │   │
│  │  - swagger_ui      GET    /api/v1/docs                               │   │
│  │  - openapi_spec    GET    /api/v1/docs/openapi.json                   │   │
│  │                                                                        │   │
│  │  Middleware:                                                           │   │
│  │  - ApiKeyAuthMiddleware  Bearer token → user context + scopes         │   │
│  │  - ApiRateLimitMiddleware  Per-key rate limiting (token bucket)       │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  ApiKeyService:                           │  │  - ApiKey (entity)      │  │
│  │  - create_api_key                         │  │  - ApiKeyScope (vo)     │  │
│  │  - list_api_keys                          │  └──────────────────────────┘  │
│  │  - revoke_api_key                         │                                │
│  │  - authenticate_api_key                   │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - ApiKeyRepository (port)                │                                │
│  │  - ApiKeyHasher (port)                    │                                │
│  │  - ApiRateLimiter (port)                  │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlApiKeyRepository  (implements ApiKeyRepository)                  │   │
│  │  - Sha256ApiKeyHasher   (implements ApiKeyHasher: SHA-256 hash)        │   │
│  │  - RedisApiRateLimiter  (implements ApiRateLimiter: token bucket)      │   │
│  │  - OpenApiDocGenerator  (generates openapi.json from route metadata)   │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. API key management endpoints (create, list, revoke) require session
   authentication. Users manage their own keys.
2. API-key-authenticated endpoints accept `Authorization: Bearer hkt_...` header.
3. `ApiKeyAuthMiddleware` intercepts requests, hashes the key, looks up the hash
   in `API_KEYS`, validates (not revoked, not expired), and attaches user context
   + effective scopes to the request.
4. If no Bearer token is present, the middleware falls through to session auth
   (both auth methods coexist).
5. `ApiRateLimitMiddleware` enforces per-key rate limits using a Redis token
   bucket.
6. OpenAPI documentation is served at `/api/v1/docs` (Swagger UI) and
   `/api/v1/docs/openapi.json` (raw spec).

---

## API Contract

---

### POST `/api/v1/api-keys`

Generate a new API key.

**Authentication:** Required (session)

**Request Body:**

```json
{
  "name": "CI/CD Pipeline",
  "scopes": ["test_case:read", "test_case:create", "test_execution:read"],
  "expires_in_days": 365
}
```

| Field | Type | Required | Default | Constraints |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | -- | 1--255 chars; unique per user (case-insensitive, excluding revoked) |
| `scopes` | string[] | No | User's current permissions | Array of valid permission codes from PERMISSIONS table; min 1 element |
| `expires_in_days` | integer | No | 365 | 1--730 (2 years max) |
| `never_expires` | boolean | No | false | If true, `expires_at` is NULL; mutually exclusive with `expires_in_days` |

**Success Response:** `201 Created`

Headers: `Location: /api/v1/api-keys/1`

```json
{
  "data": {
    "id": 1,
    "name": "CI/CD Pipeline",
    "key": "hkt_abc123def456ghi789jkl012mno345pqr678stu",
    "scopes": ["test_case:read", "test_case:create", "test_execution:read"],
    "expires_at": "2027-07-15T00:00:00Z",
    "created_at": "2026-07-15T10:00:00Z"
  }
}
```

**Notes:**
- The `key` field contains the **plaintext** API key. This is the only response
  that includes it. The client must store it securely.
- The key prefix `hkt_` identifies this as an HOA TCMS API key.
- The generated key is 32 random bytes, base64url-encoded (45 chars after prefix).
- The `key_hash` stored in the database is `SHA-256(key_plaintext)`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `409` | `DUPLICATE_API_KEY_NAME` | Key name already exists for user (excluding revoked) |
| `422` | `VALIDATION_ERROR` | Invalid name, scopes, expires_in_days, or never_expires |

---

### GET `/api/v1/api-keys`

List API keys for the authenticated user.

**Authentication:** Required (session)

**Query Parameters:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `include_revoked` | boolean | `false` | Include revoked keys in the list |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 1,
      "name": "CI/CD Pipeline",
      "scopes": ["test_case:read", "test_case:create"],
      "expires_at": "2027-07-15T00:00:00Z",
      "is_expired": false,
      "last_used_at": "2026-07-15T11:00:00Z",
      "created_at": "2026-07-15T10:00:00Z"
    }
  ]
}
```

**Notes:**
- The `key` and `key_hash` fields are never returned.
- `is_expired` is computed: `true` if `expires_at IS NOT NULL AND expires_at <
  NOW()`.
- Revoked keys are excluded unless `include_revoked=true`.
- Results are ordered by `created_at DESC`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |

---

### DELETE `/api/v1/api-keys/{id}`

Revoke an API key.

**Authentication:** Required (session)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | API key ID |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Sets `revoked_at = NOW()`. Does not hard-delete the record.
- Repeated DELETE on an already revoked key returns `404 Not Found`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | Key belongs to a different user |
| `404` | `NOT_FOUND` | Key does not exist or is already revoked |

---

### Bearer Token Authentication (all endpoints)

Any existing endpoint (test cases, test runs, etc.) that is also exposed via the
public API accepts either a session cookie or a Bearer token.

**Request Header:**

```
Authorization: Bearer hkt_abc123def456ghi789jkl012mno345pqr678stu
```

**On success:** The request proceeds with the authenticated user identity and
the key's effective scopes as permissions.

**On failure:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `INVALID_API_KEY` | Key not found, revoked, or expired |

---

## Data Model

### New Tables

#### API_KEYS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `user_id` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE CASCADE` | Key owner |
| `key_hash` | `VARCHAR(64)` | `NOT NULL`, `UNIQUE` | SHA-256 hash of the plaintext API key |
| `name` | `VARCHAR(255)` | `NOT NULL` | Human-readable label |
| `scopes` | `TEXT[]` | `NOT NULL` | Array of permission codes; validated against PERMISSIONS table |
| `expires_at` | `TIMESTAMPTZ` | | Nullable; NULL means never expires |
| `last_used_at` | `TIMESTAMPTZ` | | Updated on each successful auth |
| `revoked_at` | `TIMESTAMPTZ` | | Set on revocation; NULL means active |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |

**Constraints:**

```sql
-- Unique key name per user (excluding revoked keys)
CREATE UNIQUE INDEX uq_api_keys_user_name
  ON api_keys (user_id, LOWER(name)) WHERE revoked_at IS NULL;

-- Index for key lookup on auth (the hot path)
CREATE UNIQUE INDEX idx_api_keys_key_hash
  ON api_keys (key_hash);

-- Index for listing a user's keys
CREATE INDEX idx_api_keys_user_active
  ON api_keys (user_id, created_at DESC) WHERE revoked_at IS NULL;

-- Check constraint: scopes array must not be empty
ALTER TABLE api_keys ADD CONSTRAINT chk_api_keys_scopes_not_empty
  CHECK (array_length(scopes, 1) > 0);
```

**Design notes:**
- `key_hash` is a SHA-256 hash, always 64 hex characters. It is unique globally
  (collision probability is negligible for 256-bit hashes of 256-bit random
  values).
- `scopes` uses PostgreSQL `TEXT[]` (array type) for efficient storage and
  indexing. Each scope is a permission code like `test_case:read`.
- `ON DELETE CASCADE` on `user_id` FK: if a user is deleted, all their API keys
  are deleted.
- No `updated_at` -- keys are immutable after creation (only `last_used_at`,
  `revoked_at` change).
- The plaintext API key is **never stored**. Only `SHA-256(key)` is persisted.

---

## Sequence

### API Key Authentication Flow (per-request)

1. Request arrives at `ApiKeyAuthMiddleware`.
2. Middleware checks for `Authorization` header.
3. If missing or not `Bearer` scheme, fall through to session-based auth (end).
4. Extract the token value from the header.
5. **Rate limiting check:**
   a. Invalid or missing API keys are rate-limited **per IP address** (100 req/min)
      **before** hash computation. This prevents DoS attacks that would force expensive
      hash computations on random tokens.
   b. Valid API keys are rate-limited **per key_hash** (token bucket in Redis).
   c. If either bucket is exhausted, return `429 Too Many Requests` with `Retry-After`.
6. Compute `SHA-256(token)`.
7. Call `ApiKeyRepository::find_by_hash(hash)`.
8. If not found, return `401 INVALID_API_KEY`.
9. If `revoked_at IS NOT NULL`, return `401 INVALID_API_KEY` (same error).
10. If `expires_at IS NOT NULL AND expires_at < NOW()`, return `401 INVALID_API_KEY`
    (same error).
11. Load the key owner: `UserRepository::find_by_id(key.user_id)`.
12. If user not found, deleted, or INACTIVE, return `401 INVALID_API_KEY`.
13. Compute effective scopes: `key.scopes INTERSECT user's_current_permissions`.
14. Attach to request context: `{ user_id, username, scopes (effective),
    auth_method: "api_key", api_key_id }`.
15. Update `last_used_at` on the key (best-effort: fire and forget; failure does
    not fail the request).
16. Pass control to downstream handler.

### API Key Creation Flow

1. Client sends `POST /api/v1/api-keys` with session cookie.
2. Handler validates request body (name, scopes, expires_in_days, never_expires).
3. Handler calls `ApiKeyService::create_api_key(user_id, cmd)`.
4. Service validates scopes against the PERMISSIONS table and user's current
   permissions (if scopes are provided, each must be a valid code the user
   currently holds).
5. Service generates 32 random bytes via `crypto/rand`.
6. Service formats key as `hkt_` + base64url-encoded random bytes.
7. Service computes `key_hash = SHA-256(plaintext_key)` via `ApiKeyHasher`.
8. Service calls `ApiKeyRepository::save(api_key)` -- stores the hash, never the
   plaintext.
9. Service returns the API key entity with the plaintext key attached to the DTO.
10. Handler returns `201 Created` with the plaintext key in the response body.
    The plaintext key is not stored, logged, or persisted.

### API Key Revocation Flow

1. Client sends `DELETE /api/v1/api-keys/{id}` with session cookie.
2. Handler calls `ApiKeyService::revoke_api_key(key_id, user_id)`.
3. Service calls `ApiKeyRepository::find_by_id(key_id)`.
4. If not found or `revoked_at IS NOT NULL`, return `NotFoundError`.
5. If `key.user_id != user_id`, return `ForbiddenError`.
6. Service calls `ApiKeyRepository::revoke(key_id)` -- sets `revoked_at = NOW()`.
7. Service also calls `ApiRateLimiter::clear(key_hash)` to remove any cached rate
   limit state for the revoked key.
8. Handler returns `204 No Content`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `ApiKey` | Domain (1) | Entity: `id`, `user_id`, `key_hash`, `name`, `scopes`, `expires_at`, `last_used_at`, `revoked_at`, `created_at`. Factory method `create(user_id, name, scopes, expires_at, plaintext_key)` generates the hash and validates scopes. |
| `ApiKeyScope` | Domain (1) | Value object: validates that a scope string matches the pattern `{resource}:{action}` and is a known permission code. |
| `ApiKeyService` | Application (2) | Orchestrates: `create_api_key`, `list_api_keys`, `revoke_api_key`, `authenticate_api_key`. The authenticate method is called by the middleware. |
| `ApiKeyRepository` | Application (2) | Interface (port): `save`, `find_by_id`, `find_by_hash`, `find_by_user_id`, `revoke`, `update_last_used`. |
| `ApiKeyHasher` | Application (2) | Interface (port): `hash(key: &str) -> String` (SHA-256). |
| `ApiRateLimiter` | Application (2) | Interface (port): `check(key_hash, current_time) -> bool`, `clear(key_hash)`. Token bucket algorithm. |
| `ApiKeyHandler` | Adapters (3) | HTTP handler: `list`, `create`, `revoke` methods. |
| `ApiKeyAuthMiddleware` | Adapters (3) | Middleware: extracts Bearer token, hashes it, validates against DB, attaches user context + scopes. Falls through to session auth if no Bearer token. |
| `ApiRateLimitMiddleware` | Adapters (3) | Middleware: enforces two-tier rate limiting. Invalid/missing keys are rate-limited per IP before hash computation. Valid keys are rate-limited per key_hash after hash lookup. This prevents hash-computation DoS attacks. |
| `SwaggerHandler` | Adapters (3) | Serves Swagger UI HTML and `openapi.json` at `/api/v1/docs`. |
| `SqlApiKeyRepository` | Infrastructure (4) | Implements `ApiKeyRepository` using SQLx/Diesel. |
| `Sha256ApiKeyHasher` | Infrastructure (4) | Implements `ApiKeyHasher` using SHA-256 from the standard library or `sha2` crate. |
| `RedisApiRateLimiter` | Infrastructure (4) | Implements `ApiRateLimiter` using Redis for token bucket counters. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthMiddleware` (session) | Must coexist with `ApiKeyAuthMiddleware`. Order: ApiKey auth middleware runs first; if no Bearer token, falls through to session auth. |
| Authorization service | When request is API-key-authenticated, use the key's effective scopes instead of the user's full permissions. |
| HTTP router registration | Register three new key management routes under `/api/v1/api-keys/` (session auth). Register Swagger UI routes (no auth). All other routes must be accessible via both session and API key auth. |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session (key management) | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| Missing or invalid Bearer token | `401` | `INVALID_API_KEY` | INFO | Generic message; same for not-found, revoked, expired |
| Key belongs to different user | `403` | `FORBIDDEN` | INFO | On management endpoints only |
| Key not found or already revoked | `404` | `NOT_FOUND` | INFO | On management endpoints only |
| Duplicate key name | `409` | `DUPLICATE_API_KEY_NAME` | INFO | Per-user uniqueness |
| Validation error | `422` | `VALIDATION_ERROR` | INFO | Field-level details |
| Rate limit exceeded | `429` | `RATE_LIMIT_EXCEEDED` | INFO | Includes Retry-After header |
| Internal hashing error | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not** store the plaintext API key in the database, logs, or response after
  creation.
- **Do not** return different error messages for "key not found", "key revoked",
  or "key expired" -- all return `401 INVALID_API_KEY`.
- **Do not** allow a key's scopes to exceed the user's current permissions. The
  intersection check is mandatory on every request.
- **Do not** apply session-based CSRF to Bearer-authenticated requests (CSRF does
  not apply to stateless token auth).
- **Do not** hard-delete API keys. Revocation sets `revoked_at` and preserves the
  audit record.

---

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `API_KEY_PREFIX` | `hkt_` | Prefix for generated API keys |
| `API_KEY_LENGTH_BYTES` | `32` | Number of random bytes for key generation |
| `API_IP_RATE_LIMIT_RPM` | `100` | Requests per IP per minute for invalid/missing API keys (DoS protection before hash computation) |
| `API_RATE_LIMIT_RPM` | `60` | Requests per key_hash per minute for valid API keys |
| `API_RATE_LIMIT_BURST` | `120` | Burst capacity (token bucket max) |
| `API_RATE_LIMIT_CONCURRENT` | `10` | Maximum concurrent requests per key |
| `API_RATE_LIMIT_REDIS_URL` | (same as session Redis) | Redis connection for rate limit counters |
| `SWAGGER_UI_ENABLED` | `true` | Whether to serve Swagger UI at /api/v1/docs |
| `OPENAPI_TITLE` | `HOA TCMS API` | Title in the OpenAPI spec |
| `OPENAPI_VERSION` | `1.0.0` | Version in the OpenAPI spec |
