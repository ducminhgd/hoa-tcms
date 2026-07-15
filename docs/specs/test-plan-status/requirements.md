# Feature: Test Plan Status Transition

## Overview

Test Plan Status Transition adds a state-machine validation layer to test plan status changes.
It enforces which status transitions are allowed (e.g., you cannot jump from TODO directly to
DONE) and blocks completion when linked test runs still have IN_PROGRESS results. No new
database tables are introduced -- the feature adds validation logic to the existing
`TestPlanService` and exposes a dedicated transition endpoint.

---

## User Stories

### US-1: View Available Status Transitions

As a project member, I want to see which status transitions are available from the current
test plan status, so that the UI can render only actionable transition buttons and prevent
invalid state changes.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_plan:read` permission sends `GET /api/v1/test-plans/{id}/available-transitions`,
  THE SYSTEM SHALL return the list of target status values reachable from the test plan's
  current status, ordered by the state-machine definition.
- IF the test plan is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the user does not hold `test_plan:read` permission AND is not a System Admin, THE SYSTEM
  SHALL return `403 Forbidden`.
- IF the user is not a member of any linked project AND is not a System Admin, THE SYSTEM SHALL
  return `404 Not Found` (same as non-existent -- consistent with test-plan-crud detail
  endpoint).
- THE SYSTEM SHALL include a `blocked` flag and `blocked_reason` on any target status that
  would fail validation at transition time (e.g., DONE blocked because linked test runs
  have IN_PROGRESS results), so the UI can show the target as disabled with a tooltip.
- THE SYSTEM SHALL perform the same validation as the transition endpoint when computing
  the `blocked` flag, ensuring the endpoint and the preview are always consistent.

### US-2: Transition Test Plan Status

As a project Editor or Owner, I want to advance or revert a test plan's workflow status
(e.g., move from TODO to IN_PROGRESS, or reopen a DONE plan back to IN_PROGRESS), so that
the test plan reflects its current lifecycle stage.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_plan:update` permission sends `POST /api/v1/test-plans/{id}/transition-status`
  with a valid `{"status": "IN_PROGRESS"}`, THE SYSTEM SHALL update the test plan's status
  and return `200 OK` with the updated test plan representation.
- IF the requested transition is not valid per the state machine (e.g., TODO to DONE),
  THE SYSTEM SHALL return `422 Unprocessable Entity` with error code `INVALID_TRANSITION`
  and a message listing the allowed target statuses from the current state.
- IF the test plan is already at the requested status, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with error code `NOOP_TRANSITION`.
- IF the test plan is soft-deleted, THE SYSTEM SHALL return `404 Not Found`.
- IF the user does not hold `test_plan:update` permission or is not an Editor or Owner in
  at least one linked project AND is not a System Admin, THE SYSTEM SHALL return
  `403 Forbidden`.
- IF the requested status is not a recognised status value, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with `INVALID_STATUS_VALUE`.
- IF the target status is `CANCEL`, no linked-test-run validation is performed (cancelling
  is always allowed from TODO and IN_PROGRESS).

### US-3: Prevent Completion When Linked Test Runs Are In Progress

As a project Editor or Owner, I want the system to block transitioning a test plan to DONE
while any linked test run has test case results still IN_PROGRESS, so that we do not
accidentally close a test plan with unfinished execution work.

**Acceptance Criteria (EARS)**

- WHEN a user attempts to transition a test plan to `DONE` AND one or more test runs linked
  to this test plan contain test case results with `status = 'IN_PROGRESS'`, THE SYSTEM SHALL
  return `409 Conflict` with error code `TEST_RUNS_IN_PROGRESS` and a message listing the
  blocking test run IDs.
- WHEN a user attempts to transition a test plan to `DONE` AND all linked test runs have
  zero IN_PROGRESS results (all are PASS, FAIL, NOT_TESTED, WARNING, IGNORE), THE SYSTEM
  SHALL allow the transition.
- WHEN a user attempts to transition a test plan to `DONE` AND the test plan has zero linked
  test runs, THE SYSTEM SHALL allow the transition (an empty plan can be completed).
- THE SYSTEM SHALL only query non-deleted test runs and non-deleted test case results
  (soft-deleted records are excluded from the IN_PROGRESS check).
- THE SYSTEM SHALL perform the IN_PROGRESS check within a short database query (no long-lived
  application-level loop); the check must execute as a single `SELECT COUNT(*)` or
  `SELECT EXISTS` statement.

---

## State Machine

```
             ┌──────────┐
             │   TODO   │
             └────┬─────┘
                  │
       ┌──────────┼──────────┐
       ▼          ▼          │
  ┌──────────┐   ┌───────────┐
  │IN_PROGRESS│   │ CANCEL │
  └────┬─────┘   └───────────┘
       │               ▲
       ├──────────┐    │
       ▼          │    │
  ┌──────────┐    │    │
  │   DONE   │────┘    │
  └──────────┘         │
       ▲               │
       └───────────────┘
```

**Valid Transitions:**

| From | To | Note |
|------|----|------|
| `TODO` | `IN_PROGRESS` | Start work on the plan |
| `TODO` | `CANCEL` | Cancel before starting |
| `IN_PROGRESS` | `DONE` | Complete the plan (requires no IN_PROGRESS test runs) |
| `IN_PROGRESS` | `CANCEL` | Cancel mid-flight |
| `DONE` | `IN_PROGRESS` | Reopen a completed plan |
| `CANCEL` | `TODO` | Reactivate a cancelled plan |

Any transition NOT listed above is invalid and must return `422 INVALID_TRANSITION`.

---

## Security Considerations

**Authorization layering:** The transition endpoint requires both the `test_plan:update`
system permission AND an Editor or Owner role in at least one linked project. The System Admin
role bypasses both gates. The available-transitions endpoint requires `test_plan:read` and
basic project membership on at least one linked project.

**CSRF protection:** The `POST /api/v1/test-plans/{id}/transition-status` endpoint is
state-changing and must be protected against CSRF (session cookie with `SameSite=Lax`,
`Content-Type: application/json` header verification).

**Input sanitization:** The status value in the request body is validated against a known
enum -- no wildcard string processing or HTML rendering applies. Standard strict-mode
unrecognized field rejection applies.

**Rate limiting:** The transition endpoint allows 15 req/min (lower than standard
PATCH due to the cross-entity validation query).

---

## Out of Scope

- **Test Plan CRUD** (create, read, update fields other than status, delete -- covered in
  `test-plan-crud` spec)
- **Linking/unlinking test runs** to a test plan (covered in `test-plan-runs` spec)
- **Test Run status transitions** (each test run manages its own status independently)
- **Custom workflows** (the state machine is fixed; configurable workflows are deferred)
- **Bulk status transitions** (one test plan per request)
- **Status change notifications** (covered in `notifications-email` spec, Phase 3)

---

## Dependencies

- **test-plan-crud** -- Test plans must exist with a `status` column (TODO, IN_PROGRESS,
  DONE, CANCEL). The test plan table, repository, service, and handler are defined in
  that spec.
- **test-plan-runs** -- The link table between test plans and test runs must exist so the
  IN_PROGRESS check can query linked test runs. If the link table does not yet exist when
  this feature is implemented, the IN_PROGRESS check degrades gracefully (empty link set,
  transition allowed).
- **test-run-crud** / **test-execution-crud** -- The test run and test case result tables
  must exist for the `SELECT COUNT(*)` query that checks for IN_PROGRESS results.
- **soft-delete / audit-columns** -- The test plan table uses `deleted_at` for soft-delete
  and audit columns (`updated_at`, `updated_by`) that are set on status change.
- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated session.
- **IAM Permissions** -- The permission code `test_plan:update` must exist (seeded by
  `test-plan-crud` or this spec's seed migration).
