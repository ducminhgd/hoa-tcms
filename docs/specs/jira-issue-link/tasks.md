# Tasks: Jira Issue Link

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit of work
that maps to one or more requirements or design sections.

---

## Layer 1 — Domain

- [ ] 1. Implement `JiraIssueLink` entity and `LinkType` enum —
       `requirements.md#US-2`, `design.md#Components`
  - Fields: `test_case_id`, `jira_issue_key`, `link_type`, `summary`, `linked_by`,
    `linked_at`
  - `LinkType` enum: `Related`, `Bug`, `Requirement`
  - Factory method with domain validation (link_type is valid, issue key matches
    `[A-Z][A-Z0-9]*-[0-9]+` pattern)
  - No framework imports; pure Rust struct + impl

- [ ] 2. Define domain exceptions for Jira issue links — `design.md#Error Handling`
  - `JiraIssueNotFoundError`
  - `DuplicateJiraIssueLinkError`

---

## Layer 2 — Application

- [ ] 3. Define `JiraIssueLinkRepository` interface (port) — `design.md#Components`,
       `design.md#Data Model`
  - Methods: `find_by_test_case_id(case_id) -> Vec<JiraIssueLink>`,
    `find_by_case_and_issue(case_id, key) -> Option<JiraIssueLink>`,
    `find_by_test_case_id_and_type(case_id, link_type) -> Vec<JiraIssueLink>`,
    `insert(link: &JiraIssueLink) -> JiraIssueLink`,
    `delete(case_id, key) -> bool`

- [ ] 4. Extend `JiraApiClient` interface with issue support — `design.md#Components`
  - Add method: `get_issue(key: &str, pat: &str) -> Result<JiraIssueInfo>`
  - `JiraIssueInfo` DTO: `key: String`, `summary: String`,
    `issue_type: String` (e.g. Bug, Story, Task)

- [ ] 5. Implement `JiraIssueLinkService` — `requirements.md#US-1` through `US-3`,
       `design.md#Sequence`
  - `list_links(test_case_id, project_id, user_id, link_type_filter)`: verifies
    project membership, returns links, optionally filtered by link_type
  - `create_link(test_case_id, project_id, cmd, user_id)`: verifies permission
    (`project:update`) and role (Owner/Editor/Contributor), checks duplicates,
    resolves credentials, validates issue via Jira API, inserts link
  - `delete_link(test_case_id, project_id, jira_issue_key, user_id)`: verifies
    permission and role, checks link existence, deletes
  - Contributor role check is less restrictive than project/release link features

- [ ] 6. Define command/query DTOs — `design.md#API Contract`
  - `CreateJiraIssueLinkCommand` (jira_issue_key, link_type)
  - `JiraIssueLinkResponse` (jira_issue_key, link_type, summary, linked_by user
    object, linked_at)
  - `JiraIssueInfo` (key, summary, issue_type) — shared DTO

- [ ] 7. Write unit tests for `JiraIssueLinkService` — `requirements.md#US-1`
       through `US-3`
  - Happy path: create link with valid issue
  - Duplicate link rejection
  - Jira issue not found
  - Invalid link_type rejection
  - Jira not configured
  - Jira API error
  - Permission denial for Viewer role
  - Permission success for Contributor role
  - List with link_type filter
  - List returns empty when no links exist

---

## Layer 3 — Adapters (HTTP)

- [ ] 8. Implement `JiraIssueLinkHandler` — `design.md#API Contract`,
       `design.md#Components`
  - Three handler methods: `list`, `create`, `delete`
  - Deserialize request bodies and query params
  - Validate `link_type` enum at the HTTP boundary
  - Call `JiraIssueLinkService` methods
  - Serialize responses with proper status codes

- [ ] 9. Register Jira issue link routes in HTTP router — `design.md#Components`
  - `GET    /api/v1/projects/{pid}/test-cases/{id}/jira-issues`       -> `list`
  - `POST   /api/v1/projects/{pid}/test-cases/{id}/jira-issues`       -> `create`
  - `DELETE /api/v1/projects/{pid}/test-cases/{id}/jira-issues/{key}` -> `delete`
  - All routes require session auth middleware
  - Project existence check before handler logic

- [ ] 10. Write integration tests for Jira issue link HTTP handlers —
          `design.md#API Contract`
  - Test `GET` returns linked issues with correct fields
  - Test `GET` with `?link_type=BUG` filter
  - Test `POST` creates link and returns `201`
  - Test `POST` with duplicate key returns `409`
  - Test `POST` with invalid link_type returns `422`
  - Test `POST` with non-existent issue returns `404`
  - Test `DELETE` removes link and returns `204`
  - Test `403` for Viewer role
  - Test `201` for Contributor role
  - Mock Jira API in integration tests

---

## Layer 4 — Infrastructure

- [ ] 11. Create `JIRA_ISSUE_LINKS` database migration — `design.md#Data Model`
  - Table definition with all columns, composite PK, FKs, check constraint
  - Composite primary key: `(test_case_id, jira_issue_key)`
  - `CHECK (link_type IN ('RELATED', 'BUG', 'REQUIREMENT'))`
  - FK indexes on `linked_by`
  - Index on `link_type` for filtered queries
  - Rollback migration: `DROP TABLE IF EXISTS jira_issue_links`

- [ ] 12. Implement `SqlJiraIssueLinkRepository` — `design.md#Components`
  - All methods from `JiraIssueLinkRepository` interface
  - `find_by_test_case_id` returns links ordered by `linked_at DESC`
  - `find_by_test_case_id_and_type` filters by link_type
  - Write unit tests with a test transaction

- [ ] 13. Extend `JiraRestApiClient` with `get_issue` method — `design.md#Components`
  - `get_issue(key, pat)`: calls `GET /rest/api/3/issue/{key}` with
    `Authorization: Bearer {pat}`, query param `fields=summary,issuetype`
  - Map responses: 200 -> `JiraIssueInfo`, 404 -> `JiraIssueNotFoundError`,
    401/403/5xx -> `JiraApiError`
  - Write unit tests with mocked HTTP transport

---

## Verification & Cleanup

- [ ] 14. End-to-end verification — `requirements.md#US-1` through `US-3`
  - Link a test case to a Jira issue with BUG type
  - Verify link appears in list
  - Filter by link_type and verify correct results
  - Verify duplicate link is rejected
  - Unlink and verify removal
  - Verify Contributor can create/delete links
  - Verify Viewer cannot create links
  - Verify invalid link_type is rejected

- [ ] 15. Update `specs/README.md`
  - Mark `jira-issue-link` as having completed specs
