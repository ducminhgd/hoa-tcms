# Design: UI — Project Switcher

## Architecture

The Project Switcher uses React Context to broadcast the active project to all
descendant components. A `<ProjectSwitcher>` dropdown component in the header reads
the project list and manages the selection. All project-scoped views consume the
context to include `project_id` in their API calls.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  UI Layer                                                                     │
│                                                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  AuthenticatedLayout                                                   │   │
│  │                                                                         │   │
│  │  ┌──────────────────────────────────────────────────────────────────┐  │   │
│  │  │  Header Bar                                                        │  │   │
│  │  │  ┌──────────────────────────┐         ┌────────────────────────┐   │  │   │
│  │  │  │  <ProjectSwitcher>        │         │  User avatar / menu    │   │  │   │
│  │  │  │                          │         │                        │   │  │   │
│  │  │  │  [My Project ▼]          │         │                        │   │  │   │
│  │  │  └──────────────────────────┘         └────────────────────────┘   │  │   │
│  │  └──────────────────────────────────────────────────────────────────┘  │   │
│  │                                                                         │   │
│  │  ┌──────────────┬────────────────────────────────────────────────────┐  │   │
│  │  │  <Sidebar>    │  Page Content                                      │  │   │
│  │  │               │                                                    │  │   │
│  │  │               │  useProjectContext() → { activeProjectId, ... }    │  │   │
│  │  │               │                                                    │  │   │
│  │  │               │  // In page component:                             │  │   │
│  │  │               │  const { activeProjectId } = useProjectContext();  │  │   │
│  │  │               │  const { data } = useListTestCases(                │  │   │
│  │  │               │    { project_id: activeProjectId }                 │  │   │
│  │  │               │  );                                                │  │   │
│  │  └──────────────┴────────────────────────────────────────────────────┘  │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  React Context: ProjectContext                                          │   │
│  │  - activeProjectId: number | null                                       │   │
│  │  - activeProjectName: string | null                                     │   │
│  │  - setActiveProject: (project: Project) => void                        │   │
│  │  - clearActiveProject: () => void                                       │   │
│  │                                                                         │   │
│  │  Provider wraps AuthenticatedLayout.                                    │   │
│  │  On mount: reads sessionStorage["active-project-id"] as initial value. │   │
│  │  On setActiveProject: writes to sessionStorage.                        │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. `App` renders `<ProjectProvider>` wrapping `<AuthenticatedLayout>`.
2. On mount, `ProjectProvider` reads `sessionStorage.getItem("active-project-id")`
   to restore the previous session's project.
3. `<ProjectSwitcher>` (in the header) fetches the user's project list on mount.
4. User selects a project: `setActiveProject(project)` updates context state and
   writes to `sessionStorage`.
5. Any descendant component reading `useProjectContext()` re-renders with the new
   project ID and can re-fetch its data.

---

## Component API

### `ProjectContext` Type

```typescript
interface Project {
  id: number;
  name: string;
  description: string;
  status: string;
  member_count: number;
  created_by: number;
  created_at: string;
  updated_at: string;
}

interface ProjectContextValue {
  activeProjectId: number | null;
  activeProjectName: string | null;
  projects: Project[];
  projectsLoading: boolean;
  projectsError: Error | null;
  setActiveProject: (project: Project) => void;
  clearActiveProject: () => void;
}
```

### `<ProjectProvider>` Props

```typescript
interface ProjectProviderProps {
  children: ReactNode;
}
```

### `<ProjectSwitcher>` Props

```typescript
interface ProjectSwitcherProps {
  // None required — reads from ProjectContext
}
```

---

## Session Storage Persistence

| Property | Value |
|----------|-------|
| Storage key | `active-project-id` |
| Storage type | `sessionStorage` (tab-scoped) |
| Value | `string` — the project ID (e.g., `"42"`) |
| Set on | User selects a project in the dropdown |
| Cleared on | User membership revoked, or ID validation fails |
| Why `sessionStorage` | Tab-scoped: switching projects in one tab does not affect other tabs |

**Rationale for `sessionStorage` over `localStorage`:**
- User may open multiple tabs with different projects in each.
- `sessionStorage` is tab-scoped, so each tab maintains its own project context.
- Closing the tab clears the preference (acceptable UX for a session-scoped choice).

---

## Project List Fetching

On mount, `<ProjectSwitcher>` (or `<ProjectProvider>`) fetches the user's projects:

```
GET /api/v1/projects?limit=100
```

- The maximum limit of 100 is used to fetch all accessible projects in a single
  request (most users have under 100 projects).
- If the user has more than 100 projects, pagination is not handled in the switcher
  (edge case; add a "Show more..." option in Phase 2 if needed).
- The response is cached in React state for the session duration.

---

## State Transitions

| Event | Context State Change | Side Effects |
|-------|---------------------|--------------|
| **App loads** | Read `sessionStorage["active-project-id"]` | Validate against fetched project list |
| **Project list loaded** | `projects` updated, `projectsLoading: false` | If `activeProjectId` is invalid (not in list), clear it |
| **User selects project** | `activeProjectId` and `activeProjectName` set | Write to `sessionStorage`; all project-scoped views re-render |
| **User cleared selection** | `activeProjectId` and `activeProjectName` set to `null` | Remove from `sessionStorage` |
| **API 403 on project-scoped endpoint** | Clear active project | Toast notification; remove from `sessionStorage` |
| **Project list fetch fails** | `projectsError` set | Show error in dropdown |
| **User logs out** | Context reset to initial state | `sessionStorage` cleared by logout flow |

---

## Visual Specification

### Project Switcher Dropdown

```
┌──────────────────────┐
│  [My Project      ▼] │  ← Trigger button
└──────────────────────┘
         │
         ▼
┌──────────────────────────┐
│  Select Project          │  ← Header (optional)
│  ─────────────────────── │
│  ● My Project  (Active)  │  ← Checkmark or dot for active
│    Project Alpha          │
│    Project Beta           │
│    Project Gamma          │
│  ─────────────────────── │
│  + Create New Project    │  ← Optional: link to /projects/new
└──────────────────────────┘
```

| Element | Style |
|---------|-------|
| Trigger button | Height 36px, border 1px solid, border-radius 6px, padding 8px 12px |
| Trigger label | Project name (or "Select Project"), chevron down icon |
| Dropdown panel | Width 240px, max-height 320px (scrollable if needed), box-shadow |
| Active project | Bold text, with checkmark or dot indicator |
| List item (default) | 14px, padding 8px 12px, hover background |
| "No projects" | Muted text, "You are not a member of any project" |
| Error state | "Failed to load projects" with retry button |
| Loading state | Placeholder text "Loading projects..." with spinner |

---

## Integration Points

### With Sidebar (`ui-sidebar`)

The sidebar consumes `useProjectContext()` to build project-scoped URLs and enable/disable links:

```typescript
// In Sidebar/NavItem
const { activeProjectId } = useProjectContext();

const path = activeProjectId
  ? `${item.path}?project_id=${activeProjectId}`
  : item.path;

const disabled = isProjectScoped(item) && !activeProjectId;
```

### With Entity List Views

Each project-scoped list page consumes the project context:

```typescript
// In TestCaseListPage
const { activeProjectId } = useProjectContext();

const { data, loading, error } = useListTestCases({
  project_id: activeProjectId,
  page: currentPage,
  limit: pageSize,
});

// Re-fetch when project changes
useEffect(() => {
  fetchData();
}, [activeProjectId]);
```

### With API Client

The API client or application hooks should accept `project_id` as a parameter and
include it in the query string:

```typescript
function useListTestCases(params: ListTestCasesParams) {
  const query = new URLSearchParams();
  if (params.project_id) query.set("project_id", String(params.project_id));
  query.set("page", String(params.page));
  query.set("limit", String(params.limit));
  return useQuery(`/test-cases?${query}`);
}
```

### With Permission Denied Handling (403)

When a project-scoped API call returns `403`, the parent page should check if the
cause is revoked project membership and clear the active project:

```typescript
// In API client interceptor or error handler
if (error.status === 403 && error.code === "FORBIDDEN") {
  // Could be membership revoked; clear project context
  const { clearActiveProject } = useProjectContext();
  clearActiveProject();
  toast.error("You no longer have access to the selected project.");
}
```

---

## Files

| File | Role |
|------|------|
| `src/ui/components/ProjectSwitcher/ProjectSwitcher.tsx` | Dropdown component in header |
| `src/ui/components/ProjectSwitcher/ProjectDropdown.tsx` | Dropdown panel with project list |
| `src/application/context/ProjectContext.tsx` | React context + provider |
| `src/shared/hooks/useProjectContext.ts` | Hook to consume project context |
| `src/infrastructure/storage/sessionStorageAdapter.ts` | Session storage adapter (optional abstraction) |
