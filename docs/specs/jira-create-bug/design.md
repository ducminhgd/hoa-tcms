# Design: Jira Create Bug

## Architecture

The Jira Create Bug feature follows Clean Architecture layering. The feature depends on
three existing Jira integration features (jira-config, jira-project-link, jira-issue-link)
and mirrors the authorization model of test-case-result-update.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handler:                                                         │   │
│  │  - create_bug  POST /api/v1/test-results/{resultId}/create-jira-bug   │   │
│  └──┬────────────────────────────────────────────────────────────────────┘   │
│     │ calls                                                                  │
│     ▼                                                                        │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  JiraCreateBugService:                    │  │  - TestCaseResult       │  │
│  │  - create_bug_from_result                 │  │    (from test-case-     │  │
│  │                                           │  │     result-update)     │  │
│  │  Interfaces / Dependencies:               │  └──────────────────────────┘  │
│  │  - JiraConfigService                      │                                │
│  │  - JiraProjectLinkRepository              │                                │
│  │  - JiraIssueLinkRepository                │                                │
│  │  - TestCaseResultRepository               │                                │
│  │  - ExecutionTesterRepository              │                                │
│  │  - JiraApiClient (port)                   │                                │
│  │  - AuthorizationService                   │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - JiraRestApiClient (shared, implements JiraApiClient)                │   │
│  │  - SqlJiraIssueLinkRepository (from jira-issue-link)                   │   │
│  │  - SqlJiraProjectLinkRepository (from jira-project-link)               │   │
│  │  - SqlTestCaseResultRepository (from test-case-result-update)          │   │
│  │  - SqlExecutionTesterRepository (from test-case-result-update)         │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. The endpoint requires an authenticated session (checked by `AuthMiddleware`).
2. Handler validates the test case result exists and is not soft-deleted. If not ->
   `404 Not Found`.
3. Handler validates the result status is `FAIL`. If not -> `422 RESULT_NOT_FAILED`.
4. Handler calls `JiraCreateBugService::create_bug_from_result(...)`.
5. Service checks `test_execution:update` system permission.
6. Service resolves fine-grained authorization (mirrors test-case-result-update):
   a. System Admin -> bypass all checks, allowed.
   b. Project Owner or Editor -> allowed to create bugs from any result.
   c. Project Contributor -> allowed if they created the result OR are an assigned tester
      for the execution.
   d. Assigned tester (in `execution_testers`) -> allowed regardless of project role.
   e. Otherwise -> `403 Forbidden`.
7. Service resolves the linked Jira project:
   a. Queries `JIRA_PROJECT_LINKS` for the test case's TCMS project.
   b. If zero links -> `404 JIRA_PROJECT_NOT_LINKED`.
   c. If exactly one link -> use it.
   d. If multiple links -> `422 MULTIPLE_JIRA_PROJECTS` (client must disambiguate).
8. Service resolves Jira credentials (user-level PAT -> project-level fallback). If none ->
   `412 JIRA_NOT_CONFIGURED`.
9. Service builds the Jira bug payload from the test case result data.
10. Service inserts a **PENDING** `JIRA_ISSUE_LINK` row with `link_type = 'BUG'` and
    `jira_issue_key = NULL`. The unique partial index serves as a distributed lock. If
    a BUG link already exists for this test case, the insert fails with `409 Conflict`.
11. Service calls `JiraApiClient::create_bug()` to create the issue in Jira.
    - On Jira API failure: delete the PENDING row and return the Jira error.
12. On success, service updates the PENDING row to CONFIRMED by setting
    `jira_issue_key` and `summary` from the Jira API response.
13. Handler returns `201 Created` with the Jira issue key and URL.

---

## API Contract

### POST `/api/v1/test-results/{resultId}/create-jira-bug`

Create a Jira bug ticket from a failed Test Case Result. The bug is auto-populated with
fields derived from the test case result and linked back to the source test case via
`JIRA_ISSUE_LINKS`.

**Required Permission:** `test_execution:update`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `resultId` | integer | Test Case Result ID |

**Request Body** (all fields optional; empty body is accepted):

```json
{
  "jira_project_key": "PROJ"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `jira_project_key` | string | No | Jira project key matching a linked project. If omitted and exactly one Jira project is linked to the TCMS project, it is auto-resolved. If omitted and multiple are linked, an error with the available list is returned. |

**Success Response:** `201 Created`

```json
{
  "data": {
    "jira_issue_key": "PROJ-456",
    "jira_issue_url": "https://company.atlassian.net/browse/PROJ-456",
    "summary": "[FAIL] User cannot log in with invalid credentials",
    "link_type": "BUG",
    "test_case_id": 42,
    "created_by": 15,
    "created_at": "2026-07-15T10:00:00Z"
  }
}
```

**Auto-populated Jira Bug Fields:**

The Jira issue is created with the following fields:

| Jira Field | Value |
|------------|-------|
| `summary` | `"[FAIL] {test_case_result.summary}"` |
| `description` | Formatted text block (see template below) |
| `issuetype` | `"Bug"` (name-based; resolved to ID via Jira API metadata if needed) |
| `project` | `{"key": "{resolved_jira_project_key}"}` |

**Description template:**

```
h2. Test Failure Details
*Test Case:* {test_case_result.summary}
*Project:* {project_name}
*Priority:* {test_case_result.priority}
*Tested by:* {tester_username}
*Result updated at:* {result.updated_at}

h2. Description
{test_case_result.description}

h2. Steps to Reproduce
{formatted_test_steps}

h2. Execution Logs
{code}
{test_case_result.logs}
{code}

---
_Automatically created by TCMS from Test Result #{resultId}_
```

**Notes:**
- The request body has no required fields. Sending an empty body `{}` is valid when the
  TCMS project has exactly one linked Jira project.
- The `JIRA_ISSUE_LINK` row is created with `link_type = 'BUG'` linking the test case
  (not the result) to the new Jira issue.
- `created_by` in the response is the authenticated user who triggered the creation.
- `created_at` is the timestamp when the Jira issue was created.
- Unrecognised fields in the request body cause a `422 Unprocessable Entity` response
  (strict mode -- rejects typos and unknown fields).
- The local link is inserted in PENDING state before the Jira API call (the unique index
  acts as a distributed lock). If the Jira API call succeeds but the PENDING row update to
  CONFIRMED fails, the orphaned Jira issue key is logged at ERROR level for manual
  cleanup. The response is `500 Internal Server Error`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:update` permission, is not a project member, or (for Contributors) does not own the result and is not an assigned tester |
| `404` | `NOT_FOUND` | Test Case Result does not exist or is soft-deleted |
| `404` | `JIRA_PROJECT_NOT_LINKED` | The test case's TCMS project has no linked Jira projects |
| `409` | `BUG_ALREADY_CREATED` | A BUG-type Jira issue link already exists for this test case |
| `412` | `JIRA_NOT_CONFIGURED` | No Jira config found (neither user-level nor project-level) |
| `422` | `RESULT_NOT_FAILED` | Test Case Result status is not FAIL |
| `422` | `MULTIPLE_JIRA_PROJECTS` | Multiple Jira projects linked; `jira_project_key` must be specified |
| `422` | `VALIDATION_ERROR` | Invalid `jira_project_key` format or unrecognised request fields |
| `502` | `JIRA_API_ERROR` | Jira API call failed (create issue returned an error) |
| `503` | `JIRA_SERVICE_UNAVAILABLE` | Jira API unreachable (timeout, connection refused) |

`409` error response body when a bug already exists:

```json
{
  "error": {
    "code": "BUG_ALREADY_CREATED",
    "message": "A Jira bug already exists for this test case.",
    "details": [
      {
        "field": "jira_issue_key",
        "message": "PROJ-123"
      },
      {
        "field": "jira_issue_url",
        "message": "https://company.atlassian.net/browse/PROJ-123"
      }
    ]
  }
}
```

`422` error response body for multiple linked Jira projects:

```json
{
  "error": {
    "code": "MULTIPLE_JIRA_PROJECTS",
    "message": "Multiple Jira projects are linked to this TCMS project. Specify jira_project_key.",
    "details": [
      { "field": "jira_project_key", "message": "Available: PROJ, DEV" }
    ]
  }
}
```

---

## Data Model

### No New Tables

This feature does not create any new tables. It reuses:

- `JIRA_ISSUE_LINKS` (from `jira-issue-link`) -- stores the BUG link between the test case
  and the created Jira issue.
- `JIRA_PROJECT_LINKS` (from `jira-project-link`) -- resolves the target Jira project for
  bug creation.
- `JIRA_CONFIGS` (from `jira-config`) -- resolves Jira credentials.
- `TEST_CASE_RESULTS` (from `test-execution-import`) -- source of the bug content.
- `TEST_CASES` (from `test-case-crud`) -- parent of the result; target of the BUG link.
- `TEST_EXECUTIONS` (from `test-execution-crud`) -- parent of the result; used for tester
  assignment checks.
- `EXECUTION_TESTERS` (from `test-execution-crud`) -- used for assigned tester authorization.

### Jira Issue Link Row Inserted

When a bug is created, a row is inserted into `JIRA_ISSUE_LINKS`:

| Column | Value |
|--------|-------|
| `test_case_id` | The `test_case_id` from the Test Case Result |
| `jira_issue_key` | The key returned by the Jira API (e.g., `PROJ-456`) |
| `link_type` | `'BUG'` |
| `summary` | The Jira issue summary (cached from Jira API response) |
| `linked_by` | The authenticated user's ID |
| `linked_at` | `NOW()` |

**Uniqueness constraint:**

```sql
-- Ensure at most one BUG link per test case at the database level.
-- This is the definitive idempotency guard.
CREATE UNIQUE INDEX uq_jira_issue_links_one_bug_per_case
  ON jira_issue_links (test_case_id) WHERE link_type = 'BUG';
```

---

## Sequence

### Create Bug From Test Result Flow

1. Client sends `POST /api/v1/test-results/523/create-jira-bug` with optional
   `{"jira_project_key": "PROJ"}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates the test case result exists and is not soft-deleted. If not ->
   `404 Not Found`.
4. Handler validates the result status is `FAIL`. If not -> `422 RESULT_NOT_FAILED`.
5. Handler calls `JiraCreateBugService::create_bug_from_result(result_id, cmd,
   current_user_id)`.
6. `JiraCreateBugService` loads the result and its parent chain (execution -> run ->
   project) via `TestCaseResultRepository::find_with_chain()`.
7. `JiraCreateBugService` checks the user has `test_execution:update` system permission.
8. `JiraCreateBugService` resolves fine-grained authorization:
   a. If System Admin -> skip remaining checks.
   b. If project Owner or Editor -> allowed.
   c. If project Contributor:
      - Check if `result.created_by == current_user_id` -> allowed.
      - Check if the user is an assigned tester on the execution (via
        `ExecutionTesterRepository`) -> allowed.
      - Otherwise -> `403` ("You can only create Jira bugs from your own Test Case Results").
   d. If user is an assigned tester on the execution (but not an Owner/Editor/Contributor
      of the project, possibly a Viewer or non-member) -> allowed.
   e. Otherwise -> `403` (generic message).
9. `JiraCreateBugService` resolves the linked Jira project:
    a. Queries `JiraProjectLinkRepository::find_by_project_id(project_id)`.
    b. If `jira_project_key` was provided in the request, validates it is in the list of
       linked projects. If not found -> `404 JIRA_PROJECT_NOT_LINKED`.
    c. If `jira_project_key` was omitted and there are zero links -> `404
       JIRA_PROJECT_NOT_LINKED`.
    d. If `jira_project_key` was omitted and there is exactly one link -> use it.
    e. If `jira_project_key` was omitted and there are multiple links -> `422
       MULTIPLE_JIRA_PROJECTS` with the list of available keys.
10. `JiraCreateBugService` resolves Jira credentials via
    `JiraConfigService::resolve_pat(user_id, project_id)`. If no credentials ->
    `412 JIRA_NOT_CONFIGURED`.
11. `JiraCreateBugService` builds the bug payload:
    - `summary`: `"[FAIL] {result.summary}"`
    - `description`: formatted from result data using the description template
    - `issuetype`: `{"name": "Bug"}`
    - `project`: `{"key": "{jira_project_key}"}`
12. `JiraCreateBugService` inserts a **PENDING** `JIRA_ISSUE_LINK` row with
    `link_type = 'BUG'` and `jira_issue_key = NULL`. The unique partial index
    `uq_jira_issue_links_one_bug_per_case` acts as a distributed lock, ensuring
    only one concurrent bug-creation request can proceed per test case:
    - If the insert fails due to the unique constraint -> `409 BUG_ALREADY_CREATED`
      with the existing issue key and URL (the other transaction already holds the row).
13. `JiraCreateBugService` calls `JiraApiClient::create_bug(jira_project_key, fields, pat)`.
    - Success: receives `{key: "PROJ-456", url: "https://..."}`.
    - API error: deletes the PENDING link row and returns `JiraApiError` (mapped to
      `502`). No local state remains.
14. `JiraCreateBugService` updates the PENDING `JIRA_ISSUE_LINK` row to CONFIRMED by
    setting `jira_issue_key`, `summary`, and all other populated columns.
    - If update fails: the PENDING row is a dead entry. It is logged at ERROR level
      with the orphaned Jira issue key for manual cleanup and the response is
      `500 Internal Server Error`.
15. Handler constructs the response DTO.
16. Handler returns `201 Created`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `JiraCreateBugService` | Application (2) | Orchestrates the end-to-end bug creation flow: validates result status, checks authorization, checks for existing BUG link, resolves Jira project and credentials, builds the bug payload, calls Jira API, inserts the local link. |
| `JiraCreateBugHandler` | Adapters (3) | HTTP handler with one method: `create_bug`. Loads the test result, validates status, calls service, serializes response. |

### Extended Shared Components

| Component | Source Feature | Change |
|-----------|---------------|--------|
| `JiraApiClient` | `jira-project-link` | Add method: `create_bug(project_key: &str, fields: &BugFields, pat: &str) -> Result<CreatedIssueInfo>` |

### Reused Components (no changes)

| Component | Source Feature |
|-----------|---------------|
| `TestCaseResult` | `test-case-result-update` |
| `TestCaseResultRepository` | `test-case-result-update` |
| `ExecutionTesterRepository` | `test-case-result-update` |
| `AuthorizationService` | `auth-rbac` |
| `JiraConfigService` | `jira-config` |
| `JiraProjectLinkRepository` | `jira-project-link` |
| `JiraIssueLinkRepository` | `jira-issue-link` |
| `JiraRestApiClient` | `jira-project-link` (extended with `create_bug`) |

### Domain DTOs

| DTO | Layer | Description |
|-----|-------|-------------|
| `CreateJiraBugCommand` | Application (2) | `jira_project_key: Option<String>`. Carries the optional target Jira project key. |
| `CreateJiraBugResponse` | Application (2) | `jira_issue_key`, `jira_issue_url`, `summary`, `link_type`, `test_case_id`, `created_by`, `created_at`. |
| `BugFields` | Application (2) | `summary: String`, `description: String`, `issuetype: String`, `project_key: String`. Passed to `JiraApiClient::create_bug`. |
| `CreatedIssueInfo` | Application (2) | `key: String`, `url: String`. Returned by `JiraApiClient::create_bug`. |

---

## Permission Code

This feature uses an existing permission code defined by `test-execution-crud`:

| code | name |
|------|------|
| `test_execution:update` | Update Test Execution |

---

## Route Registration

```text
POST /api/v1/test-results/{resultId}/create-jira-bug
  -> create_bug
```

This route requires:
- Session-based authentication (`AuthMiddleware`)
- Test Case Result existence validation (result not soft-deleted)
- Result status validation (must be FAIL)
- System permission check (`test_execution:update`)
- Fine-grained authorization (mirrors test-case-result-update):
  - Owner/Editor of project -> any result
  - Contributor -> own results (created_by matches) or assigned tester
  - Assigned tester -> any result in the execution
  - System Admin -> bypasses all

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks `test_execution:update` permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks project membership for the project | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Contributor does not own the result and is not assigned tester | `403` | `FORBIDDEN` | INFO | Distinct message: "You can only create Jira bugs from your own Test Case Results" |
| Test Case Result not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Generic message |
| No linked Jira projects for the TCMS project | `404` | `JIRA_PROJECT_NOT_LINKED` | INFO | Distinct message: "No Jira projects are linked to this TCMS project" |
| BUG link already exists for the test case | `409` | `BUG_ALREADY_CREATED` | INFO | Includes existing issue key and URL in details |
| Jira not configured (user or project level) | `412` | `JIRA_NOT_CONFIGURED` | INFO | |
| Result status is not FAIL | `422` | `RESULT_NOT_FAILED` | INFO | |
| Multiple Jira projects linked, no key specified | `422` | `MULTIPLE_JIRA_PROJECTS` | INFO | Includes available keys in details |
| Invalid `jira_project_key` format | `422` | `VALIDATION_ERROR` | INFO | |
| Unrecognised fields in request body | `422` | `VALIDATION_ERROR` | INFO | Strict mode: rejects typos. Details list unknown field names. |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Returns `Retry-After` header |
| Jira API returns error on create | `502` | `JIRA_API_ERROR` | ERROR | PENDING row deleted; safe to retry |
| Jira API timeout or unreachable | `503` | `JIRA_SERVICE_UNAVAILABLE` | ERROR | |
| PENDING-to-CONFIRMED update fails after Jira bug created | `500` | `INTERNAL_ERROR` | ERROR | Orphaned Jira issue key logged for manual cleanup |

**Anti-patterns explicitly avoided:**

- **Do not create duplicate bugs** -- the unique partial index
  `uq_jira_issue_links_one_bug_per_case` guarantees at most one BUG link per test case at
  the database level. This is the definitive idempotency guard.
- **Insert a PENDING link before calling the Jira API** -- the unique index acts as a
  distributed lock. If the Jira API fails, delete the PENDING row to release the slot.
- **Do not attempt distributed transaction rollback** -- Jira bug creation is not
  transactional with the local DB. If the local link update to CONFIRMED fails after the
  Jira bug was created, log the orphaned bug for manual cleanup rather than attempting to
  delete it from Jira (which could also fail).
- **Do not expose** which authorization gate rejected the request (generic `403 FORBIDDEN`
  except for the Contributor ownership case).
- **Do not hard-code Jira field IDs** -- use Jira's field name API (`issuetype`, `project`)
  which resolves to the correct field ID per project.
- **Do not log the Jira PAT** in error messages or logs.
