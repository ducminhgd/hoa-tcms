# Tasks: UI — Pagination

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit of work.

---

## Cookie Persistence Hook

- [ ] 1. Implement `usePageSizePreference` hook — `requirements.md#US-2`,
      `design.md#Component API`
  - Read `page-size-preference` cookie on mount via `document.cookie`
  - Parse and validate: must be one of `10`, `25`, `50`, `100`; fall back to `25`
  - Provide `setPageSize` that writes cookie with path `/`, max-age 365 days,
    `SameSite=Lax`
  - Use a generic cookie helper (`src/shared/utils/cookie.ts`) for consistent
    read/write — do not inline `document.cookie` parsing
  - File: `src/shared/hooks/usePageSizePreference.ts`

- [ ] 2. Implement cookie utility helpers — `design.md#Cookie Specification`
  - `getCookie(name: string): string | null` — parses `document.cookie`
  - `setCookie(name: string, value: string, options: CookieOptions): void` —
    constructs cookie string with path, maxAge, sameSite, secure
  - Handle edge cases: cookie value with special characters (encode/decode),
    multiple cookies (parse correctly)
  - File: `src/shared/utils/cookie.ts`

---

## Core Components

- [ ] 3. Implement `PageButton` sub-component — `requirements.md#US-1`,
      `design.md#Page Number Generation`
  - Renders a single page number button or ellipsis span
  - Active page: visually distinct style (filled background, different colour)
  - Ellipsis: non-interactive text span
  - Accepts `page` (number | "..."), `isActive`, `onClick` props
  - Keyboard accessible (focusable, Enter/Space to activate)
  - File: `src/ui/components/Pagination/PageButton.tsx`

- [ ] 4. Implement `PageSizeSelector` sub-component — `requirements.md#US-2`,
      `design.md#Visual Layout`
  - Renders four toggle buttons: 10, 25, 50, 100
  - Currently selected value is highlighted
  - Calls `onChange` with the new numeric value
  - Keyboard accessible (arrow keys to switch between options)
  - File: `src/ui/components/Pagination/PageSizeSelector.tsx`

- [ ] 5. Implement `Pagination` main component — `requirements.md#US-1` and `US-2`,
      `design.md#Visual Layout`
  - Top row: "Showing {start}–{end} of {total} items" label (left) +
    `<PageSizeSelector>` (right)
  - Bottom row: "Previous" button, `<PageButton>` list, "Next" button
  - Compute `totalPages = Math.ceil(total / limit)`
  - Compute `start = (page - 1) * limit + 1`, `end = Math.min(page * limit, total)`
  - Disable Previous when `page === 1`
  - Disable Next when `page === totalPages`
  - Implement page number generation per `design.md#Page Number Generation`
  - Handle edge cases: `total = 0` (hide entire component), `totalPages = 1`
    (hide pagination, show only total count)
  - File: `src/ui/components/Pagination/Pagination.tsx`

---

## Tests & Verification

- [ ] 6. Write unit tests — `requirements.md#US-1` and `US-2`,
      `design.md#Page Number Generation`
  - Test `usePageSizePreference`: returns default (25) when no cookie set
  - Test `usePageSizePreference`: returns value from cookie when valid
  - Test `usePageSizePreference`: falls back to 25 when cookie has invalid value
  - Test `usePageSizePreference`: `setPageSize` writes cookie correctly
  - Test page number generation: 7 or fewer pages → all numbers rendered
  - Test page number generation: 8+ pages, current=1 → `[1] 2 3 ... 20`
  - Test page number generation: 8+ pages, current=4 → `1 ... 3 [4] 5 ... 20`
  - Test page number generation: 8+ pages, current=20 → `1 ... 18 19 [20]`
  - Test Previous disabled on page 1
  - Test Next disabled on last page
  - Test "Showing X–Y of Z" label is correct for first page, middle page, last page
  - Test size selector calls `onPageSizeChange` with correct value
  - Test component hidden when `total = 0`
  - Test component shows only total count when `totalPages = 1`
