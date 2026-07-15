# Tasks: Test Plan Status Transition

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Implement `TestPlanStatus` enum and state machine -- `requirements.md#State Machine`, `design.md#Components`
  - Define enum with four variants: `Todo`, `InProgress`, `Done`, `Cancelled`.
  - Implement `TestPlanStatus::valid_transitions_from(&self) -> Vec<TestPlanStatus>` that
    returns the reachable target statuses per the transition table.
  - Implement `TestPlanStatus::validate_transition(from, to) -> Result<(), TransitionError>`
    that checks if `(from, to)` is a valid transition.
  - Table-driven unit tests covering all 6 valid transitions and at least 6 invalid ones
    (e.g., TODO to DONE, DONE to CANCELLED, CANCELLED to DONE, etc.).
  - Test `valid_transitions_from` returns correct lists for each of the 4 statuses.
  - No framework imports; pure language types.

- [ ] 2. Implement `TransitionError` domain error enum -- `design.md#Components`, `design.md#Error Handling`
  - Variants: `InvalidTransition { from, to, allowed }`, `NoopTransition { status }`,
    `TestRunsInProgress { run_ids: Vec<i64> }`.
  - Each variant implements `Display` / `Error` trait with a human-readable message.
  - Unit tests that each variant formats correctly.

---

## Layer 2 -- Application

- [ ] 3. Define `TestRunRepository` interface (port) -- `design.md#Components`, `requirements.md#US-3`
  - Method: `async fn count_in_progress_results(&self, test_plan_id: i64) -> Result<Vec<i64>>`
    returning the list of test run IDs (possibly empty) that have IN_PROGRESS test case
    results linked to the given test plan.
  - Excludes soft-deleted test runs, soft-deleted test case results, and soft-deleted
    test-plan-run links.

- [ ] 4. Extend `TestPlanService` with status transition methods -- `requirements.md#US-1` through `US-3`, `design.md#Sequence`
  - Add `available_transitions(plan_id, user_id)`:
    - Checks `test_plan:read` permission and project membership.
    - Loads test plan; returns `404` if soft-deleted.
    - Calls `TestPlanStatus::valid_transitions_from(current)` to get reachable targets.
    - For DONE target, calls `TestRunRepository::count_in_progress_results()` to set
      `blocked` flag and `blocked_reason`.
  - Add `transition_status(plan_id, target_status, user_id)`:
    - Checks `test_plan:update` permission and Editor/Owner role on all linked projects.
    - Loads test plan; returns `404` if soft-deleted.
    - Calls `TestPlanStatus::validate_transition(current, target)`; returns
      `422 INVALID_TRANSITION` or `422 NOOP_TRANSITION` on failure.
    - If target is DONE: runs IN_PROGRESS check; returns `409 TEST_RUNS_IN_PROGRESS` if
      any blocking runs found.
    - Begins database transaction, acquires `SELECT FOR UPDATE` lock on the test plan row,
      re-checks IN_PROGRESS count if target is DONE (guards against concurrent result updates),
      updates `status` and `updated_by`, commits.
  - Inject `TestRunRepository` dependency into the service.

- [ ] 5. Write unit tests for `TestPlanService` transition methods -- `requirements.md#US-1` through `US-3`
  - Table-driven tests with mock `TestPlanRepository` and `TestRunRepository`:
    - Happy path: each of the 6 valid transitions.
    - Invalid transition returns `INVALID_TRANSITION`.
    - NOOP transition (TODO to TODO) returns `NOOP_TRANSITION`.
    - DONE blocked by IN_PROGRESS test runs returns `TEST_RUNS_IN_PROGRESS`.
    - DONE allowed with zero IN_PROGRESS results.
    - DONE allowed with zero linked test runs (empty plan).
    - Soft-deleted test plan returns `NOT_FOUND`.
    - Permission denied returns `FORBIDDEN`.
    - Available transitions includes `blocked` flag on DONE when IN_PROGRESS exists.
    - Available transitions for CANCELLED status returns `[TODO]`.

---

## Layer 3 -- Adapters (HTTP)

- [ ] 6. Implement `TestPlanStatusHandler` -- `design.md#API Contract`, `design.md#Components`
  - Two handler methods: `available_transitions` (GET), `transition_status` (POST).
  - Deserialize request body for POST (status field only; strict mode -- reject unknown
    fields).
  - Call `TestPlanService` methods.
  - Serialize responses with proper status codes:
    - `available_transitions`: `200 OK` with `data` array of `{status, blocked, blocked_reason}`.
    - `transition_status`: `200 OK` with full test plan representation.
  - Set `updated_by` on the test plan before saving.
  - Map `TransitionError` variants to HTTP responses per the error table in `design.md`.

- [ ] 7. Register routes in HTTP router -- `design.md#Route Registration`
  - `GET  /api/v1/test-plans/{id}/available-transitions` -> `available_transitions`
  - `POST /api/v1/test-plans/{id}/transition-status` -> `transition_status`
  - Both routes require session auth middleware.
  - Route order: register sub-resource routes after `/{id}` base route (these are
    static sub-paths and do not create conflicts since they use a different method or
    path prefix than `/{id}`).

- [ ] 8. Write integration tests for status transition endpoints -- `design.md#API Contract`
  - Full request/response cycle with a test database.
  - Test `GET /available-transitions` returns correct list for each starting status.
  - Test `GET /available-transitions` shows `blocked: true` on DONE when IN_PROGRESS
    test runs exist.
  - Test `POST /transition-status` with valid transition returns `200 OK` and updated
    status.
  - Test `POST /transition-status` with invalid transition returns `422 INVALID_TRANSITION`.
  - Test `POST /transition-status` with same-status returns `422 NOOP_TRANSITION`.
  - Test `POST /transition-status` to DONE blocked by IN_PROGRESS runs returns
    `409 TEST_RUNS_IN_PROGRESS`.
  - Test `POST /transition-status` on soft-deleted plan returns `404 NOT_FOUND`.
  - Test `POST /transition-status` with missing/wrong permission returns `403 FORBIDDEN`.
  - Test `POST /transition-status` with unrecognised status value returns
    `422 INVALID_STATUS_VALUE`.
  - Test `POST /transition-status` with unrecognised body field returns `422 VALIDATION_ERROR`.
  - Test `POST /transition-status` with Viewer role (not Editor/Owner) returns
    `403 FORBIDDEN`.
  - Test concurrent transitions on same plan (second request sees updated status).

---

## Infrastructure (Layer 4)

> **Note:** The `SqlTestRunRepository` implementation is small (a single SQL query).
> It is grouped here for completeness but should be implemented alongside the
> application-layer tasks (Task 3 and 4).

- [x] 8. Implement `SqlTestRunRepository` -- covered implicitly in Tasks 3-4
  - A single query: `SELECT COUNT(DISTINCT tr.id) ... WHERE tcr.status = 'IN_PROGRESS'`
    per the `design.md#IN_PROGRESS Check Query`.
  - Exists within the same database transaction as the status update when performing
    the guarded re-check.
