# Feature: UI — Pagination

## Overview

The Pagination feature defines a reusable server-side pagination component that appears at
the bottom of every entity list view. It provides page navigation (numbered buttons,
previous/next), a configurable page size selector (10/25/50/100), a total count display,
and persists the user's page size preference in a browser cookie so it is remembered
across sessions.

This spec defines only the frontend component and cookie persistence. The server-side
pagination contract (query parameters `page` and `limit`, response metadata `total`,
`page`, `limit`) is already specified in each entity's API contract.

---

## User Stories

### US-1: Navigate Paginated Data

As a user browsing any entity list, I want to navigate through pages of results using
numbered page buttons and previous/next controls, so that I can browse large datasets
without overwhelming the UI or the server.

**Acceptance Criteria (EARS)**

- WHEN the entity list has more than one page of results, THE SYSTEM SHALL render a
  pagination bar below the table with: "Previous" button, numbered page buttons,
  "Next" button, total count label, and page size selector.
- WHEN the user is on the first page, THE SYSTEM SHALL disable the "Previous" button.
- WHEN the user is on the last page, THE SYSTEM SHALL disable the "Next" button.
- WHEN the total number of pages is 7 or fewer, THE SYSTEM SHALL display all page
  numbers as buttons.
- WHEN the total number of pages exceeds 7, THE SYSTEM SHALL display a truncated list:
  first page, ellipsis ("..."), a window of pages around the current page, another
  ellipsis, and the last page (e.g., "1 ... 4 5 6 ... 20").
- WHEN the user clicks a page number, THE SYSTEM SHALL invoke the `onPageChange`
  callback with the new page number; the parent component is responsible for
  reloading data.
- WHEN the total count is zero, THE SYSTEM SHALL display "No items" instead of the
  pagination bar (controlled by the parent `<EntityList>` component's empty state).
- IF the server returns an error during pagination, THE SYSTEM SHALL display the error
  in the table area (handled by `<EntityList>`), not in the pagination bar.

### US-2: Change Page Size with Cookie Persistence

As a user who prefers a specific page size, I want to choose among predefined page
sizes (10, 25, 50, 100) and have my preference remembered across browser sessions,
so that I do not need to change it every time I use the system.

**Acceptance Criteria (EARS)**

- WHEN the pagination bar is rendered, THE SYSTEM SHALL display a page size selector
  dropdown with options: 10, 25, 50, 100.
- WHEN the user selects a different page size from the dropdown, THE SYSTEM SHALL
  invoke the `onPageSizeChange` callback with the new limit, reset to page 1, and
  persist the selected value to a browser cookie named `page-size-preference` with
  an expiry of 365 days and `SameSite=Lax`.
- WHEN the component mounts, THE SYSTEM SHALL read the `page-size-preference` cookie;
  if present and valid (one of 10, 25, 50, 100), use it as the initial page size;
  otherwise default to 25.
- IF the cookie value is missing or not a valid option, THE SYSTEM SHALL silently
  fall back to the default (25) without showing an error.
- THE SYSTEM SHALL only set the cookie client-side; no server endpoint is involved.

---

## Out of Scope

- **Server-side pagination implementation** — each entity CRUD spec defines its own
  paginated API response format (query params `page`, `limit`; response `meta`
  with `total`, `page`, `limit`).
- **Infinite scroll** — only traditional numbered pagination is supported in Phase 1.
- **Cursor-based pagination** — offset-based pagination is used throughout Phase 1.
- **Sticky pagination** — the pagination bar does not stick to the viewport on scroll
  in Phase 1 (could be enhanced later).

---

## Dependencies

- **All entity list API endpoints** — each list endpoint must support `page` and `limit`
  query parameters and return `meta.total` in the response.
- **ui-list-views** — the `<EntityList>` component integrates `<Pagination>` at the
  bottom of the table.
- **cookie-js** (or browser-native `document.cookie`) — used for reading/writing the
  `page-size-preference` cookie.
