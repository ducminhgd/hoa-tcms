# Design: Jira Project Link

## Architecture

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_links   GET    /api/v1/projects/{id}/jira-links               │   │
│  │  - create_link  POST   /api/v1/projects/{id}/jira-links               │   │
│  │  - delete_link  DELETE /api/v1/projects/{id}/jira-links/{key}         │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  JiraProjectLinkService:                  │  │  - JiraProjectLink      │  │
│  │  - list_links                             │  │    (entity)             │  │
│  │  - create_link                            │  └──────────────────────────┘  │
│  │  - delete_link                            │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - JiraProjectLinkRepository (port)       │                                │
│  │  - JiraApiClient (port)                   │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlJiraProjectLinkRepository (implements repository)                │   │
│  │  - JiraRestApiClient (implements JiraApiClient)                        │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All endpoints require an authenticated session and a valid, non-deleted project.
2. `GET` requires `project:read` permission and project membership.
3. `POST` and `DELETE` require `project:update` permission and Owner or Editor role.
4. `POST` resolves Jira credentials (user-level or project-level fallback), validates
   the Jira project exists via the Jira REST API, and persists the link.
5. `DELETE` removes the link without calling the Jira API.

---

## API Contract

### GET `/api/v1/projects/{id}/jira-links`

List Jira projects linked to a TCMS project.

**Required Permission:** `project:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | TCMS project ID |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "jira_project_key": "PROJ",
      "jira_project_name": "My Jira Project",
      "linked_by": {
        "user_id": 42,
        "username": "jdoe"
      },
      "linked_at": "2026-07-14T10:00:00Z"
    }
  ]
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `project:read` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |

---

### POST `/api/v1/projects/{id}/jira-links`

Link a TCMS project to a Jira project.

**Required Permission:** `project:update` (Owner or Editor role on the project)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | TCMS project ID |

**Request Body:**

```json
{
  "jira_project_key": "PROJ"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `jira_project_key` | string | Yes | Uppercase letters, digits, hyphens; 1-255 characters |

**Success Response:** `201 Created`

```json
{
  "data": {
    "jira_project_key": "PROJ",
    "jira_project_name": "My Jira Project",
    "linked_by": {
      "user_id": 42,
      "username": "jdoe"
    },
    "linked_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- The Jira project name is fetched from the Jira API at link time and stored.
- The `linked_by` user is the authenticated user making the request.
- `linked_at` is the current timestamp.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `project:update` permission or is not an Owner/Editor |
| `404` | `NOT_FOUND` | TCMS project does not exist or is soft-deleted |
| `404` | `JIRA_PROJECT_NOT_FOUND` | Jira project key not found via Jira API |
| `409` | `DUPLICATE_JIRA_LINK` | Already linked to this Jira project |
| `412` | `JIRA_NOT_CONFIGURED` | No Jira config found (user or project level) |
| `422` | `VALIDATION_ERROR` | Invalid `jira_project_key` format |
| `502` | `JIRA_API_ERROR` | Jira API unreachable or returned an error |

---

### DELETE `/api/v1/projects/{id}/jira-links/{jiraProjectKey}`

Unlink a Jira project from a TCMS project.

**Required Permission:** `project:update` (Owner or Editor role on the project)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | TCMS project ID |
| `jiraProjectKey` | string | Jira project key to unlink |

**Success Response:** `204 No Content`

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `project:update` permission or is not an Owner/Editor |
| `404` | `NOT_FOUND` | Link does not exist |
| `404` | `NOT_FOUND` | TCMS project does not exist or is soft-deleted |

---

## Data Model

### New Tables

#### JIRA_PROJECT_LINKS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `tcms_project_id` | `BIGINT` | `PK`, `NOT NULL`, `REFERENCES projects(id) ON DELETE RESTRICT` | TCMS project |
| `jira_project_key` | `VARCHAR(255)` | `PK`, `NOT NULL` | Jira project key |
| `jira_project_name` | `VARCHAR(255)` | `NOT NULL` | Cached from Jira API at link time |
| `linked_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who created the link |
| `linked_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | When the link was created |

**Constraints:**

```sql
-- Composite primary key
ALTER TABLE jira_project_links ADD PRIMARY KEY (tcms_project_id, jira_project_key);

-- FK indexes
CREATE INDEX idx_jira_project_links_linked_by ON jira_project_links (linked_by);
```

**Notes:**
- No soft-delete on this junction table — links are hard-deleted.
- No `updated_at` / `updated_by` — links are immutable (unlink recreates a new link).
- `ON DELETE RESTRICT` on `tcms_project_id` to prevent orphaning links when a project
  is accidentally deleted (soft-delete does not trigger FK cascade anyway).

---

## Sequence

### Create Link Flow

1. Client sends `POST /api/v1/projects/{id}/jira-links` with `{"jira_project_key": "PROJ"}`.
2. `AuthMiddleware` validates session, attaches user ID.
3. Handler validates input (`jira_project_key` format).
4. Handler calls `JiraProjectLinkService::create_link(project_id, jira_project_key, user_id)`.
5. Service verifies user has `project:update` permission and is an Owner or Editor of
   the project.
6. Service checks for duplicate link via `JiraProjectLinkRepository::find_by_project_and_key`.
   If duplicate, returns `DuplicateJiraLinkError`.
7. Service resolves Jira credentials via `JiraConfigService::resolve_pat(user_id, project_id)`.
   If no credentials, returns `JiraNotConfiguredError`.
8. Service calls `JiraApiClient::get_project(jira_project_key, pat)` to validate the
   Jira project exists.
   - Success: returns the Jira project name.
   - Jira API returns 404: returns `JiraProjectNotFoundError`.
   - Jira API unreachable/error: returns `JiraApiError`.
9. Service inserts the link via `JiraProjectLinkRepository::insert(link)`.
10. Handler returns `201 Created`.

### Delete Link Flow

1. Client sends `DELETE /api/v1/projects/{id}/jira-links/{key}`.
2. `AuthMiddleware` validates session.
3. Handler calls `JiraProjectLinkService::delete_link(project_id, jira_project_key, user_id)`.
4. Service verifies user has `project:update` permission and is Owner or Editor.
5. Service checks link exists via `JiraProjectLinkRepository::find_by_project_and_key`.
   If not found, returns `NotFound`.
6. Service calls `JiraProjectLinkRepository::delete(project_id, key)`.
7. Handler returns `204 No Content`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `JiraProjectLink` | Domain (1) | Entity: `tcms_project_id`, `jira_project_key`, `jira_project_name`, `linked_by`, `linked_at` |
| `JiraProjectLinkService` | Application (2) | Orchestrates list, create, delete. Validates Jira project existence via Jira API on create. |
| `JiraProjectLinkRepository` | Application (2) | Interface (port): `find_by_project_id`, `find_by_project_and_key`, `insert`, `delete` |
| `JiraApiClient` | Application (2) | Interface (port): `get_project(key, pat) -> ProjectInfo`, `get_release(release_id, pat) -> ReleaseInfo`, `get_issue(key, pat) -> IssueInfo`, `create_bug(project_key, fields, pat) -> IssueInfo` |
| `JiraProjectLinkHandler` | Adapters (3) | HTTP handler with three methods: `list`, `create`, `delete` |
| `SqlJiraProjectLinkRepository` | Infrastructure (4) | Implements `JiraProjectLinkRepository` |
| `JiraRestApiClient` | Infrastructure (4) | Implements `JiraApiClient` — makes HTTP calls to Jira REST API |

### Shared Component

| Component | Description |
|-----------|-------------|
| `JiraApiClient` | Defined in this feature but shared across all Jira integration features (project-link, release-link, issue-link, create-bug). Provides a common interface for Jira REST API calls. The implementation (`JiraRestApiClient`) handles authentication via PAT (Bearer token), error mapping, and timeout/retry logic. |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | |
| Lacking project permission or role | `403` | `FORBIDDEN` | INFO | Generic message |
| TCMS project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | |
| Jira project not found | `404` | `JIRA_PROJECT_NOT_FOUND` | INFO | Jira API returned 404 |
| Link not found | `404` | `NOT_FOUND` | INFO | |
| Duplicate link | `409` | `DUPLICATE_JIRA_LINK` | INFO | |
| Jira not configured | `412` | `JIRA_NOT_CONFIGURED` | INFO | Neither user nor project PAT |
| Invalid jira_project_key | `422` | `VALIDATION_ERROR` | INFO | |
| Jira API unreachable or error | `502` | `JIRA_API_ERROR` | ERROR | Includes timeout; log Jira response status |
| Jira PAT invalid/expired | `502` | `JIRA_API_ERROR` | WARN | Jira returns 401; consider surfacing as `JIRA_AUTH_FAILED` |

**Anti-patterns explicitly avoided:**

- **Do not silently skip Jira validation** — always validate the Jira project exists
  before persisting a link.
- **Do not cascade-delete dependent links** — jira-release-link and jira-issue-link
  records survive unlinking; they reference Jira keys directly, not the link row.
- **Do not log the Jira PAT** in error messages or logs.
