# Tasks: Jira Configuration

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit of work
that maps to one or more requirements or design sections.

---

## Layer 1 — Domain

- [ ] 1. Implement `JiraConfig` entity and `JiraConfigScope` value object —
       `requirements.md#US-1` through `US-3`, `design.md#Components`
  - Fields: `id`, `user_id`, `project_id`, `scope`, `jira_url`, `pat_encrypted`
  - Implement `JiraConfigScope` enum with `User` and `Project` variants
  - Domain validation: `jira_url` must be HTTPS; scope ownership constraint (USER
    requires `user_id`, PROJECT requires `project_id`)
  - No framework imports; pure Rust struct + impl

- [ ] 2. Define domain exceptions for Jira config — `design.md#Error Handling`
  - `JiraConfigNotFoundError`
  - `JiraConfigValidationError` (carries field-level details)
  - `JiraEncryptionError`
  - `JiraDecryptionError`

---

## Layer 2 — Application

- [ ] 3. Define `JiraConfigRepository` interface (port) — `design.md#Components`,
       `design.md#Data Model`
  - Methods: `find_by_user_id(user_id) -> Option<JiraConfig>`,
    `find_by_project_id(project_id) -> Option<JiraConfig>`,
    `upsert(config: JiraConfig) -> JiraConfig`
  - All queries filter by `scope`
  - `upsert` handles both insert and update via `ON CONFLICT`

- [ ] 4. Define `JiraEncryptionService` interface (port) — `design.md#Components`
  - Method: `encrypt(plaintext: &str) -> Result<String>` (returns
    `base64(nonce):base64(ciphertext)`)
  - Method: `decrypt(ciphertext: &str) -> Result<String>`
  - Method: `mask_last_four(ciphertext: &str) -> Result<Option<String>>` (decrypts
    and returns last 4 chars)

- [ ] 5. Implement `JiraConfigService` — `requirements.md#US-1` through `US-3`,
       `design.md#Sequence`
  - `get_user_config(user_id)`: retrieves user config, returns masked representation
  - `upsert_user_config(user_id, cmd)`: encrypts PAT if provided, upserts, returns
    masked representation
  - `get_project_config(project_id, caller_user_id)`: verifies caller is System Admin,
    retrieves project config
  - `upsert_project_config(project_id, cmd, caller_user_id)`: verifies caller is
    System Admin, encrypts PAT if provided, upserts
  - `resolve_pat(user_id, project_id) -> Result<String>`: resolution method for other
    Jira features — checks user config first, falls back to project config

- [ ] 6. Define command/query DTOs — `design.md#API Contract`
  - `UpsertJiraConfigCommand` (jira_url?, pat?)
  - `JiraConfigResponse` (jira_url, configured, pat_last_four, updated_at)

- [ ] 7. Write unit tests for `JiraConfigService` — `requirements.md#US-1` through `US-3`
  - Happy path: upsert with both URL and PAT
  - Upsert with URL only (PAT preserved)
  - Upsert with PAT only (URL preserved)
  - Get config when no record exists (returns nulls)
  - PAT masking (last 4 chars derived correctly)
  - System Admin check for project endpoints
  - Encryption/decryption round-trip

---

## Layer 3 — Adapters (HTTP)

- [ ] 8. Implement `JiraConfigHandler` — `design.md#API Contract`, `design.md#Components`
  - Four handler methods: `get_user_config`, `put_user_config`, `get_project_config`,
    `put_project_config`
  - Deserialize request bodies and query params into DTOs
  - Call `JiraConfigService` methods
  - Serialize responses with proper status codes

- [ ] 9. Register Jira config routes in HTTP router — `design.md#Components`
  - `GET  /api/v1/jira/config`                  -> `get_user_config`
  - `PUT  /api/v1/jira/config`                  -> `put_user_config`
  - `GET  /api/v1/projects/{id}/jira-config`    -> `get_project_config`
  - `PUT  /api/v1/projects/{id}/jira-config`    -> `put_project_config`
  - All routes require session auth middleware
  - Project routes additionally check System Admin role

- [ ] 10. Write integration tests for Jira config HTTP handlers —
        `design.md#API Contract`
  - Test `GET /api/v1/jira/config` returns nulls for new user
  - Test `PUT /api/v1/jira/config` stores and returns masked PAT
  - Test `GET /api/v1/jira/config` after configuration shows `configured: true`
  - Test `422` on invalid URL (non-HTTPS)
  - Test `422` on empty body
  - Test `403` on project endpoint for non-System-Admin user
  - Test `404` on project endpoint for non-existent project
  - Test project config works for System Admin

---

## Layer 4 — Infrastructure

- [ ] 11. Create `JIRA_CONFIGS` database migration — `design.md#Data Model`
  - Table definition with all columns, PK, FKs, check constraints
  - Unique partial indexes: `uq_jira_configs_user` (WHERE scope = 'USER'),
    `uq_jira_configs_project` (WHERE scope = 'PROJECT')
  - Ownership check constraint: enforces user_id for USER scope, project_id for
    PROJECT scope
  - FK indexes on `user_id`, `project_id`
  - Rollback migration: `DROP TABLE IF EXISTS jira_configs`

- [ ] 12. Implement `SqlJiraConfigRepository` — `design.md#Components`
  - All methods from `JiraConfigRepository` interface
  - `upsert` uses `INSERT ... ON CONFLICT` with update logic
  - `find_by_user_id` filters by `scope = 'USER'`
  - `find_by_project_id` filters by `scope = 'PROJECT'`
  - Write unit tests with a test transaction

- [ ] 13. Implement `Aes256GcmEncryptionService` — `design.md#Components`,
        `requirements.md#Security Considerations`
  - Load master secret from environment variable (`JIRA_ENCRYPTION_KEY`) at startup
  - Derive AES-256 key using HKDF (or equivalent)
  - `encrypt(plaintext)`: generate random 12-byte nonce, encrypt with AES-256-GCM,
    return `base64(nonce):base64(ciphertext + tag)`
  - `decrypt(ciphertext)`: parse nonce and ciphertext, decrypt, verify
    authentication tag
  - `mask_last_four(ciphertext)`: decrypt, take last 4 chars, discard plaintext
  - Write unit tests: encrypt/decrypt round-trip, tampered ciphertext rejection,
    unique nonces per encryption

---

## Verification & Cleanup

- [ ] 14. End-to-end verification — `requirements.md#US-1` through `US-3`
  - Configure personal Jira connection via API
  - Verify PAT is encrypted in database (not plaintext)
  - Verify GET returns masked PAT (last 4 chars only)
  - Verify System Admin can configure project-level connection
  - Verify non-admin cannot access project config endpoint
  - Verify PAT resolution order (user config before project config)

- [ ] 15. Update `specs/README.md`
  - Mark `jira-config` as having completed specs (requirements.md, design.md,
    tasks.md)
