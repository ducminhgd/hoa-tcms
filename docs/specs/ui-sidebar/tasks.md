# Tasks: UI — Sidebar Navigation

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit of work.

---

## Configuration & Types

- [ ] 1. Define navigation configuration — `requirements.md#US-1`, `design.md#Navigation Item Configuration`
  - Define `NavItem`, `NavSection` TypeScript interfaces
  - Define `NAV_SECTIONS` constant array with all three sections and their items
  - Each item includes `label`, `path`, and `matchPattern`
  - IAM section marked with `permission: "system_admin"`
  - File: `src/ui/components/Sidebar/navConfig.ts`

---

## Core Components

- [ ] 2. Implement `NavItem` sub-component — `requirements.md#US-1`, `design.md#Active Route Matching`
  - Renders a single sidebar navigation link using React Router `<NavLink>`
  - Applies active class when `isActive` returns true (via `NavLink` className callback)
  - Appends `?project_id={activeProjectId}` for project-scoped links (Settings and
    Test Management sections)
  - Disabled state with tooltip when no project is selected (for project-scoped links)
  - Keyboard accessible (standard `<a>` behaviour from NavLink)
  - File: `src/ui/components/Sidebar/NavItem.tsx`

- [ ] 3. Implement `NavSection` sub-component — `requirements.md#US-1` and `US-2`,
      `design.md#Sidebar Sections`
  - Renders a section heading (uppercase, muted) and a list of `<NavItem>` components
  - Accepts `section: NavSection` prop
  - When `permission: "system_admin"` is set and user is not System Admin,
    renders nothing (returns null)
  - File: `src/ui/components/Sidebar/NavSection.tsx`

- [ ] 4. Implement `UserMenu` sub-component — `requirements.md#US-3`,
      `design.md#User Menu`
  - Displays user avatar (or initials in coloured circle) and display name
  - "Profile" link: navigates to `/profile`
  - "Logout" button: calls `auth.logout()`, shows spinner during logout,
    redirects to `/login` on completion or failure
  - Handles logout API failure gracefully: clear local session state,
    redirect to login page, show brief toast "Logged out"
  - File: `src/ui/components/Sidebar/UserMenu.tsx`

- [ ] 5. Assemble `Sidebar` main component — `requirements.md#US-1` through `US-3`,
      `design.md#Architecture`
  - App brand/logo at top
  - Map `NAV_SECTIONS` to `<NavSection>` components (only render IAM if
    `isSystemAdmin`)
  - Divider line between last nav section and user menu
  - `<UserMenu>` at the bottom (flexbox: push to bottom with `margin-top: auto`)
  - Fixed width 240px, full height 100vh, overflow-y auto for long menu
  - Dark background theme with colour variables
  - File: `src/ui/components/Sidebar/Sidebar.tsx`

- [ ] 6. Implement `AuthenticatedLayout` — `design.md#Architecture`,
      `design.md#States & Edge Cases`
  - Layout shell: `<Sidebar>` on the left, `<Outlet />` for page content on the right
  - Content area: `margin-left: 240px`, full remaining width, min-height 100vh
  - If user is not authenticated, render `<Outlet />` without sidebar (plain layout
    for login page)
  - If auth state is loading, show a full-page spinner (or skeleton)
  - File: `src/ui/layouts/AuthenticatedLayout.tsx`

---

## Integration

- [ ] 7. Wire auth context to sidebar — `requirements.md#US-2` and `US-3`,
      `design.md#Integration Points`
  - Ensure `useAuth()` provides `isSystemAdmin` flag (derived from user roles)
  - Ensure `useAuth()` provides `logout()` function (calls logout API, clears state)
  - Pass auth context values to Sidebar, NavSection, and UserMenu
  - Handle role change during session: sidebar re-renders on navigation (no
    real-time push required)

- [ ] 8. Wire project context to sidebar — `design.md#Integration Points`
  - Read `activeProjectId` from `useProjectContext()`
  - Pass to `NavItem` for building project-scoped URLs
  - When `activeProjectId` is null, disable project-scoped links (Settings and
    Test Management) with tooltip "Select a project first"
  - When project switcher changes project, sidebar links update immediately
    (reactive via context)

---

## Tests & Stories

- [ ] 9. Write unit tests — `requirements.md#US-1` through `US-3`,
      `design.md#Active Route Matching`
  - Test active route matching: `/test-cases` matches Test Cases
  - Test active route matching: `/test-cases/42` matches Test Cases (sub-route)
  - Test active route matching: `/test-cases/new` matches Test Cases
  - Test active route matching: `/unrelated` matches nothing
  - Test IAM section hidden for non-admin user
  - Test IAM section visible for System Admin user
  - Test Settings/Test Management links disabled when no project selected
  - Test Settings/Test Management links enabled when project selected
  - Test logout calls `auth.logout()` and redirects to `/login`
  - Test logout graceful fallback when API fails

- [ ] 10. Create storybook stories — `design.md#Visual Specification`
  - Story: full sidebar with all sections (System Admin view)
  - Story: sidebar without IAM section (non-admin user)
  - Story: sidebar with no project selected (Settings/Test Management disabled)
  - Story: user menu with long display name (truncation)
  - Story: active link state (Test Cases highlighted)
  - File: `src/ui/components/Sidebar/Sidebar.stories.tsx`
