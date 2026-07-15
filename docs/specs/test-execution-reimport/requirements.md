# Feature: Test Execution Re-import

## Overview

When a Test Execution is created and Test Cases are imported into it, the imported
records become **snapshots** (test_case_results) of the original Test Cases at that
point in time. If a Test Case is later updated (summary, description, priority),
its previously imported snapshots become stale.

The Re-import operation refreshes snapshot fields (summary, description, priority)
from the current Test Case values, while preserving execution state that has been
acted upon. This allows testers to pull in updated Test Case metadata without losing
their progress on results that have already been set.

---

## User Stories

### US-1: Re-import a Single Execution Snapshot

As a project member with the `test_execution:reimport` permission, I want to re-import
the current Test Case metadata into a single test case result snapshot, so that the
execution reflects the latest version of the Test Case without losing my test result,
logs, or attached files.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:reimport` permission (or a System Admin) sends
  `POST /api/v1/projects/{projectId}/test-runs/{runId}/executions/{executionId}/reimport`,
  THE SYSTEM SHALL look up the given Test Execution, verify it belongs to the given Test Run
  and Project, and identify the associated Test Case Result that was imported into this
  execution.

- WHEN the associated Test Case Result has a status of `NOT_TESTED`, THE SYSTEM SHALL:
  1. Fetch the current values of `summary`, `description`, and `priority_id` from the
     source `test_cases` row.
  2. Update the corresponding snapshot columns in `test_case_results` to match the current
     Test Case values.
  3. Set `updated_by` to the authenticated user's ID and `updated_at` to the current
     timestamp.
  4. Return `200 OK` with the updated test case result representation.

- IF the associated Test Case Result has a status other than `NOT_TESTED` (e.g., `PASS`,
  `FAIL`, `IN_PROGRESS`, `WARNING`, `IGNORE`), THE SYSTEM SHALL return
  `409 Conflict` with error code `RESULT_ALREADY_STARTED` and a message indicating that
  only NOT_TESTED results can be re-imported. The execution state (status, result_logs,
  attached files) SHALL NOT be modified.

- IF the source Test Case has been soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL
  still perform the re-import -- soft-deletion of the Test Case does not invalidate existing
  snapshots. The snapshot fields pull the current (possibly stale) values from the
  soft-deleted row.

- IF the re-import succeeds, the response SHALL include the updated snapshot fields
  (`summary`, `description`, `priority_id`, `priority_name`), the preserved execution
  fields (`status`, `result_logs`), and the standard audit fields (`created_at`, `created_by`,
  `updated_at`, `updated_by`).

### US-2: Re-import All NOT_TESTED Snapshots in an Execution

As a test lead, I want to re-import all NOT_TESTED test case results in a Test Execution
at once, so that I can bulk-refresh stale snapshots before starting a new test cycle
without manually re-importing each one.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_execution:reimport` permission (or a System Admin) sends
  `POST /api/v1/projects/{projectId}/test-runs/{runId}/executions/{executionId}/reimport`
  with an optional query parameter `scope=all`, THE SYSTEM SHALL iterate over every test
  case result in the execution that has a status of `NOT_TESTED`.

- FOR each NOT_TESTED result, THE SYSTEM SHALL refresh the snapshot fields (`summary`,
  `description`, `priority_id`) from the current Test Case values.

- Results with a status other than `NOT_TESTED` SHALL be skipped silently (not updated,
  not counted as failures).

- THE SYSTEM SHALL return `200 OK` with a response body containing:
  - `imported_count`: the number of results that were successfully refreshed
  - `skipped_count`: the number of results that were skipped (status != NOT_TESTED)
  - `failed_count`: the number of results that failed to refresh (e.g., source Test Case
    hard-deleted, FK constraint violation) -- expected to be zero in normal operation
  - `failures`: an array of objects, each with `result_id` and `reason`, for any
    individual refresh that failed

- IF the execution contains zero NOT_TESTED results, THE SYSTEM SHALL return `200 OK`
  with `imported_count: 0`, `skipped_count: N`, and `failed_count: 0`.

- THE SYSTEM SHALL NOT roll back the entire operation if a subset of re-imports fail.
  Each row's refresh is independent; failures are accumulated and reported.

- IF all re-imports fail (imported_count == 0 AND failed_count > 0), THE SYSTEM SHALL
  still return `200 OK` with the counts and failures. (The client can decide whether
  this state warrants user attention.)

- THE ENTIRE OPERATION SHALL execute within a single database transaction, so the result
  set is consistent. A database-level failure rolls back all changes.

### US-3: No-op When No Changes Are Needed

As an API consumer, I want re-importing a snapshot that is already up-to-date to be a
safe no-op, returning success without unnecessary writes.

**Acceptance Criteria (EARS)**

- WHEN the current Test Case values (`summary`, `description`, `priority_id`) match the
  snapshot values exactly (including `NULL` for `priority_id`), THE SYSTEM SHALL detect
  that no changes are needed and skip the UPDATE.

- THE SYSTEM SHALL return `200 OK` with the existing test case result representation
  and response metadata indicating `changed: false`.

- The `updated_at` timestamp SHALL NOT be modified when no changes are applied (the
  BEFORE UPDATE trigger is not fired).

- FOR the bulk re-import (scope=all), unchanged rows SHALL count toward `imported_count`
  (since the intent to refresh was fulfilled) but SHALL NOT incur a database write.
  Alternatively, unchanged rows MAY be excluded from `imported_count` and reported separately
  as `unchanged_count` -- the design specifies which approach is taken.

---

## Security Considerations

### Authentication

The endpoint requires a valid authenticated session. Requests without a valid session
cookie return `401 Unauthorized` with error code `NOT_AUTHENTICATED`. This is enforced
by `AuthMiddleware` before any handler logic executes.

### Authorization

Three-layer authorization applies:

1. **System permission check**: the authenticated user must hold the `test_execution:reimport`
   system permission, OR be a System Admin (who implicitly holds all permissions).
2. **Project membership check**: the user must be a member of the project. Viewers can read
   but cannot re-import ([role matrix TBD](#out-of-scope)). At minimum, the Contributor role
   is required for mutations.
3. **Ownership check (if applicable)**: if the Test Execution has an owner/creator, and the
   user is a Contributor, ownership-based restrictions may apply (see `test-execution-crud`
   for the role matrix).

System Admin bypasses all gates. A `403 Forbidden` response uses a generic message that
does not distinguish between "missing system permission" and "wrong project role".

### Input Sanitization

The re-import operation reads data from the database (Test Case rows) and writes it into
other database columns. It does not accept user-supplied text in the request body. The
snapshot fields (`summary`, `description`) were originally sanitized when the Test Case
was created or updated, so they are assumed clean. No additional sanitization is performed
at re-import time.

### Rate Limiting

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `POST .../reimport` (single) | 30 requests | per minute |
| `POST .../reimport?scope=all` (bulk) | 10 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

---

## Out of Scope

- **Partial re-import** (selecting specific fields to refresh, e.g., summary only but not
  priority). The operation always refreshes all three snapshot fields (summary, description,
  priority_id).
- **Re-importing results with non-NOT_TESTED status** -- these results represent tester
  work and their snapshots are intentionally frozen. A separate "force re-import" feature
  may be considered in a future phase if users need to override this behaviour.
- **Re-import with scope=changed** (only refresh snapshots that differ from source).
  The `scope=all` parameter in Phase 1 covers the entire bulk case; the no-op detection
  (US-3) handles individual unchanged rows.
- **Importing new Test Cases into an existing Execution** -- this operation only refreshes
  existing snapshots. Adding new Test Cases to an Execution is covered by
  `test-execution-import`.
- **Removing stale snapshots** (Test Cases that have been removed from the Test Run since
  the execution was created) -- deferred to a future feature.
- **Test Case status propagation** -- if a Test Case is soft-deleted, its snapshots are
  not automatically removed or flagged. Re-import still works on soft-deleted Test Cases.
- **Execution-scoped metadata refresh** (refresh the Execution's own name, description,
  testers from the Test Run) -- re-import only touches test case result snapshots.
- **UI views** -- the re-import button and confirmation dialog are covered in `ui-*` specs.

---

## Dependencies

- **IAM Auth** -- Session-based authentication; the endpoint requires a valid session.
- **IAM Permissions** -- The permission code `test_execution:reimport` must be seeded in
  the `PERMISSIONS` table and assignable to roles.
- **Test Case CRUD** -- The `test_cases` table is the source of truth for snapshot fields
  (`summary`, `description`, `priority_id`). The Test Case is identified by the
  `test_case_id` FK on the `test_case_results` row.
- **Test Run CRUD** -- The Test Run must exist and belong to the given Project.
- **Test Execution Import** -- The `test_case_results` table and the snapshot mechanism
  are defined by the import feature. Re-import depends on the same data model.
- **Metadata Priorities** -- The `priority_id` FK on `test_case_results` references
  `test_priorities`. On re-import, the new `priority_id` must pass FK validation
  (it comes from the source Test Case, which is already validated).
