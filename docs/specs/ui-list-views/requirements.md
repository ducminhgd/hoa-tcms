# Feature: UI — List Views

## Overview

The List Views feature defines the standard UI pattern for displaying tabular, paginated entity lists
across all HOA TCMS views. Every entity list (projects, users, groups, roles, test cases, test plans,
test runs, test executions, metadata) uses this single consistent pattern. The list view provides
multi-select via checkboxes, action icons per row, navigable ID and Name columns, an "Add New"
button, filter/search controls, and server-side pagination.

This spec defines the frontend component pattern only. It does not define API endpoints (those are
in each entity's CRUD spec) or pagination internals (those are in `ui-pagination`).

---

## User Stories

### US-1: View Entity List with Standard Layout

As a user with read access to an entity type, I want to see a paginated table of records with
consistent columns (checkbox, actions, ID, Name, plus entity-specific fields), so that I can
quickly browse, identify, and navigate to items I need.

**Acceptance Criteria (EARS)**

- WHEN the user navigates to any entity list page (e.g., Projects, Users, Test Cases),
  THE SYSTEM SHALL render a table with the following standard columns in order:
  1. Checkbox column (select-all in header, individual checkboxes per row)
  2. Action icons column (edit icon, delete icon)
  3. ID column (hyperlinked to the entity's detail page)
  4. Name column (hyperlinked to the entity's detail page)
  5. Entity-specific columns (defined by each consuming view)
- WHEN the server returns an empty data set, THE SYSTEM SHALL display an empty-state message
  ("No items found") in the table body, not a blank table or error.
- WHEN the server returns an error (network failure, 500, etc.), THE SYSTEM SHALL display an
  inline error message above the table with a "Retry" button.
- WHEN data is loading, THE SYSTEM SHALL show a skeleton loader or spinner in place of the
  table body until the response arrives.

### US-2: Multi-Select and Row Actions

As a user managing multiple records, I want to select one or more rows via checkboxes and perform
actions on them, so that I can efficiently manage items in bulk (e.g., delete multiple, export).

**Acceptance Criteria (EARS)**

- WHEN the user clicks the header checkbox, THE SYSTEM SHALL toggle selection of all visible
  rows on the current page.
- WHEN the user clicks an individual row checkbox, THE SYSTEM SHALL toggle that row's selection
  state.
- WHEN one or more rows are selected, THE SYSTEM SHALL display a sticky action bar at the top
  (or bottom) of the table showing the count ("3 selected") and available batch actions
  (e.g., "Delete Selected", "Export Selected").
- WHEN all selected rows are deselected (either by unchecking or by navigating away),
  THE SYSTEM SHALL hide the batch action bar.
- WHEN the user clicks the edit icon on a row, THE SYSTEM SHALL navigate to the entity's edit
  page (or open an edit modal, depending on the consuming view's configuration).
- WHEN the user clicks the delete icon on a row, THE SYSTEM SHALL show a confirmation dialog
  before dispatching the delete request.
- IF the user lacks the required permission for a row action, THE SYSTEM SHALL either hide
  the action icon for that row or render it disabled with a tooltip explaining why.

### US-3: Search, Filter, and Sort

As a user browsing a large entity list, I want to search by keyword and filter by relevant
attributes, so that I can narrow down the results to the items I need.

**Acceptance Criteria (EARS)**

- WHEN the view is configured with a search bar, THE SYSTEM SHALL render a text input above
  the table with a search icon and placeholder text (configurable per entity, e.g.,
  "Search projects...").
- WHEN the user types in the search bar, THE SYSTEM SHALL debounce input (300ms default,
  configurable) and send the search query to the server as a query parameter.
- WHEN the view is configured with filter controls, THE SYSTEM SHALL render filter dropdowns
  or toggle buttons above or beside the table, populated with available filter options
  (e.g., status filter for Projects, role filter for Users).
- WHEN the user changes a filter value, THE SYSTEM SHALL immediately reload the list with the
  new filter applied, resetting to page 1.
- WHEN the user clears all filters, THE SYSTEM SHALL reload the unfiltered list.
- IF the server returns an empty result set due to active filters, THE SYSTEM SHALL display
  "No results match your filters" with a "Clear filters" link, distinct from the
  "No items found" empty state.

---

## Out of Scope

- **Pagination component internals** — the pagination bar below the table is defined in
  `ui-pagination`.
- **Entity-specific API endpoints** — each entity's CRUD endpoints are defined in their
  respective specs (e.g., `project-crud`, `test-case-crud`).
- **Column definitions for each entity** — the consuming view provides entity-specific columns;
  this spec defines only the framework and shared columns.
- **Batch operation server endpoints** — individual CRUD specs define any batch endpoints.
- **Detail and edit pages** — each entity's detail/edit views are defined in their respective
  UI specs.

---

## Dependencies

- **ui-pagination** — The pagination bar at the bottom of the list view is the Pagination
  component from `ui-pagination`.
- **iam-auth** — Authentication state determines which action icons are rendered per row
  (hide/disable edit/delete based on user permissions).
- **All entity CRUD specs** — Each entity's list endpoint provides the data for its list view.
- **ui-project-switcher** — The active project ID is sent as a query parameter (or scope
  filter) on project-scoped list views.
