# Design: UI — List Views

## Architecture

The List Views feature is a **frontend-only** pattern. It defines a reusable `<EntityList>`
component consumed by every entity list page. The component lives in the UI layer of the
React Clean Architecture layout (`src/ui/components/EntityList/`). It does not define API
endpoints, database tables, or backend logic.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  UI Layer (Presentation)                                                      │
│                                                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  EntityListPage (e.g., ProjectListPage, UserListPage)                  │   │
│  │  - Configures columns, filters, search placeholder, permission checks  │   │
│  │  - Calls application hook (e.g., useListProjects) for data            │   │
│  └──────────────────────────────┬────────────────────────────────────────┘   │
│                                 │ renders                                     │
│                                 ▼                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  <EntityList>                                                          │   │
│  │  ┌─────────────────────────────┐                                       │   │
│  │  │  Toolbar                     │                                       │   │
│  │  │  [Add New] [Search...] [Filter▼]                                    │   │
│  │  ├─────────────────────────────┤                                       │   │
│  │  │  Batch Action Bar (conditional)                                     │   │
│  │  │  "3 selected" [Delete Selected]                                     │   │
│  │  ├─────────────────────────────┤                                       │   │
│  │  │  Table                       │                                       │   │
│  │  │  ☐  | ⚡ | ID | Name | ...  │                                       │   │
│  │  │  ☐  | ⚡ | 1  | Foo  | ...  │                                       │   │
│  │  │  ☐  | ⚡ | 2  | Bar  | ...  │                                       │   │
│  │  ├─────────────────────────────┤                                       │   │
│  │  │  <Pagination />              │                                       │   │
│  │  │  ← 1 2 3 ... 10 →  [25▼]   │                                       │   │
│  │  └─────────────────────────────┘                                       │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  <Pagination> (from ui-pagination)                                     │   │
│  │  - Server-side cursor/offset pagination                               │   │
│  │  - Page size selector with cookie persistence                         │   │
│  │  - Total count display                                                │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. A consuming page (e.g., `ProjectListPage`) imports `<EntityList>` and passes
   configuration props (columns, filters, search config, permission context).
2. `<EntityList>` manages local UI state: selected row IDs, search input value,
   active filters, loading/error states.
3. On mount and on filter/search/page changes, `<EntityList>` calls the data-fetching
   callback provided by the parent (typically a custom hook like `useListProjects`).
4. The Pagination component (from `ui-pagination`) is embedded at the bottom and
   communicates page/page-size changes upward via callbacks.
5. Row actions (edit/delete) invoke callbacks provided by the parent; the parent
   handles navigation or mutation.

---

## Component API

### `<EntityList>` Props

```typescript
interface EntityListProps<T> {
  // Data
  data: T[];
  meta: PaginationMeta;
  loading: boolean;
  error: Error | null;

  // Callbacks
  onFetch: (params: FetchParams) => void;     // Triggers data reload
  onRetry: () => void;                         // Retry after error
  onRowEdit: (row: T) => void;                 // Edit action
  onRowDelete: (row: T) => void;               // Delete action (parent shows confirm dialog)
  onBatchDelete: (ids: number[]) => void;       // Batch delete
  onCreate: () => void;                         // "Add New" button click

  // Configuration
  columns: ColumnConfig<T>[];                  // Entity-specific columns
  searchPlaceholder: string;                    // e.g., "Search projects..."
  searchEnabled: boolean;                       // Show/hide search bar
  filters?: FilterConfig[];                     // Filter dropdowns
  entityName: string;                           // e.g., "projects", for labels
  permissionContext: PermissionContext;         // Permission codes for row actions
  createPermission?: string;                    // Permission code for "Add New"

  // Optional
  idField?: keyof T;                            // Default: "id"
  nameField?: keyof T;                          // Default: "name"
  detailUrlPrefix?: string;                     // e.g., "/projects/"
  editUrlPrefix?: string;                       // e.g., "/projects/edit/"
}

interface ColumnConfig<T> {
  key: string;                                  // Field key in T
  header: string;                               // Display header
  render?: (value: unknown, row: T) => ReactNode; // Custom cell render
  sortable?: boolean;                           // Enable sorting
  width?: string;                               // Column width (CSS value)
}

interface FilterConfig {
  key: string;                                  // Query param key
  label: string;                                // Display label
  options: { value: string; label: string }[];  // Dropdown options
}

interface PaginationMeta {
  total: number;
  page: number;
  limit: number;
}

interface FetchParams {
  page: number;
  limit: number;
  search?: string;
  filters?: Record<string, string>;
}
```

### Internal State

| State | Type | Default | Description |
|-------|------|---------|-------------|
| `selectedIds` | `Set<number>` | `new Set()` | Currently checked row IDs |
| `searchValue` | `string` | `""` | Current search input (debounced) |
| `activeFilters` | `Record<string, string>` | `{}` | Current filter selections |
| `pageSize` | `number` | cookie value or 25 | Items per page |

**`<EntityList>` is a fully controlled component with respect to pagination.** It does
NOT maintain an internal `currentPage` state. All page logic is derived from
`meta.page` (props). When the user clicks a page button, `<EntityList>` calls
`onFetch({ page: requestedPage, limit, search, filters })` -- the parent is responsible
for updating `meta.page` and `data` in response. This eliminates the dual-source-of-truth
problem where both the component's internal state and the props could disagree about the
current page.

---

## States & Edge Cases

| State | Visual Treatment |
|-------|-----------------|
| **Loading (initial)** | Skeleton table rows (5–10 placeholder rows with pulsing animation) |
| **Loading (re-fetch)** | Existing data stays visible; subtle overlay spinner on table body |
| **Empty (no data)** | Centered message: "No items found." with optional "Create first [entity]" link |
| **Empty (active filters)** | Centered message: "No results match your filters." with "Clear filters" link |
| **Error** | Inline error banner above table: error message + "Retry" button |
| **Permission denied** | Table renders normally but edit/delete icons are hidden; "Add New" is hidden |
| **Selection active** | Sticky batch action bar appears; selected rows have highlight background |
| **Deleting** | Row fades out with animation; batch action bar shows progress (if batch) |

### Checkbox Behavior

- **Header checkbox** selects/deselects all rows on the **current page only** (not all
  pages). A small note "All 25 on this page selected" appears when all are checked.
- Selecting rows across pages is out of scope for Phase 1.
- Navigating to a new page deselects all rows from the previous page.
- Checkbox column width: fixed 40px, not resizable.

---

## Integration Points

### With Pagination (`ui-pagination`)

The `<EntityList>` embeds `<Pagination>` at the bottom. Props passed:

```typescript
<Pagination
  total={meta.total}
  page={meta.page}
  limit={meta.limit}
  onPageChange={(page) => onFetch({ ...currentParams, page })}
  onPageSizeChange={(limit) => onFetch({ ...currentParams, limit, page: 1 })}
/>
```

### With Project Switcher (`ui-project-switcher`)

For project-scoped entities (Test Cases, Test Plans, etc.), the consuming page reads
`activeProjectId` from the project context and passes it as a filter:

```typescript
// In consuming page
const { activeProjectId } = useProjectContext();
const params = { ...fetchParams, filters: { ...fetchParams.filters, project_id: activeProjectId } };
```

### With Permission System

The consuming page passes a `PermissionContext` object that maps actions to permission codes:

```typescript
interface PermissionContext {
  canEdit?: (row: T) => boolean;
  canDelete?: (row: T) => boolean;
  canCreate?: boolean;
}
```

The entity list checks these before rendering action icons and the "Add New" button.

---

## Search and Filter Flow

```
User types in search bar
       │
       ▼
Debounce (300ms)
       │
       ▼
Update searchValue state
       │
       ▼
Call onFetch({ page: 1, limit, search: "query", filters: currentFilters })
       │
       ▼
Parent hook calls API: GET /api/v1/{entity}?page=1&limit=25&search=query&status=ACTIVE
       │
       ▼
Server returns paginated results
       │
       ▼
EntityList re-renders with new data
```

- Changing a filter resets to page 1.
- Changing page size resets to page 1.
- Search and filters are combined with AND semantics.

---

## Files

| File | Role |
|------|------|
| `src/ui/components/EntityList/EntityList.tsx` | Main component |
| `src/ui/components/EntityList/EntityListToolbar.tsx` | Search bar, filter controls, Add New button |
| `src/ui/components/EntityList/EntityListTable.tsx` | Table with checkbox, actions, columns |
| `src/ui/components/EntityList/BatchActionBar.tsx` | Conditional bar for bulk operations |
| `src/ui/components/EntityList/types.ts` | TypeScript interfaces (props, configs) |
| `src/ui/components/EntityList/EntityListSkeleton.tsx` | Loading skeleton |
| `src/ui/components/EntityList/EntityListError.tsx` | Error state with retry |
| `src/ui/components/EntityList/EntityListEmpty.tsx` | Empty state view |
