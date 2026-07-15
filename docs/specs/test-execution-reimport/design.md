# Design: Test Execution Re-import

## Architecture

The Re-import feature is a single-endpoint operation on an existing Test Execution. It
refreshes snapshot fields on `test_case_results` rows from their source `test_cases` rows.
The operation follows the same transaction pattern as the Test Execution Import feature.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handler:                                                         │   │
│  │  - reimport_execution  POST /api/v1/projects/{pid}/test-runs/{rid}/   │   │
│  │                         executions/{eid}/reimport                      │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  ExecutionReimportService:               │  │  - TestCaseResult        │  │
│  │  - reimport_single                       │  │    (snapshot entity)     │  │
│  │  - reimport_all                          │  │  - ResultStatus enum     │  │
│  │                                          │  │  - ReimportResult DTO    │  │
│  │  Interfaces:                             │  └──────────────────────────┘  │
│  │  - TestExecutionRepository (port)        │                                │
│  │  - TestCaseResultRepository (port)       │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestExecutionRepository  (implements TestExecutionRepository)    │   │
│  │  - SqlTestCaseResultRepository (implements TestCaseResultRepository)   │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. The endpoint requires an authenticated session (checked by `AuthMiddleware`).
2. The caller must hold the `test_execution:reimport` system permission and be at least a
   Contributor of the project. Contributors may be subject to an ownership check on the
   Test Execution (TBD in `test-execution-crud` spec).
3. The handler validates the project, Test Run, and Test Execution exist and are not
   soft-deleted. The Execution must belong to the given Run, and the Run must belong to the given Project
   (project scope is resolved via the chain: execution -> test_run -> project).
4. The handler determines the mode:
   - Single mode (no `scope` query param or `scope=single`): re-imports the one test
     case result associated with this execution.
   - Bulk mode (`scope=all`): re-imports all NOT_TESTED results in the execution.
5. For each target result, the service:
   a. Fetches the current Test Case row (including soft-deleted).
   b. Compares snapshot fields with current values.
   c. If unchanged: skips (no-op). If changed and status is NOT_TESTED: updates.
   d. If status is not NOT_TESTED: skips (single mode returns `409 Conflict`;
      bulk mode counts as skipped).
6. The entire operation runs within a single database transaction.

### Security Requirements

**Authorization layering:** Three independent gates apply:

1. System permission check (`test_execution:reimport`)
2. Project membership and role check (Contributor, Editor, or Owner for mutations)
3. Contributor ownership check (if the user's role is Contributor, verify they own the
   Test Execution or have been assigned as a tester -- exact rule TBD in
   `test-execution-crud`)

All three gates must pass (or the caller must be a System Admin). A `403 Forbidden`
response uses a generic message that does not distinguish between failure modes.

**Rate limiting:** POST `/reimport` with single mode allows 30 req/min; bulk mode
(`scope=all`) allows 10 req/min due to the higher database load.

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

### POST `/api/v1/projects/{projectId}/test-runs/{runId}/executions/{executionId}/reimport`

Re-import snapshot fields from the current Test Case into one or all test case results in a
Test Execution.

**Required Permission:** `test_execution:reimport` AND project role Contributor, Editor, or Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `runId` | integer | Test Run ID |
| `executionId` | integer | Test Execution ID |

**Query Parameters:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `scope` | string | `single` | `single` (reimport the single result tied to this execution) or `all` (reimport all NOT_TESTED results) |

**Request Body:** None

---

#### Single Mode (`scope=single` or omitted)

**Success Response:** `200 OK`

```json
{
  "data": {
    "id": 456,
    "execution_id": 12,
    "test_case_id": 128,
    "summary": "User can log in with valid credentials (updated v2)",
    "description": "Given a registered user with valid credentials, when they submit the login form, then they are redirected to the dashboard. Updated with SSO flow.",
    "priority": "Critical",
    "status": "NOT_TESTED",
    "result_logs": null,
    "created_by": 15,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 15,
    "updated_at": "2026-07-15T08:30:00Z"
  },
  "meta": {
    "changed": true
  }
}
```

**Notes:**
- `priority` is a denormalized name string resolved at reimport time via LEFT JOIN of
  `test_cases.priority_id` to `test_priorities.name`. If `priority_id` is null or the
  priority is soft-deleted, `priority` is `null`.
- `meta.changed` is `true` when at least one snapshot field was updated; `false` when all
  values already matched the source Test Case.
- Response excludes `files` and internal fields (`deleted_at`, `deleted_by`).
- The source Test Case is identified by the `test_case_id` FK on the `test_case_results` row.
- The Test Case row is read regardless of its soft-delete status (soft-deleted Test Cases
  still serve as the source of truth for their existing snapshots).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:reimport` permission, is not a project member, or fails ownership check |
| `404` | `NOT_FOUND` | Project, Test Run, or Test Execution does not exist, is soft-deleted, or belongs to a different project/run |
| `404` | `RESULT_NOT_FOUND` | The Test Execution has no associated test case result (data integrity edge case) |
| `409` | `RESULT_ALREADY_STARTED` | The result's status is not `NOT_TESTED`. Re-import is blocked to preserve tester work. |
| `422` | `VALIDATION_ERROR` | Invalid `scope` parameter value |
| `429` | `RATE_LIMITED` | Rate limit exceeded |

`409 Conflict` response body:

```json
{
  "error": {
    "code": "RESULT_ALREADY_STARTED",
    "message": "Cannot re-import: the test case result has already been started (status: PASS). Only NOT_TESTED results can be re-imported.",
    "details": [
      { "field": "result_id", "message": "Result status is PASS, expected NOT_TESTED" }
    ]
  }
}
```

---

#### Bulk Mode (`scope=all`)

**Success Response:** `200 OK`

```json
{
  "data": {
    "imported_count": 8,
    "skipped_count": 5,
    "failed_count": 0,
    "failures": []
  },
  "meta": {
    "total_results": 13
  }
}
```

**Notes:**
- `imported_count`: number of results whose snapshot fields were refreshed (includes rows
  that were already up-to-date and did not require a write).
- `skipped_count`: number of results whose status is not `NOT_TESTED` (preserved).
- `failed_count`: number of results that could not be refreshed due to an error (e.g.,
  source Test Case hard-deleted, FK violation on priority_id). Expected to be zero in
  normal operation.
- `failures`: array of `{ "result_id": <id>, "reason": "string" }` for each failure.
  Empty when `failed_count` is 0.
- `meta.total_results`: total number of test case results in this execution (for client
  sanity-check: imported + skipped + failed should equal total).
- Individual failures do not roll back the entire operation. Each row refresh is
  independent. A database-level failure (connection loss, deadlock) rolls back all changes.
- A bulk re-import with zero NOT_TESTED results returns `200 OK` with `imported_count: 0`.

**Error Responses (in addition to the common errors above):**

| Status | Code | Condition |
|--------|------|-----------|
| `200` | -- | Bulk operation completed; check `failed_count` for individual row issues |
| `422` | `VALIDATION_ERROR` | Invalid `scope` parameter value (anything other than `single` or `all`) |

When `failed_count > 0`, the response body includes the failure details:

```json
{
  "data": {
    "imported_count": 6,
    "skipped_count": 5,
    "failed_count": 2,
    "failures": [
      { "result_id": 401, "reason": "Source test case has been hard-deleted (id: 99)" },
      { "result_id": 403, "reason": "Source test case priority_id 7 is invalid for this project" }
    ]
  },
  "meta": {
    "total_results": 13
  }
}
```

---

## Sequence

### Single Re-import Flow

1. Client sends `POST /api/v1/projects/42/test-runs/10/executions/12/reimport` with
   session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates path parameters: project exists and is not soft-deleted, Test Run
   exists and belongs to project and is not soft-deleted, Test Execution exists and
   belongs to Test Run and is not soft-deleted. Any of these missing -> `404 Not Found`.
4. Handler validates `scope` query parameter: must be absent or `single`. Any other
   value -> `422 Unprocessable Entity`.
5. Handler calls
   `ExecutionReimportService::reimport_single(project_id, run_id, execution_id, current_user_id)`.
6. Service checks `test_execution:reimport` system permission via `AuthorizationService`.
7. Service checks user is at least a Contributor of the project. If Viewer or not a member
   and not System Admin -> `403`.
8. Service begins a database transaction.
9. Within the transaction:
   a. Look up the test case result for this execution:
      ```sql
      SELECT tcr.* FROM test_case_results tcr
      JOIN test_executions te ON tcr.execution_id = te.id
      WHERE te.id = $1 AND te.deleted_at IS NULL
        AND tcr.deleted_at IS NULL;
      ```
      If no result -> roll back and return `404 RESULT_NOT_FOUND`.
   b. Check the result status. If not `NOT_TESTED` -> roll back and return
      `409 RESULT_ALREADY_STARTED`.
   c. Fetch the current Test Case with resolved priority name:
      ```sql
      SELECT tc.id, tc.summary, tc.description, tp.name AS priority
      FROM test_cases tc
      LEFT JOIN test_priorities tp
        ON tc.priority_id = tp.id AND tp.deleted_at IS NULL
      WHERE tc.id = $1;
      ```
      (No `deleted_at IS NULL` filter on test_cases -- soft-deleted Test Cases are still
      valid sources.)
      If the Test Case does not exist (hard-deleted or never existed -- data integrity
      edge case) -> roll back and return `500 INTERNAL_ERROR`.
   d. Compare snapshot fields (`summary`, `description`, `priority`) with source values.
      If all match -> skip update; set `changed = false`.
   e. If any field differs:
      ```sql
      UPDATE test_case_results tcr
      SET summary = tc.summary,
          description = tc.description,
          priority = tp.name,
          updated_by = $2
      FROM test_cases tc
      LEFT JOIN test_priorities tp
        ON tc.priority_id = tp.id AND tp.deleted_at IS NULL
      WHERE tcr.id = $1 AND tc.id = tcr.test_case_id;
      ```
      Set `changed = true`.
   f. Re-fetch the result row:
      ```sql
      SELECT * FROM test_case_results WHERE id = $1;
      ```
10. Transaction commits.
11. Handler constructs and returns `200 OK` response.

### Bulk Re-import Flow

1. Client sends `POST /api/v1/projects/42/test-runs/10/executions/12/reimport?scope=all`
   with session cookie.
2-4. Same as single: validate session, project, Test Run, Test Execution, scope.
5. Handler calls
   `ExecutionReimportService::reimport_all(project_id, run_id, execution_id, current_user_id)`.
6-7. Same authorization checks as single mode.
8. Service begins a database transaction.
9. Within the transaction:
   a. Fetch all test case results for this execution:
      ```sql
      SELECT tcr.id, tcr.test_case_id, tcr.summary, tcr.description, tcr.priority,
             tcr.status
      FROM test_case_results tcr
      WHERE tcr.execution_id = $1 AND tcr.deleted_at IS NULL
      ORDER BY tcr.id;
      ```
   b. Collect the unique `test_case_id` values and fetch current Test Case data with
      resolved priority names in one batch query:
      ```sql
      SELECT tc.id, tc.summary, tc.description, tp.name AS priority
      FROM test_cases tc
      LEFT JOIN test_priorities tp
        ON tc.priority_id = tp.id AND tp.deleted_at IS NULL
      WHERE tc.id = ANY($1);
      ```
   c. For each result row:
      - If status != `NOT_TESTED` -> increment `skipped_count`.
      - If status == `NOT_TESTED`:
        - Look up the source Test Case in the batch result. If not found (hard-deleted)
          -> increment `failed_count`, add to `failures` with reason.
        - Compare snapshot fields with source values. If all match -> increment
          `imported_count` without writing.
        - If any field differs -> execute the UPDATE, increment `imported_count`.
10. Transaction commits.
11. Handler constructs and returns `200 OK` with counts and failures.

---

## Data Model

### Relevant Existing Tables

**`TEST_CASE_RESULTS`** (created by `test-execution-import`):

| Column | Type | Role in Re-import |
|--------|------|--------------------|
| `id` | `BIGINT PK` | Identifies the result row to update |
| `execution_id` | `BIGINT FK -> test_executions` | Scopes results to an execution |
| `test_case_id` | `BIGINT FK -> test_cases` | Source of truth for snapshot refresh |
| `summary` | `VARCHAR(500) NOT NULL` | Snapshot -- refreshed from `test_cases.summary` |
| `description` | `TEXT` | Snapshot -- refreshed from `test_cases.description` |
| `priority` | `VARCHAR(50)` | Denormalized priority name -- refreshed from `test_priorities.name` via `test_cases.priority_id` |
| `status` | `VARCHAR(50) NOT NULL` | Preserved -- re-import only allowed when `NOT_TESTED` |
| `result_logs` | `TEXT` | Preserved -- never touched by re-import |
| `updated_by` | `BIGINT FK -> users` | Set to the authenticated user on re-import |
| `updated_at` | `TIMESTAMPTZ` | Updated by `BEFORE UPDATE` trigger |

**`TEST_CASES`** (read-only source):

| Column | Role in Re-import |
|--------|--------------------|
| `id` | Joins to `test_case_results.test_case_id` |
| `summary` | Source value for `test_case_results.summary` |
| `description` | Source value for `test_case_results.description` |
| `priority_id` | Source FK; resolved to `test_priorities.name` at reimport time, written into `test_case_results.priority` |

**`TEST_EXECUTIONS`** (context):

| Column | Role in Re-import |
|--------|--------------------|
| `id` | Route parameter; scoping and auth validation |
| `run_id` | Must match the path parameter |
| (via `test_runs.project_id`) | Project scope resolved via 4-hop chain: result -> execution -> test_run -> project |

No new tables or columns are introduced by this feature. The re-import operation is purely
an UPDATE on existing `test_case_results` rows.

### Snapshot Fields Refreshed

| Snapshot Field | Source Column | Notes |
|----------------|---------------|-------|
| `summary` | `test_cases.summary` | Full replacement |
| `description` | `test_cases.description` | Full replacement; nullable |
| `priority` | `test_priorities.name` (resolved via `test_cases.priority_id`) | Full replacement; resolved at reimport time via LEFT JOIN; nullable |

### Fields Preserved (Never Modified)

| Field | Reason |
|-------|--------|
| `status` | Represents tester work; re-import is blocked when != NOT_TESTED |
| `result_logs` | Tester-written execution logs; never overwritten |
| `test_case_id` | Immutable link to source Test Case |
| `execution_id` | Immutable parent execution |
| `created_by`, `created_at` | Immutable audit fields |
| Files (via `test_case_result_files`) | Attached files are preserved and never deleted by re-import |

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `ExecutionReimportService` | Application (2) | Orchestrates both `reimport_single` and `reimport_all` use cases. Checks permissions, project membership, and ownership. Begins and manages the database transaction. Delegates data access to repositories. |
| `ReimportHandler` | Adapters (3) | HTTP handler with a single method. Validates path and query parameters. Calls `ExecutionReimportService`. Maps service results to HTTP responses. |
| `ReimportSingleResult` | Application (2) DTO | Response shape for single-mode re-import: the updated (or unchanged) test case result with `meta.changed`. |
| `ReimportBulkResult` | Application (2) DTO | Response shape for bulk-mode re-import: `imported_count`, `skipped_count`, `failed_count`, `failures[]`. |
| `ReimportFailure` | Application (2) DTO | A single failure entry: `result_id` and `reason`. |

### Modified / Extended Existing Components

| Component | Change |
|-----------|--------|
| `TestCaseResultRepository` (Application port) | Add `find_by_execution_id(execution_id)` for single mode and `find_all_by_execution_id(execution_id)` for bulk mode. Add `update_snapshot(result_id, summary, description, priority, updated_by)`. Add `find_source_test_cases(test_case_ids: Vec<i64>)` for batch lookup. |
| `SqlTestCaseResultRepository` (Infrastructure) | Implement the new repository methods with parameterized queries. |
| `AuthorizationService` (Application) | Add `test_execution:reimport` to the permission registry. |
| Seed migration | Add `('test_execution:reimport', 'Re-import Test Execution')` permission row. |
| HTTP router registration | Register `POST /api/v1/projects/{projectId}/test-runs/{runId}/executions/{executionId}/reimport`. |

---

## Route Registration

```text
POST /api/v1/projects/{projectId}/test-runs/{runId}/executions/{executionId}/reimport
```

The route requires:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- Test Run existence validation (run belongs to project, not soft-deleted)
- Test Execution existence validation (execution belongs to run, not soft-deleted)
- System permission check (`test_execution:reimport`)
- Project membership check (Contributor, Editor, or Owner)

---

## New Permission Code

Must be added to the `PERMISSIONS` table seed data using `INSERT ... ON CONFLICT (code)
DO NOTHING`:

| code | name |
|------|------|
| `test_execution:reimport` | Re-import Test Execution |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks `test_execution:reimport` permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks required project role | `403` | `FORBIDDEN` | INFO | Same generic message |
| Contributor does not own the execution | `403` | `FORBIDDEN` | INFO | Distinct message (if ownership rule applies) |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | |
| Test Run not found, soft-deleted, or wrong project | `404` | `NOT_FOUND` | INFO | Same message as project |
| Test Execution not found, soft-deleted, or wrong run | `404` | `NOT_FOUND` | INFO | Same message |
| No test case result linked to execution | `404` | `RESULT_NOT_FOUND` | INFO | Data integrity edge case |
| Single-mode: result status != NOT_TESTED | `409` | `RESULT_ALREADY_STARTED` | INFO | Preserves tester work |
| Invalid `scope` parameter | `422` | `VALIDATION_ERROR` | INFO | Must be `single` or `all` |
| Source Test Case hard-deleted (bulk mode) | `200` (with failure entry) | -- | WARN | Individual row failure; counted in `failed_count` |
| FK violation on priority_id (bulk mode) | `200` (with failure entry) | -- | WARN | Individual row failure; counted in `failed_count` |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail; transaction rolled back |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |
