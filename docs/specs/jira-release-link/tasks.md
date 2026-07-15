# Tasks: Jira Release Link

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit of work
that maps to one or more requirements or design sections.

---

## Layer 1 — Domain

- [ ] 1. Implement `JiraReleaseLink` entity — `requirements.md#US-2`, `design.md#Components`
  - Fields: `test_plan_id`, `jira_release_id`, `jira_release_name`,
    `jira_project_key`, `linked_by`, `linked_at`
  - Factory method with domain validation (release ID not empty)
  - No framework imports; pure Rust struct + impl

- [ ] 2. Define domain exceptions for Jira release links — `design.md#Error Handling`
  - `JiraReleaseNotFoundError`
  - `DuplicateJiraReleaseLinkError`
  - `JiraProjectNotLinkedError`

---

## Layer 2 — Application

- [ ] 3. Define `JiraReleaseLinkRepository` interface (port) — `design.md#Components`,
       `design.md#Data Model`
  - Methods: `find_by_test_plan_id(plan_id) -> Vec<JiraReleaseLink>`,
    `find_by_plan_and_release(plan_id, release_id) -> Option<JiraReleaseLink>`,
    `insert(link: &JiraReleaseLink) -> JiraReleaseLink`,
    `delete(plan_id, release_id) -> bool`

- [ ] 4. Extend `JiraApiClient` interface with release support — `design.md#Components`
  - Add method: `get_release(project_key: &str, release_id: &str, pat: &str)
    -> Result<JiraReleaseInfo>`
  - `JiraReleaseInfo` DTO: `id: String`, `name: String`, `project_key: String`

- [ ] 5. Implement `JiraReleaseLinkService` — `requirements.md#US-1` through `US-3`,
       `design.md#Sequence`
  - `list_links(test_plan_id, user_id)`: verifies project membership, returns links
  - `create_link(test_plan_id, jira_release_id, user_id)`: verifies permission and
    role, resolves linked Jira projects, resolves Jira credentials, validates release
    via Jira API, checks duplicates, inserts link
  - `delete_link(test_plan_id, jira_release_id, user_id)`: verifies permission and
    role, checks link existence, deletes
  - All methods accept `JiraProjectLinkRepository` and `JiraConfigService`

- [ ] 6. Define command/query DTOs — `design.md#API Contract`
  - `CreateJiraReleaseLinkCommand` (jira_release_id)
  - `JiraReleaseLinkResponse` (jira_release_id, jira_release_name, jira_project_key,
    linked_by user object, linked_at)
  - `JiraReleaseInfo` (id, name, project_key) — shared DTO

- [ ] 7. Write unit tests for `JiraReleaseLinkService` — `requirements.md#US-1`
       through `US-3`
  - Happy path: create link with valid release
  - No linked Jira projects (returns `JIRA_PROJECT_NOT_LINKED`)
  - Jira release not found in any linked project
  - Duplicate link rejection
  - Jira not configured
  - Jira API error
  - Permission denial scenarios
  - List returns correct links, empty list when none

---

## Layer 3 — Adapters (HTTP)

- [ ] 8. Implement `JiraReleaseLinkHandler` — `design.md#API Contract`,
       `design.md#Components`
  - Three handler methods: `list`, `create`, `delete`
  - Deserialize request bodies and path params into DTOs
  - Call `JiraReleaseLinkService` methods
  - Serialize responses with proper status codes

- [ ] 9. Register Jira release link routes in HTTP router — `design.md#Components`
  - `GET    /api/v1/test-plans/{id}/jira-releases`          -> `list`
  - `POST   /api/v1/test-plans/{id}/jira-releases`          -> `create`
  - `DELETE /api/v1/test-plans/{id}/jira-releases/{rid}`    -> `delete`
  - All routes require session auth middleware

- [ ] 10. Write integration tests for Jira release link HTTP handlers —
          `design.md#API Contract`
  - Test `GET` returns linked releases
  - Test `POST` creates link and returns `201`
  - Test `POST` with no linked Jira project returns `412`
  - Test `POST` with non-existent release returns `404`
  - Test `DELETE` removes link and returns `204`
  - Test `403` for unauthorized users
  - Mock Jira API in integration tests

---

## Layer 4 — Infrastructure

- [ ] 11. Create `JIRA_RELEASE_LINKS` database migration — `design.md#Data Model`
  - Table definition with all columns, composite PK, FKs
  - Composite primary key: `(test_plan_id, jira_release_id)`
  - FK indexes on `linked_by`, `jira_project_key`
  - Rollback migration: `DROP TABLE IF EXISTS jira_release_links`

- [ ] 12. Implement `SqlJiraReleaseLinkRepository` — `design.md#Components`
  - All methods from `JiraReleaseLinkRepository` interface
  - `find_by_test_plan_id` returns links ordered by `linked_at DESC`
  - Write unit tests with a test transaction

- [ ] 13. Extend `JiraRestApiClient` with `get_release` method — `design.md#Components`
  - `get_release(project_key, release_id, pat)`: calls
    `GET /rest/api/3/project/{key}/version/{id}` with `Authorization: Bearer {pat}`
  - Map responses: 200 -> `JiraReleaseInfo`, 404 -> `JiraReleaseNotFoundError`,
    401/403/5xx -> `JiraApiError`
  - Write unit tests with mocked HTTP transport

---

## Verification & Cleanup

- [ ] 14. End-to-end verification — `requirements.md#US-1` through `US-3`
  - Link a test plan to a Jira release via API
  - Verify release appears in list
  - Verify duplicate link is rejected
  - Verify unlinked release is removed
  - Verify no linked Jira project results in error
  - Verify permission enforcement

- [ ] 15. Update `specs/README.md`
  - Mark `jira-release-link` as having completed specs
