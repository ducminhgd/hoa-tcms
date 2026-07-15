# Feature: Jira Release Link

## Overview

Jira Release Link allows users to associate a TCMS Test Plan with one or more Jira
releases (versions). This enables traceability between testing activities and Jira
release cycles. Each link is validated against the Jira API to ensure the Jira release
actually exists within a linked Jira project before the association is persisted.

---

## User Stories

### US-1: List Jira Release Links

As a project member, I want to see which Jira releases are linked to a Test Plan,
so that I understand which releases this test plan is targeting.

**Acceptance Criteria (EARS)**

- WHEN a user with appropriate project membership sends `GET
  /api/v1/test-plans/{id}/jira-releases`, THE SYSTEM SHALL return a list of all linked
  Jira releases (jira_release_id, jira_release_name, linked_by username, linked_at).
- IF the user is not a member of the project(s) that own the test plan, THE SYSTEM
  SHALL return `403 Forbidden`.
- IF the test plan does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL return an empty list when no links exist.
- THE SYSTEM SHALL order results by `linked_at` descending.

### US-2: Link a Jira Release

As a project Owner or Editor, I want to link a Test Plan to a Jira release,
so that I can track which test plan covers a specific release.

**Acceptance Criteria (EARS)**

- WHEN a user with `project:update` permission and Owner or Editor role sends `POST
  /api/v1/test-plans/{id}/jira-releases` with `jira_release_id`, THE SYSTEM SHALL
  validate the Jira release exists via the Jira API, resolve the Jira project from the
  test plan's linked Jira projects, insert a row into `JIRA_RELEASE_LINKS`, and return
  `201 Created`.
- IF the test plan's project has no linked Jira projects, THE SYSTEM SHALL return
  `412 Precondition Failed` with error code `JIRA_PROJECT_NOT_LINKED`.
- IF the Jira release ID is not found via the Jira API, THE SYSTEM SHALL return
  `404 Not Found` with error code `JIRA_RELEASE_NOT_FOUND`.
- IF a link to the same Jira release already exists for this test plan, THE SYSTEM
  SHALL return `409 Conflict` with error code `DUPLICATE_JIRA_RELEASE_LINK`.
- IF the test plan does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the user lacks permission or role, THE SYSTEM SHALL return `403 Forbidden`.
- IF no Jira configuration is found, THE SYSTEM SHALL return `412 Precondition Failed`
  with error code `JIRA_NOT_CONFIGURED`.
- IF the Jira API is unreachable or returns an error, THE SYSTEM SHALL return
  `502 Bad Gateway` with error code `JIRA_API_ERROR`.
- THE SYSTEM SHALL store the `jira_release_name` as returned by the Jira API alongside
  the release ID.

### US-3: Unlink a Jira Release

As a project Owner or Editor, I want to unlink a Jira release from a Test Plan,
so that I can remove incorrect or obsolete associations.

**Acceptance Criteria (EARS)**

- WHEN a user with `project:update` permission and Owner or Editor role sends `DELETE
  /api/v1/test-plans/{id}/jira-releases/{jiraReleaseId}`, THE SYSTEM SHALL remove the
  link row from `JIRA_RELEASE_LINKS` and return `204 No Content`.
- IF the link does not exist, THE SYSTEM SHALL return `404 Not Found`.
- IF the user lacks permission or role, THE SYSTEM SHALL return `403 Forbidden`.

---

## Security Considerations

### Jira API Credential Resolution

Same as `jira-project-link`: uses the authenticated user's personal Jira PAT first,
falls back to project-level config. If neither exists, the request is rejected.

### Authorization

Linking and unlinking require `project:update` permission AND Owner or Editor role on
the project(s) that own the test plan. Listing requires project membership.
System Admin bypasses membership checks.

### Cross-Project Validation

A Test Plan may span multiple projects. The Jira release validation must check all
linked Jira projects of the test plan's projects. The release must exist in at least
one linked Jira project.

---

## Out of Scope

- **Bulk linking** (deferred to future iteration)
- **Jira release detail caching** (name stored at link time; may go stale)
- **Release lifecycle sync** (test plan status updates based on Jira release state)

---

## Dependencies

- **Jira Config** — Credential resolution for Jira API calls.
- **Jira Project Link** — Linked Jira projects provide the target project for release
  validation.
- **Test Plan CRUD** — `test_plans.id` FK reference; test plan existence and soft-delete
  checks.
- **Project Members** — Project role check.
- **IAM Auth** — Session-based authentication.
