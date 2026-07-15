# Design: Jira Configuration

## Architecture

The Jira Config feature follows Clean Architecture layering.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - get_jira_config    GET    /api/v1/jira/config                      │   │
│  │  - put_jira_config    PUT    /api/v1/jira/config                      │   │
│  │  - get_project_config GET    /api/v1/projects/{id}/jira-config        │   │
│  │  - put_project_config PUT    /api/v1/projects/{id}/jira-config        │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  JiraConfigService:                       │  │  - JiraConfig (entity)  │  │
│  │  - get_user_config                        │  │  - JiraConfigScope (vo) │  │
│  │  - upsert_user_config                     │  └──────────────────────────┘  │
│  │  - get_project_config                     │                                │
│  │  - upsert_project_config                  │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - JiraConfigRepository (port)            │                                │
│  │  - JiraEncryptionService (port)           │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlJiraConfigRepository (implements JiraConfigRepository)           │   │
│  │  - Aes256GcmEncryptionService (implements JiraEncryptionService)       │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All endpoints require an authenticated session (checked by `AuthMiddleware`).
2. User-scoped endpoints (`/api/v1/jira/config`) are scoped to the session user.
3. Project-scoped endpoints (`/api/v1/projects/{id}/jira-config`) require System Admin
   role and a valid, non-deleted project.
4. PAT is encrypted in the `JiraConfigService` before persistence and decrypted after
   retrieval. The HTTP layer never sees the plaintext PAT.

---

## API Contract

### Common Error Response Format

All errors follow the standard format:

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable description",
    "details": [
      { "field": "field_name", "message": "specific validation message" }
    ]
  }
}
```

---

### GET `/api/v1/jira/config`

Get the authenticated user's Jira configuration status.

**Required Permission:** None (authenticated users only)

**Success Response:** `200 OK`

```json
{
  "data": {
    "jira_url": "https://company.atlassian.net",
    "configured": true,
    "pat_last_four": "abcd",
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- `configured` is `true` when a PAT is stored, `false` otherwise.
- `pat_last_four` is the last 4 characters of the PAT plaintext, or `null` if not
  configured.
- If no config record exists, all fields except `configured` are `null`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |

---

### PUT `/api/v1/jira/config`

Upsert the authenticated user's Jira configuration.

**Required Permission:** None (authenticated users only)

**Request Body:**

```json
{
  "jira_url": "https://company.atlassian.net",
  "pat": "my-personal-access-token"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `jira_url` | string | No | Must be a valid HTTPS URL; max 2048 characters |
| `pat` | string | No | Max 255 characters; encrypted at rest |

**Notes:**
- At least one field must be provided (empty body returns `422`).
- If `pat` is omitted, the existing PAT is preserved and only the URL is updated.
- If `pat` is provided, it replaces any existing PAT (re-encrypted with a fresh nonce).
- The PAT is never returned in the response.
- `jira_url` and `pat` are independent; either or both can be updated.

**Success Response:** `200 OK`

```json
{
  "data": {
    "jira_url": "https://company.atlassian.net",
    "configured": true,
    "pat_last_four": "abcd",
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `422` | `VALIDATION_ERROR` | Invalid URL format, PAT too long, or empty body |

---

### GET `/api/v1/projects/{id}/jira-config`

Get project-level Jira configuration. System Admin only.

**Required Role:** System Admin

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Project ID |

**Success Response:** `200 OK`

Same shape as `GET /api/v1/jira/config`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | Caller is not a System Admin |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |

---

### PUT `/api/v1/projects/{id}/jira-config`

Upsert project-level Jira configuration. System Admin only.

**Required Role:** System Admin

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Project ID |

**Request Body:** Same as `PUT /api/v1/jira/config`.

**Success Response:** `200 OK`

Same shape as `GET /api/v1/jira/config`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | Caller is not a System Admin |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `422` | `VALIDATION_ERROR` | Invalid URL format, PAT too long, or empty body |

---

## Data Model

### New Tables

#### JIRA_CONFIGS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `user_id` | `BIGINT` | `REFERENCES users(id) ON DELETE CASCADE` | Nullable; null when scope = PROJECT |
| `project_id` | `BIGINT` | `REFERENCES projects(id) ON DELETE CASCADE` | Nullable; null when scope = USER |
| `scope` | `VARCHAR(20)` | `NOT NULL` | `CHECK (scope IN ('USER', 'PROJECT'))` |
| `jira_url` | `VARCHAR(2048)` | | Nullable; max 2048 characters |
| `pat_encrypted` | `TEXT` | | Nullable; format: `base64(nonce):base64(ciphertext)` |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by trigger |

**Constraints:**

```sql
-- One config per user (USER scope)
CREATE UNIQUE INDEX uq_jira_configs_user
  ON jira_configs (user_id) WHERE scope = 'USER';

-- One config per project (PROJECT scope)
CREATE UNIQUE INDEX uq_jira_configs_project
  ON jira_configs (project_id) WHERE scope = 'PROJECT';

-- Check constraint on scope
ALTER TABLE jira_configs ADD CONSTRAINT chk_jira_configs_scope
  CHECK (scope IN ('USER', 'PROJECT'));

-- Enforce exactly one of user_id or project_id is set, depending on scope
ALTER TABLE jira_configs ADD CONSTRAINT chk_jira_configs_ownership
  CHECK (
    (scope = 'USER' AND user_id IS NOT NULL AND project_id IS NULL) OR
    (scope = 'PROJECT' AND project_id IS NOT NULL AND user_id IS NULL)
  );
```

---

## Sequence

### Upsert User Config Flow

1. Client sends `PUT /api/v1/jira/config` with `{"jira_url": "...", "pat": "..."}`.
   Session cookie is included.
2. `AuthMiddleware` validates the session and attaches the authenticated user ID.
3. HTTP handler deserializes and validates the request body (URL format, PAT length).
4. Handler calls `JiraConfigService::upsert_user_config(user_id, cmd)`.
5. `JiraConfigService` encrypts the PAT (if provided) via `JiraEncryptionService::encrypt(pat)`.
   - `Aes256GcmEncryptionService` generates a random 12-byte nonce.
   - Derives the AES key from the master secret (env-var / secrets manager).
   - Encrypts the PAT and returns `base64(nonce):base64(ciphertext + tag)`.
6. `JiraConfigService` calls `JiraConfigRepository::upsert(config)`:
   - Queries for existing config by `user_id` + `scope = 'USER'`.
   - If found, updates `jira_url` (if provided) and `pat_encrypted` (if provided).
   - If not found, inserts a new row.
7. `JiraConfigService` returns the config representation with PAT masked.
8. Handler returns `200 OK`.

### Get User Config Flow

1. Client sends `GET /api/v1/jira/config`.
2. `AuthMiddleware` validates the session.
3. Handler calls `JiraConfigService::get_user_config(user_id)`.
4. `JiraConfigService` calls `JiraConfigRepository::find_by_user_id(user_id)`.
5. If found and PAT exists, derives `pat_last_four` by decrypting the stored PAT
   and taking the last 4 characters. The full plaintext is discarded immediately.
6. If not found, returns a default response with `configured: false` and nulls.
7. Handler returns `200 OK`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `JiraConfig` | Domain (1) | Entity: `id`, `user_id`, `project_id`, `scope`, `jira_url`, `pat_encrypted`. |
| `JiraConfigScope` | Domain (1) | Value object: enum `User` / `Project`. |
| `JiraConfigService` | Application (2) | Orchestrates config reads and upserts. Encrypts PAT before storage; derives `pat_last_four` for responses. |
| `JiraConfigRepository` | Application (2) | Interface (port): `find_by_user_id`, `find_by_project_id`, `upsert`. |
| `JiraEncryptionService` | Application (2) | Interface (port): `encrypt(plaintext) -> ciphertext`, `decrypt(ciphertext) -> plaintext`. |
| `JiraConfigHandler` | Adapters (3) | HTTP handler with four methods. Deserializes requests, calls `JiraConfigService`, serializes responses. |
| `SqlJiraConfigRepository` | Infrastructure (4) | Implements `JiraConfigRepository`. Upsert uses `INSERT ... ON CONFLICT` or equivalent. |
| `Aes256GcmEncryptionService` | Infrastructure (4) | Implements `JiraEncryptionService`. Derives key from master secret; generates random nonce per encryption. |

### Dependency Resolution for Jira Credentials

Other Jira features (project-link, release-link, issue-link, create-bug) need a Jira PAT
to call the Jira API. The resolution order is:

1. **User-level config**: The `jira-config` feature provides a service method
   `resolve_pat(user_id, project_id)` that first checks the user's own `JIRA_CONFIGS`
   row for `scope = 'USER'`.
2. **Project-level fallback**: If the user has no personal config, the resolver checks
   the project-level config (`scope = 'PROJECT'`) for the given `project_id`.
3. **No config**: If neither exists, the Jira API call fails with a clear error
   indicating that Jira is not configured.

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| Not a System Admin (project endpoint) | `403` | `FORBIDDEN` | INFO | Generic message |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | |
| Invalid jira_url format | `422` | `VALIDATION_ERROR` | INFO | Must be HTTPS |
| PAT exceeds max length | `422` | `VALIDATION_ERROR` | INFO | Max 255 chars |
| Empty request body (no fields) | `422` | `VALIDATION_ERROR` | INFO | |
| Encryption key unavailable | `500` | `INTERNAL_ERROR` | ERROR | Master secret not configured |
| Encryption failure | `500` | `INTERNAL_ERROR` | ERROR | Do not leak crypto details |
| Decryption failure (tampered data) | `500` | `INTERNAL_ERROR` | ERROR | Log as security alert |

**Anti-patterns explicitly avoided:**

- **Never return the full PAT** in any API response or log.
- **Do not validate PAT against Jira API** on save — this is a separate concern handled
  at link time. Premature validation couples config save to Jira availability.
- **Do not store the PAT in plaintext** anywhere.
- **Do not hard-code the encryption key** — derive from environment variable or secrets
  manager.
