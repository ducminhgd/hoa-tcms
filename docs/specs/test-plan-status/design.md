# Design: Test Plan Status Transition

## Architecture

The Test Plan Status Transition feature adds a state-machine validation layer on top of the
existing Test Plan entity. It introduces no new database tables. The feature spans the Domain,
Application, and Adapter layers, with a small Infrastructure impact (a query to check test run
statuses).

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - available_transitions  GET  /api/v1/test-plans/{id}/avail-transitions│  │
│  │  - transition_status      POST /api/v1/test-plans/{id}/transition-status│  │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  TestPlanService (extended):              │  │  - TestPlanStatus   (enum)│  │
│  │  - available_transitions                  │  │  - StatusTransition    │  │
│  │  - transition_status                      │  │    (value object)       │  │
│  │                                           │  └──────────────────────────┘  │
│  │  Interfaces:                              │                                │
│  │  - TestPlanRepository (extended port)     │                                │
│  │  - TestRunRepository (port, read-only)    │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestRunRepository  (implements read-only TestRunRepository)      │   │
│  │  - Existing SqlTestPlanRepository  (saves status change)               │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. The `available-transitions` endpoint requires `test_plan:read` permission and project
   membership on at least one linked project (or System Admin). It returns the list of
   valid target statuses reachable from the current state, each annotated with a `blocked`
   flag if the transition would fail at commit time.
2. The `transition-status` endpoint requires `test_plan:update` permission AND an Editor or
   Owner role in at least one linked project (or System Admin). It validates the requested transition
   against the state machine, checks for blocking test runs when transitioning to DONE, and
   commits the status change if all gates pass.

### Security Requirements

**Input validation:** The target status from the request body is validated against the
`TestPlanStatus` enum. Unrecognised values return `422 INVALID_STATUS_VALUE`. Strict mode
rejects unknown fields in the request body.

**Authorization layering:** Three independent gates:
1. System permission check (`test_plan:update` for POST, `test_plan:read` for GET)
2. Project role check (Editor or Owner for POST; any member for GET) in at least one linked project
3. System Admin bypasses both gates

**CSRF protection:** Same requirements as all state-changing endpoints (session cookie
`SameSite=Lax`, `Content-Type: application/json` verification).

---

## API Contract

### Common Error Response Format

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

### GET `/api/v1/test-plans/{id}/available-transitions`

Return the list of statuses reachable from the test plan's current status, each annotated
with a `blocked` flag and `blocked_reason` where applicable.

**Required Permission:** `test_plan:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Plan ID |

**Success Response:** `200 OK`

```json
{
  "data": [
    { "status": "IN_PROGRESS", "blocked": false, "blocked_reason": null },
    { "status": "CANCEL", "blocked": false, "blocked_reason": null }
  ]
}
```

For a plan at IN_PROGRESS with blocking test runs:

```json
{
  "data": [
    { "status": "DONE", "blocked": true, "blocked_reason": "2 test runs have IN PROGRESS results: [101, 104]" },
    { "status": "CANCEL", "blocked": false, "blocked_reason": null }
  ]
}
```

**Notes:**
- The order of entries matches the state-machine definition.
- `blocked` is computed by running the same validation logic as the transition endpoint,
  including the IN_PROGRESS test-run check for the DONE target.
- An empty `data` array is returned when the test plan is at a terminal status with no
  outgoing transitions (there are no terminal statuses in this state machine -- every status
  has at least one outgoing transition).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_plan:read` permission |
| `404` | `NOT_FOUND` | Test plan does not exist, is soft-deleted, or user is not a member of any linked project |

---

### POST `/api/v1/test-plans/{id}/transition-status`

Execute a status transition on a test plan. Validates the transition against the state machine
and, when transitioning to DONE, checks that no linked test run has IN_PROGRESS results.

**Required Permission:** `test_plan:update` AND project role Editor or Owner in at least one linked
project

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `id` | integer | Test Plan ID |

**Request Body:**

```json
{
  "status": "IN_PROGRESS"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `status` | string | Yes | Must be one of: `TODO`, `IN_PROGRESS`, `DONE`, `CANCEL` |

**Success Response:** `200 OK`

Response body is the full test plan representation (same shape as `GET /api/v1/test-plans/{id}`
from `test-plan-crud`), reflecting the new status and updated audit timestamps.

**Notes:**
- Unrecognised fields in the request body cause a `422 Unprocessable Entity` response
  (strict mode).
- The status value is case-sensitive -- only uppercase `TODO`, `IN_PROGRESS`, `DONE`,
  `CANCEL` are accepted.
- On success, `updated_at` is set by the database trigger; `updated_by` is set by the
  application layer to the authenticated user's ID.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_plan:update` permission or is not an Editor/Owner in at least one linked project |
| `404` | `NOT_FOUND` | Test plan does not exist or is soft-deleted |
| `409` | `TEST_RUNS_IN_PROGRESS` | Cannot transition to DONE -- one or more linked test runs have IN_PROGRESS results |
| `422` | `INVALID_TRANSITION` | The requested status is not reachable from the current status per the state machine |
| `422` | `NOOP_TRANSITION` | The test plan is already at the requested status |
| `422` | `INVALID_STATUS_VALUE` | The status field is missing or not one of the four recognised values |
| `422` | `VALIDATION_ERROR` | Unrecognised fields in request body, or other validation errors |

`409 Conflict` response body for blocked DONE transition:

```json
{
  "error": {
    "code": "TEST_RUNS_IN_PROGRESS",
    "message": "Cannot set status to DONE: 2 linked test runs have IN PROGRESS results.",
    "details": [
      { "field": "status", "message": "Test run IDs with IN PROGRESS results: [101, 104]" }
    ]
  }
}
```

`422` response body for invalid transition:

```json
{
  "error": {
    "code": "INVALID_TRANSITION",
    "message": "Cannot transition from TODO to DONE. Allowed transitions from TODO: IN_PROGRESS, CANCEL.",
    "details": [
      { "field": "status", "message": "Invalid transition for current status 'TODO'" }
    ]
  }
}
```

---

## Data Model

### No New Tables

This feature introduces no new database tables. Its validation logic operates on the
following existing tables (defined in other specs):

**TEST_PLANS** (owned by `test-plan-crud`):

| Column | Used By This Feature |
|--------|---------------------|
| `id` | Entity identity |
| `status` | Current status value (read on load, written on transition) |
| `deleted_at` | Soft-delete check (transition rejected if non-null) |
| `updated_by` | Set to authenticated user ID on status change |
| `updated_at` | Set by database trigger on any UPDATE |

**TEST_PLAN_TEST_RUNS** (owned by `test-plan-runs`, junction table):

| Column | Used By This Feature |
|--------|---------------------|
| `test_plan_id` | Find all linked test runs |
| `test_run_id` | Resolve to test run for IN_PROGRESS check |

**TEST_RUNS** (owned by `test-run-crud`):

| Column | Used By This Feature |
|--------|---------------------|
| `id` | Identity; returned in error message blocking run IDs |
| `deleted_at` | Exclude soft-deleted runs from IN_PROGRESS check |

**TEST_CASE_RESULTS** (owned by `test-execution-crud`):

| Column | Used By This Feature |
|--------|---------------------|
| `test_run_id` | Filter results by linked test runs |
| `status` | Check for `IN_PROGRESS` values |
| `deleted_at` | Exclude soft-deleted results from count |

### IN_PROGRESS Check Query

```sql
SELECT tr.id
FROM test_runs tr
JOIN test_plan_test_runs tpr ON tpr.test_run_id = tr.id
JOIN test_case_results tcr ON tcr.test_run_id = tr.id
WHERE tpr.test_plan_id = $1
  AND tr.deleted_at IS NULL
  AND tcr.deleted_at IS NULL
  AND tcr.status = 'IN_PROGRESS'
LIMIT 1
```

If any row is returned, the transition to DONE is blocked. The query uses `LIMIT 1` because
only existence matters (not the count) -- this is the fastest possible check. The actual
count of blocking runs (for the error message) is fetched in a separate query only when
blocked, using `SELECT DISTINCT tr.id` without the limit.

**Alternative: single query with count**

If the additional round-trip for the count is undesirable, a single query can be used:

```sql
SELECT COUNT(DISTINCT tr.id) AS blocking_count,
       ARRAY_AGG(DISTINCT tr.id ORDER BY tr.id) AS blocking_ids
FROM test_runs tr
JOIN test_plan_test_runs tpr ON tpr.test_run_id = tr.id
JOIN test_case_results tcr ON tcr.test_run_id = tr.id
WHERE tpr.test_plan_id = $1
  AND tr.deleted_at IS NULL
  AND tcr.deleted_at IS NULL
  AND tcr.status = 'IN_PROGRESS'
```

If `blocking_count = 0`, the transition proceeds. Otherwise, the transition is blocked and
`blocking_ids` is used in the error message.

---

## Sequence

### Transition Status Flow

```
Client                Handler             Service              Repo
  │                      │                   │                   │
  │  POST /transition    │                   │                   │
  │  {"status":"DONE"}   │                   │                   │
  │─────────────────────>│                   │                   │
  │                      │  validate body    │                   │
  │                      │  transition_status│                   │
  │                      │──────────────────>│                   │
  │                      │                   │  find_by_id       │
  │                      │                   │──────────────────>│
  │                      │                   │  TestPlan         │
  │                      │                   │<──────────────────│
  │                      │                   │                   │
  │                      │                   │  check permission │
  │                      │                   │  (test_plan:update│
  │                      │                   │   + Editor/Owner  │
  │                      │                   │   in at least one │
  │                      │                   │   project)        │
  │                      │                   │                   │
  │                      │                   │  ValidateTransition│
  │                      │                   │  (from=IN_PROGRESS│
  │                      │                   │   to=DONE)        │
  │                      │                   │                   │
  │                      │                   │  [if to==DONE]    │
  │                      │                   │  count_in_progress│
  │                      │                   │  _test_runs(id)   │
  │                      │                   │──────────────────>│
  │                      │                   │  0 IN_PROGRESS    │
  │                      │                   │<──────────────────│
  │                      │                   │                   │
  │                      │                   │  BEGIN TRANSACTION│
  │                      │                   │                   │
  │                      │                   │  SELECT FOR UPDATE│
  │                      │                   │  on test plan row │
  │                      │                   │──────────────────>│
  │                      │                   │  (lock acquired)  │
  │                      │                   │<──────────────────│
  │                      │                   │                   │
  │                      │                   │  Re-check         │
  │                      │                   │  IN_PROGRESS count│
  │                      │                   │  (guard against   │
  │                      │                   │   concurrent      │
  │                      │                   │   result updates) │
  │                      │                   │──────────────────>│
  │                      │                   │<──────────────────│
  │                      │                   │                   │
  │                      │                   │  update           │
  │                      │                   │  (set status,     │
  │                      │                   │   updated_by)     │
  │                      │                   │──────────────────>│
  │                      │                   │  OK               │
  │                      │                   │<──────────────────│
  │                      │                   │                   │
  │                      │                   │  COMMIT           │
  │                      │                   │                   │
  │                      │  200 OK           │                   │
  │                      │  (updated plan)   │                   │
  │<─────────────────────│                   │                   │
```

**Key design decisions:**

1. **Double-check IN_PROGRESS count inside the transaction:** The pre-validation check (step 4
   in the diagram) is optimistic -- it verifies the transition is likely valid. Inside the
   transaction, the check is repeated while holding a `SELECT ... FOR UPDATE` lock on the test
   plan row. This prevents a race where a linked test run's results are updated to IN_PROGRESS
   between the optimistic check and the status write.

2. **SELECT FOR UPDATE on the test plan row:** The lock serialises concurrent transitions on
   the same test plan. Concurrent transitions are not a common use case (one user at a time
   manages a test plan's status), so the lock is uncontended in practice. If two requests do
   race, the second one blocks on the lock, then sees the updated status and returns
   `422 NOOP_TRANSITION` or a valid new transition.

3. **No lock on test runs or test case results:** The IN_PROGRESS check reads the current
   committed state. The `SELECT ... FOR UPDATE` on the test plan row, combined with the
   re-check inside the transaction, provides sufficient isolation against concurrent writes
   to test case results without locking the entire result set.

### Available Transitions Flow

1. Client sends `GET /api/v1/test-plans/{id}/available-transitions` with session cookie.
2. `AuthMiddleware` validates the session and attaches user context.
3. Handler calls `TestPlanService::available_transitions(test_plan_id, current_user_id)`.
4. `TestPlanService` checks `test_plan:read` system permission.
5. `TestPlanService` checks the user is a member of at least one linked project (or
   System Admin).
6. `TestPlanService` loads the test plan via `TestPlanRepository::find_by_id(id)`.
   If not found or soft-deleted -> `404`.
7. `TestPlanService` calls `TestPlanStatus::valid_transitions_from(current_status)` to
   get the list of target statuses.
8. For each target status, if the target is DONE, call
   `TestRunRepository::count_in_progress_results(test_plan_id)` to determine the `blocked`
   flag. For all other targets, `blocked` is `false`.
9. Handler returns `200 OK` with the annotated list.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestPlanStatus` | Domain (1) | Enum with four variants: `Todo`, `InProgress`, `Done`, `Cancelled`. Method `valid_transitions_from(&self) -> Vec<Self>` returns reachable statuses. Method `validate_transition(from, to) -> Result<(), TransitionError>` validates a transition against the state machine. No framework imports. |
| `StatusTransition` | Domain (1) | Value object representing a single transition rule: `from: TestPlanStatus`, `to: TestPlanStatus`. Used internally by `TestPlanStatus` to define the transition table. |
| `TransitionError` | Domain (1) | Domain error enum with variants: `InvalidTransition { from, to, allowed }`, `NoopTransition { status }`, `TestRunsInProgress { run_ids }`. |
| `TestRunRepository` | Application (2) | Read-only interface (port): `count_in_progress_results(test_plan_id) -> Result<Vec<i64>>`. Returns the list of test run IDs that have IN_PROGRESS test case results, or an empty vector if none. The implementation lives in Infrastructure. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `TestPlanService` (Application) | Add two methods: `available_transitions(plan_id, user_id) -> Vec<AvailableTransition>` and `transition_status(plan_id, target_status, user_id) -> TestPlan`. Inject `TestRunRepository` dependency. |
| `TestPlanRepository` (Application) | Add method `find_by_id_with_lock(plan_id) -> Result<TestPlan>` for pessimistic locking during transition. This is a variant of `find_by_id` that adds `SELECT ... FOR UPDATE`. |
| `SqlTestPlanRepository` (Infrastructure) | Implement `find_by_id_with_lock`. |
| `SqlTestRunRepository` (Infrastructure) | New struct implementing `TestRunRepository`. Executes the IN_PROGRESS count query. |
| HTTP router registration | Register two new routes under `/api/v1/test-plans/{id}/`. Route `/available-transitions` (GET) before `/{id}` sub-routes to avoid conflicts. |

---

## Route Registration

```text
# Assuming test-plan-crud registers these base routes:
GET    /api/v1/test-plans                         -> list
POST   /api/v1/test-plans                         -> create
GET    /api/v1/test-plans/{id}                    -> get
PATCH  /api/v1/test-plans/{id}                    -> update
DELETE /api/v1/test-plans/{id}                    -> delete

# test-plan-status adds these:
GET    /api/v1/test-plans/{id}/available-transitions -> available_transitions
POST   /api/v1/test-plans/{id}/transition-status     -> transition_status
```

Note: The existing `PATCH /api/v1/test-plans/{id}` endpoint (from `test-plan-crud`) does NOT
accept `status` in its request body. All status changes go through the dedicated
`POST .../transition-status` endpoint to ensure state-machine validation is always applied.
This is a deliberate separation: PATCH updates metadata fields (name, description, type,
version); POST transition-status handles workflow state changes.

---

## Test Plan Status vs Existing PATCH Endpoint

| Concern | PATCH /api/v1/test-plans/{id} | POST .../transition-status |
|---------|------------------------------|---------------------------|
| Updates | name, description, type, version, etc. | status only |
| Validates | field-level constraints (length, uniqueness) | state machine, linked test run status |
| Permission | `test_plan:update` + Editor/Owner | Same |
| On DONE request | Rejected (status not accepted in PATCH body) | Accepted with IN_PROGRESS guard |

This separation means the `test-plan-crud` PATCH handler must explicitly reject the `status`
field in the request body (strict mode: unrecognised or forbidden field).

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks `test_plan:update` permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User is not Editor/Owner in at least one linked project | `403` | `FORBIDDEN` | INFO | Same generic message |
| Test plan not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Linked test runs have IN_PROGRESS results | `409` | `TEST_RUNS_IN_PROGRESS` | INFO | Includes blocking test run IDs in details |
| Invalid state-machine transition | `422` | `INVALID_TRANSITION` | INFO | Message lists allowed targets from current state |
| Already at requested status | `422` | `NOOP_TRANSITION` | INFO | Idempotency hint for clients |
| Unrecognised status value in request body | `422` | `INVALID_STATUS_VALUE` | INFO | Lists the four valid values |
| Unrecognised fields in request body | `422` | `VALIDATION_ERROR` | INFO | Strict mode; lists unrecognised field names |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not accept `status` in the generic PATCH endpoint** -- all status changes must go
  through `transition-status` to guarantee state-machine validation.
- **Do not silently ignore invalid transitions** -- return `422 INVALID_TRANSITION` with
  explicit guidance on what is allowed.
- **Do not hardcode allowed transitions in the HTTP handler** -- the handler delegates to
  `TestPlanStatus::validate_transition` in the Domain layer.
- **Do not return `200` with an error in the body** -- the HTTP status code IS the status.
- **Do not return `400 Bad Request`** for business-logic errors -- use `409 Conflict` or
  `422 Unprocessable Entity` as appropriate.
