# Feature: Jira Issue Link

## Overview

Jira Issue Link allows users to associate a TCMS Test Case with one or more Jira issues.
Three link types are supported: RELATED (general association), BUG (discovered bug tracked
in Jira), and REQUIREMENT (the test case verifies a Jira requirement). Each link is validated
against the Jira API to ensure the Jira issue actually exists before the association is
persisted. A cached summary is stored at link time for display purposes.

---

## User Stories

### US-1: List Jira Issue Links

As a project member, I want to see which Jira issues are linked to a Test Case,
so that I understand the relationship between test artifacts and Jira work items.

**Acceptance Criteria (EARS)**

- WHEN a user with appropriate project membership sends `GET
  /api/v1/projects/{pid}/test-cases/{id}/jira-issues`, THE SYSTEM SHALL return a list
  of all linked Jira issues (jira_issue_key, link_type, summary, linked_by username,
  linked_at).
- IF the user is not a member of the project, THE SYSTEM SHALL return `403 Forbidden`.
- IF the test case does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL return an empty list when no links exist.
- THE SYSTEM SHALL support an optional `link_type` query parameter to filter by type
  (RELATED, BUG, REQUIREMENT).
- THE SYSTEM SHALL order results by `linked_at` descending.

### US-2: Link a Jira Issue

As a project Owner, Editor, or Contributor, I want to link a Test Case to a Jira issue
with a specific link type, so that I can track which test case relates to or verifies a
Jira issue, or record a bug discovered during testing.

**Acceptance Criteria (EARS)**

- WHEN a user with `project:update` permission and Owner, Editor, or Contributor project
  role sends `POST /api/v1/projects/{pid}/test-cases/{id}/jira-issues` with
  `jira_issue_key` and `link_type`, THE SYSTEM SHALL validate the Jira issue exists via
  the Jira API, insert a row into `JIRA_ISSUE_LINKS`, and return `201 Created`.
- IF the Jira issue key is not found via the Jira API, THE SYSTEM SHALL return
  `404 Not Found` with error code `JIRA_ISSUE_NOT_FOUND`.
- IF a link to the same Jira issue already exists for this test case (regardless of
  link_type), THE SYSTEM SHALL return `409 Conflict` with error code
  `DUPLICATE_JIRA_ISSUE_LINK`.
- IF the `link_type` is not one of `RELATED`, `BUG`, or `REQUIREMENT`, THE SYSTEM SHALL
  return `422 Unprocessable Entity`.
- IF the test case does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the user lacks permission or role, THE SYSTEM SHALL return `403 Forbidden`.
- IF no Jira configuration is found, THE SYSTEM SHALL return `412 Precondition Failed`
  with error code `JIRA_NOT_CONFIGURED`.
- IF the Jira API is unreachable or returns an error, THE SYSTEM SHALL return
  `502 Bad Gateway` with error code `JIRA_API_ERROR`.
- THE SYSTEM SHALL store the `summary` as returned by the Jira API alongside the issue
  key.
- THE SYSTEM SHALL accept `BUG` link type for Contributor role (expanded access compared
  to other Jira link features which require Owner/Editor).

### US-3: Unlink a Jira Issue

As a project Owner, Editor, or Contributor, I want to unlink a Jira issue from a
Test Case, so that I can remove incorrect or obsolete associations.

**Acceptance Criteria (EARS)**

- WHEN a user with `project:update` permission and Owner, Editor, or Contributor role
  sends `DELETE /api/v1/projects/{pid}/test-cases/{id}/jira-issues/{jiraIssueKey}`,
  THE SYSTEM SHALL remove the link row from `JIRA_ISSUE_LINKS` and return
  `204 No Content`.
- IF the link does not exist, THE SYSTEM SHALL return `404 Not Found`.
- IF the user lacks permission or role, THE SYSTEM SHALL return `403 Forbidden`.

---

## Security Considerations

### Jira API Credential Resolution

Same pattern as other Jira features: user-level PAT first, project-level fallback.

### Authorization

Contributor role is allowed for issue linking and unlinking (unlike project/release
linking which requires Owner/Editor). This is because linking bugs and requirements to
test cases is a frequent contributor activity. System Admin bypasses membership checks.

### Link Type Semantics

The `BUG` link type is semantically important: bugs created via `jira-create-bug` are
automatically linked with `BUG` type. Manual linking with `BUG` type allows recording
pre-existing bugs. The `REQUIREMENT` type establishes traceability from test case to
requirement.

---

## Out of Scope

- **Bulk linking** (deferred)
- **Link type migration** (changing link_type after creation — delete and re-create)
- **Issue detail caching refresh** (summary stored at link time)
- **Jira webhook ingestion** (auto-unlink when Jira issue is deleted)

---

## Dependencies

- **Jira Config** — Credential resolution.
- **Jira Project Link** — Resolves linked Jira projects for the test case's project.
- **Test Case CRUD** — `test_cases.id` FK reference; test case existence and soft-delete
  checks.
- **Project CRUD** — `projects.id` parent of test case; project scope for authorization.
- **Project Members** — Project role check (Contributor or higher for mutating ops).
- **IAM Auth** — Session-based authentication.
