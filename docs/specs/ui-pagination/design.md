# Design: UI — Pagination

## Architecture

The Pagination component is a **frontend-only** presentational component with a small
cookie-persistence hook. It receives pagination metadata and callbacks as props; it
manages no server state.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  UI Layer                                                                     │
│                                                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  <EntityList>                                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │   │
│  │  │  Table ...                                                       │   │   │
│  │  ├─────────────────────────────────────────────────────────────────┤   │   │
│  │  │  <Pagination                                                      │   │   │
│  │  │    total={500}                                                    │   │   │
│  │  │    page={3}                                                       │   │   │
│  │  │    limit={25}                                                     │   │   │
│  │  │    onPageChange={fn}                                              │   │   │
│  │  │    onPageSizeChange={fn}                                          │   │   │
│  │  │  />                                                               │   │   │
│  │  │                                                                   │   │   │
│  │  │  Rendering:                                                       │   │   │
│  │  │  ┌────────────────────────────────────────────────────────────┐   │   │   │
│  │  │  │  Showing 51–75 of 500 items         [10▼] [25] [50] [100] │   │   │   │
│  │  │  ├────────────────────────────────────────────────────────────┤   │   │   │
│  │  │  │  ← Previous   1 ... 4 [5] 6 ... 20   Next →               │   │   │   │
│  │  │  └────────────────────────────────────────────────────────────┘   │   │   │
│  │  └─────────────────────────────────────────────────────────────────┘   │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  usePageSizePreference() — custom hook                                 │   │
│  │  - Reads cookie "page-size-preference" on mount                        │   │
│  │  - Returns [pageSize, setPageSize]                                     │   │
│  │  - setPageSize writes cookie (365-day expiry, SameSite=Lax)           │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. `<Pagination>` receives `total`, `page`, `limit`, `onPageChange`, `onPageSizeChange`
   as props.
2. On mount, `usePageSizePreference()` reads the `page-size-preference` cookie. The
   parent (`<EntityList>`) uses this as the initial `limit` for its data-fetching
   callback.
3. When the user clicks a page button, `onPageChange(newPage)` is called; the parent
   updates its state and re-fetches data.
4. When the user selects a new page size, `onPageSizeChange(newLimit)` is called AND
   the cookie is updated. The parent resets to page 1 and re-fetches.

---

## Component API

### `<Pagination>` Props

```typescript
interface PaginationProps {
  total: number;                              // Total number of items across all pages
  page: number;                               // Current page (1-based)
  limit: number;                              // Items per page (10, 25, 50, 100)
  onPageChange: (page: number) => void;       // Callback when user navigates pages
  onPageSizeChange: (limit: number) => void;  // Callback when user changes page size
}
```

### `usePageSizePreference` Hook

```typescript
const DEFAULT_PAGE_SIZE = 25;
const VALID_PAGE_SIZES = [10, 25, 50, 100] as const;
type PageSize = (typeof VALID_PAGE_SIZES)[number];

function usePageSizePreference(): [PageSize, (size: PageSize) => void];
```

**Behavior:**

- On mount, reads `document.cookie` for `page-size-preference`.
- Parses the value: if it is one of `[10, 25, 50, 100]`, returns it; otherwise
  returns `DEFAULT_PAGE_SIZE` (25).
- `setPageSize` writes the new value to `document.cookie` with:
  - Path: `/`
  - Max-Age: `31536000` (365 days)
  - SameSite: `Lax`

---

## Page Number Generation

The component computes which page buttons to render:

```
function getPageNumbers(currentPage: number, totalPages: number): (number | "...")[] {
  if (totalPages <= 7) {
    return range(1, totalPages);
  }

  // Always show first and last page
  // Show a window of pages around currentPage
  const result: (number | "...")[] = [1];

  if (currentPage > 3) {
    result.push("...");
  }

  const windowStart = Math.max(2, currentPage - 1);
  const windowEnd = Math.min(totalPages - 1, currentPage + 1);

  for (let i = windowStart; i <= windowEnd; i++) {
    result.push(i);
  }

  if (currentPage < totalPages - 2) {
    result.push("...");
  }

  result.push(totalPages);
  return result;
}
```

Examples with `totalPages = 20`:

| Current Page | Rendered Buttons |
|-------------|-----------------|
| 1 | `[1] 2 3 ... 20` |
| 2 | `1 [2] 3 4 ... 20` |
| 3 | `1 2 [3] 4 5 ... 20` |
| 4 | `1 ... 3 [4] 5 ... 20` |
| 19 | `1 ... 17 18 [19] 20` |
| 20 | `1 ... 18 19 [20]` |

---

## Visual Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│  Showing 51–75 of 500 items              [10▼]  [25]  [50]  [100]    │
│  ← Previous   1 ... 4 [5] 6 ... 20   Next →                          │
└──────────────────────────────────────────────────────────────────────┘
```

- **Top row:** "Showing X–Y of Z items" (left), page size selector buttons (right).
- **Bottom row:** Previous button, page buttons, ellipsis spans, Next button.
- Current page button is visually distinct (filled background, different colour).
- Page buttons are equally spaced; ellipsis is non-interactive text.
- Entire bar is hidden when `total === 0` (parent `<EntityList>` handles this via
  conditional rendering).
- Page size selector: visual toggle or dropdown; currently selected size is highlighted.
  All four options always visible.

### Responsive Behavior

- On viewports narrower than 480px: hide the "Showing X–Y of Z" text; reduce page
  button window to current page only + Previous/Next.

---

## Cookie Specification

| Property | Value |
|----------|-------|
| Cookie name | `page-size-preference` |
| Cookie value | One of `"10"`, `"25"`, `"50"`, `"100"` |
| Path | `/` |
| Max-Age | `31536000` (365 days in seconds) |
| SameSite | `Lax` |
| Secure | `true` (only set in production; omit in local dev) |
| HttpOnly | `false` (must be readable by JavaScript) |

---

## Integration Points

### With EntityList (`ui-list-views`)

The `<EntityList>` component wraps `<Pagination>`:

```typescript
// Inside EntityList
const [pageSize, setPageSize] = usePageSizePreference();

function handlePageChange(newPage: number) {
  onFetch({ ...currentParams, page: newPage });
}

function handlePageSizeChange(newLimit: number) {
  setPageSize(newLimit);
  onFetch({ ...currentParams, page: 1, limit: newLimit });
}

return (
  <div className="entity-list">
    <EntityListToolbar ... />
    <BatchActionBar ... />
    <EntityListTable ... />
    {meta.total > 0 && (
      <Pagination
        total={meta.total}
        page={meta.page}
        limit={pageSize}
        onPageChange={handlePageChange}
        onPageSizeChange={handlePageSizeChange}
      />
    )}
  </div>
);
```

### Server API Contract (Reference)

All list endpoints must support:

**Request:**
```
GET /api/v1/{entity}?page=2&limit=25
```

**Response metadata:**
```json
{
  "data": [...],
  "meta": {
    "total": 500,
    "page": 2,
    "limit": 25
  }
}
```

The server is responsible for:
- Enforcing minimum page (1), maximum page (ceiling of total/limit)
- Enforcing minimum limit (1), maximum limit (100)
- Returning the exact same `page` and `limit` values in the response `meta`
- Returning the correct `total` count of matching items (unpaginated count)

---

## Files

| File | Role |
|------|------|
| `src/ui/components/Pagination/Pagination.tsx` | Main component |
| `src/ui/components/Pagination/PageButton.tsx` | Individual page number button |
| `src/ui/components/Pagination/PageSizeSelector.tsx` | Page size toggle/dropdown |
| `src/shared/hooks/usePageSizePreference.ts` | Cookie read/write hook |
| `src/ui/components/Pagination/types.ts` | TypeScript interfaces |
