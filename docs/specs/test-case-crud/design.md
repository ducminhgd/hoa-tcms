# Design: Test Case CRUD

## Architecture

The Test Case CRUD feature follows Clean Architecture layering. Test cases are
per-project core entities. All endpoints are nested under the project resource path
(`/api/v1/projects/{projectId}/test-cases`) and require both system RBAC permission and
project membership scope checks. Additionally, Contributors have ownership-based
restrictions on update and delete operations.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_test_cases     GET    /api/v1/projects/{pid}/test-cases       │   │
│  │  - create_test_case    POST   /api/v1/projects/{pid}/test-cases       │   │
│  │  - get_test_case       GET    /api/v1/projects/{pid}/test-cases/{id}  │   │
│  │  - update_test_case    PATCH  /api/v1/projects/{pid}/test-cases/{id}  │   │
│  │  - delete_test_case    DELETE /api/v1/projects/{pid}/test-cases/{id}  │   │
│  │  - select_test_cases   GET    /api/v1/projects/{pid}/test-cases/select│   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  TestCaseService:                         │  │  - TestCase (entity)    │  │
│  │  - create_test_case                       │  └──────────────────────────┘  │
│  │  - list_test_cases                        │                                │
│  │  - get_test_case                          │                                │
│  │  - update_test_case                       │                                │
│  │  - delete_test_case                       │                                │
│  │  - select_test_cases                      │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - TestCaseRepository (port)              │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestCaseRepository  (implements TestCaseRepository)              │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All test case endpoints require an authenticated session (checked by `AuthMiddleware`).
2. `POST` endpoints require the caller to be a Contributor, Editor, or Owner of the project
   (or a System Admin), on top of the system permission check. Viewers cannot create.
3. `PATCH` and `DELETE` endpoints require the caller to be at least a Contributor, with an
   additional ownership check for Contributors: they can only update or delete test cases
   where `created_by` matches their user ID. Owners and Editors can update or delete any
   test case in the project.
4. `GET` endpoints require any project membership (or System Admin).
5. Create validates unique summary, category FK, and priority FK within a single database
   transaction to prevent TOCTOU races.
6. Update validates unique summary (if summary is being changed), category FK, and priority
   FK within a single database transaction. Rejects updates on soft-deleted records.
7. Soft-delete requires no referential integrity checks (test cases are soft-delete leaf entities (no cascade needed), but
   `TEST_CASE_FILES` does reference `TEST_CASES` via FK -- files become
   inaccessible when their parent test case is soft-deleted (gated at
   application query layer)). Only the ownership check for Contributors applies.

### Security Requirements

**Input sanitization:** Test case `summary`, `description`, and `notes` fields must be
sanitized on input (strip disallowed HTML tags) before storage. Output-encoding must be
applied at the presentation layer. See PRD 5.3 for the project-wide XSS prevention policy.

**CSRF protection:** All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must be
protected against CSRF. Session cookies must carry `SameSite=Lax` (or stricter).
`POST`/`PATCH`/`DELETE` requests with `Content-Type: application/json` must verify the
`Content-Type` header to block simple form-based CSRF attacks. Additional anti-CSRF
tokens should be considered for defense in depth.

**Authorization layering:** Three independent authorization gates apply:

1. System permission check (e.g., `test_case:update`)
2. Project membership and role check (Contributor, Editor, Owner for mutations; any role
   for reads; Viewer excluded from all mutations)
3. Contributor ownership check (on update and delete only): if the user's project role is
   Contributor, verify `created_by` matches the authenticated user ID

All three gates must pass (or the caller must be a System Admin, who implicitly holds all
system permissions, bypasses all project membership checks, and bypasses the ownership
check). Authorization checks are performed against live data on every request -- permissions
and project membership are never cached in the session. If a user's role is changed
mid-session, the new role takes effect on their next request. A `403 Forbidden` response
must use a generic message that does not distinguish between "missing system permission"
and "wrong project role". The Contributor ownership check is the exception: it returns a
distinct message because the user has already passed the project membership gate.

**Audit field visibility:** The `created_by` field is included in list and detail API
responses; `updated_by` is included in detail and create responses only (not in list or
select). Both fields are visible to all project members (including Viewers). This is
intentional: within a project, audit transparency (who created/modified a test case) aids
collaboration. The fields carry numeric user IDs, not emails or names, limiting direct PII
exposure.

**Rate limiting:** All endpoints are protected by rate limiting configured at the
middleware layer. State-changing endpoints (POST/PATCH/DELETE) allow 30 req/min; read
endpoints (GET list/detail) allow 60 req/min; the select endpoint allows 120 req/min
due to frequent UI usage. See requirements.md Security Considerations for the full table.

**FK validation (same-project constraint):** Before inserting or updating a test case with
a non-null `category_id` or `priority_id`, the system must verify that the referenced row
exists, is not soft-deleted, and has a `project_id` matching the test case's `project_id`.
This prevents cross-project data injection. The validation queries are:

```sql
-- Validate category_id
SELECT 1 FROM test_categories
WHERE id = $1 AND project_id = $2 AND deleted_at IS NULL;

-- Validate priority_id
SELECT 1 FROM test_priorities
WHERE id = $1 AND project_id = $2 AND deleted_at IS NULL;
```

These checks execute within the same database transaction as the insert/update.

---

## API Contract

### Common Error Response Format

All errors follow the standard project format:

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable description",
    "details": [
      { "field": "field_name", "message": "specific validation message" }
    ]
  }
}
```

---

### GET `/api/v1/projects/{projectId}/test-cases`

List test cases in a project with pagination, filtering, and sorting.

**Required Permission:** `test_case:read_list`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Query Parameters:**

| Param | Type | Default | Max | Description |
|-------|------|---------|-----|-------------|
| `page` | integer | 1 | -- | Page number (1-based) |
| `limit` | integer | 25 | 100 | Items per page |
| `category_id` | integer | -- | -- | Filter by category ID |
| `priority_id` | integer | -- | -- | Filter by priority ID |
| `automated` | boolean | -- | -- | Filter by automation status (`true` or `false`) |
| `search` | string | -- | 255 | Case-insensitive substring match on `summary` and `description` |
| `sort` | string | `-id` | -- | Sort field: `id`, `-id`, `summary`, `-summary`, `created_at`, `-created_at` |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 42,
      "summary": "User can log in with valid credentials",
      "category_id": 3,
      "priority_id": 1,
      "automated": true,
      "created_by": 15,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_at": "2026-07-14T10:00:00Z"
    },
    {
      "id": 41,
      "summary": "Password reset email is sent within 60 seconds",
      "category_id": null,
      "priority_id": 2,
      "automated": false,
      "created_by": 15,
      "created_at": "2026-07-14T09:30:00Z",
      "updated_at": "2026-07-15T08:00:00Z"
    }
  ],
  "meta": {
    "total": 150,
    "page": 1,
    "limit": 25
  }
}
```

**Notes:**
- Results are scoped to the given project and exclude soft-deleted test cases
  (`WHERE project_id = $1 AND deleted_at IS NULL`).
- Default sort is `-id` (newest first, descending).
- `search` applies `ILIKE` on both `summary` and `description` when provided;
  when absent, no filter is applied. The search value is escaped in this order: first
  `\` is doubled to `\\`, then `%` is escaped to `\%`, then `_` is escaped to `\_`.
  This ensures backslash literals are not consumed as PostgreSQL escape prefixes. The
  search value must not exceed 255 characters. Leading and trailing whitespace is
  trimmed; an all-whitespace search is treated as "no filter."
- The `sort` parameter accepts: `id` (oldest first), `-id` (newest first, default),
  `summary` (A-Z), `-summary` (Z-A), `created_at` (oldest first),
  `-created_at` (newest first).
- `category_id` and `priority_id` filters: if the referenced category/priority does not
  exist, is soft-deleted, or belongs to a different project, the result set is empty
  (not an error -- the filter simply matches nothing).
- List response excludes `description`, `notes`, `updated_by`, `category_name`,
  `priority_name`, `deleted_at`, and `deleted_by` to keep the payload compact.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:read_list` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `422` | `VALIDATION_ERROR` | Invalid pagination, sort parameter, or filter value |

---

### POST `/api/v1/projects/{projectId}/test-cases`

Create a new test case within a project.

**Required Permission:** `test_case:create` AND project role Contributor, Editor, or Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Request Body:**

```json
{
  "summary": "User can log in with valid credentials",
  "category_id": 3,
  "priority_id": 1,
  "automated": true,
  "description": "Given a registered user with valid credentials, when they submit the login form, then they are redirected to the dashboard.",
  "notes": "This is a critical path test. Ensure the test database has a seeded user."
}
```

| Field | Type | Required | Default | Constraints |
|-------|------|----------|---------|-------------|
| `summary` | string | Yes | -- | 1-500 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input (HTML stripped); unique per project (case-insensitive) |
| `category_id` | integer | No | `null` | Must reference a non-deleted category in the same project |
| `priority_id` | integer | No | `null` | Must reference a non-deleted priority in the same project |
| `automated` | boolean | No | `false` | `true` or `false` only |
| `description` | string | No | `null` | Max 10000 characters; sanitized on input (HTML stripped). Empty string `""` stored as-is. Explicit `null` stores `NULL`. Omitting defaults to `null`. |
| `notes` | string | No | `null` | Max 5000 characters; sanitized on input (HTML stripped). Empty string `""` stored as-is. Explicit `null` stores `NULL`. Omitting defaults to `null`. |

**Success Response:** `201 Created`

Headers: `Location: /api/v1/projects/42/test-cases/128`

```json
{
  "data": {
    "id": 128,
    "summary": "User can log in with valid credentials",
    "description": "Given a registered user with valid credentials, when they submit the login form, then they are redirected to the dashboard.",
    "notes": "This is a critical path test. Ensure the test database has a seeded user.",
    "automated": true,
    "project_id": 42,
    "category_id": 3,
    "category_name": "Login",
    "priority_id": 1,
    "priority_name": "Critical",
    "created_by": 15,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 15,
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- `project_id` in the response mirrors the path parameter.
- `created_by` and `updated_by` are both set to the authenticated user's ID.
- `category_name` and `priority_name` are resolved via JOIN on create (the referenced
  category/priority is guaranteed to exist and be non-deleted at this point).
- The unique summary check uses `LOWER(summary) = LOWER($1)` and includes
  `WHERE project_id = $2 AND deleted_at IS NULL`.
- Soft-deleted test cases with the same summary do not block creation.
- Category and priority FK validation runs within the same transaction as the insert to
  prevent TOCTOU races.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:create` permission or is not Contributor/Editor/Owner of the project |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `409` | `DUPLICATE_TEST_CASE_SUMMARY` | Test case summary already exists in this project (case-insensitive) |
| `422` | `VALIDATION_ERROR` | Summary empty, exceeds max length; description/notes too long; invalid boolean for automated |
| `422` | `INVALID_CATEGORY` | `category_id` does not exist, is soft-deleted, or belongs to a different project |
| `422` | `INVALID_PRIORITY` | `priority_id` does not exist, is soft-deleted, or belongs to a different project |

`409 Conflict` response body for duplicate summary:

```json
{
  "error": {
    "code": "DUPLICATE_TEST_CASE_SUMMARY",
    "message": "A test case with the summary 'User can log in with valid credentials' already exists in this project.",
    "details": [
      { "field": "summary", "message": "Test case summary must be unique within the project" }
    ]
  }
}
```

`422` response body for invalid category:

```json
{
  "error": {
    "code": "INVALID_CATEGORY",
    "message": "The specified category does not exist or does not belong to this project.",
    "details": [
      { "field": "category_id", "message": "Category must exist and belong to the same project" }
    ]
  }
}
```

---

### GET `/api/v1/projects/{projectId}/test-cases/{id}`

Get full detail of a single test case, including resolved category and priority names.

**Required Permission:** `test_case:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Test Case ID |

**Success Response:** `200 OK`

```json
{
  "data": {
    "id": 128,
    "summary": "User can log in with valid credentials",
    "description": "Given a registered user with valid credentials, when they submit the login form, then they are redirected to the dashboard.",
    "notes": "This is a critical path test. Ensure the test database has a seeded user.",
    "automated": true,
    "project_id": 42,
    "category_id": 3,
    "category_name": "Login",
    "priority_id": 1,
    "priority_name": "Critical",
    "created_by": 15,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 15,
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- `category_name` is resolved by LEFT JOIN on `test_categories`. If `category_id` is null
  or the category is soft-deleted, `category_name` is `null`.
- `priority_name` is resolved by LEFT JOIN on `test_priorities`. If `priority_id` is null
  or the priority is soft-deleted, `priority_name` is `null`.
- `deleted_at` and `deleted_by` are never returned to the client.
- The test case must belong to the specified project; if the test case exists but belongs
  to a different project, `404 Not Found` is returned (same as non-existent).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:read` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, or test case does not exist / is soft-deleted / belongs to a different project |

---

### PATCH `/api/v1/projects/{projectId}/test-cases/{id}`

Update a test case's fields. Contributors can only update their own test cases.

**Required Permission:** `test_case:update` AND project role Contributor (own only), Editor, or Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Test Case ID |

**Request Body** (all fields optional; at least one required):

```json
{
  "summary": "Updated summary",
  "category_id": null,
  "priority_id": 2,
  "automated": false,
  "description": "Updated description",
  "notes": "Updated notes"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `summary` | string | No | 1-500 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input (HTML stripped); unique per project (case-insensitive) |
| `category_id` | integer | No | Must reference a non-deleted category in the same project. Sending `null` clears the reference. Omitting preserves the current value. |
| `priority_id` | integer | No | Must reference a non-deleted priority in the same project. Sending `null` clears the reference. Omitting preserves the current value. |
| `automated` | boolean | No | `true` or `false` only |
| `description` | string | No | Max 10000 characters; sanitized on input (HTML stripped). Omitting preserves current. `null` clears. `""` stores empty string. |
| `notes` | string | No | Max 5000 characters; sanitized on input (HTML stripped). Omitting preserves current. `null` clears. `""` stores empty string. |

**Success Response:** `200 OK`

Response body is the updated test case representation (same shape as GET detail).

**Notes:**
- At least one field must be provided (empty body returns `422`).
- `id`, `project_id`, `created_at`, and `created_by` are immutable.
- `deleted_at` and `deleted_by` cannot be modified through this endpoint.
- `updated_at` and `updated_by` are set automatically on change.
- The unique summary check is only performed if `summary` is provided and differs
  from the current value.
- Soft-deleted test cases cannot be updated (per FR-54c).
- For Contributors: the `created_by` field is checked against the authenticated user's ID.
  If they don't match, `403 Forbidden` is returned with a distinct message.
- Unrecognised fields in the request body cause a `422 Unprocessable Entity` response
  (strict mode -- rejects typos and unknown fields for defence-in-depth).
- The test case must belong to the specified project; if it belongs to a different
  project, `404 Not Found` is returned.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:update` permission, is a Viewer, or (for Contributors) does not own the test case |
| `404` | `NOT_FOUND` | Project or test case does not exist, is soft-deleted, or test case belongs to a different project |
| `409` | `DUPLICATE_TEST_CASE_SUMMARY` | Updated summary conflicts with another test case in the same project |
| `422` | `VALIDATION_ERROR` | No fields provided, invalid field values |
| `422` | `INVALID_CATEGORY` | `category_id` does not exist, is soft-deleted, or belongs to a different project |
| `422` | `INVALID_PRIORITY` | `priority_id` does not exist, is soft-deleted, or belongs to a different project |

---

### DELETE `/api/v1/projects/{projectId}/test-cases/{id}`

Soft-delete a test case. Contributors can only delete their own test cases.

**Required Permission:** `test_case:delete` AND project role Contributor (own only), Editor, or Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Test Case ID |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Sets `deleted_at = NOW()` and `deleted_by = <current_user_id>`.
- For Contributors: the `created_by` field is checked against the authenticated user's ID.
  If they don't match, `403 Forbidden` is returned with a distinct message.
- No referential integrity check is performed before soft-deleting a test case. Test cases are soft-delete leaf entities (no cascade needed), but
  `TEST_CASE_FILES` does reference `TEST_CASES` via FK. Files become
  inaccessible when their parent test case is soft-deleted (gated at the
  application query layer).
- Repeated DELETE on an already soft-deleted test case returns `404`.
- The test case must belong to the specified project.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:delete` permission, is a Viewer, or (for Contributors) does not own the test case |
| `404` | `NOT_FOUND` | Project or test case does not exist, is soft-deleted, or belongs to a different project |

---

### GET `/api/v1/projects/{projectId}/test-cases/select`

Return a compact list of all non-deleted test cases for dropdown/selection UI components.

**Required Permission:** `test_case:select`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Success Response:** `200 OK`

```json
{
  "data": [
    { "id": 128, "summary": "Password reset email is sent within 60 seconds" },
    { "id": 85, "summary": "Session expires after 30 minutes of inactivity" },
    { "id": 42, "summary": "User can log in with valid credentials" }
  ]
}
```

**Notes:**
- Unpaginated: returns all non-deleted test cases for the project. The SQL query uses
  `LIMIT 1000`. If the result set has 1000 rows, results are truncated, a warning is
  logged, and an `X-Result-Truncated: true` response header is set so the UI can
  surface this to the user.
- Each entry contains only `id` and `summary` (minimal payload for dropdown rendering).
- Results are ordered by `summary` ascending (case-insensitive, `ORDER BY LOWER(summary)`).
- Soft-deleted test cases are excluded.
- Route registration order matters: the `/select` path must be registered **before**
  the `/{id}` path to prevent `select` from being interpreted as an ID.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:select` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |

---

## Data Model

### New Table: TEST_CASES

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `project_id` | `BIGINT` | `NOT NULL`, `REFERENCES projects(id) ON DELETE RESTRICT` | Each test case belongs to exactly one project |
| `category_id` | `BIGINT` | `REFERENCES test_categories(id) ON DELETE RESTRICT` | Nullable; must belong to same project (enforced at app layer) |
| `priority_id` | `BIGINT` | `REFERENCES test_priorities(id) ON DELETE RESTRICT` | Nullable; must belong to same project (enforced at app layer) |
| `summary` | `VARCHAR(500)` | `NOT NULL` | Test case summary; unique per project (case-insensitive); see constraint below |
| `automated` | `BOOLEAN` | `NOT NULL`, `DEFAULT false` | Whether the test case is automated |
| `description` | `TEXT` | | Nullable; max 10000 chars enforced at app layer and by `CHECK` constraint below |
| `notes` | `TEXT` | | Nullable; max 5000 chars enforced at app layer and by `CHECK` constraint below |
| `created_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who created the test case |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Last user who modified the test case |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by `BEFORE UPDATE` trigger |
| `deleted_by` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Nullable; set on soft-delete |
| `deleted_at` | `TIMESTAMPTZ` | | Nullable; set on soft-delete |

**Constraints:**

```sql
-- Description length guard (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_cases
  ADD CONSTRAINT chk_test_cases_description
  CHECK (description IS NULL OR char_length(description) <= 10000);

-- Notes length guard (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_cases
  ADD CONSTRAINT chk_test_cases_notes
  CHECK (notes IS NULL OR char_length(notes) <= 5000);

-- Case-insensitive unique summary per project (PostgreSQL)
-- Only enforced for non-deleted rows, allowing a soft-deleted row and a new row
-- with the same summary to coexist.
CREATE UNIQUE INDEX uq_test_cases_summary_project
  ON test_cases (project_id, LOWER(summary))
  WHERE deleted_at IS NULL;

-- Foreign key indexes
CREATE INDEX idx_test_cases_project_id ON test_cases (project_id);
CREATE INDEX idx_test_cases_category_id ON test_cases (category_id);
CREATE INDEX idx_test_cases_priority_id ON test_cases (priority_id);
CREATE INDEX idx_test_cases_created_by ON test_cases (created_by);
CREATE INDEX idx_test_cases_updated_by ON test_cases (updated_by);
CREATE INDEX idx_test_cases_deleted_by ON test_cases (deleted_by);

-- Composite index for common filter + sort pattern (list with category filter)
CREATE INDEX idx_test_cases_project_category ON test_cases (project_id, category_id)
  WHERE deleted_at IS NULL;

-- Composite index for common filter + sort pattern (list with priority filter)
CREATE INDEX idx_test_cases_project_priority ON test_cases (project_id, priority_id)
  WHERE deleted_at IS NULL;

-- Partial index for active test cases (most queries filter out soft-deleted)
CREATE INDEX idx_test_cases_active ON test_cases (project_id, id DESC)
  WHERE deleted_at IS NULL;
```

**Design notes:**

- **Partial unique index** `uq_test_cases_summary_project` enforces summary uniqueness
  only among non-deleted rows. This allows creating a new test case with the same summary
  as a previously soft-deleted one.
- **`project_id` FK with `RESTRICT`** prevents deleting a project that has test cases.
  Project soft-delete does not cascade (test cases remain with their original
  `deleted_at IS NULL`; they are hidden because queries filter on
  `WHERE project.deleted_at IS NULL`).
- **`ON DELETE RESTRICT` on user FKs** (`created_by`, `updated_by`, `deleted_by`)
  prevents deleting a user who has created, updated, or deleted test cases. This
  preserves audit trail integrity.
- **`category_id` FK with `RESTRICT`** prevents hard-deleting a category that is
  referenced by test cases. The category soft-delete is already prevented by the
  `CATEGORY_IN_USE` check in the categories feature, making this a defence-in-depth
  measure.
- **`priority_id` FK with `RESTRICT`** prevents hard-deleting a priority that is
  referenced by test cases.
- **`deleted_at` and `deleted_by`** follow the project-wide soft-delete convention:
  both set together on soft-delete, both `NULL` for active records.
- **`summary` as the unique field** instead of a separate `name` column. The summary
  serves as both the human-readable identifier and the unique constraint target, which
  is the domain convention for test cases (they are identified by what they test, not
  by a short label).

### BEFORE UPDATE Trigger

```sql
CREATE OR REPLACE FUNCTION trg_test_cases_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_test_cases_updated_at
  BEFORE UPDATE ON test_cases
  FOR EACH ROW
  EXECUTE FUNCTION trg_test_cases_updated_at();
```

The trigger ensures `updated_at` is always set to the current timestamp on every `UPDATE`,
regardless of which code path performs the update. The application layer does not need to
set `updated_at` explicitly.

### Relationship to Other Tables

```
PROJECTS ──< TEST_CASES >── TEST_CATEGORIES
                │
                └── TEST_PRIORITIES
```

- `TEST_CASES.project_id` -> `PROJECTS.id` (each test case belongs to one project)
- `TEST_CASES.category_id` -> `TEST_CATEGORIES.id` (optional reference; must belong to same project)
- `TEST_CASES.priority_id` -> `TEST_PRIORITIES.id` (optional reference; must belong to same project)

---

## Sequence

### Create Test Case Flow

1. Client sends `POST /api/v1/projects/{projectId}/test-cases` with
   `{"summary": "...", "category_id": 3, ...}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler deserializes and validates the request body (summary required, lengths,
   boolean for automated, strict mode for unrecognised fields).
4. Handler calls `TestCaseService::create_test_case(project_id, cmd, current_user_id)`.
5. `TestCaseService` checks the user has `test_case:create` system permission
   (via `AuthorizationService`).
6. `TestCaseService` checks the user is a Contributor, Editor, or Owner of the project
   (via `ProjectMemberRepository`). If Viewer or not a member, and not System Admin ->
   `403`.
7. `TestCaseService` begins a database transaction.
8. Within the transaction:
   a. `SELECT ... FOR UPDATE` on the project row to verify it exists and is not
      soft-deleted. If not found or soft-deleted -> roll back and return `404 Not Found`.
   b. Check for duplicate summary: `TestCaseRepository::find_by_summary_in_project(project_id, summary)`.
      If a non-deleted duplicate exists -> roll back and return `409 Conflict`.
   c. If `category_id` is provided and non-null: validate the category exists, is
      non-deleted, and belongs to the same project. If invalid -> roll back and return
      `422` with `INVALID_CATEGORY`.
   d. If `priority_id` is provided and non-null: validate the priority exists, is
      non-deleted, and belongs to the same project. If invalid -> roll back and return
      `422` with `INVALID_PRIORITY`.
   e. Construct a `TestCase` entity and call `TestCaseRepository::save(test_case)`.
      If the INSERT fails with a PostgreSQL duplicate key violation (error 23505), catch
      it and return `409 Conflict` with `DUPLICATE_TEST_CASE_SUMMARY` as a fallback for
      concurrent inserts that bypass the SELECT check.
9. Transaction commits.
10. Handler constructs the `Location` header from the new test case ID and returns
    `201 Created`.

### Update Test Case Flow

1. Client sends `PATCH /api/v1/projects/{projectId}/test-cases/{id}` with
   `{"summary": "...", "automated": false}` and session cookie.
2-4. Same as Create: validate session, project exists, test case exists and belongs
     to project and is not soft-deleted.
5. Handler calls `TestCaseService::update_test_case(project_id, test_case_id, cmd, current_user_id)`.
6. `TestCaseService` checks `test_case:update` system permission.
7. `TestCaseService` checks the user is at least a Contributor of the project. If
   Viewer or not a member, and not System Admin -> `403`.
8. `TestCaseService` checks if the user is a Contributor: if so, verify
   `test_case.created_by == current_user_id`. If not matching -> `403` (ownership
   restriction). Owners and Editors skip this check.
9. `TestCaseService` begins a database transaction.
10. Within the transaction:
    a. If `summary` is provided and differs from current: check for duplicate summary
       via `TestCaseRepository::find_by_summary_in_project`. If a different test case has
       the same summary -> roll back and return `409 Conflict`.
    b. If `category_id` is provided: validate (if non-null) or clear (if null). Same
       validation as create.
    c. If `priority_id` is provided: validate (if non-null) or clear (if null). Same
       validation as create.
    d. Apply updates via `test_case.apply_update(cmd)` and save via repository.
11. Transaction commits.
12. Handler returns `200 OK` with the updated test case.

### Soft-Delete Test Case Flow

1. Client sends `DELETE /api/v1/projects/{projectId}/test-cases/{id}` with session
   cookie.
2-4. Same as Create: validate session, project exists, test case exists and belongs
     to project and is not soft-deleted.
5. Handler calls `TestCaseService::delete_test_case(project_id, test_case_id, current_user_id)`.
6. `TestCaseService` checks `test_case:delete` system permission.
7. `TestCaseService` checks the user is at least a Contributor of the project. If
   Viewer or not a member, and not System Admin -> `403`.
8. `TestCaseService` checks if the user is a Contributor: if so, verify
   `test_case.created_by == current_user_id`. If not matching -> `403` (ownership
   restriction).
9. `TestCaseService` calls `TestCaseRepository::soft_delete(test_case_id, current_user_id)`
   which sets `deleted_at = NOW()`, `deleted_by = current_user_id`.
10. Handler returns `204 No Content`.

### List Test Cases Flow

1. Client sends `GET /api/v1/projects/{projectId}/test-cases?page=1&limit=25&category_id=3&search=login&sort=-id`
   with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler validates the project exists and is not soft-deleted.
4. Handler calls `TestCaseService::list_test_cases(project_id, query, current_user_id)`.
5. `TestCaseService` checks `test_case:read_list` system permission.
6. `TestCaseService` checks the user is a member of the project (any role) or System Admin.
7. `TestCaseService` calls `TestCaseRepository::find_by_project(project_id, page, limit, filters, search, sort)`.
8. Repository executes a parameterized query with `WHERE project_id = $1 AND deleted_at IS NULL`,
   optional `category_id`/`priority_id`/`automated` filters, `ILIKE` filters on
   summary/description if `search` is provided, and `ORDER BY` based on `sort`.
9. Repository returns the paginated results and total count.
10. `TestCaseService` returns the response DTO with `data` and `meta`.
11. Handler returns `200 OK`.

### Select Test Cases Flow

1. Client sends `GET /api/v1/projects/{projectId}/test-cases/select` with session cookie.
2-3. Same as List: validate session and project.
4. Handler calls `TestCaseService::select_test_cases(project_id, current_user_id)`.
5. `TestCaseService` checks `test_case:select` system permission.
6. `TestCaseService` checks the user is a project member (any role) or System Admin.
7. `TestCaseService` calls `TestCaseRepository::find_all_active_by_project(project_id)`
   which selects only `id` and `summary`, ordered by `LOWER(summary)`.
8. Handler returns `200 OK` with the flat array.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestCase` | Domain (1) | Entity: `id`, `project_id`, `category_id`, `priority_id`, `summary`, `automated`, `description`, `notes`, audit fields. Factory method `create(project_id, summary, category_id, priority_id, automated, description, notes, created_by)` performs domain validation (summary not empty, description/notes length, automated must be boolean). Method `apply_update(cmd)` returns a modified entity with changed fields validated. No ORM or framework imports. |
| `TestCaseService` | Application (2) | Orchestrates all test case use cases: `create_test_case`, `list_test_cases`, `get_test_case`, `update_test_case`, `delete_test_case`, `select_test_cases`. Each method checks the required system permission, project membership, and (for Contributor update/delete) ownership. Then delegates to the repository. |
| `TestCaseRepository` | Application (2) | Interface (port): `find_by_id(project_id, test_case_id)`, `find_by_summary_in_project(project_id, summary)`, `find_by_project(project_id, page, limit, filters, search, sort)`, `find_all_active_by_project(project_id)`, `save(test_case)`, `update(test_case)`, `soft_delete(test_case_id, deleted_by)`. Also `validate_category_in_project(category_id, project_id)` and `validate_priority_in_project(priority_id, project_id)` for FK validation. |
| `TestCaseHandler` | Adapters (3) | HTTP handler with six methods (`list`, `create`, `get`, `update`, `delete`, `select`). Deserializes requests, calls `TestCaseService`, serializes responses. The select handler is registered before the `/{id}` handler. |
| `SqlTestCaseRepository` | Infrastructure (4) | Implements `TestCaseRepository` using PostgreSQL. All queries include `WHERE deleted_at IS NULL` for active record filtering. Uses parameterized queries exclusively. Resolves `category_name` and `priority_name` via LEFT JOIN on detail queries. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (Application) | Add permission codes `test_case:create`, `test_case:read`, `test_case:read_list`, `test_case:update`, `test_case:delete`, `test_case:select` to the permission registry. Add role-based checks: Contributor/Editor/Owner can create; all members can read/read_list/select; Contributor (own only)/Editor/Owner can update/delete; Viewer is excluded from all mutations. |
| Seed migration (Infrastructure, from `iam-permissions`) | Add 6 new permission rows for `test_case:*` codes. |
| HTTP router registration | Register six new routes under `/api/v1/projects/{projectId}/test-cases/` -- all require session auth. Route order: `/select` before `/{id}`. |

---

## Route Registration

```text
# Order matters: /select must be registered before /{id}
GET    /api/v1/projects/{projectId}/test-cases           -> list
POST   /api/v1/projects/{projectId}/test-cases           -> create
GET    /api/v1/projects/{projectId}/test-cases/select    -> select   (static path)
GET    /api/v1/projects/{projectId}/test-cases/{id}      -> get      (dynamic path)
PATCH  /api/v1/projects/{projectId}/test-cases/{id}      -> update
DELETE /api/v1/projects/{projectId}/test-cases/{id}      -> delete
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- System permission check (respective `test_case:*` code)
- Project membership check:
  - `POST`: Contributor, Editor, or Owner
  - `PATCH`, `DELETE`: Contributor (own only), Editor, or Owner
  - `GET` (all read endpoints): any project role
- Contributor ownership check (on `PATCH` and `DELETE` only)

---

## New Permission Codes

These must be added to the `PERMISSIONS` table seed data. Use `INSERT ... ON CONFLICT (code)
DO NOTHING` and let the database assign IDs -- do not hardcode numeric IDs in the migration:

| code | name |
|------|------|
| `test_case:create` | Create Test Case |
| `test_case:read` | Read Test Case |
| `test_case:read_list` | Read Test Case List |
| `test_case:update` | Update Test Case |
| `test_case:delete` | Delete Test Case |
| `test_case:select` | Select Test Case |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks required system permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks required project role | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Contributor does not own the test case | `403` | `FORBIDDEN` | INFO | Distinct message: "Contributors can only update/delete their own test cases" |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before any test case operation |
| Test case not found, soft-deleted, or wrong project | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Duplicate test case summary in project | `409` | `DUPLICATE_TEST_CASE_SUMMARY` | INFO | Case-insensitive; only among non-deleted rows |
| Category does not exist or wrong project | `422` | `INVALID_CATEGORY` | INFO | Checked during create and update |
| Priority does not exist or wrong project | `422` | `INVALID_PRIORITY` | INFO | Checked during create and update |
| Validation error (bad request body) | `422` | `VALIDATION_ERROR` | INFO | Includes field-level details; unrecognised fields rejected in strict mode |
| Invalid pagination, sort parameter, or filter value | `422` | `VALIDATION_ERROR` | INFO | page < 1 or page > 1000, limit < 1 or limit > 100, invalid sort, invalid automated filter, search > 255 chars, non-integer page/limit |
| DB duplicate key violation (race condition) | `409` | `DUPLICATE_TEST_CASE_SUMMARY` | INFO | Caught from PostgreSQL error 23505; mapped to same 409 response as application-level check |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Per-endpoint limits; returns `Retry-After` header |
| Whitespace-only summary | `422` | `VALIDATION_ERROR` | INFO | Summary must contain at least one non-whitespace character |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not reveal** whether a `404` is caused by a non-existent project, non-existent
  test case, soft-deleted test case, or wrong-project test case (same message for all).
- **Do not return `400`** for business logic errors like duplicate summary, invalid
  category, or invalid priority -- use `409 Conflict` or `422 Unprocessable Entity` as
  specified.
- **Do not hard-delete** any record -- all deletions are soft-deletes.
- **Do not allow UPDATE on soft-deleted rows** -- the repository layer checks
  `deleted_at IS NULL` before applying mutations.
- **Do not omit `deleted_by`** -- every soft-delete sets both `deleted_at` and
  `deleted_by`.
- **Do not check for referential integrity on test case delete** -- test cases are soft-delete leaf entities (no cascade needed), but
  `TEST_CASE_FILES` does reference `TEST_CASES` via FK. Files become
  inaccessible when their parent test case is soft-deleted (gated at the
  application query layer).
- **Do not use `GET` with a body** -- all state changes use `POST`, `PATCH`, or `DELETE`.
- **Do not allow cross-project FK injection** -- category and priority FK validation
  always checks `project_id` matches the test case's project.
