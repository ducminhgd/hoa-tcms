# Feature: Test Run Executions

## Overview

Display the list of Test Executions linked to a Test Run on the run detail view.
The foreign key is from Test Executions (`test_run_id`) to Test Runs, defined in
`test-execution-crud`. This is a read-only display feature -- no new database table,
just a JOIN query over existing data. Any project member can view the linked
executions.

## User Stories

### US-01: List Executions Linked to a Test Run

As a project member viewing a Test Run's detail page, I want to see all Test
Executions that are linked to this run, so that I can understand what execution
cycles have been created from this run and navigate to their detail pages.

**Acceptance Criteria (EARS)**

- WHEN I submit a `GET /api/v1/projects/{projectId}/test-runs/{id}/executions`
  request with a valid session, THE SYSTEM SHALL return `200 OK` with a paginated
  list of Test Executions linked to this Test Run, ordered by `created_at`
  descending (newest first).
- IF no executions are linked to the run, THE SYSTEM SHALL return `200 OK` with an
  empty `data` array and `meta.total = 0`.
- IF the Test Run does not exist, is soft-deleted, or belongs to a different
  project, THE SYSTEM SHALL return `404 Not Found`.
- IF the Project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the user is not authenticated, THE SYSTEM SHALL return `401 Not Authenticated`.
- IF the user lacks the required system permission or is not a member of the
  project, THE SYSTEM SHALL return `403 Forbidden` with a generic message that
  does not distinguish between "missing permission" and "wrong project role".

### US-02: Execution Summary Fields on List

As a project member viewing the linked executions list, I want to see key summary
fields for each execution (name, status, testers, dates) so that I can quickly
assess the state of each execution cycle without navigating to its detail page.

**Acceptance Criteria (EARS)**

- WHEN the linked executions list is returned, THE SYSTEM SHALL include for each
  execution: `id`, `name`, `status`, `created_at`, and `updated_at`.
- WHEN the execution status corresponds to a known enumeration, THE SYSTEM SHALL
  return the status value in its canonical string form.
- WHEN the execution has testers assigned, THE SYSTEM SHALL include a `tester_count`
  field indicating the number of assigned testers.
- WHEN the execution has imported test cases, THE SYSTEM SHALL include a
  `case_count` field indicating the number of imported test cases.
- IF the execution is soft-deleted, THE SYSTEM SHALL exclude it from the list
  (`WHERE deleted_at IS NULL`).

## Security Considerations

### Input Validation

The `projectId` and `id` path parameters must be validated as positive integers.
Invalid path parameters return `404 Not Found` (do not distinguish between
non-existent and malformed).

### Authorization Layering

Two independent gates protect this endpoint:

1. **System permission check**: the caller must hold the `test_run:read` system
   permission (defined in `test-run-crud`).
2. **Project membership check**: the caller must be a member of the project (any
   role -- Owner, Editor, Contributor, or Viewer).

Both gates must pass, or the caller must be a System Admin who implicitly holds
all system permissions and bypasses all project membership checks. A `403 Forbidden`
response must use a generic message that does not distinguish between "missing
system permission" and "wrong project role".

### Data Exposure

- The execution list does not return full execution details (no test case results,
  logs, or file attachments). Only summary-level fields are exposed.
- No new data is created or modified by this endpoint -- it is purely read-only.
- Only executions belonging to the specified Test Run and not soft-deleted are
  returned.

### Rate Limiting

This is a read endpoint. Apply the standard read rate limit: 60 requests per
minute per user.

## Out of Scope

- Creating, updating, or deleting Test Executions (handled by
  `test-execution-crud`).
- Filtering or sorting the executions list beyond the default sort
  (`created_at` descending). Advanced filtering can be added in a future
  iteration if needed.
- Displaying execution detail inline (the UI navigates to the execution detail
  page by ID).
- Bulk operations on linked executions.
- Aggregating execution statistics across the run (handled by
  `test-run-statistics`).

## Dependencies

- **TEST_RUNS table** -- must exist with `id` PK and `project_id` FK (from
  `test-run-crud` feature).
- **TEST_EXECUTIONS table** -- must exist with `test_run_id` FK referencing
  `test_runs(id)` and `deleted_at` column (from `test-execution-crud` feature).
- **Auth middleware** -- session verification must be in place to authenticate
  requests (from `iam-auth` feature).
- **System permission `test_run:read`** -- required for accessing this endpoint
  (from `test-run-crud` / `iam-permissions` feature).
- **Project membership scope check** -- required to verify the caller is a
  member of the project (from `auth-project-scope` or `project-members` feature).
