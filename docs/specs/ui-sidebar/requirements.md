# Feature: UI — Sidebar Navigation

## Overview

The Sidebar Navigation defines the primary navigation pattern for HOA TCMS. It is a
persistent vertical sidebar visible on all authenticated pages (except login and public
pages). The sidebar groups navigation links into three sections -- IAM (System Admin only),
Settings (project-scoped metadata), and Test Management -- plus a user menu at the bottom
with Profile and Logout. The sidebar highlights the currently active section based on the
current route.

This spec defines the frontend component pattern only. It does not define routing logic,
authorization checks, or API endpoints.

---

## User Stories

### US-1: Navigate Between Sections

As an authenticated user, I want to use a sidebar to navigate between the major sections
of the application, so that I can quickly access any feature area without returning to a
home page.

**Acceptance Criteria (EARS)**

- WHEN the user is authenticated (has a valid session), THE SYSTEM SHALL render a
  vertical sidebar on the left side of the viewport with a fixed position (does not
  scroll with page content).
- THE SIDEBAR SHALL contain the following navigation sections in order:
  1. **IAM** (heading): Users, Groups, Roles
  2. **Settings** (heading): Categories, Templates, Priorities, Plan Types
  3. **Test Management** (heading): Test Cases, Test Plans, Test Runs, Test Executions
- WHEN the user clicks a navigation link, THE SYSTEM SHALL navigate to the corresponding
  list page (e.g., `/test-cases` for Test Cases).
- WHEN the user navigates to a new page, THE SYSTEM SHALL highlight the matching
  sidebar link as active (visually distinct background and/or left border accent).
- IF the current route does not match any sidebar link (e.g., a detail page like
  `/test-cases/42`), THE SYSTEM SHALL highlight the parent section's link (e.g.,
  Test Cases) as active.
- WHEN the user is on the login page or any unauthenticated page, THE SYSTEM SHALL NOT
  render the sidebar.

### US-2: Conditional IAM Section (System Admin Only)

As a System Admin, I want to see the IAM section in the sidebar so that I can manage
users, groups, and roles. As a non-admin user, I want IAM to be hidden so the sidebar
stays focused on what I can access.

**Acceptance Criteria (EARS)**

- WHEN the authenticated user holds the System Admin role, THE SYSTEM SHALL render the
  IAM section with links: Users (`/users`), Groups (`/groups`), Roles (`/roles`).
- WHEN the authenticated user does NOT hold the System Admin role, THE SYSTEM SHALL
  hide the entire IAM section (heading and all links).
- IF the user's role changes during a session (e.g., promoted to System Admin), THE
  SYSTEM SHALL reflect the change on the next full page navigation or explicit sidebar
  re-render (not requiring a full logout/login, but not requiring real-time push either).

### US-3: User Menu (Profile and Logout)

As an authenticated user, I want persistent access to my profile and logout from the
sidebar, so that I can manage my account and securely end my session.

**Acceptance Criteria (EARS)**

- THE SIDEBAR SHALL render a user menu section at the bottom, separated from the
  navigation links by a divider line.
- THE USER MENU SHALL display the authenticated user's display name (or username as
  fallback) and avatar (or initials as fallback).
- WHEN the user clicks "Profile", THE SYSTEM SHALL navigate to the user's profile
  page (`/profile`).
- WHEN the user clicks "Logout", THE SYSTEM SHALL call the logout API endpoint,
  clear the session, and redirect to the login page.
- IF the logout API call fails, THE SYSTEM SHALL still clear local session state and
  redirect to the login page (degraded gracefully -- the server session will expire).

---

## Out of Scope

- **Responsive/mobile navigation** — the sidebar is always visible on desktop. Mobile
  navigation (hamburger menu, bottom tab bar) is deferred to Phase 2.
- **Collapsible sidebar** — the sidebar is always expanded in Phase 1. A collapse
  toggle (icon-only mode) could be added later.
- **Dynamic menu items** — menu items are statically defined, not fetched from a server.
- **Permission-based item visibility beyond System Admin** — only the IAM section uses
  role-based visibility in Phase 1. All other links are visible to all authenticated users.
- **Nested routes in sidebar** — each menu item links to the entity list page. Sub-pages
  (detail, edit, create) are reachable from the list page but do not appear as sidebar
  items.

---

## Dependencies

- **iam-auth** — Session state and logout API endpoint. The sidebar depends on the
  current authentication state to decide visibility.
- **iam-users** — User profile endpoint to fetch display name and avatar.
- **iam-roles** — System Admin role check to conditionally render the IAM section.
- **ui-project-switcher** — The Settings and Test Management sections change their
  scope based on the active project, which is managed by `ui-project-switcher`.
