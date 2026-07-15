# Feature: Test Plan Runs

## Overview

Test Plan Runs enable linking Test Runs to a Test Plan so that execution progress
can be tracked in the context of a plan. A Test Plan can have zero or more linked
Test Runs; a Test Run may be linked to multiple Test Plans. The relationship is
stored in a junction table (`TEST_PLAN_TEST_RUNS`) with hard-delete semantics
for unlinking.

The feature provides three REST endpoints nested under the Test Plan resource path
(`/api/v1/test-plans/{id}/runs`). Authorization follows the same model as the
parent Test Plan: users with Owner or Editor role in any project linked to the
Test Plan can link and unlink Test Runs; any project member can view the linked
runs list.

---

## User Stories

### US-1: List Linked Test Runs

As a project member, I want to view the list of Test Runs linked to a Test Plan,
so that I can see the execution status of all runs associated with that plan.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_plan:read` permission (or a System Admin) sends
  `GET /api/v1/test-plans/{id}/runs`, THE SYSTEM SHALL return a paginated list of
  Test Runs linked to that Test Plan, ordered by `linked_at` descending (most
  recently linked first).
- IF the user does not hold the `test_plan:read` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `test_plan:read` but is not a member of any project linked to
  the Test Plan AND is not a System Admin, THE SYSTEM SHALL return `403 Forbidden`
  (same generic message).
- IF the Test Plan does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL support pagination via query parameters `page` (default 1, minimum 1,
  maximum 1000) and `limit` (default 25, minimum 1, maximum 100). Requests with
  `page > 1000` return `422`.
- THE SYSTEM SHALL include pagination metadata (`total`, `page`, `limit`) in the response.
- Each entry in the response SHALL include the Test Run's `id`, `summary`, `status`, and
  the link's `linked_by` (user ID) and `linked_at` timestamp.
- IF no Test Runs are linked to the Test Plan, THE SYSTEM SHALL return an empty `data`
  array with `total: 0`.
- IF the user is a System Admin, THE SYSTEM SHALL return all linked runs for the Test Plan
  regardless of the admin's project membership (admin bypass).

### US-2: Link Test Runs to a Test Plan

As a project Owner or Editor with `test_plan:update` permission, I want to link one
or more Test Runs to a Test Plan in a single request, so that I can efficiently
associate execution work with the planning context.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_plan:update` permission (or a System Admin) sends
  `POST /api/v1/test-plans/{id}/runs` with `{"run_ids": [101, 102, 103]}`,
  THE SYSTEM SHALL insert rows into the `TEST_PLAN_TEST_RUNS` junction table
  within a database transaction, set `linked_by` to the authenticated user's ID,
  set `linked_at = NOW()`, and return `200 OK` with a summary of linked run IDs.
- IF the user does not hold the `test_plan:update` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF the user holds `test_plan:update` but is not an Owner or Editor of any project
  linked to the Test Plan AND is not a System Admin, THE SYSTEM SHALL return
  `403 Forbidden` (same generic message).
- IF the Test Plan does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the `run_ids` array is empty or missing, THE SYSTEM SHALL return
  `422 Unprocessable Entity`.
- IF any run ID in the array does not correspond to an existing, non-deleted Test Run,
  THE SYSTEM SHALL return `422 Unprocessable Entity` with field-level details indicating
  which run IDs are invalid.
- IF any run ID in the array is already linked to this Test Plan, THE SYSTEM SHALL
  silently skip it (idempotent linking). The response SHALL include only the newly linked
  run IDs in the summary, not the duplicates.
- IF the request body includes unrecognised fields, THE SYSTEM SHALL return
  `422 Unprocessable Entity` (strict mode -- rejects typos and unknown fields for
  defence-in-depth).
- THE SYSTEM SHALL process all valid run IDs within a single database transaction. If
  any run ID is invalid, THE SYSTEM SHALL roll back the entire transaction and return
  `422 Unprocessable Entity` with all validation errors aggregated (fail-fast for the
  entire batch).

### US-3: Unlink a Test Run from a Test Plan

As a project Owner or Editor with `test_plan:update` permission, I want to remove
a Test Run link from a Test Plan, so that I can keep the plan's run list accurate
when a run is no longer relevant.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_plan:update` permission (or a System Admin) sends
  `DELETE /api/v1/test-plans/{id}/runs/{runId}`, THE SYSTEM SHALL hard-delete the row
  from `TEST_PLAN_TEST_RUNS` where `plan_id = {id} AND run_id = {runId}` and return
  `204 No Content`.
- IF the user does not hold the `test_plan:update` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the user holds `test_plan:update` but is not an Owner or Editor of any project
  linked to the Test Plan AND is not a System Admin, THE SYSTEM SHALL return
  `403 Forbidden`.
- IF the Test Plan does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the Test Run does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF no link exists between the Test Plan and the Test Run (already unlinked or never
  linked), THE SYSTEM SHALL return `404 Not Found`.
- THE SYSTEM SHALL NOT soft-delete the junction row. Unlinking is a hard delete
  (consistent with the project-wide convention for junction tables).

### US-4: Linked Runs Display on Test Plan Detail View

As a project member viewing a Test Plan's detail page, I want to see the list of
linked Test Runs with their current status, so that I can assess the plan's overall
execution progress at a glance.

**Acceptance Criteria (EARS)**

- WHEN the Test Plan detail view is rendered, THE UI SHALL call
  `GET /api/v1/test-plans/{id}/runs` to fetch the linked runs and display them in a
  paginated table or list.
- THE UI SHALL display for each linked run: its ID, summary, current status, and the
  date it was linked to the plan.
- IF no runs are linked, THE UI SHALL display an empty state message indicating that
  no Test Runs have been linked to this plan yet.
- THE UI SHALL provide action controls (buttons or icons) for Owners and Editors to
  link new runs and unlink existing runs, if their project role permits.

**Scope note:** The UI implementation of this user story is covered in the
`ui-list-views` and `ui-pagination` specs. This spec defines the API that the UI
consumes. The US-4 acceptance criteria are included here to ensure the API design
supports the display requirements.

---

## Security Considerations

### Authentication

All endpoints require a valid authenticated session. Requests without a valid session
cookie return `401 Unauthorized` with error code `NOT_AUTHENTICATED`. This is enforced
by `AuthMiddleware` before any handler logic executes.

### CSRF Protection

All state-changing endpoints (`POST`, `DELETE`) must be protected against Cross-Site
Request Forgery (CSRF). Session cookies must carry the `SameSite=Lax` attribute (or
stricter). Implementations must verify `Content-Type` headers and/or include
anti-CSRF tokens for defense in depth.

### Authorization Consistency

The system permission check is the first authorization gate; the project membership and
role check is the second gate. Both must pass (or the caller must be a System Admin).
Authorization follows the same model as the parent Test Plan resource:

- `test_plan:read` for listing linked runs; any project member across all linked projects
  can view.
- `test_plan:update` for linking and unlinking; only Owner or Editor in at least one
  linked project can mutate.

The `403 Forbidden` response must use a generic message that does not distinguish between
"missing system permission" and "wrong project role", preventing information leakage about
Test Plan existence or project membership.

Authorization checks are performed against live data on every request -- not cached in
the session. If a user's role or permissions are changed, the new authorization takes
effect on their next request.

### Rate Limiting

All endpoints apply rate limiting configured at the infrastructure/middleware layer:

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../test-plans/{id}/runs` (list) | 60 requests | per minute |
| `POST .../test-plans/{id}/runs` (link) | 30 requests | per minute |
| `DELETE .../test-plans/{id}/runs/{runId}` (unlink) | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### Input Validation

- `run_ids` array: each element must be a positive integer. Non-integer, zero, or negative
  values are rejected with `422`.
- Maximum 100 run IDs per `POST` request (prevents excessively large transactions).
  Larger batches return `422`.
- Duplicate run IDs within the `run_ids` array are deduplicated before processing.
- Unrecognised top-level fields in the `POST` body are rejected (strict mode).

---

## Out of Scope

- **Bulk unlink** (unlinking multiple Test Runs in a single request is not supported by
  `DELETE`; only single-run unlink via `DELETE /api/v1/test-plans/{id}/runs/{runId}`)
- **Linking Test Runs from the Test Run detail view** (linking is initiated from the
  Test Plan context only in Phase 1)
- **Reorder linked runs** (no ordering beyond `linked_at` descending; manual reordering
  is out of scope)
- **Soft-delete on the junction table** (unlinking is a hard delete per project-wide
  junction table convention)
- **Audit log for link/unlink** (the `linked_by` and `linked_at` columns provide basic
  attribution; a full audit log of link/unlink events is deferred to a cross-cutting
  audit feature)
- **Linking to soft-deleted Test Runs** (the API rejects references to soft-deleted runs)
- **Cross-Plan bulk link** (linking a run to multiple plans in one request -- each plan
  must be targeted separately)
- **UI views** (the list rendering, empty states, and link/unlink controls are covered
  in `ui-*` specs; this spec defines only the API contract and display requirements)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission codes `test_plan:read` and `test_plan:update`
  must be seeded in the `PERMISSIONS` table (defined in the `test-plan-crud` spec) and
  assignable to roles.
- **IAM Users** -- `users.id` FK reference for `linked_by` column.
- **Test Plan CRUD** -- The `TEST_PLANS` table must exist; the junction table references
  `test_plans(id)` via `plan_id` FK. The Test Plan's detail view includes the linked
  runs list.
- **Test Run CRUD** -- The `TEST_RUNS` table must exist; the junction table references
  `test_runs(id)` via `run_id` FK. The API validates that referenced runs exist and are
  not soft-deleted.
- **Project CRUD** -- Test Plan to Project linking determines authorization scope (the
  user must be an Owner or Editor in at least one project linked to the Test Plan for
  mutations).
- **Project Members** -- Project membership and roles (Owner, Editor, Contributor,
  Viewer) are used for authorization scope checks on the Test Plan.
- **Auth RBAC** -- System permission checks for `test_plan:read` and `test_plan:update`.
- **Auth Project Scope** -- Project membership scope check via the Test Plan's linked
  projects.
- **UI List Views** -- The linked runs list on the Test Plan detail view follows the
  standard list-view pattern (action icons, ID/Summary navigation).
- **UI Pagination** -- Paginated display of linked runs with configurable page sizes.
