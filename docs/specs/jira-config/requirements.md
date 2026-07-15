# Feature: Jira Configuration

## Overview

Jira Config provides per-user Jira Personal Access Token (PAT) configuration with encryption
at rest. Users store their Jira URL and PAT, which is encrypted using AES-256-GCM before
persistence. The encrypted PAT is used transparently by all other Jira integration features
(jira-project-link, jira-release-link, jira-issue-link, jira-create-bug). System Admins can
additionally configure a project-level Jira connection that serves as a fallback for team-wide
operations.

---

## User Stories

### US-1: Configure Personal Jira Connection

As an authenticated user, I want to configure my Jira URL and Personal Access Token,
so that my credentials can be used when linking TCMS objects to Jira.

**Acceptance Criteria (EARS)**

- WHEN a user sends a valid `PUT /api/v1/jira/config` request with `jira_url` and `pat`,
  THE SYSTEM SHALL encrypt the PAT using AES-256-GCM, upsert the configuration into
  `JIRA_CONFIGS`, and return `200 OK` with the configuration representation (PAT masked,
  showing only the last 4 characters).
- IF the user sends `PUT /api/v1/jira/config` without a `pat` field (to update only the
  URL), THE SYSTEM SHALL upsert the `jira_url` while preserving the existing encrypted PAT
  and return `200 OK`.
- IF the `jira_url` is not a valid HTTPS URL, THE SYSTEM SHALL return `422 Unprocessable
  Entity` with field-level validation details.
- IF the `pat` field exceeds 255 characters, THE SYSTEM SHALL return `422 Unprocessable
  Entity`.
- IF neither `jira_url` nor `pat` is provided, THE SYSTEM SHALL return `422 Unprocessable
  Entity`.

### US-2: View Personal Jira Configuration

As an authenticated user, I want to view my current Jira configuration status,
so that I can verify whether my Jira connection is set up and which URL it points to.

**Acceptance Criteria (EARS)**

- WHEN a user sends `GET /api/v1/jira/config`, THE SYSTEM SHALL return `200 OK` with
  `jira_url`, `configured` (boolean indicating whether a PAT exists), and `pat_last_four`
  (the last 4 characters of the PAT, or `null` if not configured).
- THE SYSTEM SHALL NEVER return the full PAT in any response, including the GET endpoint.
- IF the user has no configuration record, THE SYSTEM SHALL return `200 OK` with
  `jira_url: null`, `configured: false`, `pat_last_four: null`.
- IF the user is not authenticated, THE SYSTEM SHALL return `401 Unauthorized`.

### US-3: Configure Project-Level Jira Connection (System Admin)

As a System Admin, I want to configure a project-level Jira connection for a specific
TCMS project, so that team members without personal Jira configs can use the project
connection as a fallback.

**Acceptance Criteria (EARS)**

- WHEN a System Admin sends `PUT /api/v1/projects/{id}/jira-config` with `jira_url` and
  `pat`, THE SYSTEM SHALL encrypt the PAT, upsert the project-level configuration into
  `JIRA_CONFIGS` with `scope = 'PROJECT'` and `project_id = {id}`, and return `200 OK`.
- IF the caller is not a System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF `jira_url` is not a valid HTTPS URL, THE SYSTEM SHALL return `422 Unprocessable Entity`.
- WHEN a System Admin sends `GET /api/v1/projects/{id}/jira-config`, THE SYSTEM SHALL return
  the project-level configuration (same shape as US-2).
- THE SYSTEM SHALL NEVER return the full PAT in any response.

---

## Security Considerations

### PAT Encryption at Rest

The PAT must be encrypted using AES-256-GCM before storage. The encryption key must be
derived from a master secret (stored in environment variable or secrets manager, never in
the database). Each encryption operation must use a unique, random 12-byte nonce. The
nonce and ciphertext must be stored together (e.g., base64-encoded `nonce:ciphertext`).
Decryption must verify the authentication tag and reject tampered ciphertexts.

### PAT in Logs

The full PAT value MUST NOT appear in any log message. Log statements surrounding
configuration save/update must reference only the user ID and the fact that a
configuration was updated, never the token content.

### Authorization

Only the owning user (or System Admin for project-level) can read or update a Jira
configuration. The application must enforce ownership checks server-side — never rely on
the client to scope the request correctly.

### Rate Limiting

The PUT endpoint must be rate-limited to prevent brute-force attempts to guess or
overwrite other users' configurations. Consider a limit of 5 requests per minute per user.

---

## Out of Scope

- **Jira connection health check** (validating the PAT against the Jira API as part of
  configuration save — deferred; validation happens lazily at link time)
- **OAuth 2.0 Jira authentication** (only Personal Access Tokens are supported in Phase 2)
- **Multiple PATs per user** (one configuration per user; one per project)
- **PAT rotation or expiry tracking** (users manage their own PAT lifecycle)
- **Bulk PAT configuration by System Admin** (only per-project, not per-user)

---

## Dependencies

- **IAM Auth** — Session-based authentication; all endpoints require a valid session.
- **Project CRUD** — `projects.id` FK reference for project-level configurations.
