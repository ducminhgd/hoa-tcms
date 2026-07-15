# Design: Jira Issue Link

## Architecture

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_links   GET    /api/v1/projects/{pid}/test-cases/{id}/        │   │
│  │                         jira-issues                                    │   │
│  │  - create_link  POST   /api/v1/projects/{pid}/test-cases/{id}/        │   │
│  │                         jira-issues                                    │   │
│  │  - delete_link  DELETE /api/v1/projects/{pid}/test-cases/{id}/        │   │
│  │                         jira-issues/{key}                              │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  JiraIssueLinkService:                    │  │  - JiraIssueLink        │  │
│  │  - list_links                             │  │    (entity)             │  │
│  │  - create_link                            │  │  - LinkType (enum)      │  │
│  │  - delete_link                            │  └──────────────────────────┘  │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - JiraIssueLinkRepository (port)         │                                │
│  │  - JiraApiClient (port — shared)          │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlJiraIssueLinkRepository (implements repository)                  │   │
│  │  - JiraRestApiClient (shared, implements JiraApiClient)                │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All endpoints require an authenticated session.
2. `GET` requires project membership.
3. `POST` and `DELETE` require `project:update` permission and Owner, Editor, or
   Contributor role (Contributors can link/unlink issues but not project/release links).
4. `POST` validates the Jira issue exists via the Jira REST API and persists the link.
5. A `link_type` filter on GET allows clients to view only bugs, only requirements, etc.

---

## API Contract

### GET `/api/v1/projects/{pid}/test-cases/{id}/jira-issues`

List Jira issues linked to a Test Case.

**Required Permission:** Project membership

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `pid` | integer | Project ID |
| `id` | integer | Test Case ID |

**Query Parameters:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `link_type` | string | — | Optional filter: `RELATED`, `BUG`, `REQUIREMENT` |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "jira_issue_key": "PROJ-123",
      "link_type": "BUG",
      "summary": "Login page crashes on invalid email",
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
| `403` | `FORBIDDEN` | User is not a member of the project |
| `404` | `NOT_FOUND` | Test case does not exist or is soft-deleted |

---

### POST `/api/v1/projects/{pid}/test-cases/{id}/jira-issues`

Link a Test Case to a Jira issue.

**Required Permission:** `project:update` (Owner, Editor, or Contributor role)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `pid` | integer | Project ID |
| `id` | integer | Test Case ID |

**Request Body:**

```json
{
  "jira_issue_key": "PROJ-123",
  "link_type": "BUG"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `jira_issue_key` | string | Yes | Valid Jira issue key format (PROJECT-123); 1-255 characters |
| `link_type` | string | Yes | One of: `RELATED`, `BUG`, `REQUIREMENT` |

**Success Response:** `201 Created`

```json
{
  "data": {
    "jira_issue_key": "PROJ-123",
    "link_type": "BUG",
    "summary": "Login page crashes on invalid email",
    "linked_by": {
      "user_id": 42,
      "username": "jdoe"
    },
    "linked_at": "2026-07-14T10:00:00Z"
  }
}
```

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks permission or sufficient role |
| `404` | `NOT_FOUND` | Test case does not exist or is soft-deleted |
| `404` | `JIRA_ISSUE_NOT_FOUND` | Jira issue key not found via Jira API |
| `409` | `DUPLICATE_JIRA_ISSUE_LINK` | Already linked to this Jira issue |
| `412` | `JIRA_NOT_CONFIGURED` | No Jira config found |
| `422` | `VALIDATION_ERROR` | Invalid `link_type` or `jira_issue_key` format |
| `502` | `JIRA_API_ERROR` | Jira API unreachable or error |

---

### DELETE `/api/v1/projects/{pid}/test-cases/{id}/jira-issues/{jiraIssueKey}`

Unlink a Jira issue from a Test Case.

**Required Permission:** `project:update` (Owner, Editor, or Contributor role)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `pid` | integer | Project ID |
| `id` | integer | Test Case ID |
| `jiraIssueKey` | string | Jira issue key to unlink |

**Success Response:** `204 No Content`

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks permission or sufficient role |
| `404` | `NOT_FOUND` | Link does not exist |
| `404` | `NOT_FOUND` | Test case does not exist or is soft-deleted |

---

## Data Model

### New Tables

#### JIRA_ISSUE_LINKS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `test_case_id` | `BIGINT` | `PK`, `NOT NULL`, `REFERENCES test_cases(id) ON DELETE RESTRICT` | |
| `jira_issue_key` | `VARCHAR(255)` | `PK`, `NOT NULL` | Jira issue key (e.g. PROJ-123) |
| `link_type` | `VARCHAR(20)` | `NOT NULL` | `CHECK (link_type IN ('RELATED', 'BUG', 'REQUIREMENT'))` |
| `summary` | `TEXT` | | Cached from Jira API at link time |
| `linked_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | |
| `linked_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | |

**Constraints:**

```sql
ALTER TABLE jira_issue_links ADD PRIMARY KEY (test_case_id, jira_issue_key);

ALTER TABLE jira_issue_links ADD CONSTRAINT chk_jira_issue_links_type
  CHECK (link_type IN ('RELATED', 'BUG', 'REQUIREMENT'));

CREATE INDEX idx_jira_issue_links_linked_by ON jira_issue_links (linked_by);
CREATE INDEX idx_jira_issue_links_type ON jira_issue_links (link_type);
```

---

## Sequence

### Create Link Flow

1. Client sends `POST /api/v1/projects/{pid}/test-cases/{id}/jira-issues` with
   `{"jira_issue_key": "PROJ-123", "link_type": "BUG"}`.
2. `AuthMiddleware` validates session.
3. Handler validates input (link_type enum, issue key format).
4. Handler calls `JiraIssueLinkService::create_link(test_case_id, project_id, cmd, user_id)`.
5. Service verifies user has `project:update` permission and is Owner, Editor, or
   Contributor on the project.
6. Service checks for duplicate link. If duplicate, returns `DuplicateJiraIssueLinkError`.
7. Service resolves Jira credentials.
8. Service calls `JiraApiClient::get_issue(jira_issue_key, pat)` to validate the issue.
   - Success: returns issue summary.
   - Not found: returns `JiraIssueNotFoundError`.
   - API error: returns `JiraApiError`.
9. Service inserts the link via `JiraIssueLinkRepository::insert(link)`.
10. Handler returns `201 Created`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `JiraIssueLink` | Domain (1) | Entity: `test_case_id`, `jira_issue_key`, `link_type`, `summary`, `linked_by`, `linked_at` |
| `LinkType` | Domain (1) | Enum: `Related`, `Bug`, `Requirement` |
| `JiraIssueLinkService` | Application (2) | Orchestrates list, create, delete. |
| `JiraIssueLinkRepository` | Application (2) | Interface (port): `find_by_test_case_id`, `find_by_case_and_issue`, `find_by_test_case_id_and_type`, `insert`, `delete` |
| `JiraIssueLinkHandler` | Adapters (3) | HTTP handler with three methods |
| `SqlJiraIssueLinkRepository` | Infrastructure (4) | Implements `JiraIssueLinkRepository` |

### Extended Shared Components

| Component | Change |
|-----------|--------|
| `JiraApiClient` | Add method: `get_issue(key, pat) -> IssueInfo` |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level |
|------------|-------------|------------|-----------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO |
| Lacking permission or role | `403` | `FORBIDDEN` | INFO |
| Test case not found or soft-deleted | `404` | `NOT_FOUND` | INFO |
| Jira issue not found | `404` | `JIRA_ISSUE_NOT_FOUND` | INFO |
| Link not found | `404` | `NOT_FOUND` | INFO |
| Duplicate link | `409` | `DUPLICATE_JIRA_ISSUE_LINK` | INFO |
| Jira not configured | `412` | `JIRA_NOT_CONFIGURED` | INFO |
| Invalid link_type or issue key | `422` | `VALIDATION_ERROR` | INFO |
| Jira API unreachable or error | `502` | `JIRA_API_ERROR` | ERROR |

**Anti-patterns explicitly avoided:**

- **Do not allow multiple link_types for the same issue** — one test case can only link
  to a Jira issue once. If the semantics change, delete and re-create the link.
- **Do not allow VIEWER role to link** — only Owner, Editor, Contributor can mutate
  issue links. This is the only Jira link feature where Contributors have write access.
