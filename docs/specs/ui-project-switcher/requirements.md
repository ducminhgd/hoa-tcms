# Feature: UI — Project Switcher

## Overview

The Project Switcher defines a dropdown/selector component in the application header that
allows the user to switch the active project context. Changing the active project scopes
all Settings (metadata) and Test Management views to the selected project. The active
project is persisted in `sessionStorage` so it survives page refreshes within the same
browser tab.

This spec defines the frontend component, context management, and persistence only.
It does not define the project list API endpoint (that is in `project-crud`).

---

## User Stories

### US-1: Switch Active Project

As a user who belongs to multiple projects, I want to select which project is currently
active via a dropdown in the header, so that all Settings and Test Management views
reflect the correct project scope.

**Acceptance Criteria (EARS)**

- WHEN the user is authenticated, THE SYSTEM SHALL render a project selector dropdown
  in the application header (next to the sidebar, in the top bar).
- THE DROPDOWN SHALL display the name of the currently active project as its label, or
  "Select Project" if no project is active.
- WHEN the user clicks the dropdown, THE SYSTEM SHALL list all projects the user is a
  member of, fetched from the project list API (`GET /api/v1/projects?limit=100`).
- WHEN the user selects a different project from the dropdown, THE SYSTEM SHALL:
  1. Set the selected project as the active project in React context.
  2. Persist the selected project ID to `sessionStorage` under the key `active-project-id`.
  3. Update the dropdown label to the new project name.
  4. Trigger re-fetching of data on the current page (if project-scoped) to reflect
     the new project context.
- IF the user belongs to zero projects, THE SYSTEM SHALL display "No projects" in the
  dropdown and disable it.
- IF the project list API call fails, THE SYSTEM SHALL show an error message in the
  dropdown ("Failed to load projects") with a retry option.
- IF the persisted project ID in `sessionStorage` does not match any project the user
  belongs to (e.g., membership was revoked in another tab), THE SYSTEM SHALL clear the
  persisted ID and set no active project, then display "Select Project".

### US-2: View Responds to Project Context Change

As a user working with project-scoped data (metadata, test cases, plans, runs, executions),
I want the list view to automatically reload when I switch projects, so that I see the
data for the newly selected project without manual refresh.

**Acceptance Criteria (EARS)**

- WHEN the active project changes via the project switcher, THE SYSTEM SHALL cause all
  currently rendered project-scoped list views to re-fetch their data with the new
  project ID.
- WHEN a user navigates to a Settings or Test Management page without an active project
  selected, THE SYSTEM SHALL display a prompt: "Select a project to view {entity}" with
  a focused project switcher dropdown.
- WHEN the active project is `null` (no project selected), THE SYSTEM SHALL disable
  sidebar links for Settings and Test Management sections (per `ui-sidebar` spec).
- IF the user's membership in the active project is revoked (detected via a `403` on
  a subsequent API call), THE SYSTEM SHALL clear the active project, show a toast
  notification "You no longer have access to project X", and re-render without a
  project context.

---

## Out of Scope

- **Project CRUD from the switcher** — creating, editing, or deleting projects from the
  switcher dropdown is not supported in Phase 1. Users navigate to the Projects list page.
- **Cross-tab synchronization** — project changes are scoped to the current browser tab
  via `sessionStorage`. Switching projects in one tab does not affect other tabs.
- **Multi-project views** — only one project is active at a time. Views that aggregate
  data across projects are out of scope for Phase 1.
- **Project context in URL** — the active project is managed via React context, not URL
  path segments. URL-based project scoping could be considered in Phase 2.

---

## Dependencies

- **project-crud** — The `GET /api/v1/projects` endpoint provides the list of projects
  for the current user.
- **ui-sidebar** — Sidebar navigation links for Settings and Test Management are
  disabled when no project is active. They append `?project_id=` to their URLs when
  a project is selected.
- **All entity list pages** — Each project-scoped list view reads `activeProjectId`
  from context and includes it in its API request.
