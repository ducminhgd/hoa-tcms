# Feature: Automation Script Fields

## Overview

Test cases gain two metadata fields -- `script` and `script_args` -- that store a path
or command and its arguments to trigger or reference external automation runs. The
system stores these fields but never executes scripts. External automation systems
read test cases via the API and use these fields to drive their own execution.

This is a Phase 2 feature. It extends the existing Test Case CRUD without introducing
new endpoints or permission codes. Permission follows the existing test case model:
Contributors can set script fields on their own test cases; Editors and Owners can set
them on any test case in the project.

---

## User Stories

### US-1: Set Script Fields on Test Case Create

As a project member with `test_case:create` permission, I want to provide `script` and
`script_args` when creating a test case, so that external automation systems can later
identify which script to run and with what parameters.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case:create` permission sends a valid
  `POST /api/v1/projects/{projectId}/test-cases` request that includes `script` and/or
  `script_args`, THE SYSTEM SHALL store both values as-is in the `TEST_CASES` table
  alongside the other test case fields, and return them in the `201 Created` response.
- IF `script` is omitted from the request body, THE SYSTEM SHALL default it to `null`.
- IF `script_args` is omitted from the request body, THE SYSTEM SHALL default it to `null`.
- IF `script` is provided and exceeds 2000 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with field-level validation details.
- IF `script_args` is provided and exceeds 5000 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with field-level validation details.
- IF `script` is provided as `null` (explicit), THE SYSTEM SHALL store `NULL`.
  An empty string `""` is stored as-is (distinct from `null`).
- IF `script_args` is provided as `null` (explicit), THE SYSTEM SHALL store `NULL`.
  An empty string `""` is stored as-is (distinct from `null`).
- THE SYSTEM SHALL NOT validate that `script` points to a valid file or command --
  the fields are opaque metadata strings.
- THE SYSTEM SHALL apply the same HTML sanitization to `script` and `script_args` as
  it applies to other text fields (`summary`, `description`, `notes`), stripping
  disallowed HTML tags for XSS prevention.
- IF the user does not hold the `test_case:create` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (same behaviour as existing test case
  creation).
- IF the user holds `test_case:create` but is a Viewer of the project AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (same behaviour as existing test case
  creation).

### US-2: Update Script Fields on Test Case

As a project member with `test_case:update` permission, I want to update the `script`
and `script_args` fields on an existing test case, so that automation references stay
current as the test suite evolves.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case:update` permission sends
  `PATCH /api/v1/projects/{projectId}/test-cases/{id}` with `script` and/or
  `script_args` in the request body, THE SYSTEM SHALL update the respective fields
  and return `200 OK` with the updated test case representation.
- IF `script` is provided and exceeds 2000 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity`.
- IF `script_args` is provided and exceeds 5000 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity`.
- IF `script` is omitted from the request body, THE SYSTEM SHALL preserve the current
  value (same PATCH semantics as `description` and `notes`).
- IF `script_args` is omitted from the request body, THE SYSTEM SHALL preserve the
  current value.
- IF `script` is provided as `null` (explicit), THE SYSTEM SHALL clear the field
  (set to `NULL`).
- IF `script_args` is provided as `null` (explicit), THE SYSTEM SHALL clear the field
  (set to `NULL`).
- IF `script` is provided as an empty string `""`, THE SYSTEM SHALL store the empty
  string (distinct from `null`).
- IF `script_args` is provided as an empty string `""`, THE SYSTEM SHALL store the
  empty string (distinct from `null`).
- THE SYSTEM SHALL apply HTML sanitization to `script` and `script_args` on update,
  same as create.
- IF the user is a Contributor and does not own the test case (`created_by` does not
  match), THE SYSTEM SHALL return `403 Forbidden` with the distinct Contributor
  ownership message (same ownership rule as all other test case fields).
- IF the test case is soft-deleted, THE SYSTEM SHALL return `404 Not Found` (same
  behaviour as existing test case update).

### US-3: View Script Fields in Test Case Responses

As a project member or an external automation system reading test cases via the API,
I want to see the `script` and `script_args` fields in list and detail responses, so
that I can determine which automation to trigger.

**Acceptance Criteria (EARS)**

- WHEN a user with `test_case:read` permission sends
  `GET /api/v1/projects/{projectId}/test-cases/{id}`, THE SYSTEM SHALL include
  `script` and `script_args` in the response body alongside the existing test case
  fields.
- WHEN a user with `test_case:read_list` permission sends
  `GET /api/v1/projects/{projectId}/test-cases`, THE SYSTEM SHALL include `script`
  and `script_args` in each item of the `data` array.
  - **Rationale:** Automation systems reading the list endpoint need `script` and
    `script_args` to discover which test cases have automation configured without
    fetching each detail individually. Unlike `description` and `notes` (which are
    large prose fields excluded from the list for size), `script` and `script_args`
    are short reference strings that enable the primary use case of this feature.
- IF `script` is `NULL`, THE SYSTEM SHALL return `"script": null` in the JSON
  response.
- IF `script_args` is `NULL`, THE SYSTEM SHALL return `"script_args": null` in the
  JSON response.
- WHEN a user with `test_case:select` permission sends
  `GET /api/v1/projects/{projectId}/test-cases/select`, THE SYSTEM SHALL NOT include
  `script` or `script_args` in the response (the select endpoint returns only `id`
  and `summary` for dropdown rendering; script fields are irrelevant to selection UI).
- THE SYSTEM SHALL apply output encoding to `script` and `script_args` values at the
  presentation layer for XSS prevention (same defence-in-depth as other text fields).
- IF the user lacks the required system permission or project membership (same as
  existing list/detail endpoints), THE SYSTEM SHALL return `403 Forbidden`.

---

## Security Considerations

### Authentication

No change from existing test case endpoints. All endpoints require a valid
authenticated session checked by `AuthMiddleware`.

### Input Sanitization (XSS Prevention)

`script` and `script_args` are text fields subject to the same XSS prevention policy
as `summary`, `description`, and `notes`:
- Input sanitization: strip disallowed HTML tags before storage.
- Output encoding: encode at the presentation layer before rendering in HTML contexts.

This implements the project-wide policy defined in PRD 5.3. The script fields carry
no elevated risk -- they are plain text metadata, not executable content within the
TCMS system itself.

### Authorization

No new permission codes are introduced. The existing `test_case:create`,
`test_case:update`, `test_case:read`, and `test_case:read_list` permissions govern
access to the script fields. The Contributor ownership rule applies to script fields
on update just as it applies to all other test case fields.

### Script Validation (Deliberately Absent)

The system deliberately does not validate that `script` points to an existing file,
directory, or executable. Reasons:
- The TCMS server may not have access to the file system where scripts reside
  (CI/CD runners, external automation agents).
- Script paths may use environment-specific variables or network paths that are not
  resolvable from the server.
- Validation would couple the TCMS to the automation infrastructure, violating the
  Clean Architecture principle of keeping Domain logic free of infrastructure
  concerns.

The fields are opaque metadata. Validation of script existence and correctness is
the responsibility of the external automation system at execution time.

### Rate Limiting

No new endpoints are introduced. Existing rate limits for test case endpoints apply:
- `GET .../test-cases` (list) -- 60 req/min
- `GET .../test-cases/{id}` (detail) -- 60 req/min
- `POST .../test-cases` (create) -- 30 req/min
- `PATCH .../test-cases/{id}` (update) -- 30 req/min

---

## Out of Scope

- **Script execution** -- the TCMS never executes scripts. This feature only stores
  metadata that external systems consume.
- **Script validation** -- no checking of file existence, path validity, or command
  syntax.
- **Automation result reporting** -- storing automation run results or linking them
  back to test cases is a separate feature.
- **Automation triggers** -- the TCMS does not trigger automation runs based on
  script fields. External systems poll the API and decide when to run.
- **New endpoints** -- no new API routes. The script fields are added to existing
  test case create, update, list, and detail endpoints.
- **New permission codes** -- the existing `test_case:*` permission set covers all
  operations.
- **UI views** -- updating forms, list columns, and detail views to display script
  fields is covered in `ui-*` specs.

---

## Dependencies

- **Test Case CRUD** -- The `TEST_CASES` table must exist with all Phase 1 columns.
  Script fields are added as new columns on this table. The test case create, update,
  list, and detail endpoints are extended (not replaced).
- **IAM Auth** -- Session-based authentication via `AuthMiddleware` (no change).
- **IAM Permissions** -- The existing `test_case:create`, `test_case:update`,
  `test_case:read`, and `test_case:read_list` permission codes cover script field
  access (no new codes needed).
- **Project Members** -- Contributor ownership rules apply to script fields on
  update (no change to authorization logic).
- **Soft Delete & Audit** -- The `BEFORE UPDATE` trigger on `TEST_CASES` already
  sets `updated_at = NOW()`. No trigger changes are needed for the new columns --
  `updated_at` is set whenever any column changes, including `script` and
  `script_args`.
