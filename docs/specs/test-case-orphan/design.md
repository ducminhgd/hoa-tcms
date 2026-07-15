# Design: Orphan Test Case

## Architecture

The Orphan Test Case feature extends the existing Test Case CRUD feature. Orphan test
cases use the same `TEST_CASES` table as regular project-scoped test cases, with
`project_id` set to `NULL`. A new `TEST_CASE_SHARES` junction table controls visibility
for non-creator users. The feature introduces its own endpoints under
`/api/v1/test-cases/orphaned` (no project path prefix) with a distinct authorization
model based on creator ownership and explicit sharing rather than project membership.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_orphaned         GET    /api/v1/test-cases/orphaned           │   │
│  │  - create_orphaned       POST   /api/v1/test-cases/orphaned           │   │
│  │  - get_orphaned          GET    /api/v1/test-cases/orphaned/{id}      │   │
│  │  - update_orphaned       PATCH  /api/v1/test-cases/orphaned/{id}      │   │
│  │  - delete_orphaned       DELETE /api/v1/test-cases/orphaned/{id}      │   │
│  │  - list_shares           GET    /api/v1/test-cases/orphaned/{id}/shares│   │
│  │  - add_shares            POST   /api/v1/test-cases/orphaned/{id}/shares│   │
│  │  - remove_share          DELETE /api/v1/test-cases/orphaned/{id}/shares│   │
│  │                                                                       │   │
│  │  /{userId}                                                             │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  OrphanTestCaseService:                   │  │  - TestCase (entity)    │  │
│  │  - create_orphaned_test_case              │  │    (reused, project_id  │  │
│  │  - list_orphaned_test_cases               │  │     can be null)        │  │
│  │  - get_orphaned_test_case                 │  │  - TestCaseShare        │  │
│  │  - update_orphaned_test_case              │  │    (entity)             │  │
│  │  - delete_orphaned_test_case              │  └──────────────────────────┘  │
│  │  - assign_to_project                      │                                │
│  │  - list_shares                            │                                │
│  │  - add_shares                             │                                │
│  │  - remove_share                           │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - OrphanTestCaseRepository (port)        │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlOrphanTestCaseRepository (implements OrphanTestCaseRepository)   │   │
│  │  - Migrations: project_id nullable + TEST_CASE_SHARES table            │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All orphan test case endpoints require an authenticated session (checked by
   `AuthMiddleware`).
2. `POST` endpoints require the `test_case:create` system permission. There is no
   project membership check because the test case is not in a project.
3. `PATCH` and `DELETE` endpoints require the `test_case:update` / `test_case:delete`
   system permission AND creator ownership (`created_by == current_user_id`). System
   Admin bypasses the ownership check.
4. `GET` endpoints (list, detail, shares) require the `test_case:read_orphaned`
   system permission AND creator ownership or sharing. System Admin bypasses both.
5. The assign-to-project operation (triggered when `project_id` is included in a
   `PATCH` request) adds a target project membership check: the creator must be a
   Contributor, Editor, or Owner of the target project (or be a System Admin).
6. Create validates unique summary among the user's own orphan test cases (case-insensitive).
7. Update validates unique summary (if summary is being changed) among the user's own
   orphan test cases. Rejects updates on soft-deleted records.
8. Soft-delete requires no referential integrity checks (test cases are leaf entities).
9. Sharing operations (`POST`/`DELETE` on shares) require the `test_case:share` system
   permission AND creator ownership.
10. Shared users have read-only access -- they cannot update, delete, share, or assign
    the test case.

### Security Requirements

**Input sanitization:** Same as project test cases. `summary`, `description`, and `notes`
fields must be sanitized on input (strip disallowed HTML tags) before storage.
Output-encoding must be applied at the presentation layer.

**CSRF protection:** All state-changing endpoints must be protected with the same
`SameSite=Lax` cookies and `Content-Type` verification as project test case endpoints.

**Authorization layering:** Two independent authorization gates apply:

1. System permission check (e.g., `test_case:update`)
2. Creator ownership check: the authenticated user must be the creator (`created_by`)
   of the orphan test case for mutations (update, delete, share). For reads, the user
   must be either the creator or a shared user.

System Admin implicitly holds all system permissions and bypasses the creator ownership
check. Authorization checks are performed against live data on every request.

**Visibility masking:** If a user requests an orphan test case they are neither the
creator of nor shared with, the response must be `404 Not Found` with the same generic
message used for non-existent or soft-deleted test cases. Do not reveal whether the
orphan test case exists.

**Rate limiting:** All endpoints are protected by rate limiting. See requirements.md
Security Considerations for the full table.

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

### POST `/api/v1/test-cases/orphaned`

Create a new orphan test case (no project context).

**Required Permission:** `test_case:create`

**Request Body:**

```json
{
  "summary": "User can log in with valid credentials",
  "automated": true,
  "description": "Given a registered user with valid credentials, when they submit the login form, then they are redirected to the dashboard.",
  "notes": "This is a critical path test. Ensure the test database has a seeded user."
}
```

| Field | Type | Required | Default | Constraints |
|-------|------|----------|---------|-------------|
| `summary` | string | Yes | -- | 1-500 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input (HTML stripped); unique per creator among orphan test cases (case-insensitive) |
| `automated` | boolean | No | `false` | `true` or `false` only |
| `description` | string | No | `null` | Max 10000 characters; sanitized on input (HTML stripped). Empty string `""` stored as-is. Explicit `null` stores `NULL`. Omitting defaults to `null`. |
| `notes` | string | No | `null` | Max 5000 characters; sanitized on input (HTML stripped). Empty string `""` stored as-is. Explicit `null` stores `NULL`. Omitting defaults to `null`. |

**Note:** `category_id` and `priority_id` are NOT accepted. Orphan test cases have no
project context for FK validation. These fields can only be set during the
assign-to-project operation.

**Success Response:** `201 Created`

Headers: `Location: /api/v1/test-cases/orphaned/128`

```json
{
  "data": {
    "id": 128,
    "summary": "User can log in with valid credentials",
    "description": "Given a registered user with valid credentials, when they submit the login form, then they are redirected to the dashboard.",
    "notes": "This is a critical path test. Ensure the test database has a seeded user.",
    "automated": true,
    "project_id": null,
    "category_id": null,
    "category_name": null,
    "priority_id": null,
    "priority_name": null,
    "created_by": 15,
    "created_at": "2026-07-15T10:00:00Z",
    "updated_by": 15,
    "updated_at": "2026-07-15T10:00:00Z",
    "shared": false
  }
}
```

**Notes:**
- `project_id` is always `null` for orphan test cases.
- `category_id`, `category_name`, `priority_id`, `priority_name` are always `null`
  for newly created orphan test cases.
- `shared` is `false` for the creator (the creator is not a "shared" user).
- `created_by` and `updated_by` are both set to the authenticated user's ID.
- The unique summary check uses `LOWER(summary) = LOWER($1)` and includes
  `WHERE created_by = $2 AND project_id IS NULL AND deleted_at IS NULL`.
- Soft-deleted orphan test cases with the same summary do not block creation.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:create` permission |
| `409` | `DUPLICATE_TEST_CASE_SUMMARY` | Test case summary already exists among user's orphan test cases (case-insensitive) |
| `422` | `VALIDATION_ERROR` | Summary empty, exceeds max length; description/notes too long; invalid boolean for automated; `category_id` or `priority_id` provided |
| `429` | `RATE_LIMITED` | Rate limit exceeded |

---

### GET `/api/v1/test-cases/orphaned`

List orphan test cases visible to the authenticated user (own + shared with them).

**Required Permission:** `test_case:read_orphaned`

**Query Parameters:**

| Param | Type | Default | Max | Description |
|-------|------|---------|-----|-------------|
| `page` | integer | 1 | -- | Page number (1-based) |
| `limit` | integer | 25 | 100 | Items per page |
| `automated` | boolean | -- | -- | Filter by automation status (`true` or `false`) |
| `search` | string | -- | 255 | Case-insensitive substring match on `summary` and `description` |
| `sort` | string | `-id` | -- | Sort field: `id`, `-id`, `summary`, `-summary`, `created_at`, `-created_at` |
| `ownership` | string | -- | -- | `mine` (created by me) or `shared` (shared with me). Omitted = both. |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 42,
      "summary": "User can log in with valid credentials",
      "automated": true,
      "created_by": 15,
      "created_at": "2026-07-15T10:00:00Z",
      "updated_at": "2026-07-15T10:00:00Z",
      "shared": false
    },
    {
      "id": 55,
      "summary": "Password reset email is sent within 60 seconds",
      "automated": false,
      "created_by": 22,
      "created_at": "2026-07-14T09:30:00Z",
      "updated_at": "2026-07-15T08:00:00Z",
      "shared": true
    }
  ],
  "meta": {
    "total": 12,
    "page": 1,
    "limit": 25
  }
}
```

**Notes:**
- Results are scoped to orphan test cases (`project_id IS NULL`) and exclude soft-deleted
  test cases (`deleted_at IS NULL`).
- Visibility filter: `created_by = <user_id>` OR the user is in `TEST_CASE_SHARES` for
  that test case.
- Default sort is `-id` (newest first, descending).
- `search` applies `ILIKE` on both `summary` and `description`.
- `ownership=mine` filters to `created_by = <user_id>`.
- `ownership=shared` filters to test cases where the user is in `TEST_CASE_SHARES`.
- `shared` is `true` when the authenticated user is in `TEST_CASE_SHARES` for that test
  case, `false` when the user is the creator.
- List response excludes `description`, `notes`, `project_id`, `category_id`,
  `priority_id`, `category_name`, `priority_name`, `updated_by`, `deleted_at`, and
  `deleted_by` to keep the payload compact.
- System Admin sees all non-deleted orphan test cases regardless of ownership or sharing.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:read_orphaned` permission |
| `422` | `VALIDATION_ERROR` | Invalid pagination, sort parameter, filter value, or ownership value |

---

### GET `/api/v1/test-cases/orphaned/{id}`

Get full detail of a single orphan test case.

**Required Permission:** `test_case:read_orphaned`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
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
    "project_id": null,
    "category_id": null,
    "category_name": null,
    "priority_id": null,
    "priority_name": null,
    "created_by": 15,
    "created_at": "2026-07-15T10:00:00Z",
    "updated_by": 15,
    "updated_at": "2026-07-15T10:00:00Z",
    "shared": false
  }
}
```

**Notes:**
- The test case must be an orphan (`project_id IS NULL`).
- The user must be the creator OR a shared user (or System Admin).
- If the user is neither the creator nor a shared user, `404 Not Found` is returned
  (same message as non-existent or soft-deleted, to avoid information leakage).
- `category_name` is resolved via LEFT JOIN on `test_categories`. For orphan test cases
  this is expected to be `null`, but the JOIN handles any value defensively.
- `priority_name` is resolved via LEFT JOIN on `test_priorities`. Same defensive handling.
- `deleted_at` and `deleted_by` are never returned to the client.
- `shared` is `true` when the authenticated user is in `TEST_CASE_SHARES` for this test
  case, `false` when the user is the creator.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:read_orphaned` permission |
| `404` | `NOT_FOUND` | Test case does not exist, is soft-deleted, or user has no visibility (neither creator nor shared) |

---

### PATCH `/api/v1/test-cases/orphaned/{id}`

Update an orphan test case's fields. Only the creator can update. If `project_id` is
included, the request is treated as an assign-to-project operation (see below).

**Required Permission:** `test_case:update` AND creator ownership

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Case ID |

**Request Body -- Regular Update** (all fields optional; at least one required):

```json
{
  "summary": "Updated summary",
  "automated": false,
  "description": "Updated description",
  "notes": "Updated notes"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `summary` | string | No | 1-500 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input (HTML stripped); unique per creator among orphan test cases (case-insensitive) |
| `automated` | boolean | No | `true` or `false` only |
| `description` | string | No | Max 10000 characters; sanitized on input. Omitting preserves current. `null` clears. `""` stores empty string. |
| `notes` | string | No | Max 5000 characters; sanitized on input. Omitting preserves current. `null` clears. `""` stores empty string. |

**Note:** `category_id` and `priority_id` are NOT accepted in regular update requests.

**Request Body -- Assign to Project** (must include `project_id`):

```json
{
  "project_id": 42,
  "category_id": 3,
  "priority_id": 1
}
```

When `project_id` is present in the request body, the endpoint treats the request as an
assign-to-project operation. `category_id` and `priority_id` MAY be included in the same
request and are validated against the target project. `summary`, `automated`,
`description`, and `notes` MAY also be updated in the same request.

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `project_id` | integer | Yes | Must reference a non-deleted project. The caller must be a Contributor, Editor, or Owner of this project (or System Admin). |
| `category_id` | integer | No | Must reference a non-deleted category in the target project. Sending `null` explicitly clears. |
| `priority_id` | integer | No | Must reference a non-deleted priority in the target project. Sending `null` explicitly clears. |
| `summary` | string | No | Same constraints as regular update; uniqueness checked against the target project. |
| `automated` | boolean | No | Same as regular update. |
| `description` | string | No | Same as regular update. |
| `notes` | string | No | Same as regular update. |

**Success Response -- Regular Update:** `200 OK`

Response body is the updated test case representation (same shape as GET detail).

**Success Response -- Assign to Project:** `200 OK`

Headers: `Location: /api/v1/projects/42/test-cases/128`

Response body is the updated test case representation with `project_id` set to the new
project and resolved `category_name` / `priority_name` if provided.

**Notes:**
- For regular updates: at least one field must be provided (empty body returns `422`).
- For assign-to-project: `project_id` is the only required field. All other fields are
  optional and preserve their current values if omitted.
- `id`, `created_at`, and `created_by` are immutable.
- `deleted_at` and `deleted_by` cannot be modified through this endpoint.
- `updated_at` and `updated_by` are set automatically on change.
- The unique summary check for regular updates is only performed if `summary` is provided
  and differs from the current value, and checks among the user's orphan test cases.
- The unique summary check for assign-to-project checks among the target project's test
  cases (using the existing project-scoped uniqueness logic).
- For assign-to-project: orphan sharing rows are deleted within the same transaction after
  the test case is updated.
- After successful assignment, the `Location` header points to the new project-scoped URL.
- Soft-deleted orphan test cases cannot be updated.
- For the assign-to-project operation, the user must also be the creator of the orphan
  test case (or a System Admin).
- Unrecognised fields in the request body cause a `422 Unprocessable Entity` response
  (strict mode).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:update` permission or is not the creator (regular update) |
| `403` | `FORBIDDEN` | User lacks `test_case:update` permission, is not the creator, or is not a Contributor/Editor/Owner of the target project (assign-to-project) |
| `404` | `NOT_FOUND` | Test case does not exist or is soft-deleted |
| `404` | `NOT_FOUND` | Target project does not exist or is soft-deleted (assign-to-project) |
| `409` | `DUPLICATE_TEST_CASE_SUMMARY` | Updated summary conflicts with another test case (among user's orphans for regular update; in target project for assign-to-project) |
| `422` | `VALIDATION_ERROR` | No fields provided, invalid field values, `category_id`/`priority_id` provided in regular update |
| `422` | `INVALID_CATEGORY` | `category_id` does not exist, is soft-deleted, or belongs to a different project (assign-to-project) |
| `422` | `INVALID_PRIORITY` | `priority_id` does not exist, is soft-deleted, or belongs to a different project (assign-to-project) |

---

### DELETE `/api/v1/test-cases/orphaned/{id}`

Soft-delete an orphan test case. Only the creator can delete.

**Required Permission:** `test_case:delete` AND creator ownership

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Case ID |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Sets `deleted_at = NOW()` and `deleted_by = <current_user_id>`.
- Only the creator can delete (shared users cannot).
- No referential integrity check is performed before soft-deleting.
- Repeated DELETE on an already soft-deleted test case returns `404`.
- `TEST_CASE_SHARES` rows are NOT deleted on soft-delete (preserved for audit trail;
  they are naturally excluded by queries that filter on `test_cases.deleted_at IS NULL`).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:delete` permission or is not the creator |
| `404` | `NOT_FOUND` | Test case does not exist or is already soft-deleted |

---

### GET `/api/v1/test-cases/orphaned/{id}/shares`

List the users with whom an orphan test case is shared.

**Required Permission:** `test_case:read_orphaned` AND creator ownership or sharing

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Case ID |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 22,
      "username": "jane.doe",
      "email": "jane.doe@example.com",
      "shared_at": "2026-07-15T10:30:00Z"
    },
    {
      "id": 33,
      "username": "bob.smith",
      "email": "bob.smith@example.com",
      "shared_at": "2026-07-15T10:31:00Z"
    }
  ]
}
```

**Notes:**
- Only the creator and users who are themselves shared can view the share list.
- System Admin can view shares for any orphan test case.
- `shared_at` is the `created_at` timestamp of the share row.
- Results are ordered by `shared_at` ascending (oldest share first).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:read_orphaned` permission |
| `404` | `NOT_FOUND` | Test case does not exist, is soft-deleted, or user has no visibility |

---

### POST `/api/v1/test-cases/orphaned/{id}/shares`

Add one or more share entries for an orphan test case. Only the creator can share.

**Required Permission:** `test_case:share` AND creator ownership

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Case ID |

**Request Body:**

```json
{
  "user_ids": [22, 33, 44]
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `user_ids` | array of integer | Yes | Non-empty array of user IDs to share with. Each ID must reference an existing, ACTIVE user. |

**Success Response:** `200 OK`

Response body is the updated share list (same shape as GET shares).

**Notes:**
- Only the creator can share (shared users cannot re-share).
- Users already shared are silently skipped (idempotent -- no duplicate rows).
- The creator's own user ID is silently skipped.
- Users must be ACTIVE; referencing INACTIVE users returns `422` with details.
- If `user_ids` is empty, returns the current share list (no-op).
- The test case must still be an orphan (`project_id IS NULL`). If it has been assigned
  to a project, return `422 Unprocessable Entity` with an appropriate message.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:share` permission or is not the creator |
| `404` | `NOT_FOUND` | Test case does not exist or is soft-deleted |
| `422` | `VALIDATION_ERROR` | `user_ids` is missing, empty array, or contains invalid user IDs |
| `422` | `INVALID_USER` | One or more user IDs reference non-existent or INACTIVE users |
| `422` | `ALREADY_ASSIGNED` | Test case has already been assigned to a project |

---

### DELETE `/api/v1/test-cases/orphaned/{id}/shares/{userId}`

Remove a share entry for a specific user. Only the creator can remove shares.

**Required Permission:** `test_case:share` AND creator ownership

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Case ID |
| `userId` | integer | User ID to unshare |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Only the creator can remove shares.
- If the share does not exist, return `404 Not Found`.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_case:share` permission or is not the creator |
| `404` | `NOT_FOUND` | Test case does not exist, is soft-deleted, or the share does not exist |

---

## Data Model

### Existing Table Change: TEST_CASES

The `project_id` column must be altered from `NOT NULL` to nullable to accommodate orphan
test cases.

**Migration:**

```sql
ALTER TABLE test_cases
  ALTER COLUMN project_id DROP NOT NULL;
```

No other column changes are required. All other columns and constraints remain as defined
in the test-case-crud migration.

**Impact on existing constraints and indexes:**

1. **Partial unique index `uq_test_cases_summary_project`:** This index applies
   `WHERE deleted_at IS NULL`. In PostgreSQL, unique indexes treat all NULL values as
   distinct -- the existing `uq_test_cases_summary_project` index does NOT enforce
   uniqueness for rows with `project_id IS NULL`. This is why a separate index
   (`uq_orphan_test_cases_summary_creator`) is needed to enforce per-creator summary
   uniqueness among orphan test cases (see below).

2. **Composite index `idx_test_cases_project_id`:** Uses `(project_id)` which is valid
   for `NULL` values. No change needed.

3. **Composite partial indexes** (`idx_test_cases_project_category`,
   `idx_test_cases_project_priority`, `idx_test_cases_active`): These all include
   `project_id` in the index key. For orphan test cases, the `NULL` project_id means
   these indexes are less effective for orphan queries, but they do not break. New
   indexes for orphan queries are added below.

### New Partial Unique Index for Orphan Test Cases

```sql
-- Case-insensitive unique summary per creator among orphan test cases
CREATE UNIQUE INDEX uq_orphan_test_cases_summary_creator
  ON test_cases (created_by, LOWER(summary))
  WHERE deleted_at IS NULL AND project_id IS NULL;
```

This ensures that a user cannot have two active orphan test cases with the same summary
(case-insensitive), while allowing different users to have orphan test cases with the
same summary.

### New Indexes for Orphan Queries

```sql
-- Active orphan test cases for a specific creator (list "mine")
CREATE INDEX idx_test_cases_orphan_creator
  ON test_cases (created_by, id DESC)
  WHERE deleted_at IS NULL AND project_id IS NULL;

-- All active orphan test cases (admin bypass list)
CREATE INDEX idx_test_cases_orphan_active
  ON test_cases (id DESC)
  WHERE deleted_at IS NULL AND project_id IS NULL;
```

### New Table: TEST_CASE_SHARES

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `test_case_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_cases(id) ON DELETE CASCADE` | The shared orphan test case |
| `user_id` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE CASCADE` | The user with whom it's shared |
| `shared_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | The user who created the share |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | When the share was created |

**Constraints:**

```sql
CREATE TABLE test_case_shares (
  test_case_id BIGINT NOT NULL REFERENCES test_cases(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  shared_by BIGINT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (test_case_id, user_id)
);

-- Index for "find all shares for a test case"
CREATE INDEX idx_test_case_shares_test_case
  ON test_case_shares (test_case_id);

-- Index for "find all orphan test cases shared with a user"
CREATE INDEX idx_test_case_shares_user
  ON test_case_shares (user_id);
```

**Design notes:**

- **`ON DELETE CASCADE` on `test_case_id`:** When a test case is hard-deleted (which does
  not happen in Phase 1 but the FK is defined defensively), shares are automatically
  removed. Soft-deletes do NOT cascade (the FK constraint is not triggered).
- **`ON DELETE CASCADE` on `user_id`:** If a user account is deleted, their shares are
  automatically removed. This is acceptable because the audit trail for sharing is not
  critical (unlike the test case audit columns which use `RESTRICT`).
- **`ON DELETE RESTRICT` on `shared_by`:** Prevents deleting a user who has shared
  orphan test cases, preserving the sharing audit trail.
- **Composite PK on `(test_case_id, user_id)`:** Enforces that a user can only be shared
  once per test case (no duplicate share rows).
- **No `deleted_at` column:** Sharing is a binary state (shared or not). Removing a share
  deletes the row. There is no concept of a "soft-deleted share."

### Relationship Diagram

```
┌──────────┐       ┌───────────────┐       ┌──────────────────┐
│  USERS   │───<┐  │  TEST_CASES   │  ┌───>│ TEST_CASE_SHARES │
└──────────┘    │  └───────────────┘  │    └──────────────────┘
                │         │           │             │
                │         │ project_id (nullable)   │ user_id
                │         │           │             │
                │    ┌────┘           │    ┌────────┘
                │    │                │    │
                └────┤ (created_by,   └────┤
                     │  updated_by,        │
                     │  deleted_by)        │
                     │                     │
              ┌──────┴──────┐       ┌──────┴──────┐
              │  USERS (FK) │       │  USERS (FK) │
              └─────────────┘       └─────────────┘
```

Orphan test cases are `TEST_CASES` rows with `project_id IS NULL`.
Visibility is determined by `created_by` (creator) or `TEST_CASE_SHARES.user_id` (shared).

---

## Sequence

### Create Orphan Test Case Flow

1. Client sends `POST /api/v1/test-cases/orphaned` with
   `{"summary": "...", "automated": true, ...}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler deserializes and validates the request body (summary required, lengths,
   boolean for automated, strict mode for unrecognised fields, reject `category_id`
   and `priority_id`).
4. Handler calls `OrphanTestCaseService::create_orphaned(cmd, current_user_id)`.
5. `OrphanTestCaseService` checks the user has `test_case:create` system permission
   (via `AuthorizationService`). No project membership check.
6. `OrphanTestCaseService` begins a database transaction.
7. Within the transaction:
   a. Check for duplicate summary:
      `OrphanTestCaseRepository::find_by_summary_for_creator(created_by, summary)`.
      If a non-deleted duplicate exists -> roll back and return `409 Conflict`.
   b. Construct a `TestCase` entity with `project_id = NULL`,
      `category_id = NULL`, `priority_id = NULL`.
   c. Call `OrphanTestCaseRepository::save(test_case)`.
      If the INSERT fails with a PostgreSQL duplicate key violation (error 23505), catch
      it and return `409 Conflict` with `DUPLICATE_TEST_CASE_SUMMARY` as a fallback.
8. Transaction commits.
9. Handler constructs the `Location` header from the new test case ID and returns
   `201 Created`.

### List Orphan Test Cases Flow

1. Client sends `GET /api/v1/test-cases/orphaned?page=1&limit=25&ownership=mine&sort=-id`
   with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler validates query parameters (pagination, sort, ownership).
4. Handler calls `OrphanTestCaseService::list_orphaned(query, current_user_id)`.
5. `OrphanTestCaseService` checks `test_case:read_orphaned` system permission.
6. `OrphanTestCaseService` calls
   `OrphanTestCaseRepository::find_visible_to_user(user_id, page, limit, filters, search, sort)`.
7. Repository executes a parameterized query with:
   - `WHERE project_id IS NULL AND deleted_at IS NULL`
   - Visibility: `(created_by = $1 OR test_case_id IN (SELECT test_case_id FROM test_case_shares WHERE user_id = $1))`
   - Optional `ownership=mine` filter: `AND created_by = $1`
   - Optional `ownership=shared` filter: `AND created_by != $1` (only shared)
   - Optional `search`, `automated` filters, and `sort`
   - `shared` flag computed as `created_by != $1` in the select list
8. Repository returns the paginated results and total count.
9. `OrphanTestCaseService` returns the response DTO with `data` and `meta`.
10. Handler returns `200 OK`.

**System Admin bypass:** If the user is a System Admin, the visibility filter is omitted
  entirely -- all non-deleted orphan test cases are returned.

### Get Orphan Test Case Detail Flow

1. Client sends `GET /api/v1/test-cases/orphaned/{id}` with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler calls `OrphanTestCaseService::get_orphaned(test_case_id, current_user_id)`.
4. `OrphanTestCaseService` checks `test_case:read_orphaned` system permission.
5. `OrphanTestCaseService` calls
   `OrphanTestCaseRepository::find_visible_by_id(test_case_id, user_id)`.

   For non-admin users, the query is:
   ```sql
   SELECT tc.*,
          cat.name AS category_name,
          pri.name AS priority_name,
          EXISTS(
            SELECT 1 FROM test_case_shares tcs
            WHERE tcs.test_case_id = tc.id AND tcs.user_id = $2
          ) AS shared
   FROM test_cases tc
   LEFT JOIN test_categories cat
     ON tc.category_id = cat.id AND cat.deleted_at IS NULL
   LEFT JOIN test_priorities pri
     ON tc.priority_id = pri.id AND pri.deleted_at IS NULL
   WHERE tc.id = $1
     AND tc.project_id IS NULL
     AND tc.deleted_at IS NULL
     AND (tc.created_by = $2 OR EXISTS(
       SELECT 1 FROM test_case_shares tcs2
       WHERE tcs2.test_case_id = tc.id AND tcs2.user_id = $2
     ))
   ```
   For System Admin, the `(tc.created_by = $2 OR EXISTS(...))` clause is omitted.

6. If no row found -> `404 Not Found` (same message whether test case doesn't exist, is
   deleted, or user has no visibility).
7. `OrphanTestCaseService` returns the response DTO.
8. Handler returns `200 OK`.

### Update Orphan Test Case Flow (Regular Update)

1. Client sends `PATCH /api/v1/test-cases/orphaned/{id}` with
   `{"summary": "...", "automated": false}` and session cookie.
2-3. Same as Create: validate session, check `test_case:update` permission.
4. Handler calls `OrphanTestCaseService::update_orphaned(test_case_id, cmd, current_user_id)`.
5. `OrphanTestCaseService` fetches the orphan test case to verify:
   - It exists, is not soft-deleted, is an orphan (`project_id IS NULL`).
   - The user is the creator (`created_by == current_user_id`) or System Admin.
   - If the user is not the creator and not System Admin -> `403 Forbidden`.
6. `OrphanTestCaseService` begins a database transaction.
7. Within the transaction:
   a. If `summary` is provided and differs from current: check duplicate among user's
      orphans via `find_by_summary_for_creator`. If a different test case has the same
      summary -> roll back and return `409 Conflict`.
   b. Apply updates and save via repository.
8. Transaction commits.
9. Handler returns `200 OK` with the updated test case.

### Assign to Project Flow

1. Client sends `PATCH /api/v1/test-cases/orphaned/{id}` with
   `{"project_id": 42, "category_id": 3}` and session cookie.
2-3. Same: validate session, check `test_case:update` permission.
4. Handler detects `project_id` in the request body and calls
   `OrphanTestCaseService::assign_to_project(test_case_id, project_id, cmd, current_user_id)`.
5. `OrphanTestCaseService` fetches the orphan test case to verify:
   - It exists, is not soft-deleted, is an orphan (`project_id IS NULL`).
   - The user is the creator (`created_by == current_user_id`) or System Admin.
6. `OrphanTestCaseService` begins a database transaction.
7. Within the transaction:
   a. `SELECT ... FOR UPDATE` on the target project row to verify it exists and is not
      soft-deleted. This prevents TOCTOU races with concurrent project soft-deletes.
   b. Verify the user is a Contributor, Editor, or Owner of the target project (via
      `ProjectMemberRepository`) or is System Admin.
8. Continue within the transaction:
   a. If `category_id` is provided and non-null: validate the category exists, is
      non-deleted, and belongs to the target project.
   b. If `priority_id` is provided and non-null: validate the priority exists, is
      non-deleted, and belongs to the target project.
   c. If `summary` is provided and differs from current: check duplicate among the target
      project's test cases using the existing project-scoped duplicate check.
   d. Update the test case: set `project_id` to target, apply other field changes,
      update `updated_by` and `updated_at`.
   e. Delete all rows from `TEST_CASE_SHARES` where `test_case_id = <id>`.
   f. If the insert/update fails with unique constraint violation (23505), catch and
      return `409 Conflict`.
9. Transaction commits.
10. Handler constructs the new `Location` header pointing to the project-scoped URL and
    returns `200 OK`.

### Share Orphan Test Case Flow (Add Shares)

1. Client sends `POST /api/v1/test-cases/orphaned/{id}/shares` with
   `{"user_ids": [22, 33]}` and session cookie.
2. `AuthMiddleware` validates the session.
3. Handler calls `OrphanTestCaseService::add_shares(test_case_id, user_ids, current_user_id)`.
4. `OrphanTestCaseService` checks `test_case:share` system permission.
5. `OrphanTestCaseService` fetches the orphan test case to verify:
   - It exists, is not soft-deleted, is an orphan (not assigned to a project).
   - The user is the creator or System Admin. If not -> `403 Forbidden`.
6. `OrphanTestCaseService` validates each `user_id`:
   - Each user must exist and be ACTIVE (via `UserRepository`).
   - Invalid user IDs collected and returned as `422` with details.
7. `OrphanTestCaseService` begins a database transaction.
8. Within the transaction:
   a. For each valid user ID (skipping the creator's own ID and already-shared users):
      insert into `TEST_CASE_SHARES` (`test_case_id`, `user_id`, `shared_by`).
      Use `INSERT ... ON CONFLICT (test_case_id, user_id) DO NOTHING` for idempotency.
9. Transaction commits.
10. Handler returns `200 OK` with the updated share list.

### Soft-Delete Orphan Test Case Flow

1. Client sends `DELETE /api/v1/test-cases/orphaned/{id}` with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler calls `OrphanTestCaseService::delete_orphaned(test_case_id, current_user_id)`.
4. `OrphanTestCaseService` checks `test_case:delete` system permission.
5. `OrphanTestCaseService` fetches the orphan test case to verify:
   - It exists and is not already soft-deleted.
   - It is an orphan (`project_id IS NULL`).
   - The user is the creator or System Admin. If not -> `403 Forbidden`.
6. `OrphanTestCaseService` calls
   `OrphanTestCaseRepository::soft_delete(test_case_id, current_user_id)`
   which sets `deleted_at = NOW()`, `deleted_by = current_user_id`.
7. Handler returns `204 No Content`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `OrphanTestCaseService` | Application (2) | Orchestrates all orphan test case use cases: `create_orphaned`, `list_orphaned`, `get_orphaned`, `update_orphaned`, `delete_orphaned`, `assign_to_project`, `list_shares`, `add_shares`, `remove_share`. Each method checks system permission, creator ownership (for mutations), and sharing visibility (for reads). For assign-to-project, additionally checks target project membership. |
| `OrphanTestCaseRepository` | Application (2) | Interface (port): `find_visible_by_id(test_case_id, user_id)`, `find_visible_to_user(user_id, page, limit, filters, search, sort, ownership)`, `find_by_summary_for_creator(created_by, summary)`, `save(test_case)`, `update(test_case)`, `soft_delete(test_case_id, deleted_by)`, `assign_to_project(test_case_id, project_id, updates)`, `find_shares(test_case_id)`, `add_shares(test_case_id, user_ids, shared_by)`, `remove_share(test_case_id, user_id)`, `delete_all_shares(test_case_id)`. Also `validate_category_in_project(category_id, project_id)` and `validate_priority_in_project(priority_id, project_id)` (reused from TestCaseRepository). All methods accept a transaction context. |
| `OrphanTestCaseHandler` | Adapters (3) | HTTP handler with eight methods (`create`, `list`, `get`, `update`, `delete`, `list_shares`, `add_shares`, `remove_share`). Deserializes requests, calls `OrphanTestCaseService`, serializes responses. The update handler detects `project_id` in the body and routes to the assign-to-project flow. |
| `SqlOrphanTestCaseRepository` | Infrastructure (4) | Implements `OrphanTestCaseRepository` using PostgreSQL. All queries include `WHERE deleted_at IS NULL` for active record filtering and visibility checks (`created_by = $N OR EXISTS(SELECT 1 FROM test_case_shares ...)`). Uses parameterized queries exclusively. Resolves `category_name` and `priority_name` via LEFT JOIN on detail queries. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `TestCase` entity (Domain) | Allow `project_id` to be `None`/`null`. The factory method and `apply_update` must handle `project_id = None`. Validation of `category_id`/`priority_id` is skipped when `project_id` is `None`. |
| `TestCaseRepository` (Application) | No change to the interface. The existing implementation's queries already scope by `project_id`; orphan test cases with `project_id IS NULL` are naturally excluded from project-scoped queries. |
| `SqlTestCaseRepository` (Infrastructure) | No change. The `WHERE project_id = $N` clause naturally excludes orphan test cases (`NULL` never equals any value). |
| `AuthorizationService` (Application) | Add permission codes `test_case:read_orphaned` and `test_case:share` to the permission registry. The existing `test_case:create`, `test_case:update`, `test_case:delete` permissions are reused for both project and orphan test cases. |
| Seed migration (Infrastructure, from `iam-permissions`) | Add 2 new permission rows for `test_case:read_orphaned` and `test_case:share`. The existing 6 `test_case:*` permissions are reused. |
| HTTP router registration | Register 8 new routes under `/api/v1/test-cases/orphaned/` -- all require session auth. Route order: static paths before dynamic paths. |

---

## Route Registration

```text
# Routes under /api/v1/test-cases/orphaned/
# Order matters: static path segments before dynamic {id} segments

POST   /api/v1/test-cases/orphaned                     -> create
GET    /api/v1/test-cases/orphaned                     -> list
GET    /api/v1/test-cases/orphaned/{id}                -> get      (dynamic path)
PATCH  /api/v1/test-cases/orphaned/{id}                -> update   (dynamic path)
DELETE /api/v1/test-cases/orphaned/{id}                -> delete   (dynamic path)
GET    /api/v1/test-cases/orphaned/{id}/shares         -> list shares
POST   /api/v1/test-cases/orphaned/{id}/shares         -> add shares
DELETE /api/v1/test-cases/orphaned/{id}/shares/{userId} -> remove share
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- System permission check (respective `test_case:*` code)
- Creator ownership check (for mutations: update, delete, share)
- Visibility check (for reads: creator OR shared user OR System Admin)
- No project membership checks (except for assign-to-project, which checks target project)

---

## New Permission Codes

These must be added to the `PERMISSIONS` table seed data. The existing `test_case:create`,
`test_case:update`, `test_case:delete` permissions are reused for both project and orphan
test cases:

| code | name |
|------|------|
| `test_case:read_orphaned` | Read Orphan Test Case |
| `test_case:share` | Share Orphan Test Case |

The existing permission codes reused by this feature:

| code | Used For |
|------|----------|
| `test_case:create` | Create orphan test case |
| `test_case:update` | Update orphan test case; assign to project |
| `test_case:delete` | Delete orphan test case |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks required system permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User is not the creator (update/delete/share) | `403` | `FORBIDDEN` | INFO | Generic message, same as missing permission |
| User is not Contributor/Editor/Owner of target project (assign-to-project) | `403` | `FORBIDDEN` | INFO | Generic message |
| Test case not found, soft-deleted, or user has no visibility | `404` | `NOT_FOUND` | INFO | Same message regardless of cause (information leakage prevention) |
| Target project not found or soft-deleted (assign-to-project) | `404` | `NOT_FOUND` | INFO | Checked during assign-to-project |
| Share entry not found | `404` | `NOT_FOUND` | INFO | Remove share on non-existent share |
| Duplicate test case summary (among user's orphans) | `409` | `DUPLICATE_TEST_CASE_SUMMARY` | INFO | Case-insensitive; only among non-deleted rows |
| Duplicate test case summary (in target project during assignment) | `409` | `DUPLICATE_TEST_CASE_SUMMARY` | INFO | Uses project-scoped uniqueness check |
| Category does not exist or wrong project (assign-to-project) | `422` | `INVALID_CATEGORY` | INFO | Checked during assign-to-project |
| Priority does not exist or wrong project (assign-to-project) | `422` | `INVALID_PRIORITY` | INFO | Checked during assign-to-project |
| `category_id` or `priority_id` in create/update (not assign-to-project) | `422` | `VALIDATION_ERROR` | INFO | These fields are not allowed on orphan test cases |
| Invalid user IDs in share request (non-existent or INACTIVE) | `422` | `INVALID_USER` | INFO | Includes field-level details identifying invalid IDs |
| Test case already assigned to project (share attempt) | `422` | `ALREADY_ASSIGNED` | INFO | Sharing is only for orphan test cases |
| Validation error (bad request body) | `422` | `VALIDATION_ERROR` | INFO | Includes field-level details; unrecognised fields rejected in strict mode |
| Invalid pagination, sort parameter, or filter value | `422` | `VALIDATION_ERROR` | INFO | page < 1 or page > 1000, limit < 1 or limit > 100, invalid sort, invalid automated filter, invalid ownership value, search > 255 chars |
| DB duplicate key violation (race condition) | `409` | `DUPLICATE_TEST_CASE_SUMMARY` | INFO | Caught from PostgreSQL error 23505 |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Per-endpoint limits; returns `Retry-After` header |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not reveal** whether a `404` is caused by a non-existent test case, soft-deleted
  test case, or visibility restriction (same message for all).
- **Do not return `400`** for business logic errors -- use `409 Conflict` or `422
  Unprocessable Entity` as specified.
- **Do not hard-delete** any record -- all deletions are soft-deletes.
- **Do not allow UPDATE on soft-deleted rows** -- the repository layer checks
  `deleted_at IS NULL`.
- **Do not cascade soft-delete to shares** -- shares are preserved for audit trail.
- **Do not allow `category_id` or `priority_id` on orphan create or regular update** --
  these fields require a project context for FK validation.
- **Do not allow the creator to share with themselves** -- silently skipped.
- **Do not allow shared users to mutate** -- shared users have read-only access.
- **Do not use `GET` with a body** -- all state changes use `POST`, `PATCH`, or `DELETE`.
