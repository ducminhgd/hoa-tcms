# Feature: Jira Create Bug

## Overview

Jira Create Bug allows testers to create a Jira bug ticket directly from a failed Test Case
Result. The Jira issue is auto-populated with the test case summary, description, steps, and
result logs. The created bug is automatically linked back to the source test case via the
existing Jira Issue Link infrastructure with `BUG` link type. This feature eliminates manual
copy-paste between TCMS and Jira when a test failure is discovered during execution.

---

## User Stories

### US-1: Create Jira Bug from a Failed Test Result

As a tester who has marked a Test Case Result as FAIL, I want to create a corresponding Jira
bug ticket with one click, so that I can report the discovered defect without manually
re-entering test context into Jira.

**Acceptance Criteria (EARS)**

- WHEN a user with permission (Contributor who owns the result, Owner/Editor of the project,
  or assigned tester on the execution) sends `POST
  /api/v1/test-results/{resultId}/create-jira-bug`, THE SYSTEM SHALL resolve Jira
  credentials, resolve the linked Jira project, call the Jira REST API to create a Bug issue
  with auto-populated fields, insert a row into `JIRA_ISSUE_LINKS` linking the test case to
  the new Jira issue with `link_type = 'BUG'`, and return `201 Created` with the issue key
  and URL.
- THE SYSTEM SHALL auto-populate the Jira bug fields as follows:
  - `summary`: `"[FAIL] {test_case_result.summary}"`
  - `description`: A formatted block containing the test case description, steps (from the
    test case template), and the result logs.
  - `issuetype`: `"Bug"` (resolved via Jira API metadata if the project uses a custom Bug
    issue type ID; otherwise the default name-based lookup).
- IF the Test Case Result status is not `FAIL`, THE SYSTEM SHALL return `422 Unprocessable
  Entity` with error code `RESULT_NOT_FAILED` (bugs can only be created from failed results).
- IF the result is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the `jira_project_key` does not match any linked Jira project for the test case's TCMS
  project, THE SYSTEM SHALL return `404 Not Found` with code `JIRA_PROJECT_NOT_LINKED`.
- IF no Jira configuration is found (neither user-level nor project-level), THE SYSTEM SHALL
  return `412 Precondition Failed` with error code `JIRA_NOT_CONFIGURED`.
- IF the Jira API returns an error or is unreachable, THE SYSTEM SHALL return `502 Bad
  Gateway` with error code `JIRA_API_ERROR`.
- IF the test case is already linked to a Jira issue with `BUG` type (idempotency guard),
  THE SYSTEM SHALL return `409 Conflict` with code `BUG_ALREADY_CREATED` and include the
  existing issue key in the response details.
- THE SYSTEM SHALL record the creating user as `linked_by` in the `JIRA_ISSUE_LINKS` row.

### US-2: Auto-Resolve Jira Project When Only One Is Linked

As a tester working on a project with a single Jira integration, I want the system to
auto-detect the target Jira project so that I do not need to specify it in every request.

**Acceptance Criteria (EARS)**

- WHEN the TCMS project has exactly one linked Jira project (via `jira-project-link`), THE
  SYSTEM SHALL use that Jira project automatically without requiring it in the request body.
- IF the TCMS project has zero linked Jira projects, THE SYSTEM SHALL return `404 Not Found`
  with error code `JIRA_PROJECT_NOT_LINKED` and a message indicating no Jira projects are
  linked to this TCMS project.
- IF the TCMS project has multiple linked Jira projects, THE SYSTEM SHALL return `422
  Unprocessable Entity` with error code `MULTIPLE_JIRA_PROJECTS` and include the list of
  available `jira_project_key` values in the error details, so the client can present a
  choice to the user.

### US-3: Idempotent Bug Creation

As a tester, I want the bug creation to be safe to retry, so that a network interruption or
accidental double-click does not create duplicate Jira issues for the same failure.

**Acceptance Criteria (EARS)**

- WHEN a user sends a duplicate `POST .../create-jira-bug` for a test case that already has
  a `BUG`-type `JIRA_ISSUE_LINK`, THE SYSTEM SHALL detect the existing link, return `409
  Conflict` with error code `BUG_ALREADY_CREATED`, and include the existing issue key and
  URL in the response body so the client can redirect the user to the existing bug.
- THE SYSTEM SHALL check for existing BUG-type links on the test case (not the test result),
  because multiple results may exist for the same test case across different executions, but
  only one active bug ticket should track the defect per test case.
- THE SYSTEM SHALL distinguish between "a BUG link exists for this test case" (return 409
  with existing info) and "the Jira API returned success but the link insert failed" (retry
  is safe; the link insert is the idempotency guard, not the Jira API call).

---

## Security Considerations

### Authorization

The authorization model mirrors `test-case-result-update`:

1. System Admin: bypasses all checks, allowed.
2. Project Owner or Editor: allowed to create bugs from any result in the project.
3. Project Contributor: allowed only if they own the result (`created_by` matches current
   user) or are an assigned tester on the execution.
4. Assigned tester on the execution: allowed regardless of project role (including Viewers).
5. Otherwise: `403 Forbidden`.

The `403 Forbidden` response must use a generic message for missing system permission or
project role. The Contributor ownership restriction returns a distinct message ("You can
only create Jira bugs from your own Test Case Results").

### PAT Handling

The Jira PAT must never appear in logs, error messages, or API responses. The PAT resolution
order (user-level first, project-level fallback) follows the pattern established in
`jira-config`. If the PAT is invalid or expired (Jira returns 401), the error is surfaced as
`502 JIRA_API_ERROR` with a generic message; the raw Jira response is not exposed.

### Rate Limiting

The endpoint allows 10 req/min per user (state-changing and calls an external API with its
own rate limits).

### Input Validation

The request body is validated for strict schema compliance (unrecognised fields rejected).
The Jira project key, if resolved from links, must exist in the `JIRA_PROJECT_LINKS` table
for the test case's TCMS project.

---

## Out of Scope

- **Creating bugs from non-FAIL statuses** (e.g., WARNING). Only FAIL results can trigger
  bug creation for MVP.
- **Customizing Jira fields before creation** (assignee, priority, labels, components). The
  bug is created with auto-populated fields only.
- **Attaching screenshots or test case files to the Jira bug** (deferred to a future phase).
- **Bulk bug creation** from multiple failed results.
- **Linking the Jira bug to the Test Execution or Test Run** (only the test case link is
  created).
- **Updating the Jira bug when the result is re-tested** (e.g., adding a comment when the
  result transitions from FAIL to PASS).

---

## Dependencies

- **Jira Config** (`jira-config`) -- Jira credential resolution (user-level PAT, project-level
  fallback).
- **Jira Project Link** (`jira-project-link`) -- Resolves linked Jira projects for the test
  case's TCMS project. Also provides the shared `JiraApiClient` interface with
  `create_bug` method.
- **Jira Issue Link** (`jira-issue-link`) -- Reuses the `JIRA_ISSUE_LINKS` table and
  `JiraIssueLinkRepository` to persist the BUG link. The `BUG` link type already exists.
- **Test Case Result Update** (`test-case-result-update`) -- The result must exist and be in
  FAIL status. Authorization mirrors this feature exactly. The result's `test_case_id`,
  `summary`, `description`, and `logs` fields are used to populate the Jira bug.
- **Test Execution Import** (`test-execution-import`) -- The test case result is created
  during import; this feature operates on already-imported results.
- **Project Members** (`project-members`) -- Project role checks for authorization.
- **IAM Auth** (`iam-auth`) -- Session-based authentication.
