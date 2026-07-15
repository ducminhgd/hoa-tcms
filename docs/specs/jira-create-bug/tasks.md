# Tasks: Jira Create Bug

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit of work
that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Define domain exceptions for Jira bug creation -- `design.md#Error Handling`
  - `BugAlreadyCreatedError` (carries existing `jira_issue_key` and `jira_issue_url`)
  - `ResultNotFailedError` (current status does not allow bug creation)
  - `JiraProjectNotLinkedError`
  - `MultipleJiraProjectsError` (carries list of available `jira_project_key` values)
  - `JiraNotConfiguredError`
  - `JiraApiError` (carries status code and message from Jira API response)
  - No framework imports; pure domain code

---

## Layer 2 -- Application

- [ ] 2. Extend `JiraApiClient` interface with bug creation -- `design.md#Components`
  - Add method: `create_bug(project_key: &str, fields: &BugFields, pat: &str)
    -> Result<CreatedIssueInfo>`
  - `BugFields` DTO: `summary`, `description`, `issuetype`, `project_key`
  - `CreatedIssueInfo` DTO: `key: String`, `url: String`

- [ ] 3. Define command/response DTOs -- `design.md#API Contract`
  - `CreateJiraBugCommand` (jira_project_key: Option<String>)
    - If `jira_project_key` is provided, it must match `[A-Z][A-Z0-9_]*` and be a linked
      project
    - If omitted, auto-resolved from linked Jira projects (exactly one must exist)
  - `CreateJiraBugResponse` (jira_issue_key, jira_issue_url, summary, link_type,
    test_case_id, created_by, created_at)
  - `BugFields` (summary, description, issuetype, project_key)
  - `CreatedIssueInfo` (key, url)

- [ ] 4. Implement bug description builder -- `design.md#API Contract`
  - Function: `build_bug_description(result: &TestCaseResult, project_name: &str,
    tester_username: &str, test_steps: &str) -> String`
  - Constructs the formatted description from test case result data using the template
    defined in the design (Test Failure Details, Description, Steps to Reproduce,
    Execution Logs, auto-created footer)
  - Formats execution logs in a `{code}` block for Jira wiki markup
  - Sanitizes input to prevent Jira markup injection in user-supplied fields
  - Unit-testable independently of HTTP or database

- [ ] 5. Implement `JiraCreateBugService` -- `requirements.md#US-1` through `US-3`,
       `design.md#Sequence`
  - Single method: `create_bug_from_result(result_id, cmd, current_user_id)`
  - Dependencies injected: `TestCaseResultRepository`, `ExecutionTesterRepository`,
    `AuthorizationService`, `JiraConfigService`, `JiraProjectLinkRepository`,
    `JiraIssueLinkRepository`, `JiraApiClient`
  - Steps:
    1. Load result and parent chain (execution -> run -> project) via
       `TestCaseResultRepository::find_with_chain()`
    2. Verify result status is `FAIL`; if not -> `ResultNotFailedError`
    3. Check `test_execution:update` system permission via `AuthorizationService`
    4. Resolve fine-grained authorization (mirrors test-case-result-update):
       a. System Admin -> allowed
       b. Project Owner or Editor -> allowed
       c. Project Contributor + owns result (`created_by == current_user_id`) -> allowed
       d. Project Contributor + assigned tester (via `ExecutionTesterRepository`) -> allowed
       e. Assigned tester (any project role, including Viewer) -> allowed
       f. Otherwise -> `403 Forbidden`
    5. Check for existing BUG link on test case via
       `JiraIssueLinkRepository::find_by_test_case_id_and_type(test_case_id, BUG)`;
       if found -> `BugAlreadyCreatedError`
    6. Resolve linked Jira project(s) via `JiraProjectLinkRepository`:
       a. If `jira_project_key` provided: validate it is in the linked list
       b. If omitted and zero links -> `JiraProjectNotLinkedError`
       c. If omitted and exactly one link -> use it
       d. If omitted and multiple links -> `MultipleJiraProjectsError`
    7. Resolve Jira credentials via `JiraConfigService::resolve_pat(user_id, project_id)`;
       if none -> `JiraNotConfiguredError`
    8. Build bug payload: summary `"[FAIL] {result.summary}"`, description from
       `build_bug_description()`, issuetype `"Bug"`, project key
    9. Call `JiraApiClient::create_bug(jira_project_key, &bug_fields, &pat)`
    10. On success: insert `JIRA_ISSUE_LINK` with `link_type = 'BUG'` via
        `JiraIssueLinkRepository::insert(link)`
    11. If link insert fails: log orphaned Jira issue at ERROR level, return
        `500 Internal Server Error`
    12. Map result to `CreateJiraBugResponse`

- [ ] 6. Write unit tests for `JiraCreateBugService` -- `requirements.md#US-1` through `US-3`
  - Table-driven tests covering:
    - Happy path: create bug from failed result, returns issue key and URL
    - Result status is not FAIL -> `RESULT_NOT_FAILED`
    - Bug already exists for test case -> `BUG_ALREADY_CREATED` with existing key
    - No linked Jira projects -> `JIRA_PROJECT_NOT_LINKED`
    - Multiple linked Jira projects, no key specified -> `MULTIPLE_JIRA_PROJECTS`
    - `jira_project_key` provided and valid -> resolves correctly
    - `jira_project_key` provided but not linked -> `JIRA_PROJECT_NOT_LINKED`
    - Jira not configured -> `JIRA_NOT_CONFIGURED`
    - Jira API error during creation -> `JIRA_API_ERROR`, no local link created
    - Local link insert fails after Jira bug created -> `500`, orphan logged
    - Missing system permission -> `403`
    - Contributor updating own result -> allowed
    - Contributor updating another's result (not assigned tester) -> `403`
    - Assigned tester (Viewer role) creating bug -> allowed
    - System Admin bypassing all checks
    - Description builder produces correct formatted output
    - Description builder sanitizes input (no Jira markup injection)

---

## Layer 3 -- Adapters (HTTP)

- [ ] 7. Implement `JiraCreateBugHandler` -- `design.md#API Contract`, `design.md#Components`
  - Single handler method: `create_bug`
  - Validates path parameter `resultId` is present and a positive integer
  - Deserializes request body into `CreateJiraBugCommand` with strict mode (reject unknown
    fields)
  - Validates `jira_project_key` format if provided (matches `[A-Z][A-Z0-9_]*`)
  - Loads test result by ID; if not found or soft-deleted -> `404 Not Found`
  - Validates result status is `FAIL` before delegating to service
  - Calls `JiraCreateBugService::create_bug_from_result()`
  - On success: serializes `CreateJiraBugResponse` and returns `201 Created`
  - Maps domain errors to HTTP status codes per the error handling table
  - Maps `BugAlreadyCreatedError` to `409 Conflict` with existing issue key and URL in
    the details

- [ ] 8. Register Jira create bug route in HTTP router -- `design.md#Route Registration`
  - `POST /api/v1/test-results/{resultId}/create-jira-bug` -> `create_bug`
  - Route requires session auth middleware
  - Route ordering: ensure this route is registered after test-results routes exist
  - No path ordering conflicts expected (this is a unique action endpoint, not a CRUD
    resource route)

- [ ] 9. Write integration tests for Jira create bug HTTP handler --
       `design.md#API Contract`
  - Full request/response cycle with a test database and mocked Jira API
  - Test `201 Created` on valid bug creation with correct response shape
  - Test `201 Created` with `jira_project_key` explicitly provided
  - Test `201 Created` with empty body and single linked Jira project (auto-resolve)
  - Test Jira issue link row exists in database after successful creation
  - Test `409 Conflict` on duplicate creation with existing issue key in response
  - Test `422 RESULT_NOT_FAILED` when result status is IN_PROGRESS or PASS
  - Test `422 MULTIPLE_JIRA_PROJECTS` when multiple projects linked and no key specified
  - Test `422 VALIDATION_ERROR` on unrecognised fields in body (strict mode)
  - Test `422 VALIDATION_ERROR` on invalid `jira_project_key` format
  - Test `404 NOT_FOUND` for non-existent result
  - Test `404 NOT_FOUND` for soft-deleted result
  - Test `404 JIRA_PROJECT_NOT_LINKED` when TCMS project has no Jira links
  - Test `403 FORBIDDEN` for user without `test_execution:update` permission
  - Test `403 FORBIDDEN` for project Contributor updating another contributor's result
    (not assigned tester) -- distinct message
  - Test `201 Created` for assigned tester (Viewer role on project) creating bug
  - Test `201 Created` for System Admin creating bug
  - Test `412 JIRA_NOT_CONFIGURED` when no Jira config exists
  - Test `502 JIRA_API_ERROR` when mocked Jira API returns an error
  - Test `503 JIRA_SERVICE_UNAVAILABLE` when mocked Jira API times out

---

## Layer 4 -- Infrastructure

- [ ] 10. Implement `JiraRestApiClient::create_bug` -- `design.md#Components`
  - `create_bug(project_key, fields, pat)`: calls `POST /rest/api/3/issue` with
    `Authorization: Bearer {pat}` and JSON body:
    ```json
    {
      "fields": {
        "project": {"key": "{project_key}"},
        "summary": "{summary}",
        "description": "{description}",
        "issuetype": {"name": "Bug"}
      }
    }
    ```
  - Map responses:
    - `201 Created` -> parse `key` and `self` from response, construct `CreatedIssueInfo`
      with `url = "{jira_base_url}/browse/{key}"`
    - `400 Bad Request` -> `JiraApiError` (validation error from Jira, surface message)
    - `401 Unauthorized` / `403 Forbidden` -> `JiraApiError` (auth failure; do not
      expose raw PAT)
    - `404 Not Found` -> `JiraApiError` (project not found)
    - `5xx` -> `JiraApiError` (Jira server error)
  - Timeout: 30 seconds; connection refused -> `JiraServiceUnavailableError`
  - Write unit tests with mocked HTTP transport:
    - Successful bug creation returns key and URL
    - Jira 401 mapped correctly
    - Jira 404 mapped correctly
    - Timeout handled
    - Request body matches expected Jira API format

- [ ] 11. Implement orphaned bug logging -- `design.md#Error Handling`
  - In `JiraCreateBugService`, after successful Jira API call but failed
    `JiraIssueLinkRepository::insert`:
    - Log at ERROR level: `"Orphaned Jira bug created: key={key}, url={url},
      test_case_id={test_case_id}, user_id={user_id}"`
    - Include all relevant identifiers for manual reconciliation
  - No automated retry or reconciliation mechanism for MVP
  - The Jira bug exists and is functional; only the TCMS link is missing

---

## Verification & Cleanup

- [ ] 12. End-to-end verification -- `requirements.md#US-1` through `US-3`
  - Walkthrough of all three user stories with a real database and a real or sandbox Jira
    instance
  - Create a bug from a failed test result and verify it appears in Jira
  - Verify auto-populated fields: summary format `[FAIL] ...`, description with test steps
    and logs
  - Verify `BUG`-type `JIRA_ISSUE_LINK` is created in TCMS database
  - Verify duplicate bug creation is rejected with existing issue key and URL
  - Verify non-failed result is rejected with `RESULT_NOT_FAILED`
  - Verify permission enforcement for each authorization level:
    - Owner can create bug from any result
    - Contributor can only create from own results
    - Contributor cannot create from another contributor's result (unless assigned tester)
    - Assigned tester (Viewer role) can create bug
    - System Admin bypasses all checks
  - Verify Jira project auto-resolution: single linked project works without specifying key
  - Verify `MULTIPLE_JIRA_PROJECTS` error when multiple projects are linked
  - Verify `JIRA_PROJECT_NOT_LINKED` error when no Jira projects are linked
  - Verify `JIRA_NOT_CONFIGURED` error when no Jira config exists
  - Verify orphaned bug log message when link insert fails

- [ ] 13. Update `specs/README.md` -- `specs/README.md`
  - Mark `jira-create-bug` as having completed specs (requirements.md, design.md,
    tasks.md)

---

## Security & Hardening

- [ ] 14. **Add authorization integration tests** -- `design.md#Security Requirements`
  - Test that a user with `test_execution:update` permission but no project membership
    or tester assignment gets `403` for a project-scoped result
  - Test that a project Viewer (not assigned tester) gets `403`
  - Test that a project Viewer assigned as a tester gets `201` (can create bugs)
  - Test that a non-member assigned as a tester gets `201` (tester assignment overrides
    lack of project membership)
  - Test that a Contributor creating a bug from a result they did not create (and are not
    assigned tester) gets a distinct `403` message
  - Test that System Admin bypasses all authorization checks
