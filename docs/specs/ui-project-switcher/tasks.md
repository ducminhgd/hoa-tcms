# Tasks: UI — Project Switcher

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit of work.

---

## Context Layer

- [ ] 1. Implement `ProjectContext` and `ProjectProvider` — `requirements.md#US-1`,
      `design.md#Component API`
  - Create React Context with `ProjectContextValue` type
  - `ProjectProvider` manages state: `activeProjectId`, `activeProjectName`,
    `projects`, `projectsLoading`, `projectsError`
  - On mount: read `sessionStorage.getItem("active-project-id")`, parse as integer,
    set as initial `activeProjectId`
  - On mount: fetch project list via `GET /api/v1/projects?limit=100`
  - After project list load: validate `activeProjectId` exists in the fetched list;
    if not, clear it (user membership revoked in another tab)
  - Provide `setActiveProject(project)` and `clearActiveProject()` functions
  - `setActiveProject`: update context state + `sessionStorage.setItem("active-project-id", project.id)`
  - `clearActiveProject`: set state to null + `sessionStorage.removeItem("active-project-id")`
  - Files: `src/application/context/ProjectContext.tsx`

- [ ] 2. Implement `useProjectContext` hook — `design.md#Component API`
  - Consume `ProjectContext` via `useContext`
  - Throw descriptive error if used outside `ProjectProvider` (not silently
    return undefined)
  - File: `src/shared/hooks/useProjectContext.ts`

---

## UI Components

- [ ] 3. Implement `ProjectDropdown` sub-component — `requirements.md#US-1`,
      `design.md#Visual Specification`
  - Renders dropdown panel with list of projects
  - Active project highlighted with checkmark/dot indicator
  - Each item clickable: calls `setActiveProject(project)`
  - Loading state: "Loading projects..." with spinner
  - Error state: "Failed to load projects" with "Retry" button
  - Empty state (zero projects): "You are not a member of any project"
  - Handles long project names: text truncation with ellipsis
  - Keyboard accessible: up/down arrow navigation, Enter to select, Escape to close
  - File: `src/ui/components/ProjectSwitcher/ProjectDropdown.tsx`

- [ ] 4. Implement `ProjectSwitcher` main component — `requirements.md#US-1`,
      `design.md#Architecture`
  - Trigger button showing active project name (or "Select Project" if none)
  - Chevron down/up icon (rotates based on open state)
  - Toggles dropdown open/close on click
  - Closes dropdown on click outside (use click-outside hook or event listener)
  - Renders `<ProjectDropdown>` when open
  - File: `src/ui/components/ProjectSwitcher/ProjectSwitcher.tsx`

- [ ] 5. Place `ProjectSwitcher` in the header bar — `design.md#Architecture`
  - Add to `AuthenticatedLayout` header bar, left side (next to sidebar, before
    user menu)
  - Wrap `AuthenticatedLayout` (or the entire app) in `<ProjectProvider>`
  - Ensure header bar provides appropriate height and alignment for the switcher
  - File: update `src/ui/layouts/AuthenticatedLayout.tsx`

---

## Integration & Wiring

- [ ] 6. Integrate project context with sidebar — `requirements.md#US-2`,
      `design.md#Integration Points`
  - Sidebar `NavItem` reads `activeProjectId` from `useProjectContext()`
  - Project-scoped links append `?project_id={activeProjectId}` when a project
    is selected
  - Disable project-scoped links when `activeProjectId` is null, with tooltip
    "Select a project first"
  - (Coordination with `ui-sidebar` tasks — may be handled in that spec)

- [ ] 7. Integrate project context with entity list views — `requirements.md#US-2`,
      `design.md#Integration Points`
  - Create a pattern/hook for project-scoped data fetching that automatically
    re-fetches when `activeProjectId` changes
  - Example: `useProjectScopedQuery(key, fetchFn, activeProjectId)` that
    triggers refetch on project change
  - On pages without a project selected: show prompt "Select a project to view {entity}"
  - Handle `403` errors: detect membership revocation, clear active project,
    show toast notification

---

## Tests & Verification

- [ ] 8. Write unit tests — `requirements.md#US-1` and `US-2`,
      `design.md#State Transitions`
  - Test `ProjectProvider` restores project ID from `sessionStorage` on mount
  - Test `ProjectProvider` clears invalid project ID (not in fetched list)
  - Test `setActiveProject` writes to `sessionStorage`
  - Test `clearActiveProject` removes from `sessionStorage`
  - Test project list loading state in dropdown
  - Test project list error state with retry in dropdown
  - Test zero projects empty state in dropdown
  - Test clicking project in dropdown calls `setActiveProject`
  - Test dropdown closes on outside click
  - Test dropdown closes on Escape key
  - Test project context change triggers re-fetch in consuming component
  - Test `403` error clears active project and shows toast
