# Tasks: Jira Project Link

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit of work
that maps to one or more requirements or design sections.

---

## Layer 1 — Domain

- [ ] 1. Implement `JiraProjectLink` entity — `requirements.md#US-2`, `design.md#Components`
  - Fields: `tcms_project_id`, `jira_project_key`, `jira_project_name`, `linked_by`,
    `linked_at`
  - Factory method `JiraProjectLink::create(tcms_project_id, jira_project_key,
    jira_project_name, linked_by)` with domain validation (key format: uppercase
    letters, digits, hyphens, 1-255 chars)
  - No framework imports; pure Rust struct + impl

- [ ] 2. Define domain exceptions for Jira project links — `design.md#Error Handling`
  - `JiraProjectNotFoundError` (Jira API returned 404)
  - `DuplicateJiraLinkError`
  - `JiraNotConfiguredError`
  - `JiraApiError` (Jira API unreachable or error response)

---

## Layer 2 — Application

- [ ] 3. Define `JiraProjectLinkRepository` interface (port) — `design.md#Components`,
       `design.md#Data Model`
  - Methods: `find_by_project_id(project_id) -> Vec<JiraProjectLink>`,
    `find_by_project_and_key(project_id, key) -> Option<JiraProjectLink>`,
    `insert(link: &JiraProjectLink) -> JiraProjectLink`,
    `delete(project_id, key) -> bool`

- [ ] 4. Define `JiraApiClient` interface (port) — `design.md#Components`
  - Method: `get_project(key: &str, pat: &str) -> Result<JiraProjectInfo>`
  - `JiraProjectInfo` DTO: `key: String`, `name: String`
  - This interface is shared across all Jira features; other methods
    (`get_release`, `get_issue`, `create_bug`) are added in subsequent features
  - Returns a domain-level error (not HTTP status codes) — error mapping is the
    implementation's responsibility

- [ ] 5. Implement `JiraProjectLinkService` — `requirements.md#US-1` through `US-3`,
       `design.md#Sequence`
  - `list_links(project_id, user_id)`: verifies `project:read` permission and project
    membership, returns list of links
  - `create_link(project_id, jira_project_key, user_id)`: verifies `project:update`
    permission and Owner/Editor role, checks for duplicates, resolves Jira PAT,
    validates Jira project via API, inserts link
  - `delete_link(project_id, jira_project_key, user_id)`: verifies
    `project:update` permission and Owner/Editor role, checks link exists, deletes
  - All methods accept `JiraConfigService` for credential resolution

- [ ] 6. Define command/query DTOs — `design.md#API Contract`
  - `CreateJiraProjectLinkCommand` (jira_project_key)
  - `JiraProjectLinkResponse` (jira_project_key, jira_project_name, linked_by user
    object, linked_at)
  - `JiraProjectInfo` (key, name) — shared DTO returned by `JiraApiClient`

- [ ] 7. Write unit tests for `JiraProjectLinkService` — `requirements.md#US-1`
       through `US-3`
  - Happy path: create link with valid Jira project
  - Duplicate link rejection
  - Jira project not found (404 from Jira API)
  - Jira not configured (no PAT found)
  - Jira API error (simulate timeout/5xx)
  - Permission denial (each combination)
  - List returns correct links, empty list when none exist
  - Delete removes link, returns 404 if already deleted

---

## Layer 3 — Adapters (HTTP)

- [ ] 8. Implement `JiraProjectLinkHandler` — `design.md#API Contract`,
       `design.md#Components`
  - Three handler methods: `list`, `create`, `delete`
  - Deserialize request bodies and path params into DTOs
  - Call `JiraProjectLinkService` methods
  - Serialize responses with proper status codes
  - Set `Location` header on `201 Created` (optional — link URL is composite key)

- [ ] 9. Register Jira project link routes in HTTP router — `design.md#Components`
  - `GET    /api/v1/projects/{id}/jira-links`           -> `list`
  - `POST   /api/v1/projects/{id}/jira-links`           -> `create`
  - `DELETE /api/v1/projects/{id}/jira-links/{key}`     -> `delete`
  - All routes require session auth middleware
  - Project existence check before any handler logic

- [ ] 10. Write integration tests for Jira project link HTTP handlers —
         `design.md#API Contract`
  - Test `GET` returns linked Jira projects
  - Test `POST` creates link and returns `201`
  - Test `POST` with duplicate key returns `409`
  - Test `POST` with non-existent Jira project returns `404 JIRA_PROJECT_NOT_FOUND`
  - Test `POST` with no Jira config returns `412`
  - Test `DELETE` removes link and returns `204`
  - Test `DELETE` non-existent link returns `404`
  - Test `403` for non-member and non-Owner/Editor roles
  - Mock Jira API responses in integration tests

---

## Layer 4 — Infrastructure

- [ ] 11. Create `JIRA_PROJECT_LINKS` database migration — `design.md#Data Model`
  - Table definition with all columns, composite PK, FKs
  - Composite primary key: `(tcms_project_id, jira_project_key)`
  - FK indexes on `linked_by`
  - `ON DELETE RESTRICT` for both FKs
  - Rollback migration: `DROP TABLE IF EXISTS jira_project_links`

- [ ] 12. Implement `SqlJiraProjectLinkRepository` — `design.md#Components`
  - All methods from `JiraProjectLinkRepository` interface
  - `find_by_project_id` returns all links for a project, ordered by `linked_at DESC`
  - `find_by_project_and_key` does exact match on composite key
  - `insert` and `delete` use standard SQL operations
  - Write unit tests with a test transaction

- [ ] 13. Implement `JiraRestApiClient` (initial version) — `design.md#Components`
  - HTTP client wrapper for Jira REST API
  - `get_project(key, pat)`: calls `GET /rest/api/3/project/{key}` with
    `Authorization: Bearer {pat}`
  - Configure connection timeout (e.g. 10 seconds) and read timeout (e.g. 30 seconds)
  - Map HTTP responses: 200 -> `JiraProjectInfo`, 404 -> `JiraProjectNotFoundError`,
    401/403 -> `JiraApiError` (auth failure), 5xx -> `JiraApiError`
  - Do not log the PAT value — log only the fact that a request was made
  - Write unit tests by mocking the HTTP transport layer

---

## Verification & Cleanup

- [ ] 14. End-to-end verification — `requirements.md#US-1` through `US-3`
  - Link a TCMS project to a Jira project via API
  - Verify link appears in list
  - Verify duplicate link is rejected
  - Verify non-existent Jira project is rejected
  - Unlink and verify link is removed
  - Verify permission enforcement (non-member, non-Owner/Editor)
  - Verify no Jira config results in `412 Precondition Failed`

- [ ] 15. Update `specs/README.md`
  - Mark `jira-project-link` as having completed specs (requirements.md, design.md,
    tasks.md)
