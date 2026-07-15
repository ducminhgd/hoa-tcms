# Design: Test Case Result Update

## Architecture

The Test Case Result Update feature follows Clean Architecture layering. Results are nested
under the execution -> test run -> project path, enforcing that the result belongs to the
specified execution, which belongs to the specified run. The feature reuses the
`test_execution:update` system permission and applies fine-grained authorization based on
project role, result ownership, and execution tester assignment.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handler:                                                         │   │
│  │  - update_result  PATCH /api/v1/projects/{pid}/test-runs/{rid}/       │   │
│  │                           executions/{eid}/results/{resultId}          │   │
│  └──┬────────────────────────────────────────────────────────────────────┘   │
│     │ calls                                                                  │
│     ▼                                                                        │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  TestCaseResultService:                   │  │  - TestCaseResult       │  │
│  │  - update_result                          │  │  - ResultStatus (enum)  │  │
│  │                                           │  │  - StatusTransition     │  │
│  │  Interfaces:                              │  └──────────────────────────┘  │
│  │  - TestCaseResultRepository (port)        │                                │
│  └──┬───────────────────────────────────────┘                                │
│     │ delegates to                                                           │
│     ▼                                                                        │
│  Infrastructure (Layer 4)                                                    │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestCaseResultRepository (implements TestCaseResultRepository)   │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. The endpoint requires an authenticated session (checked by `AuthMiddleware`).
2. Handler validates the project exists and is not soft-deleted. If the project does not
   exist or is soft-deleted -> `404 Not Found`.
3. Handler validates the test run exists, is not soft-deleted, and belongs to the specified
   project (if the run has a `project_id`). If not -> `404 Not Found`.
4. Handler validates the test execution exists, is not soft-deleted, and belongs to the
   specified test run. If not -> `404 Not Found`.
5. Handler validates the test case result exists, is not soft-deleted, belongs to the
   specified execution, and is not orphaned (its parent execution and run are not
   soft-deleted). If not -> `404 Not Found`.
6. Handler calls `TestCaseResultService::update_result(...)`.
7. Service checks `test_execution:update` system permission.
8. Service resolves authorization:
   a. System Admin -> bypass all checks, allowed.
   b. Project Owner or Editor -> allowed to update any result.
   c. Project Contributor -> allowed if they created the result OR are an assigned tester
      for the execution.
   d. Assigned tester (in `execution_testers`) -> allowed to update any result in that
      execution, regardless of project role (even Viewers who are assigned testers can
      update).
   e. Otherwise -> `403 Forbidden`.
9. Service validates the status transition against the state machine. If invalid ->
   `422 Unprocessable Entity` with `INVALID_STATUS_TRANSITION`.
10. Service applies the update atomically (status + logs in a single DB transaction).
11. Service updates the parent execution's `updated_at` to reflect activity on its
    results (defence-in-depth for cache invalidation).
12. Handler returns `200 OK` with the updated result.

### Security Requirements

**Input sanitization:** The `result_logs` field must be sanitized on input (strip
disallowed HTML tags) before storage. Output-encoding must be applied at the presentation
layer. See PRD 5.3 for the project-wide XSS prevention policy.

**CSRF protection:** The `PATCH` state-changing endpoint must be protected against CSRF.
Session cookies must carry `SameSite=Lax` (or stricter). The handler must verify
`Content-Type: application/json` to block simple form-based CSRF attacks.

**Authorization layering:** Three authorization gates apply:

1. System permission check (`test_execution:update`)
2. Project membership and role check (at minimum, the user must have access to the
   project; Viewers who are NOT assigned testers are excluded)
3. Fine-grained check: Contributors can only update their own results (where
   `created_by` matches) or results on executions where they are assigned; Owners and
   Editors can update any result; assigned testers can update any result in their
   execution

All three gates must pass (or the caller must be a System Admin, who bypasses all
checks). Authorization checks are performed against live data on every request. A
`403 Forbidden` response must use a generic message that does not distinguish between
"missing system permission" and "wrong project role". The Contributor ownership
restriction is the exception: it returns a distinct message.

**Concurrent update semantics:** The system uses last-write-wins. The database handles
row-level locking; the application does not implement optimistic concurrency control
(e.g., no ETag or version column) for MVP.

**Rate limiting:** 60 req/min on the result update endpoint.

---

## API Contract

### PATCH `/api/v1/projects/{projectId}/test-runs/{runId}/executions/{executionId}/results/{resultId}`

Update a Test Case Result's status and/or result logs. Contributors can only update their
own results (or results on executions where they are assigned testers). Owners, Editors,
and assigned testers can update any result in the execution.

**Required Permission:** `test_execution:update`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `runId` | integer | Test Run ID (must belong to the project) |
| `executionId` | integer | Test Execution ID (must belong to the test run) |
| `resultId` | integer | Test Case Result ID (must belong to the execution) |

**Request Body** (all fields optional; at least one of `status` or `result_logs` required):

```json
{
  "status": "PASS",
  "result_logs": "All assertions passed. Login redirect works correctly on Chrome 120, Firefox 121, and Safari 17."
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `status` | string | No | One of: `IN_PROGRESS`, `PASS`, `FAIL`, `WARNING`, `IGNORE`. Must be a valid transition from current status (see status transition diagram). |
| `result_logs` | string | No | Max 10000 characters; sanitized on input (HTML stripped). `null` clears logs. Omitting preserves current value. |

**Success Response:** `200 OK`

```json
{
  "data": {
    "id": 523,
    "execution_id": 12,
    "test_case_id": 42,
    "summary": "User can log in with valid credentials",
    "description": "Given a registered user with valid credentials, when they submit the login form, then they are redirected to the dashboard.",
    "priority": "HIGH",
    "status": "PASS",
    "result_logs": "All assertions passed. Login redirect works correctly on Chrome 120, Firefox 121, and Safari 17.",
    "tested_by": 15,
    "created_by": 15,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 22,
    "updated_at": "2026-07-15T14:30:00Z"
  }
}
```

**Notes:**
- At least one of `status` or `result_logs` must be provided (empty body returns `422`).
- `id`, `execution_id`, `test_case_id`, `summary`, `description`, `priority`, `created_at`,
  and `created_by` are immutable through this endpoint (set at import time).
- `tested_by` is set once when the result first transitions away from NOT_TESTED. It is
  not modified on subsequent updates or re-opens.
- `updated_by` and `updated_at` are set automatically on every update.
- `deleted_at` and `deleted_by` cannot be modified through this endpoint.
- The parent execution's `updated_at` is also updated to reflect activity.
- Unrecognised fields in the request body cause a `422 Unprocessable Entity` response
  (strict mode -- rejects typos and unknown fields).
- The result must belong to the specified execution, which must belong to the specified
  run, which must belong to the specified project. If any link in the chain is broken ->
  `404 Not Found` (same generic message).
- Soft-deleted results cannot be updated (per FR-54c).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:update` permission, is not a project member, or (for Contributors) does not own the result and is not an assigned tester |
| `404` | `NOT_FOUND` | Project, test run, test execution, or result does not exist, is soft-deleted, or relationship chain is broken |
| `422` | `VALIDATION_ERROR` | No fields provided, `result_logs` too long, or unrecognised fields present |
| `422` | `INVALID_STATUS_TRANSITION` | Requested status transition is not allowed from the current status |

`422` error response body for an invalid transition (e.g., trying to go NOT_TESTED -> PASS directly):

```json
{
  "error": {
    "code": "INVALID_STATUS_TRANSITION",
    "message": "Cannot transition status from NOT_TESTED to PASS.",
    "details": [
      {
        "field": "status",
        "message": "Valid transitions from NOT_TESTED: IN_PROGRESS"
      }
    ]
  }
}
```

---

## Status Transition State Machine

```
                   ┌─────────────────────────────────────────┐
                   │                                         │
                   ▼                                         │
  ┌──────────┐    ┌──────────────┐    ┌──────────────────┐   │
  │          │    │              │    │  PASS            │   │
  │  NOT_    ├───►│  IN_PROGRESS ├───►│  FAIL            │   │
  │  TESTED  │    │              │    │  WARNING         │───┘
  │          │    └──────────────┘    │  IGNORE          │
  └──────────┘        ▲               └──────┬───────────┘
                      │                      │
                      └──────────────────────┘
                           (re-open)
```

| From | To | Allowed? | Notes |
|------|----|----------|-------|
| `NOT_TESTED` | `IN_PROGRESS` | Yes | Sets `tested_by` to current user. First transition away from NOT_TESTED. |
| `NOT_TESTED` | `PASS` | **No** | Must go through IN_PROGRESS first. |
| `NOT_TESTED` | `FAIL` | **No** | Must go through IN_PROGRESS first. |
| `NOT_TESTED` | `WARNING` | **No** | Must go through IN_PROGRESS first. |
| `NOT_TESTED` | `IGNORE` | **No** | Must go through IN_PROGRESS first. |
| `IN_PROGRESS` | `PASS` | Yes | Final state (can re-open). |
| `IN_PROGRESS` | `FAIL` | Yes | Final state (can re-open). |
| `IN_PROGRESS` | `WARNING` | Yes | Final state (can re-open). |
| `IN_PROGRESS` | `IGNORE` | Yes | Final state (can re-open). |
| `IN_PROGRESS` | `NOT_TESTED` | **No** | Cannot revert to initial state. |
| `PASS` | `IN_PROGRESS` | Yes | Re-open for re-testing. `tested_by` unchanged. |
| `FAIL` | `IN_PROGRESS` | Yes | Re-open for re-testing. `tested_by` unchanged. |
| `WARNING` | `IN_PROGRESS` | Yes | Re-open for re-testing. `tested_by` unchanged. |
| `IGNORE` | `IN_PROGRESS` | Yes | Re-open for re-testing. `tested_by` unchanged. |
| `PASS` | `FAIL` | **No** | Must go through IN_PROGRESS. |
| `FAIL` | `PASS` | **No** | Must go through IN_PROGRESS. |
| Any | Same status | Yes | No-op; still records `updated_at`/`updated_by`. |

**Design rationale:**
- Forcing the NOT_TESTED -> IN_PROGRESS transition captures the `tested_by` identity
  accurately -- the first person who started working on the result.
- Requiring IN_PROGRESS between final states prevents accidental status jumps and ensures
  the audit trail captures who re-opened the result.
- Re-open preserves `tested_by` to maintain accountability: the original tester is still
  recorded as the one who first engaged with the result.

---

## Data Model

### Existing Table: TEST_CASE_RESULTS

The `TEST_CASE_RESULTS` table is created by the `test-execution-import` migration. This
feature only interacts with the existing table; it does not create it.

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `execution_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_executions(id) ON DELETE RESTRICT` | Parent execution |
| `test_case_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_cases(id) ON DELETE RESTRICT` | Source test case |
| `summary` | `VARCHAR(500)` | `NOT NULL` | Snapshot from test case at import time |
| `description` | `TEXT` | | Snapshot from test case at import time |
| `priority` | `VARCHAR(50)` | | Snapshot from test case at import time |
| `status` | `VARCHAR(50)` | `NOT NULL`, `DEFAULT 'NOT_TESTED'` | Current status: NOT_TESTED, IN_PROGRESS, PASS, FAIL, WARNING, IGNORE |
| `result_logs` | `TEXT` | | Free-form execution notes/logs. Max 10000 chars enforced at app layer and by CHECK constraint. |
| `tested_by` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Set once on first transition from NOT_TESTED; not modified thereafter |
| `created_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who imported the result |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Last user who modified the result |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by `BEFORE UPDATE` trigger |
| `deleted_by` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Nullable; set on soft-delete |
| `deleted_at` | `TIMESTAMPTZ` | | Nullable; set on soft-delete |

**Constraints added by this feature:**

```sql
-- Logs length guard (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_case_results
  ADD CONSTRAINT chk_test_case_results_result_logs
  CHECK (result_logs IS NULL OR char_length(result_logs) <= 10000);

-- Partial index for active results (most queries filter out soft-deleted)
CREATE INDEX idx_test_case_results_active
  ON test_case_results (execution_id, id)
  WHERE deleted_at IS NULL;
```

**Design notes:**
- **`tested_by` vs `updated_by`:** `tested_by` captures who first started testing the
  result (the first NOT_TESTED -> IN_PROGRESS transition). It is never modified after
  being set. `updated_by` captures who last modified the result, which changes on every
  update including re-opens.
- **`status` field:** A `VARCHAR(50)` with a CHECK constraint is preferred over a
  database ENUM for flexibility (e.g., adding new statuses in future phases without
  an ALTER TYPE).
- **`priority` snapshot vs FK:** At import time, the priority label is copied from the
  source test case as a plain string. This is by design: the snapshot must not change
  when the source test case's priority is updated. Re-import refreshes this field.
- **No separate audit log table for MVP:** The status change audit trail is satisfied by
  the `status` column (current status), `updated_by` (who last changed it), and
  `updated_at` (when it was last changed). A full history table is deferred.

### BEFORE UPDATE Trigger

The `BEFORE UPDATE` trigger on `TEST_CASE_RESULTS` is defined in the
`test-execution-import` migration. This feature relies on it for auto-maintaining
`updated_at`. The trigger:

```sql
CREATE OR REPLACE FUNCTION trg_test_case_results_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_test_case_results_updated_at
  BEFORE UPDATE ON test_case_results
  FOR EACH ROW
  EXECUTE FUNCTION trg_test_case_results_updated_at();
```

### Relationship to Other Tables

```
PROJECTS ──< TEST_RUNS ──< TEST_EXECUTIONS ──< TEST_CASE_RESULTS
                                                   │
                                         TEST_CASES ─── (snapshot via test_case_id)
```

- `TEST_CASE_RESULTS.execution_id` -> `TEST_EXECUTIONS.id`
- `TEST_CASE_RESULTS.test_case_id` -> `TEST_CASES.id` (source; snapshot at import)
- `TEST_CASE_RESULTS.tested_by` -> `USERS.id`
- Parent chain validation: `result.execution_id = execution.id AND execution.run_id =
  run.id AND run.project_id = project.id` (when the run has a project)

---

## Sequence

### Update Test Case Result Flow

1. Client sends `PATCH /api/v1/projects/42/test-runs/5/executions/12/results/523` with
   `{"status": "PASS", "result_logs": "All assertions passed."}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler validates the project exists and is not soft-deleted. If not -> `404`.
4. Handler validates the test run exists, is not soft-deleted, and (if the run has a
   `project_id`) belongs to the specified project. If not -> `404`.
5. Handler validates the test execution exists, is not soft-deleted, and belongs to the
   specified test run. If not -> `404`.
6. Handler validates the test case result exists, is not soft-deleted, and belongs to the
   specified execution. If not -> `404`.
7. Handler calls `TestCaseResultService::update_result(project_id, run_id, execution_id,
   result_id, cmd, current_user_id)`.
8. `TestCaseResultService` checks the user has `test_execution:update` system permission.
9. `TestCaseResultService` resolves fine-grained authorization:
   a. If System Admin -> skip remaining checks.
   b. If project Owner or Editor -> allowed.
   c. If project Contributor:
      - Check if `result.created_by == current_user_id` -> allowed.
      - Check if the user is an assigned tester on the execution (via
        `ExecutionTesterRepository`) -> allowed.
      - Otherwise -> `403` ("You can only update your own Test Case Results").
   d. If user is an assigned tester on the execution (but not an Owner/Editor/Contributor
      of the project, possibly a Viewer or non-member) -> allowed.
   e. Otherwise -> `403` (generic message).
10. `TestCaseResultService` validates the status transition (if `status` is provided):
    `ResultStatus::transition(current_status, requested_status)`. If invalid -> `422`
    with `INVALID_STATUS_TRANSITION` and the list of valid transitions.
11. `TestCaseResultService` begins a database transaction.
12. Within the transaction:
    a. Build update: if `status` changed from NOT_TESTED and `tested_by` is NULL, set
       `tested_by = current_user_id`. If the status is changing to any valid target, set
       `status = new_status`.
    b. If `result_logs` is provided: set `result_logs` to the sanitized value (or NULL if
       explicitly sent as null).
    c. Set `updated_by = current_user_id`.
    d. Call `TestCaseResultRepository::update(result)`.
    e. Touch the parent execution: `UPDATE test_executions SET updated_at = NOW() WHERE
       id = $1` to reflect activity (for cache invalidation and list views).
13. Transaction commits.
14. Handler constructs the response DTO from the updated entity.
15. Handler returns `200 OK`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `ResultStatus` | Domain (1) | Enum with variants: `NotTested`, `InProgress`, `Pass`, `Fail`, `Warning`, `Ignore`. Method `transition(from, to) -> Result<Self>` validates the status transition against the state machine rules. Returns the new status or a `StatusTransitionError` listing valid transitions from the current status. |
| `TestCaseResult` | Domain (1) | Entity representing a row from `TEST_CASE_RESULTS`. Fields: `id`, `execution_id`, `test_case_id`, `summary`, `description`, `priority`, `status` (ResultStatus), `result_logs`, `tested_by`, audit columns. Method `apply_update(cmd, current_user_id)` validates the transition, sets `status`, `result_logs`, `tested_by` (if first transition), and `updated_by`. Domain validation only; no ORM or framework imports. |
| `TestCaseResultService` | Application (2) | Single method: `update_result(project_id, run_id, execution_id, result_id, cmd, current_user_id)`. Orchestrates: loads result and parent entities, checks system permission, resolves fine-grained authorization, validates status transition, applies update atomically. Returns the updated result DTO. |
| `TestCaseResultRepository` | Application (2) | Interface (port): `find_by_id(result_id) -> Option<TestCaseResult>`, `find_with_chain(project_id, run_id, execution_id, result_id) -> Option<TestCaseResult>` (validates the full parent chain), `update(result: &TestCaseResult) -> Result<()>`, `touch_execution(execution_id) -> Result<()>` (updates parent execution's `updated_at`). |
| `ExecutionTesterRepository` | Application (2) | Interface (port): `is_assigned_tester(execution_id, user_id) -> bool`. Checks the `execution_testers` junction table. |
| `TestCaseResultHandler` | Adapters (3) | HTTP handler for `update_result`. Deserializes request body, validates project/run/execution/result existence and chain integrity, calls service, serializes response. |
| `SqlTestCaseResultRepository` | Infrastructure (4) | Implements `TestCaseResultRepository` using PostgreSQL. `find_with_chain` executes a JOIN across `test_case_results`, `test_executions`, `test_runs` to validate the full parent chain. All queries include `WHERE deleted_at IS NULL`. |
| `SqlExecutionTesterRepository` | Infrastructure (4) | Implements `ExecutionTesterRepository`. Simple SELECT on `execution_testers` junction table. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (Application) | Add permission code `test_execution:update` to the permission registry if not already present from `test-execution-crud`. |
| HTTP router registration | Register the new `PATCH` route under the nested path. Route must be registered after `test-runs` and `test-executions` routes. |

---

## Route Registration

```text
PATCH /api/v1/projects/{projectId}/test-runs/{runId}/executions/{executionId}/results/{resultId}
  -> update_result
```

This route requires:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- Test Run existence validation (run not soft-deleted, belongs to project if run has `project_id`)
- Test Execution existence validation (execution not soft-deleted, belongs to run)
- Test Case Result existence validation (result not soft-deleted, belongs to execution)
- System permission check (`test_execution:update`)
- Fine-grained authorization:
  - Owner/Editor of project -> any result
  - Contributor -> own results (created_by matches) or assigned tester
  - Assigned tester -> any result in the execution
  - System Admin -> bypasses all

---

## Permission Code

This feature uses an existing permission code defined by `test-execution-crud`:

| code | name |
|------|------|
| `test_execution:update` | Update Test Execution |

If this permission code has not yet been seeded by the `test-execution-crud` feature, it
must be added to the `PERMISSIONS` table seed data. Use `INSERT ... ON CONFLICT (code)
DO NOTHING` and let the database assign IDs.

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks `test_execution:update` permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks project membership for the project-scoped run | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Contributor does not own the result and is not assigned tester | `403` | `FORBIDDEN` | INFO | Distinct message: "You can only update your own Test Case Results" |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked first |
| Test Run not found, soft-deleted, or wrong project | `404` | `NOT_FOUND` | INFO | Same generic message as project not found |
| Test Execution not found, soft-deleted, or wrong run | `404` | `NOT_FOUND` | INFO | Same generic message |
| Test Case Result not found, soft-deleted, or wrong execution | `404` | `NOT_FOUND` | INFO | Same generic message |
| Invalid status transition | `422` | `INVALID_STATUS_TRANSITION` | INFO | Includes valid transitions from current status in details |
| No fields provided (empty body) | `422` | `VALIDATION_ERROR` | INFO | At least one of `status` or `result_logs` required |
| `result_logs` exceeds 10000 characters | `422` | `VALIDATION_ERROR` | INFO | Field-level detail: "result_logs must not exceed 10000 characters" |
| `status` value is not a recognised status | `422` | `VALIDATION_ERROR` | INFO | Field-level detail: "status must be one of: IN_PROGRESS, PASS, FAIL, WARNING, IGNORE" |
| Unrecognised fields in request body | `422` | `VALIDATION_ERROR` | INFO | Strict mode: rejects typos. Details list unknown field names. |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Returns `Retry-After` header |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not reveal** whether a `404` is caused by a non-existent project, test run,
  execution, or result (same generic message for all).
- **Do not expose** which authorization gate rejected the request (generic `403 FORBIDDEN`
  except for the Contributor ownership case).
- **Do not allow** status transitions that skip IN_PROGRESS.
- **Do not allow** UPDATE on soft-deleted results (per FR-54c).
- **Do not modify** `tested_by` after the first transition from NOT_TESTED.
- **Do not hard-delete** any record.
- **Do not use `GET` with a body** -- the only state change is via `PATCH`.
