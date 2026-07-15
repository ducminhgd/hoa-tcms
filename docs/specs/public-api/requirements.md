# Feature: Public REST API

## Overview

The Public REST API enables third-party integrations (CI/CD pipelines, automation
frameworks, external reporting tools) to interact with HOA TCMS programmatically.
Users can generate and manage API keys with scoped permissions, authenticate via
`Bearer` token, and access a documented, versioned REST API. Rate limiting is
enforced per API key. The API is documented with OpenAPI 3.x (Swagger).

---

## User Stories

### US-1: Generate API Key

As an authenticated user, I want to generate a new API key with a name and a set of
scoped permissions, so that I can grant programmatic access to third-party tools
without exposing my session credentials.

**Acceptance Criteria (EARS)**

- WHEN a user sends `POST /api/v1/api-keys` with `name` and optional `scopes`
  (array of permission codes) and optional `expires_in_days`, THE SYSTEM SHALL
  generate a cryptographically random API key (minimum 32 bytes, base64-encoded),
  store its SHA-256 hash in the `API_KEYS` table (never store the plaintext key),
  and return `201 Created` with the generated key in the response body. The
  plaintext key is shown only once in this response.
- IF the user is not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.
- IF `name` is empty, contains only whitespace, or exceeds 255 characters, THE
  SYSTEM SHALL return `422 Unprocessable Entity` with field-level validation
  details. Leading and trailing whitespace is trimmed before length validation and
  storage.
- IF a key with the same `name` already exists for the user (case-insensitive,
  excluding revoked keys), THE SYSTEM SHALL return `409 Conflict` with error code
  `DUPLICATE_API_KEY_NAME`.
- IF `scopes` is provided: THE SYSTEM SHALL validate that each scope is a known
  permission code (from the seeded `PERMISSIONS` table). Unknown scopes return
  `422 Unprocessable Entity`. If `scopes` is empty (`[]`), THE SYSTEM SHALL return
  `422` (at least one scope is required).
- IF `scopes` is omitted, THE SYSTEM SHALL default the scopes to the user's
  currently assigned permissions (from direct roles and group-inherited roles).
  This is the safest default: a key cannot gain permissions the user does not have.
- IF `expires_in_days` is provided: THE SYSTEM SHALL set `expires_at = NOW() +
  expires_in_days days`. The value must be between 1 and 730 (2 years). Values
  outside this range return `422`.
- IF `expires_in_days` is omitted, THE SYSTEM SHALL default to 365 days (1 year).
- THE SYSTEM SHALL support an optional `never_expires` boolean flag (default
  `false`). If `true`, `expires_at` is set to `NULL`. If `true` and
  `expires_in_days` is also provided, `expires_in_days` takes precedence (the two
  are mutually exclusive in intent; providing both returns `422`).
- The response SHALL include the plaintext key exactly once:
  ```json
  {
    "data": {
      "id": 1,
      "name": "CI/CD Pipeline",
      "key": "hkt_abc123def456ghi789jkl012mno345pqr678stu",
      "scopes": ["test_case:read", "test_case:create"],
      "expires_at": "2027-07-15T00:00:00Z",
      "created_at": "2026-07-15T10:00:00Z"
    }
  }
  ```
  The `key` field contains the plaintext API key. The key prefix `hkt_` is a
  human-readable identifier for key type detection.
- THE SYSTEM SHALL NOT include the `key` field in any subsequent API response
  (list, get). After the create response, the plaintext key is irretrievable.
  Users must store it securely at generation time.

### US-2: List and View API Keys

As an authenticated user, I want to view my generated API keys (with metadata but
without the secret key value), so that I can audit my keys, check their expiry
status, and identify which keys to revoke.

**Acceptance Criteria (EARS)**

- WHEN a user sends `GET /api/v1/api-keys`, THE SYSTEM SHALL return a list of
  non-revoked API keys belonging to the authenticated user, ordered by `created_at`
  descending, each containing `id`, `name`, `scopes`, `expires_at`, `last_used_at`,
  and `created_at`.
- IF the user is not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.
- THE SYSTEM SHALL NOT return the `key_hash` or the plaintext key in any list/detail
  response.
- THE SYSTEM SHALL include an `is_expired` computed boolean field in each key object
  (`true` if `expires_at IS NOT NULL AND expires_at < NOW()`).
- THE SYSTEM SHALL exclude revoked keys (`revoked_at IS NOT NULL`) from the list by
  default, but support an optional `include_revoked` query parameter (boolean
  `true`). Default is `false`.

### US-3: Revoke API Key

As an authenticated user, I want to revoke an API key I previously generated, so
that I can immediately terminate access for a compromised or deprecated key without
deleting the audit record.

**Acceptance Criteria (EARS)**

- WHEN a user sends `DELETE /api/v1/api-keys/{id}`, THE SYSTEM SHALL set
  `revoked_at = NOW()` on the key record (soft-revoke, preserving the audit record)
  and return `204 No Content`.
- IF the user is not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.
- IF the key does not exist, THE SYSTEM SHALL return `404 Not Found`.
- IF the key's `user_id` does not match the authenticated user's ID, THE SYSTEM
  SHALL return `403 Forbidden` (users cannot revoke other users' keys).
- IF the key is already revoked, THE SYSTEM SHALL return `404 Not Found` (same
  response as non-existent to avoid revealing key state).
- After revocation, THE SYSTEM SHALL immediately reject authentication attempts
  using this key (return `401 Unauthorized`).

### US-4: Authenticate via API Key

As a third-party integration using an API key, I want to authenticate my requests
by passing the API key as a Bearer token, so that I can call HOA TCMS API endpoints
programmatically.

**Acceptance Criteria (EARS)**

- WHEN a request includes an `Authorization: Bearer <api_key>` header, THE SYSTEM
  SHALL compute the SHA-256 hash of the key material, look up the hash in the
  `API_KEYS` table, and if found and valid (not revoked, not expired), authenticate
  the request as the key's owner user with the key's scoped permissions.
- IF the `Authorization` header is missing or does not use the `Bearer` scheme, THE
  SYSTEM SHALL fall through to session-based authentication (this feature coexists
  with session auth; it does not replace it).
- IF the key is not found in the database (hash mismatch), THE SYSTEM SHALL return
  `401 Unauthorized` with error code `INVALID_API_KEY`.
- IF the key is found but `revoked_at IS NOT NULL` or (`expires_at IS NOT NULL AND
  expires_at < NOW()`), THE SYSTEM SHALL return `401 Unauthorized` with error code
  `INVALID_API_KEY` (same error as hash mismatch -- do not reveal whether the key
  is revoked or expired).
- WHEN a request is authenticated via API key, THE SYSTEM SHALL restrict the
  effective permissions to the intersection of the key's scopes and the user's
  current permissions. A key cannot grant permissions the user no longer possesses.
- WHEN a request is authenticated via API key, THE SYSTEM SHALL update
  `last_used_at = NOW()` on the key record. This update is best-effort (fire and
  forget; failure to update `last_used_at` does not fail the request).
- THE SYSTEM SHALL apply rate limiting per API key (see Security Considerations).

---

## Security Considerations

### API Key Generation
API keys are generated using a cryptographically secure random number generator
(`/dev/urandom` or equivalent). The key format is `hkt_` prefix followed by 45
characters of base64url-encoded random bytes (32 bytes of entropy). Example:
`hkt_abc123def456ghi789jkl012mno345pqr678stu`. The `hkt_` prefix enables key type
identification in logs and auditing.

### API Key Storage
Only the SHA-256 hash of the API key is stored in the database. The plaintext key
is never stored, never logged, and never returned after the initial creation
response. Key hashing uses a single round of SHA-256 (not PBKDF2 -- keys have
sufficient entropy that a KDF is unnecessary; SHA-256 is faster for lookup on every
API request).

### Scope Restriction
API key scopes are validated against the user's current permissions on every
request. If a user's permissions are downgraded after a key is generated, the key's
effective permissions are limited to the intersection. This prevents privilege
escalation from stale keys.

### Rate Limiting

| Limit Type | Default | Configurable |
|-----------|---------|-------------|
| Requests per key per minute | 60 | Yes (`API_RATE_LIMIT_RPM`) |
| Requests per key per minute (burst) | 120 | Yes (`API_RATE_LIMIT_BURST`) |
| Concurrent requests per key | 10 | Yes (`API_RATE_LIMIT_CONCURRENT`) |

Rate limit headers (`X-RateLimit-Limit`, `X-RateLimit-Remaining`,
`X-RateLimit-Reset`) are returned on all API-key-authenticated responses. Exceeding
the limit returns `429 Too Many Requests` with a `Retry-After` header.

### API Key Rotation
Users are encouraged to rotate keys periodically. The system supports having
multiple active keys per user, so a new key can be generated before the old key is
revoked (zero-downtime rotation).

### CSRF Protection
API-key-authenticated requests are stateless (no cookies). CSRF does not apply to
Bearer token authentication. CSRF protection remains active for session-based
requests only.

### Security Headers
All API responses carry standard security headers (see RESTful API best practices
§42): `Strict-Transport-Security`, `X-Content-Type-Options: nosniff`,
`X-Frame-Options: DENY`.

---

## Out of Scope

- **OAuth 2.0 / OpenID Connect** (Bearer token via API key in Phase 3; OAuth
  deferred to a future phase)
- **API key usage analytics** (request count per key, endpoint breakdown --
  deferred)
- **IP allowlisting / CIDR restrictions** on API keys (deferred to a future phase)
- **Per-endpoint scope configuration** (scopes are coarse-grained permission codes
  in Phase 3; fine-grained endpoint-level scoping deferred)
- **API version negotiation via headers** (only URI versioning `/v1/` in Phase 3)
- **Webhook / callback registration** (outbound API calls from HOA TCMS to external
  systems -- separate feature)
- **API key last-used details** (only `last_used_at` timestamp; full request log
  with IP, endpoint, user-agent deferred)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; the API key auth middleware
  coexists with `AuthMiddleware`. Endpoints accept either a valid session cookie OR
  a valid API key.
- **IAM Users** -- `users.id` FK reference for `API_KEYS.user_id`. API keys
  authenticate as their owning user.
- **IAM Permissions** -- The `PERMISSIONS` table is the source of truth for valid
  scope values. Key scopes are validated against this table on creation.
- **OpenAPI / Swagger** -- OpenAPI 3.x specification must be generated from code
  annotations or maintained alongside code. A Swagger UI endpoint (`GET
  /api/v1/docs`) serves the interactive documentation.
