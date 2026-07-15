# Design: UI — Sidebar Navigation

## Architecture

The Sidebar is a **frontend-only** layout component rendered within the authenticated
shell layout. It reads auth state from the application context and routes from the
router.

```
┌─────────────────────────────────────────────────────────────────────────┐
│  UI Layer (Presentation)                                                  │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │  AuthenticatedLayout                                                 │ │
│  │                                                                       │ │
│  │  ┌──────────────┬─────────────────────────────────────────────────┐  │ │
│  │  │  <Sidebar>    │  <Outlet />  (page content via React Router)    │  │ │
│  │  │               │                                                  │  │ │
│  │  │  ┌──────────┐ │  ┌──────────────────────────────────────────┐  │  │ │
│  │  │  │ App Logo  │ │  │                                          │  │  │ │
│  │  │  │ (+ Title) │ │  │  Current page content                    │  │  │ │
│  │  │  ├──────────┤ │  │                                          │  │  │ │
│  │  │  │ IAM       │ │  │                                          │  │  │ │
│  │  │  │  Users    │ │  │                                          │  │  │ │
│  │  │  │  Groups   │ │  │                                          │  │  │ │
│  │  │  │  Roles    │ │  │                                          │  │  │ │
│  │  │  ├──────────┤ │  │                                          │  │  │ │
│  │  │  │ Settings  │ │  │                                          │  │  │ │
│  │  │  │  Categories│ │  │                                          │  │  │ │
│  │  │  │  Templates │ │  │                                          │  │  │ │
│  │  │  │  Priorities│ │  │                                          │  │  │ │
│  │  │  │  Plan Types│ │  │                                          │  │  │ │
│  │  │  ├──────────┤ │  │                                          │  │  │ │
│  │  │  │ Test Mgmt │ │  │                                          │  │  │ │
│  │  │  │  Test Cases│ │  │                                          │  │  │ │
│  │  │  │  Test Plans│ │  │                                          │  │  │ │
│  │  │  │  Test Runs │ │  │                                          │  │  │ │
│  │  │  │  Test Exec │ │  │                                          │  │  │ │
│  │  │  ├──────────┤ │  │                                          │  │  │ │
│  │  │  │ Divider   │ │  │                                          │  │  │ │
│  │  │  │ User Menu │ │  │                                          │  │  │ │
│  │  │  │  Profile  │ │  │                                          │  │  │ │
│  │  │  │  Logout   │ │  │                                          │  │  │ │
│  │  │  └──────────┘ │  │                                          │  │  │ │
│  │  └──────────────┴─────────────────────────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

**Layout:**
- Sidebar: fixed width 240px, full viewport height, fixed position, left side.
- Content area: `margin-left: 240px`, takes remaining width.
- Sidebar background: dark theme (or brand primary colour).

---

## Component API

### `<Sidebar>` Props

The Sidebar component reads from React Context and the router; it accepts minimal props.

```typescript
interface SidebarProps {
  // None required — reads from context
}

// Internal — reads from:
// - useAuth(): { user, isSystemAdmin, logout }
// - useLocation(): current route path
// - useProjectContext(): { activeProjectId }
```

### Navigation Item Configuration

Menu items are defined as a static configuration constant:

```typescript
interface NavSection {
  heading: string;                          // Section heading label
  permission?: "system_admin";              // If set, entire section hidden unless role matches
  items: NavItem[];
}

interface NavItem {
  label: string;                            // Display text
  path: string;                             // Route path
  matchPattern: string | RegExp;            // Pattern to match for active state
}

const NAV_SECTIONS: NavSection[] = [
  {
    heading: "IAM",
    permission: "system_admin",
    items: [
      { label: "Users",    path: "/users",    matchPattern: "/users" },
      { label: "Groups",   path: "/groups",   matchPattern: "/groups" },
      { label: "Roles",    path: "/roles",    matchPattern: "/roles" },
    ],
  },
  {
    heading: "Settings",
    items: [
      { label: "Categories",  path: "/categories",  matchPattern: "/categories" },
      { label: "Templates",   path: "/templates",   matchPattern: "/templates" },
      { label: "Priorities",  path: "/priorities",  matchPattern: "/priorities" },
      { label: "Plan Types",  path: "/plan-types",  matchPattern: "/plan-types" },
    ],
  },
  {
    heading: "Test Management",
    items: [
      { label: "Test Cases",      path: "/test-cases",       matchPattern: "/test-cases" },
      { label: "Test Plans",      path: "/test-plans",       matchPattern: "/test-plans" },
      { label: "Test Runs",       path: "/test-runs",        matchPattern: "/test-runs" },
      { label: "Test Executions", path: "/test-executions",  matchPattern: "/test-executions" },
    ],
  },
];
```

---

## Active Route Matching

The sidebar determines the active link by matching the current route (`useLocation().pathname`)
against each `NavItem.matchPattern`. The match algorithm:

1. If the current pathname starts with the `matchPattern`, the item is active.
2. Detail/edit/create sub-routes also match the parent list route:
   - `/test-cases/42` matches Test Cases (`/test-cases`)
   - `/test-cases/new` matches Test Cases
   - `/test-cases/42/edit` matches Test Cases
3. If no item matches, no link is highlighted (fallback: do not crash).
4. The active link receives a distinct visual style: highlighted background colour
   and/or a left-border accent (3px solid brand accent colour).

Example match logic:

```typescript
function isActive(pathname: string, matchPattern: string): boolean {
  // Exact match or pathname starts with matchPattern followed by '/' or end
  return pathname === matchPattern || pathname.startsWith(matchPattern + "/");
}
```

---

## Sidebar Sections

### IAM Section

- Rendered only when `isSystemAdmin === true`.
- Links: Users (`/users`), Groups (`/groups`), Roles (`/roles`).
- When hidden, the section is completely removed from the DOM (not just `display: none`).

### Settings Section

- Always visible to all authenticated users.
- Links: Categories, Templates, Priorities, Plan Types.
- These are project-scoped. The active project ID is included as a query parameter
  or context when navigating: `/categories?project_id=1`.
  - The navigation link appends `?project_id={activeProjectId}` automatically.
  - If no project is selected, the link is disabled with a tooltip "Select a project first".

### Test Management Section

- Always visible to all authenticated users.
- Links: Test Cases, Test Plans, Test Runs, Test Executions.
- Project-scoped: same query parameter pattern as Settings.
  - Navigation link: `/test-cases?project_id={activeProjectId}`.
  - Disabled if no project selected.

### User Menu

- Always visible at the bottom of the sidebar.
- Displays user info: avatar (or initials in a circle), display name.
- Two action items: Profile, Logout.
- Profile: links to `/profile` (user's own profile page).
- Logout: calls `auth.logout()` which clears session and redirects to `/login`.

---

## Visual Specification

| Element | Style |
|---------|-------|
| Sidebar width | 240px (fixed) |
| Sidebar height | 100vh (full height, sticky) |
| Background | Dark: `#1e293b` (slate-800) or brand primary |
| Section heading | Uppercase, small font (11px), muted colour (`#94a3b8`), letter-spacing |
| Nav item (default) | 14px, text colour `#cbd5e1`, padding 8px 16px, no background |
| Nav item (hover) | Background `#334155` (slate-700), cursor pointer |
| Nav item (active) | Background `#0f172a` (slate-900), left border 3px solid `#3b82f6` (blue-500) |
| Nav item (disabled) | Opacity 0.4, cursor not-allowed, tooltip on hover |
| Divider | 1px solid `#334155` (slate-700) |
| User menu | Pinned to bottom; padding 12px 16px |
| App logo/name | Top of sidebar; padding 16px; brand logo + "HOA TCMS" text |
| Scroll | Sidebar content scrolls if it exceeds viewport height (overflow-y: auto) |

---

## Integration Points

### With Auth Context

```typescript
// Inside Sidebar
const { user, isSystemAdmin, logout } = useAuth();

// IAM section visibility
{isSystemAdmin && <NavSection section={NAV_SECTIONS[0]} />}

// User menu
<button onClick={logout}>Logout</button>
```

### With Project Context

```typescript
const { activeProjectId } = useProjectContext();

// Appending project context to nav links
function buildNavPath(basePath: string): string {
  if (activeProjectId) {
    return `${basePath}?project_id=${activeProjectId}`;
  }
  return basePath;
}
```

### With Router

The sidebar uses `<Link>` (or `<NavLink>`) from React Router for navigation items to
enable client-side routing without full page reloads.

```typescript
import { NavLink } from "react-router-dom";

<NavLink
  to={buildNavPath(item.path)}
  className={({ isActive }) => isActive ? "nav-item active" : "nav-item"}
>
  {item.label}
</NavLink>
```

---

## States & Edge Cases

| State | Visual Treatment |
|-------|-----------------|
| **User not authenticated** | Sidebar hidden; render `<Outlet />` only in a plain layout |
| **Auth loading** | Sidebar hidden or skeleton; page content shows spinner |
| **No project selected** | Settings and Test Management links disabled with tooltip |
| **Logout in progress** | Logout button shows spinner; other links remain clickable |
| **Logout fails** | Clear local state anyway; redirect to login; brief toast "Logged out" |
| **Long user name** | Truncate with ellipsis (max-width on name container) |

---

## Files

| File | Role |
|------|------|
| `src/ui/components/Sidebar/Sidebar.tsx` | Main sidebar component |
| `src/ui/components/Sidebar/NavSection.tsx` | Renders a section heading + nav items |
| `src/ui/components/Sidebar/NavItem.tsx` | Renders a single navigation link |
| `src/ui/components/Sidebar/UserMenu.tsx` | Bottom user menu (profile, logout) |
| `src/ui/components/Sidebar/navConfig.ts` | Static NAV_SECTIONS configuration |
| `src/ui/layouts/AuthenticatedLayout.tsx` | Layout shell that renders Sidebar + Outlet |
