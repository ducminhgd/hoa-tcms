# Design: Test Plan Runs

## Architecture

The Test Plan Runs feature follows Clean Architecture layering. It introduces a single
junction table (`TEST_PLAN_TEST_RUNS`) linking Test Plans to Test Runs. All endpoints
are nested under the Test Plan resource path (`/api/v1/test-plans/{id}/runs`) and
require the same authorization model as the parent Test Plan resource.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_linked_runs  GET    /api/v1/test-plans/{id}/runs              │   │
│  │  - link_runs         POST   /api/v1/test-plans/{id}/runs              │   │
│  │  - unlink_run        DELETE /api/v1/test-plans/{id}/runs/{runId}      │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  TestPlanRunService:                      │  │  - TestPlanRun (entity)  │  │
│  │  - list_linked_runs                       │  └──────────────────────────┘  │
│  │  - link_runs                              │                                │
│  │  - unlink_run                             │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - TestPlanRunRepository (port)           │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestPlanRunRepository  (implements TestPlanRunRepository)        │   │
│  │  - Migration: TEST_PLAN_TEST_RUNS table creation                       │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All endpoints require an authenticated session (checked by `AuthMiddleware`).
2. Authorization follows the parent Test Plan's model: `test_plan:read` for listing,
   `test_plan:update` for linking and unlinking. The caller must be a project member
   (any role for reads; Owner or Editor for mutations) in at least one project linked
   to the Test Plan.
3. `POST /api/v1/test-plans/{id}/runs` validates all run IDs in the request body,
   checks for existence and non-deleted status, then inserts junction rows for all
   valid IDs within a single database transaction.
4. `DELETE /api/v1/test-plans/{id}/runs/{runId}` hard-deletes the junction row.
   No soft-delete on junction tables (per project-wide convention).

### Security Requirements

**Input sanitization:** This feature does not accept free-text input (only numeric
run IDs). No HTML or script sanitization is required at this boundary.

**CSRF protection:** All state-changing endpoints (`POST`, `DELETE`) must be
protected against CSRF. Session cookies must carry `SameSite=Lax` (or stricter).
`POST` requests with `Content-Type: application/json` must verify the
`Content-Type` header to block simple form-based CSRF attacks. Additional
anti-CSRF tokens should be considered for defense in depth.

**Authorization layering:** Two independent authorization gates apply:

1. System permission check (`test_plan:read` or `test_plan:update`)
2. Project membership and role check (any role for reads; Owner or Editor for mutations)

Both gates must pass (or the caller must be a System Admin, who implicitly holds all
system permissions and bypasses all project membership checks). Authorization checks
are performed against live data on every request -- permissions and project membership
are never cached in the session. A `403 Forbidden` response must use a generic message
that does not distinguish between "missing system permission" and "wrong project role".

**Referential integrity:** Before linking, every `run_id` is validated against the
`TEST_RUNS` table to ensure the Test Run exists and is not soft-deleted. This check
runs within the same transaction as the insert. Referenced Test Plans are validated
at the handler layer before the service is called.

**Rate limiting:** All endpoints are protected by rate limiting configured at the
middleware layer. State-changing endpoints (POST/DELETE) allow 30 req/min; read
endpoints (GET list) allow 60 req/min. See requirements.md Security Considerations
for the full table.

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

### GET `/api/v1/test-plans/{id}/runs`

List Test Runs linked to a Test Plan with pagination.

**Required Permission:** `test_plan:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Plan ID |

**Query Parameters:**

| Param | Type | Default | Max | Description |
|-------|------|---------|-----|-------------|
| `page` | integer | 1 | -- | Page number (1-based) |
| `limit` | integer | 25 | 100 | Items per page |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "run_id": 101,
      "summary": "Regression test suite for v2.3",
      "status": "IN PROGRESS",
      "project_id": 3,
      "project_name": "Web App",
      "linked_by": 15,
      "linked_at": "2026-07-15T09:30:00Z"
    },
    {
      "run_id": 87,
      "summary": "Smoke tests for release v2.3",
      "status": "PASS",
      "project_id": 3,
      "project_name": "Web App",
      "linked_by": 15,
      "linked_at": "2026-07-14T14:00:00Z"
    }
  ],
  "meta": {
    "total": 5,
    "page": 1,
    "limit": 25
  }
}
```

**Notes:**
- Results are scoped to the given Test Plan (`WHERE tp.plan_id = $1`).
- Test Runs that are soft-deleted are excluded from the results.
- Default sort is `linked_at` descending (most recently linked first).
- `run_id`, `summary`, and `status` are resolved from the `TEST_RUNS` table via JOIN.
- `project_id` and `project_name` are resolved from the Test Run's project context.
- `linked_by` is the user ID of the person who created the link.
- `linked_at` is the timestamp when the link was created.
- The `total` count includes only non-deleted, linked Test Runs.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_plan:read` permission or is not a project member of any linked project |
| `404` | `NOT_FOUND` | Test Plan does not exist or is soft-deleted |
| `422` | `VALIDATION_ERROR` | Invalid pagination parameters |

---

### POST `/api/v1/test-plans/{id}/runs`

Link one or more Test Runs to a Test Plan. Already-linked runs are silently skipped
(idempotent).

**Required Permission:** `test_plan:update` AND project role Owner or Editor in at
least one project linked to the Test Plan

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Plan ID |

**Request Body:**

```json
{
  "run_ids": [101, 102, 103]
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `run_ids` | array of integers | Yes | Non-empty; max 100 entries; each entry must be a positive integer referencing a non-deleted Test Run |

**Success Response:** `200 OK`

```json
{
  "data": {
    "linked": [101, 102, 103]
  }
}
```

**Notes:**
- The response includes only the run IDs that were **newly linked** (not already linked).
  If all requested run IDs are already linked, `linked` is an empty array but the
  response is still `200 OK` (idempotent).
- Duplicate run IDs within the `run_ids` array are deduplicated before processing.
- All inserts execute within a single database transaction. If any run ID is invalid,
  the entire transaction is rolled back and `422` is returned with all errors
  aggregated.
- Each inserted row sets `linked_by` to the authenticated user's ID and
  `linked_at = NOW()`.
- On `INSERT ... ON CONFLICT (plan_id, run_id) DO NOTHING` handles the idempotency
  at the database level.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_plan:update` permission or is not Owner/Editor of any linked project |
| `404` | `NOT_FOUND` | Test Plan does not exist or is soft-deleted |
| `422` | `VALIDATION_ERROR` | `run_ids` array empty or missing |
| `422` | `VALIDATION_ERROR` | `run_ids` exceeds 100 entries |
| `422` | `VALIDATION_ERROR` | One or more run IDs do not exist or reference soft-deleted runs |
| `422` | `VALIDATION_ERROR` | Unrecognised fields in request body (strict mode) |

`422` response body when one or more run IDs are invalid:

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "One or more Test Run IDs are invalid.",
    "details": [
      { "field": "run_ids[1]", "message": "Test Run 999 does not exist or has been deleted." },
      { "field": "run_ids[3]", "message": "Test Run 555 does not exist or has been deleted." }
    ]
  }
}
```

---

### DELETE `/api/v1/test-plans/{id}/runs/{runId}`

Unlink a specific Test Run from a Test Plan. This is a hard delete on the junction table.

**Required Permission:** `test_plan:update` AND project role Owner or Editor in at
least one project linked to the Test Plan

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Plan ID |
| `runId` | integer | Test Run ID to unlink |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Hard-deletes the row from `TEST_PLAN_TEST_RUNS` where `plan_id = {id} AND run_id = {runId}`.
- No referential integrity check is required (the junction row is a leaf -- no other
  tables reference it).
- If no link exists between the Test Plan and the Test Run, `404 Not Found` is returned
  (consistent with the project-wide convention: repeated DELETE on a non-existent
  resource returns `404`).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_plan:update` permission or is not Owner/Editor of any linked project |
| `404` | `NOT_FOUND` | Test Plan does not exist or is soft-deleted, Test Run does not exist, or no link exists |
| `422` | `VALIDATION_ERROR` | Invalid `runId` (non-integer, zero, or negative) |

---

## Data Model

### New Table: TEST_PLAN_TEST_RUNS

A junction table linking Test Plans to Test Runs. Hard-delete only (no `deleted_at` /
`deleted_by`). Since the link itself has no mutable fields beyond its existence, the
only audit field is `linked_by`/`linked_at` (set at creation, never updated).

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `plan_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_plans(id) ON DELETE RESTRICT` | Part of composite PK |
| `run_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_runs(id) ON DELETE RESTRICT` | Part of composite PK |
| `linked_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who created the link |
| `linked_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | When the link was created |

**Constraints:**

```sql
-- Composite primary key enforces uniqueness (no duplicate links)
ALTER TABLE test_plan_test_runs
  ADD CONSTRAINT pk_test_plan_test_runs
  PRIMARY KEY (plan_id, run_id);

-- Foreign key indexes
CREATE INDEX idx_test_plan_test_runs_plan_id ON test_plan_test_runs (plan_id);
CREATE INDEX idx_test_plan_test_runs_run_id ON test_plan_test_runs (run_id);
CREATE INDEX idx_test_plan_test_runs_linked_by ON test_plan_test_runs (linked_by);

-- Composite index for the most common query pattern: all runs for a plan ordered by link time
CREATE INDEX idx_test_plan_test_runs_plan_linked
  ON test_plan_test_runs (plan_id, linked_at DESC);
```

**Design notes:**

- **Composite PK** `(plan_id, run_id)` -- enforces uniqueness at the database level.
  Prevents duplicate links between the same Test Plan and Test Run. Also enables
  `INSERT ... ON CONFLICT DO NOTHING` for idempotent linking.
- **No surrogate `id` column** -- the composite PK is the natural key for this junction.
  Queries filter by `plan_id` for listing and by `(plan_id, run_id)` for unlinking.
- **No `updated_at` / `updated_by`** -- once a link is created, it has no mutable
  properties. It either exists or is deleted. This is the simplest form of junction
  table, similar to many-to-many association tables.
- **Hard delete** -- unlinking is a `DELETE` from the junction table. No soft-delete
  on junction tables (per project-wide convention established in the `project-members`
  spec and FR-52).
- **`ON DELETE RESTRICT` on both FKs** -- prevents deleting a Test Plan or Test Run
  that has active links. Consumers must unlink runs before deleting the plan, and
  unlink plans before deleting the run. This is a safety measure to prevent accidental
  data loss.
- **Composite index `idx_test_plan_test_runs_plan_linked`** on `(plan_id, linked_at DESC)`
  supports the most common query pattern: listing all runs for a plan ordered by link
  time.
- **`linked_by` FK with `RESTRICT`** -- prevents deleting a user who has created links.
  This preserves attribution integrity.

### Relationship to Other Tables

```
TEST_PLANS ──< TEST_PLAN_TEST_RUNS >── TEST_RUNS
```

- `TEST_PLAN_TEST_RUNS.plan_id` -> `TEST_PLANS.id` (each link belongs to one Test Plan)
- `TEST_PLAN_TEST_RUNS.run_id` -> `TEST_RUNS.id` (each link references one Test Run)
- `TEST_PLAN_TEST_RUNS.linked_by` -> `USERS.id` (who created the link)

---

## Sequence

### List Linked Runs Flow

1. Client sends `GET /api/v1/test-plans/{id}/runs?page=1&limit=25` with session
   cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates the Test Plan exists and is not soft-deleted
   (call `TestPlanRepository::find_by_id`). If not found -> `404`.
4. Handler validates pagination parameters (`page`, `limit`).
5. Handler calls `TestPlanRunService::list_linked_runs(plan_id, page, limit, current_user_id)`.
6. `TestPlanRunService` checks the user has `test_plan:read` system permission
   (via `AuthorizationService`).
7. `TestPlanRunService` checks the user is a member of at least one project linked to
   the Test Plan (via `ProjectMemberRepository`). If not, and not System Admin -> `403`.
8. `TestPlanRunService` calls
   `TestPlanRunRepository::find_by_plan(plan_id, page, limit)`.
9. Repository executes a parameterized query with JOIN on `TEST_RUNS` to resolve
   `summary`, `status`, `project_id`, and `project_name`. Filters out soft-deleted
   runs (`WHERE tr.deleted_at IS NULL`). Orders by `linked_at DESC`.
10. Repository returns the paginated results and total count.
11. `TestPlanRunService` returns the response DTO with `data` and `meta`.
12. Handler returns `200 OK`.

### Link Runs Flow

1. Client sends `POST /api/v1/test-plans/{id}/runs` with
   `{"run_ids": [101, 102, 103]}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates the Test Plan exists and is not soft-deleted
   (call `TestPlanRepository::find_by_id`). If not found -> `404`.
4. Handler deserializes and validates the request body:
   - `run_ids` must be present and a non-empty array.
   - `run_ids` must not exceed 100 entries.
   - Each element must be a positive integer.
   - No unrecognised top-level fields (strict mode).
5. Handler deduplicates `run_ids` (removes duplicates within the array).
6. Handler calls
   `TestPlanRunService::link_runs(plan_id, run_ids, current_user_id)`.
7. `TestPlanRunService` checks the user has `test_plan:update` system permission
   (via `AuthorizationService`).
8. `TestPlanRunService` checks the user is an Owner or Editor of at least one project
   linked to the Test Plan (via `ProjectMemberRepository`). If not, and not System
   Admin -> `403`.
9. `TestPlanRunService` begins a database transaction.
10. Within the transaction:
    a. For each run ID in the deduplicated list, validate the Test Run exists, is not
       soft-deleted, and is not already linked to this plan. Invalid run IDs are
       collected; already-linked run IDs are silently skipped.
    b. If any run IDs are invalid, roll back and return `422` with field-level
       details for each invalid ID.
    c. For each valid, not-yet-linked run ID, insert a row into `TEST_PLAN_TEST_RUNS`
       with `plan_id`, `run_id`, `linked_by = current_user_id`,
       `linked_at = NOW()`. Use `INSERT ... ON CONFLICT (plan_id, run_id) DO NOTHING`
       to handle concurrent duplicate inserts safely.
11. Transaction commits.
12. Handler returns `200 OK` with the list of newly linked run IDs.

### Unlink Run Flow

1. Client sends `DELETE /api/v1/test-plans/{id}/runs/{runId}` with session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates the Test Plan exists and is not soft-deleted
   (call `TestPlanRepository::find_by_id`). If not found -> `404`.
4. Handler validates `runId` is a positive integer.
5. Handler calls
   `TestPlanRunService::unlink_run(plan_id, run_id, current_user_id)`.
6. `TestPlanRunService` checks the user has `test_plan:update` system permission
   (via `AuthorizationService`).
7. `TestPlanRunService` checks the user is an Owner or Editor of at least one project
   linked to the Test Plan (via `ProjectMemberRepository`). If not, and not System
   Admin -> `403`.
8. `TestPlanRunService` calls
   `TestPlanRunRepository::delete(plan_id, run_id)`.
9. Repository executes `DELETE FROM test_plan_test_runs WHERE plan_id = $1 AND run_id = $2`.
   If no row was deleted (0 rows affected) -> raise `TestPlanRunNotFoundError`.
10. `TestPlanRunService` maps the not-found result to a domain error.
11. Handler maps `TestPlanRunNotFoundError` -> `404 Not Found`, or returns
    `204 No Content` on success.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestPlanRun` | Domain (1) | Entity representing a link: `plan_id`, `run_id`, `linked_by`, `linked_at`. Factory method `link(plan_id, run_id, linked_by)` validates that both IDs are positive, sets `linked_at` to the current timestamp. No ORM or framework imports. |
| `TestPlanRunService` | Application (2) | Orchestrates all use cases: `list_linked_runs`, `link_runs`, `unlink_run`. Each method checks the required system permission (`test_plan:read` or `test_plan:update`), project membership/role, then delegates to the repository. |
| `TestPlanRunRepository` | Application (2) | Interface (port): `find_by_plan(plan_id, page, limit) -> (Vec<LinkedRunItem>, u64 total)`, `link(plan_id, run_id, linked_by)`, `bulk_link(plan_id, run_ids, linked_by) -> Vec<i64>`, `delete(plan_id, run_id) -> bool`, `exists(plan_id, run_id) -> bool`, `validate_run_ids(run_ids) -> Vec<i64>` (returns the subset of IDs that exist and are non-deleted). |
| `TestPlanRunHandler` | Adapters (3) | HTTP handler with three methods (`list`, `link`, `unlink`). Deserializes requests, validates parameters, calls `TestPlanRunService`, serializes responses. Deduplicates `run_ids` in the link handler before passing to the service. |
| `SqlTestPlanRunRepository` | Infrastructure (4) | Implements `TestPlanRunRepository` using PostgreSQL. Uses parameterized queries exclusively. The `bulk_link` method uses a single `INSERT ... ON CONFLICT DO NOTHING` with an unnest or multiple VALUES rows. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (Application) | No new permission codes required. This feature reuses `test_plan:read` and `test_plan:update` permission codes defined in the `test-plan-crud` spec. The authorization guard for project membership and role is extended to support Test Plan-scoped endpoints (resolving the user's role from the Test Plan's linked projects). |
| Test Plan detail handler (from `test-plan-crud`) | Include linked runs data or provide a link to the `/runs` sub-resource in the Test Plan detail response. At minimum, the detail response should include a `runs_count` field or a `runs_url` reference so the UI can fetch the linked runs list. |
| HTTP router registration | Register three new routes under `/api/v1/test-plans/{id}/runs/` -- all require session auth. No route ordering conflict as `/{runId}` is numeric and `/runs` has no static sub-paths. |
| DI container / wiring (if applicable) | Register `SqlTestPlanRunRepository` as the implementation of `TestPlanRunRepository`. Register `TestPlanRunService` with its dependencies. Register `TestPlanRunHandler` with `TestPlanRunService`. |

---

## Route Registration

```text
GET    /api/v1/test-plans/{id}/runs         -> list
POST   /api/v1/test-plans/{id}/runs         -> link
DELETE /api/v1/test-plans/{id}/runs/{runId} -> unlink
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- Test Plan existence validation (plan not soft-deleted)
- System permission check (`test_plan:read` for GET; `test_plan:update` for POST/DELETE)
- Project membership check (any role for reads; Owner/Editor in at least one linked project for mutations)

No `/{runId}` vs static path conflict exists because `/{runId}` is an integer path segment
and there is no static sub-path like `/select` that could shadow it.

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks `test_plan:read` system permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks `test_plan:update` system permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks required project role (not member, not Owner/Editor) | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Test Plan not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before any operation |
| No link exists (unlink) | `404` | `NOT_FOUND` | INFO | Delete on non-existent junction row |
| `run_ids` array empty or missing | `422` | `VALIDATION_ERROR` | INFO | At least one run ID required |
| `run_ids` exceeds 100 entries | `422` | `VALIDATION_ERROR` | INFO | Batch size limit |
| One or more run IDs invalid (non-existent or soft-deleted) | `422` | `VALIDATION_ERROR` | INFO | Field-level details for each invalid ID |
| Unrecognised fields in `POST` body | `422` | `VALIDATION_ERROR` | INFO | Strict mode -- rejects unknown fields |
| Invalid pagination parameters | `422` | `VALIDATION_ERROR` | INFO | page < 1 or page > 1000, limit < 1 or limit > 100, non-integer values |
| Invalid `runId` path parameter | `422` | `VALIDATION_ERROR` | INFO | Non-integer, zero, or negative |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Per-endpoint limits; returns `Retry-After` header |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not soft-delete** junction rows -- unlinking is a hard `DELETE`.
- **Do not return `200` with an error in the body** -- use `422` for validation failures.
- **Do not reveal** whether a `404` is caused by a non-existent Test Plan, non-existent
  Test Run, or missing link (same message for all).
- **Do not return `400`** for validation errors -- use `422 Unprocessable Entity`.
- **Do not allow linking to soft-deleted Test Runs** -- the validation check includes
  `WHERE deleted_at IS NULL`.
- **Do not accept more than 100 run IDs** in a single `POST` request to prevent
  excessively large transactions.
- **Do not use `GET` with a body** -- all state changes use `POST` or `DELETE`.
- **Do not break on duplicate run IDs** within the `run_ids` array -- deduplicate before
  processing.
- **Do not error on already-linked runs** -- silently skip them (idempotent linking).
