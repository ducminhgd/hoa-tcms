# Feature: Jira Project Link

## Overview

Jira Project Link allows users to associate a TCMS project with one or more Jira projects.
This link enables cross-referencing between TCMS and Jira and serves as the foundation for
other Jira integrations (jira-release-link, jira-issue-link, jira-create-bug). Each link
is validated against the Jira API to ensure the Jira project actually exists before the
association is persisted.

---

## User Stories

### US-1: List Jira Project Links

As a project member, I want to see which Jira projects are linked to my TCMS project,
so that I understand which external systems this project is integrated with.

**Acceptance Criteria (EARS)**

- WHEN a user with `project:read` permission sends `GET
  /api/v1/projects/{id}/jira-links`, THE SYSTEM SHALL return a list of all linked Jira
  projects (jira_project_key, jira_project_name, linked_by username, linked_at).
- IF the user lacks `project:read` permission or is not a project member, THE SYSTEM
  SHALL return `403 Forbidden`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL return an empty list (not an error) when no links exist.
- THE SYSTEM SHALL order results by `linked_at` descending (newest first).

### US-2: Link a Jira Project

As a project Owner or Editor, I want to link my TCMS project to a Jira project,
so that I can associate TCMS test artifacts with that Jira project's issues and releases.

**Acceptance Criteria (EARS)**

- WHEN a user with `project:update` permission and Owner or Editor project role sends
  `POST /api/v1/projects/{id}/jira-links` with `jira_project_key`, THE SYSTEM SHALL
  validate the Jira project exists via the Jira API, insert a row into
  `JIRA_PROJECT_LINKS`, and return `201 Created`.
- IF the Jira project key is not found via the Jira API, THE SYSTEM SHALL return
  `404 Not Found` with error code `JIRA_PROJECT_NOT_FOUND`.
- IF a link to the same Jira project already exists for this TCMS project, THE SYSTEM
  SHALL return `409 Conflict` with error code `DUPLICATE_JIRA_LINK`.
- IF the project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the user lacks `project:update` permission or is not an Owner or Editor of the
  project, THE SYSTEM SHALL return `403 Forbidden`.
- IF no Jira configuration is found (neither user-level nor project-level), THE SYSTEM
  SHALL return `412 Precondition Failed` with error code `JIRA_NOT_CONFIGURED`.
- IF the Jira API is unreachable or returns an error, THE SYSTEM SHALL return
  `502 Bad Gateway` with error code `JIRA_API_ERROR`.
- THE SYSTEM SHALL store the `jira_project_name` as returned by the Jira API alongside
  the key.

### US-3: Unlink a Jira Project

As a project Owner or Editor, I want to unlink a Jira project from my TCMS project,
so that I can remove stale or incorrect associations.

**Acceptance Criteria (EARS)**

- WHEN a user with `project:update` permission and Owner or Editor project role sends
  `DELETE /api/v1/projects/{id}/jira-links/{jiraProjectKey}`, THE SYSTEM SHALL remove
  the link row from `JIRA_PROJECT_LINKS` and return `204 No Content`.
- IF the link does not exist, THE SYSTEM SHALL return `404 Not Found`.
- IF the user lacks permission or role, THE SYSTEM SHALL return `403 Forbidden`.
- THE SYSTEM SHALL NOT cascade-delete dependent links (jira-release-link,
  jira-issue-link) — those remain intact, referencing the Jira project key directly.
  If the TCMS project is re-linked to the same Jira project later, the dependent links
  become valid again.

---

## Security Considerations

### Jira API Credential Resolution

The Jira API call must use the authenticated user's personal Jira PAT if configured
(jira-config, USER scope). If the user has no personal config, the project-level config
(PROJECT scope) is used as a fallback. If neither exists, the request is rejected with
`412 Precondition Failed`.

### Authorization

Linking and unlinking require `project:update` permission AND the Owner or Editor
project role. Listing requires `project:read` permission AND project membership.
The System Admin role bypasses project membership checks.

### Input Validation

The `jira_project_key` must be validated against the Jira project key format (uppercase
letters, digits, and hyphens; max 255 characters). Reject obviously malformed keys at
the HTTP boundary before calling the Jira API.

---

## Out of Scope

- **Bulk linking** (link multiple Jira projects in a single request — defer to future
  iteration)
- **Bi-directional sync** (Jira webhooks updating TCMS when Jira projects are deleted)
- **Link metadata** (custom labels, notes on links — out of Phase 2 scope)
- **Jira project detail caching** (the name is stored at link time; if the Jira project
  is renamed, the stored name may go stale — refreshing is out of scope)

---

## Dependencies

- **Jira Config** — `jira-config` must be implemented first. Credential resolution
  depends on `JiraConfigService::resolve_pat`.
- **Project CRUD** — `projects.id` FK reference; project existence and soft-delete checks.
- **Project Members** — Project role check (Owner or Editor for mutating operations).
- **IAM Auth** — Session-based authentication.
