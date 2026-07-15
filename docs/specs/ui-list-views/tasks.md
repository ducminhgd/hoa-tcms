# Tasks: UI — List Views

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit of work.

---

## Types & Constants

- [ ] 1. Define TypeScript types and column config interface —
      `design.md#Component API`, `requirements.md#US-1`
  - `ColumnConfig<T>`, `FilterConfig`, `FetchParams`, `PaginationMeta`,
    `PermissionContext` interfaces
  - `EntityListProps<T>` interface with all props documented
  - File: `src/ui/components/EntityList/types.ts`

---

## Core Component

- [ ] 2. Implement `EntityListTable` sub-component — `requirements.md#US-1`,
      `design.md#Component API`
  - Renders `<table>` with fixed columns: checkbox, actions, ID, Name, plus
    configurable entity columns from `columns` prop
  - Header row: checkbox in first column, action header (gear icon), ID header,
    Name header, then each configured column header
  - Body rows: checkbox per row (reads from `selectedIds` set), action icons
    (edit → pencil, delete → trash), ID as `<Link>` to `{detailUrlPrefix}/{id}`,
    Name as `<Link>` to `{detailUrlPrefix}/{id}`, then entity-specific cells
  - Row hover highlight; selected row background accent
  - Handle null/undefined cell values gracefully (show "—" dash)
  - File: `src/ui/components/EntityList/EntityListTable.tsx`

- [ ] 3. Implement `EntityListToolbar` sub-component — `requirements.md#US-3`,
      `design.md#Component API`
  - "Add New" button (left-aligned, primary colour) — conditionally rendered
    based on `createPermission` and `permissionContext.canCreate`
  - Search input (right-aligned) with search icon, clearable, placeholder from
    `searchPlaceholder` prop
  - Filter dropdowns (between Add New and Search) — each filter renders a
    `<select>` (or custom dropdown) populated from `FilterConfig.options`
  - Debounce search input by 300ms (use `useDebounce` hook from `src/shared/hooks/`)
  - Changing a filter or search calls `onFetch` with updated params and page=1
  - File: `src/ui/components/EntityList/EntityListToolbar.tsx`

- [ ] 4. Implement `BatchActionBar` sub-component — `requirements.md#US-2`,
      `design.md#Component API`
  - Renders conditionally when `selectedIds.size > 0`
  - Sticky positioning at table top (or bottom)
  - Shows count: "{N} selected" + batch action buttons
  - At minimum: "Delete Selected" button that calls `onBatchDelete(selectedIds)`
  - "Deselect All" link/button to clear selection
  - File: `src/ui/components/EntityList/BatchActionBar.tsx`

- [ ] 5. Implement state sub-components (skeleton, error, empty) —
      `requirements.md#US-1`, `design.md#States & Edge Cases`
  - `EntityListSkeleton`: 5–10 table rows with pulsing placeholder cells; matches
    column count from config
  - `EntityListError`: inline banner with error message and "Retry" button calling
    `onRetry`
  - `EntityListEmpty`: centered message, variant for "No items" vs "No results"
    (when filters active), "Clear filters" link when filters are active
  - Files: `EntityListSkeleton.tsx`, `EntityListError.tsx`, `EntityListEmpty.tsx`

- [ ] 6. Assemble `EntityList` main component — `requirements.md#US-1` through `US-3`,
      `design.md#Architecture`
  - Compose `EntityListToolbar`, `BatchActionBar`, `EntityListTable` (or
    skeleton/error/empty), and `<Pagination>` (from `ui-pagination`) in a single
    container
  - Manage internal state: `selectedIds`, `searchValue`, `activeFilters`
    (use `useState` or `useReducer`)
  - Handle all state transitions: loading → data, loading → error, data → empty,
    data → search, data → filter, data → paginate, idle → select, select → deselect
  - Pass pagination state to `<Pagination>` and handle `onPageChange`/`onPageSizeChange`
    callbacks
  - File: `src/ui/components/EntityList/EntityList.tsx`

---

## Integration & Battery

- [ ] 7. Write unit tests — `requirements.md#US-1` through `US-3`,
      `design.md#States & Edge Cases`
  - Test empty state renders correctly (no data)
  - Test loading state shows skeleton
  - Test error state shows banner with retry
  - Test selection: clicking header checkbox selects/deselects all rows on page
  - Test batch action bar appears/disappears with selection
  - Test search debounce (with fake timers)
  - Test filter change resets page to 1
  - Test action icons invoke correct callbacks (edit, delete)
  - Test permission hiding: edit/delete icons hidden when permission check fails
  - Test Add New button hidden when `createPermission` not granted

- [ ] 8. Create demo/storybook story for EntityList — `design.md#Component API`
  - Story with mock data (10 items), all columns, pagination meta
  - Story in empty state
  - Story in loading state
  - Story in error state
  - Story with active filters
  - Story with batch selection active
  - Story with permissions restricted (no edit/delete)
  - File: `src/ui/components/EntityList/EntityList.stories.tsx`
