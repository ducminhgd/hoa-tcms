# Design: Jira Release Link

## Architecture

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_links   GET    /api/v1/test-plans/{id}/jira-releases          │   │
│  │  - create_link  POST   /api/v1/test-plans/{id}/jira-releases          │   │
│  │  - delete_link  DELETE /api/v1/test-plans/{id}/jira-releases/{rid}    │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  JiraReleaseLinkService:                  │  │  - JiraReleaseLink      │  │
│  │  - list_links                             │  │    (entity)             │  │
│  │  - create_link                            │  └──────────────────────────┘  │
│  │  - delete_link                            │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - JiraReleaseLinkRepository (port)       │                                │
│  │  - JiraApiClient (port — shared)          │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlJiraReleaseLinkRepository (implements repository)                │   │
│  │  - JiraRestApiClient (shared, implements JiraApiClient)                │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All endpoints require an authenticated session and a valid, non-deleted test plan.
2. `GET` requires project membership on the test plan's project(s).
3. `POST` and `DELETE` require `project:update` permission and Owner or Editor role.
4. `POST` resolves the test plan's linked Jira projects, resolves Jira credentials,
   validates the Jira release exists via the Jira REST API, and persists the link.

---

## API Contract

### GET `/api/v1/test-plans/{id}/jira-releases`

List Jira releases linked to a Test Plan.

**Required Permission:** Project membership (project:read scope)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Plan ID |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "jira_release_id": "12345",
      "jira_release_name": "Sprint 42 Release",
      "jira_project_key": "PROJ",
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
| `403` | `FORBIDDEN` | User is not a member of the test plan's project(s) |
| `404` | `NOT_FOUND` | Test plan does not exist or is soft-deleted |

---

### POST `/api/v1/test-plans/{id}/jira-releases`

Link a Test Plan to a Jira release.

**Required Permission:** `project:update` (Owner or Editor role)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Plan ID |

**Request Body:**

```json
{
  "jira_release_id": "12345"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `jira_release_id` | string | Yes | Jira release/version ID; 1-255 characters |

**Success Response:** `201 Created`

```json
{
  "data": {
    "jira_release_id": "12345",
    "jira_release_name": "Sprint 42 Release",
    "jira_project_key": "PROJ",
    "linked_by": {
      "user_id": 42,
      "username": "jdoe"
    },
    "linked_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- The test plan's project must have at least one Jira project link.
- The Jira release is validated against the linked Jira project(s).
- The `jira_release_name` is fetched from the Jira API at link time.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `project:update` permission or is not an Owner/Editor |
| `404` | `NOT_FOUND` | Test plan does not exist or is soft-deleted |
| `404` | `JIRA_RELEASE_NOT_FOUND` | Jira release ID not found via Jira API |
| `409` | `DUPLICATE_JIRA_RELEASE_LINK` | Already linked to this Jira release |
| `412` | `JIRA_NOT_CONFIGURED` | No Jira config found |
| `412` | `JIRA_PROJECT_NOT_LINKED` | Test plan's project has no linked Jira projects |
| `422` | `VALIDATION_ERROR` | Invalid `jira_release_id` |
| `502` | `JIRA_API_ERROR` | Jira API unreachable or error |

---

### DELETE `/api/v1/test-plans/{id}/jira-releases/{jiraReleaseId}`

Unlink a Jira release from a Test Plan.

**Required Permission:** `project:update` (Owner or Editor role)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Plan ID |
| `jiraReleaseId` | string | Jira release ID to unlink |

**Success Response:** `204 No Content`

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `project:update` permission or is not an Owner/Editor |
| `404` | `NOT_FOUND` | Link does not exist |
| `404` | `NOT_FOUND` | Test plan does not exist or is soft-deleted |

---

## Data Model

### New Tables

#### JIRA_RELEASE_LINKS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `test_plan_id` | `BIGINT` | `PK`, `NOT NULL`, `REFERENCES test_plans(id) ON DELETE RESTRICT` | |
| `jira_release_id` | `VARCHAR(255)` | `PK`, `NOT NULL` | Jira release/version ID |
| `jira_release_name` | `VARCHAR(255)` | `NOT NULL` | Cached from Jira API |
| `jira_project_key` | `VARCHAR(255)` | `NOT NULL` | Which linked Jira project the release belongs to |
| `linked_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | |
| `linked_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | |

**Constraints:**

```sql
ALTER TABLE jira_release_links ADD PRIMARY KEY (test_plan_id, jira_release_id);

CREATE INDEX idx_jira_release_links_linked_by ON jira_release_links (linked_by);
CREATE INDEX idx_jira_release_links_project_key ON jira_release_links (jira_project_key);
```

---

## Sequence

### Create Link Flow

1. Client sends `POST /api/v1/test-plans/{id}/jira-releases` with `{"jira_release_id": "12345"}`.
2. `AuthMiddleware` validates session.
3. Handler validates input and calls `JiraReleaseLinkService::create_link(...)`.
4. Service loads the test plan and resolves its project(s).
5. Service checks the user has `project:update` permission and Owner/Editor role on
   the project(s).
6. Service resolves linked Jira projects for the test plan's project via
   `JiraProjectLinkRepository`. If none, returns `JiraProjectNotLinkedError`.
7. Service checks for duplicate link. If duplicate, returns
   `DuplicateJiraReleaseLinkError`.
8. Service resolves Jira credentials via `JiraConfigService::resolve_pat`.
9. For each linked Jira project key, service calls
   `JiraApiClient::get_release(jira_project_key, jira_release_id, pat)` until the
   release is found or all linked projects are exhausted.
   - Success: returns the release name.
   - Not found in any linked project: returns `JiraReleaseNotFoundError`.
10. Service inserts the link via `JiraReleaseLinkRepository::insert(link)`.
11. Handler returns `201 Created`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `JiraReleaseLink` | Domain (1) | Entity: `test_plan_id`, `jira_release_id`, `jira_release_name`, `jira_project_key`, `linked_by`, `linked_at` |
| `JiraReleaseLinkService` | Application (2) | Orchestrates list, create, delete. Resolves linked Jira projects, validates release via Jira API. |
| `JiraReleaseLinkRepository` | Application (2) | Interface (port): `find_by_test_plan_id`, `find_by_plan_and_release`, `insert`, `delete` |
| `JiraReleaseLinkHandler` | Adapters (3) | HTTP handler with three methods |
| `SqlJiraReleaseLinkRepository` | Infrastructure (4) | Implements `JiraReleaseLinkRepository` |

### Extended Shared Components

| Component | Change |
|-----------|--------|
| `JiraApiClient` | Add method: `get_release(project_key, release_id, pat) -> ReleaseInfo` |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level |
|------------|-------------|------------|-----------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO |
| Lacking permission or role | `403` | `FORBIDDEN` | INFO |
| Test plan not found or soft-deleted | `404` | `NOT_FOUND` | INFO |
| Jira release not found | `404` | `JIRA_RELEASE_NOT_FOUND` | INFO |
| Link not found | `404` | `NOT_FOUND` | INFO |
| Duplicate link | `409` | `DUPLICATE_JIRA_RELEASE_LINK` | INFO |
| Jira not configured | `412` | `JIRA_NOT_CONFIGURED` | INFO |
| No linked Jira projects | `412` | `JIRA_PROJECT_NOT_LINKED` | INFO |
| Invalid jira_release_id | `422` | `VALIDATION_ERROR` | INFO |
| Jira API unreachable or error | `502` | `JIRA_API_ERROR` | ERROR |

**Anti-patterns explicitly avoided:**

- **Do not skip Jira project link check** — a test plan must have at least one linked
  Jira project before release linking.
- **Do not cascade-delete** from test plan deletion — use `ON DELETE RESTRICT`.
