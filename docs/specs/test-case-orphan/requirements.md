# Feature: Orphan Test Case

## Overview

Orphan test cases are test cases created outside of any project context. They belong to no
project (`project_id IS NULL`) and are visible only to their creator and users explicitly
shared by the creator. Orphan test cases use the same underlying `TEST_CASES` table as
regular project-scoped test cases, with `project_id` set to `NULL`.

The feature allows users to draft test cases before deciding which project they belong to,
or to maintain personal test case collections. An orphan test case can later be assigned to
a project, at which point it transitions into a regular project-scoped test case and its
orpahn-specific sharing is removed (project membership takes over authorization).

Because orphan test cases have no project context, project-scoped FKs (`category_id`,
`priority_id`) are not settable while orphaned. They can only be set during the
assign-to-project operation or remain `NULL`.

---

## User Stories

### US-1: Create Orphan Test Case

As an authenticated user with the `test_case:create` system permission, I want to create a
test case without specifying a project, so that I can draft test scenarios before deciding
their project home or maintain personal test case collections.

**Acceptance Criteria (EARS)**

- WHEN an authenticated user with `test_case:create` permission (or a System Admin) sends a
  valid `POST /api/v1/test-cases/orphaned` request with `summary` and optional fields
  (`automated`, `description`, `notes`), THE SYSTEM SHALL insert the test case into
  `TEST_CASES` with `project_id = NULL` within a database transaction (to prevent TOCTOU
  races on the duplicate-summary check), set `created_by` and `updated_by` to the
  authenticated user's ID, and return `201 Created` with the test case representation and a
  `Location` header pointing to the new resource.
- IF the user does not hold the `test_case:create` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (generic message).
- IF `summary` is empty, contains only whitespace, or exceeds 500 characters, THE SYSTEM
  SHALL return `422 Unprocessable Entity` with field-level validation details. Leading and
  trailing whitespace is trimmed before length validation and storage.
- IF a non-deleted orphan test case with the same `summary` already exists for the same
  creator (case-insensitive comparison), THE SYSTEM SHALL return `409 Conflict` with error
  code `DUPLICATE_TEST_CASE_SUMMARY`.
- IF a soft-deleted orphan test case with the same `summary` exists for the same creator,
  THE SYSTEM SHALL allow creation of the new test case (the soft-deleted record does not
  prevent re-creation).
- IF `automated` is provided, THE SYSTEM SHALL accept only boolean `true` or `false`. Any
  other value (string, number, null) returns `422 Unprocessable Entity`.
- IF `automated` is omitted, THE SYSTEM SHALL default it to `false`.
- IF `description` is provided and exceeds 10000 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity`. An empty string `""` is stored as-is (distinct from `null`).
  Sending `"description": null` explicitly clears the field (sets it to `NULL` in the DB).
  Omitting `description` from the request body defaults to `null`.
- IF `notes` is provided and exceeds 5000 characters, THE SYSTEM SHALL return
  `422 Unprocessable Entity`. An empty string `""` is stored as-is (distinct from `null`).
  Sending `"notes": null` explicitly clears the field (sets it to `NULL` in the DB).
  Omitting `notes` from the request body defaults to `null`.
- IF the request body includes `category_id` or `priority_id`, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a message indicating these fields are not allowed for
  orphan test cases. Orphan test cases have no project context for FK validation.
- IF the request body includes unrecognised fields, THE SYSTEM SHALL return
  `422 Unprocessable Entity` (strict mode -- rejects typos and unknown fields for
  defence-in-depth).

### US-2: List and View Orphan Test Cases

As an authenticated user, I want to list and view orphan test cases that I created or that
have been shared with me, so that I can browse my personal test case drafts and view their
details.

**Acceptance Criteria (EARS)**

- WHEN an authenticated user with `test_case:read_orphaned` permission (or a System Admin)
  sends `GET /api/v1/test-cases/orphaned`, THE SYSTEM SHALL return a paginated list of
  non-deleted orphan test cases where the user is the creator (`created_by = <user_id>`) OR
  the user is in the `TEST_CASE_SHARES` table for that test case, ordered by `id`
  descending (newest first).
- IF the user does not hold the `test_case:read_orphaned` system permission AND is not a
  System Admin, THE SYSTEM SHALL return `403 Forbidden`.
- THE SYSTEM SHALL support pagination via query parameters `page` (default 1, minimum 1,
  maximum 1000) and `limit` (default 25, minimum 1, maximum 100). Requests with
  `page > 1000` return `422`.
- THE SYSTEM SHALL include pagination metadata (`total`, `page`, `limit`) in the response.
- THE SYSTEM SHALL support an optional `search` query parameter that performs a
  case-insensitive substring match against `summary` and `description`. The `search` value
  must not exceed 255 characters; longer values return `422`. The characters `%`, `_`, and
  `\` in the search string are escaped in this order: escape `\` to `\\` first, then `%` to `\%`, then `_` to `\_` (treated as literals, not ILIKE wildcards or escape
  characters).
- THE SYSTEM SHALL support an optional `automated` query parameter (boolean `true` or
  `false`) that filters test cases by their automation status. Invalid values return `422`.
- THE SYSTEM SHALL support an optional `sort` query parameter with values
  `-id` (default, newest first / descending), `id` (ascending, oldest first),
  `summary`, `-summary` (descending), `created_at`, `-created_at` (descending).
  Invalid sort values return `422`.
- THE SYSTEM SHALL support an optional `ownership` query parameter with values `mine`
  (test cases created by the authenticated user) and `shared` (test cases shared with the
  user). When omitted, both are included. Invalid values return `422`.
- WHILE a test case is soft-deleted (`deleted_at IS NOT NULL`), THE SYSTEM SHALL exclude
  it from the list.
- IF the user is a System Admin, THE SYSTEM SHALL return all non-deleted orphan test cases
  regardless of ownership or sharing (admin bypass).
- WHEN an authenticated user with `test_case:read_orphaned` permission (or a System Admin)
  sends `GET /api/v1/test-cases/orphaned/{id}`, THE SYSTEM SHALL return the full test case
  fields (id, summary, description, notes, automated, project_id, category_id, category_name,
  priority_id, priority_name, created_by, created_at, updated_by, updated_at), excluding
  soft-deleted test cases.
- IF the orphan test case does not exist, is soft-deleted, or is not visible to the user
  (the user is neither the creator nor a shared user), THE SYSTEM SHALL return
  `404 Not Found` (same message to avoid information leakage about which orphan test cases
  exist and who has access).
- THE SYSTEM SHALL resolve `category_name` by joining `TEST_CATEGORIES` on `category_id`.
  If `category_id` is `NULL` or the category is soft-deleted, `category_name` SHALL be
  `null`. For orphan test cases `category_id` is expected to be `NULL` (categories cannot
  be set until assignment to a project), but the resolution logic handles any value
  defensively.
- THE SYSTEM SHALL resolve `priority_name` by joining `TEST_PRIORITIES` on `priority_id`.
  If `priority_id` is `NULL` or the priority is soft-deleted, `priority_name` SHALL be
  `null`. Same rationale as `category_name`.
- THE SYSTEM SHALL NOT return `deleted_at` or `deleted_by` in the response body.
- THE SYSTEM SHALL include a `shared` boolean field in list and detail responses indicating
  whether the test case is shared with the authenticated user (`true`) or owned by the
  authenticated user (`false`). This allows the UI to distinguish between owned and shared
  items.

### US-3: Share Orphan Test Case

As the creator of an orphan test case, I want to share it with other users so that they can
view it and collaborate on the test case before it is assigned to a project.

**Acceptance Criteria (EARS)**

- WHEN the creator of an orphan test case (or a System Admin) sends
  `POST /api/v1/test-cases/orphaned/{id}/shares` with `{"user_ids": [2, 3, 4]}`, THE
  SYSTEM SHALL insert rows into `TEST_CASE_SHARES` for each user who is not already shared,
  set `shared_by` to the authenticated user's ID, and return `200 OK` with the updated list
  of shares.
- IF the user does not hold the `test_case:share` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the authenticated user is not the creator of the orphan test case AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (only the creator can share).
- IF the orphan test case does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF any `user_id` in the payload does not exist or belongs to an INACTIVE user, THE SYSTEM
  SHALL return `422 Unprocessable Entity` with field-level details identifying the invalid
  user IDs and the reason.
- IF any `user_id` is already shared, THE SYSTEM SHALL silently skip that user (idempotent
  -- no error, no duplicate share row).
- IF `user_ids` includes the creator's own user ID, THE SYSTEM SHALL silently skip it.
- IF `user_ids` is an empty array, THE SYSTEM SHALL return `200 OK` with the current share
  list (no-op).
- IF the test case has already been assigned to a project (`project_id IS NOT NULL`), THE
  SYSTEM SHALL return `422 Unprocessable Entity` with a message indicating that project
  test cases cannot be shared via the orphan sharing mechanism (project membership governs
  access for project-scoped test cases).
- WHEN the creator of an orphan test case (or a System Admin) sends
  `DELETE /api/v1/test-cases/orphaned/{id}/shares/{userId}`, THE SYSTEM SHALL remove the
  share row and return `204 No Content`.
- IF the share does not exist, THE SYSTEM SHALL return `404 Not Found`.
- WHEN an authenticated user sends `GET /api/v1/test-cases/orphaned/{id}/shares`, THE
  SYSTEM SHALL return the list of users the test case is shared with, including each user's
  `id`, `username`, and `email`.
- IF the user does not have visibility of the orphan test case (neither creator nor shared
  user) AND is not a System Admin, THE SYSTEM SHALL return `404 Not Found`.
- Shared users have read-only access to the orphan test case. They cannot update, delete,
  assign, or re-share it.

### US-4: Update, Delete, and Assign to Project

As the creator of an orphan test case, I want to update its fields, soft-delete it, or
assign it to a project so that it becomes a regular project-scoped test case with full
category and priority metadata.

**Acceptance Criteria (EARS)**

#### Update

- WHEN the creator of an orphan test case (or a System Admin) sends
  `PATCH /api/v1/test-cases/orphaned/{id}` with one or more updatable fields (`summary`,
  `automated`, `description`, `notes`), THE SYSTEM SHALL apply the changes within a
  database transaction (to prevent TOCTOU races on the duplicate-summary check), set
  `updated_by` to the authenticated user's ID, set `updated_at = NOW()`, and return
  `200 OK` with the updated test case representation.
- IF the user does not hold the `test_case:update` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the authenticated user is not the creator of the orphan test case AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (only the creator can update; shared users
  have read-only access).
- IF the orphan test case does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF the updated `summary` conflicts with another non-deleted orphan test case belonging to
  the same creator (case-insensitive comparison), THE SYSTEM SHALL return `409 Conflict` with
  `DUPLICATE_TEST_CASE_SUMMARY`.
- IF `category_id` or `priority_id` is provided, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with a message that these fields cannot be set on orphan test
  cases outside of the assign-to-project operation.
- IF `project_id` is provided, THE SYSTEM SHALL treat the request as an assign-to-project
  operation (see below). If `project_id` is provided alongside `category_id` or
  `priority_id`, those are validated against the target project.
- IF no updatable fields are provided (empty body), THE SYSTEM SHALL return
  `422 Unprocessable Entity`.
- IF the request body includes unrecognised fields, THE SYSTEM SHALL reject the request with
  `422 Unprocessable Entity` (strict mode).
- THE SYSTEM SHALL NOT update `id`, `project_id` (except via assign-to-project), `created_at`,
  `created_by`, `deleted_at`, or `deleted_by`.
- Validation rules for `summary`, `automated`, `description`, and `notes` are identical to
  those in US-1 (create).

#### Assign to Project

- WHEN the creator of an orphan test case (or a System Admin) sends
  `PATCH /api/v1/test-cases/orphaned/{id}` with `{"project_id": 42}` and optionally
  `category_id` and/or `priority_id`, THE SYSTEM SHALL validate:
  1. The user holds `test_case:update` system permission (or is System Admin).
  2. The user is a Contributor, Editor, or Owner of the target project (or is System Admin).
  3. The target project exists and is not soft-deleted.
  4. If `category_id` is provided: the category exists, is non-deleted, and belongs to the
     target project.
  5. If `priority_id` is provided: the priority exists, is non-deleted, and belongs to the
     target project.
  6. The updated `summary` (if also being changed in the same request) does not conflict
     with an existing test case in the target project.
- Upon successful validation, THE SYSTEM SHALL within a single database transaction:
  - Update `project_id` to the target project ID.
  - Set `category_id` and `priority_id` if provided.
  - Update `updated_by` and `updated_at`.
  - Remove all rows from `TEST_CASE_SHARES` for this test case (orphan sharing is
    superseded by project membership).
  - Return `200 OK` with the updated test case representation.
- IF the user is not a member of the target project at a sufficient role AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the target project does not exist or is soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- IF `category_id` is provided and invalid (non-existent, soft-deleted, or wrong project),
  THE SYSTEM SHALL return `422 Unprocessable Entity` with error code `INVALID_CATEGORY`.
- IF `priority_id` is provided and invalid (non-existent, soft-deleted, or wrong project),
  THE SYSTEM SHALL return `422 Unprocessable Entity` with error code `INVALID_PRIORITY`.
- IF the updated summary conflicts with an existing non-deleted test case in the target
  project, THE SYSTEM SHALL return `409 Conflict` with `DUPLICATE_TEST_CASE_SUMMARY`.
- IF the request includes `project_id` but no other fields, THE SYSTEM SHALL assign the
  test case to the project while preserving all existing field values (only `project_id`,
  `updated_by`, and `updated_at` change).
- After successful assignment, the test case is no longer accessible via orphan test case
  endpoints. It is now accessible via the regular project-scoped test case endpoints
  (`/api/v1/projects/{projectId}/test-cases/{id}`). The `Location` header in the response
  reflects the new project-scoped URL.
- The response for an assign-to-project operation includes a `Location` header pointing to
  the new project-scoped resource URL.

#### Soft-Delete

- WHEN the creator of an orphan test case (or a System Admin) sends
  `DELETE /api/v1/test-cases/orphaned/{id}`, THE SYSTEM SHALL set
  `deleted_at = NOW()` and `deleted_by = <current_user_id>` and return `204 No Content`.
- IF the user does not hold the `test_case:delete` system permission AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden`.
- IF the authenticated user is not the creator of the orphan test case AND is not a System
  Admin, THE SYSTEM SHALL return `403 Forbidden` (shared users cannot delete).
- IF the orphan test case does not exist or is already soft-deleted, THE SYSTEM SHALL return
  `404 Not Found`.
- THE SYSTEM SHALL NOT check for referential integrity constraints before soft-deleting an
  orphan test case (same as project test cases -- test cases are leaf entities not
  referenced by other tables).
- THE SYSTEM SHALL NOT hard-delete any record.
- Soft-deleting an orphan test case does not cascade to `TEST_CASE_SHARES` (shares are
  preserved for audit trail; they are naturally excluded by queries that filter on
  `test_cases.deleted_at IS NULL`).

---

## Security Considerations

### Authentication

All endpoints require a valid authenticated session. Requests without a valid session
cookie return `401 Unauthorized` with error code `NOT_AUTHENTICATED`. This is enforced by
`AuthMiddleware` before any handler logic executes.

### Input Sanitization (XSS Prevention)

All user-supplied text fields (`summary`, `description`, `notes`) must be sanitized on
input to strip disallowed HTML tags and malicious script content, and output-encoded at the
presentation layer to prevent Cross-Site Scripting (XSS). This implements the project-wide
policy defined in PRD 5.3 (XSS prevention).

### CSRF Protection

All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must be protected against
Cross-Site Request Forgery (CSRF). Session cookies must carry the `SameSite=Lax` attribute
(or stricter). Implementations must verify `Content-Type` headers and/or include anti-CSRF
tokens for defense in depth.

### Authorization Model

Orphan test cases use a different authorization model than project-scoped test cases:

| Operation | Who Can Perform It |
|-----------|-------------------|
| Create | Any authenticated user with `test_case:create` permission (or System Admin) |
| List / View | Creator (`created_by`) or users in `TEST_CASE_SHARES` for that test case (or System Admin) |
| Share | Creator only, with `test_case:share` permission (or System Admin) |
| Update | Creator only, with `test_case:update` permission (or System Admin) |
| Delete | Creator only, with `test_case:delete` permission (or System Admin) |
| Assign to Project | Creator only, with `test_case:update` permission AND Contributor/Editor/Owner role in the target project (or System Admin) |

System Admin implicitly holds all system permissions and bypasses the creator-only
restriction (can view, update, delete, share, and assign any orphan test case).

Shared users have read-only access. The ownership-based model is simpler than the
project-scoped model because there are no roles -- there is only the creator and shared
viewers.

A `403 Forbidden` response for visibility failures (user is neither creator nor shared)
must return the same generic message as `404 Not Found` -- do not reveal whether the
orphan test case exists.

### Rate Limiting

All endpoints apply rate limiting configured at the infrastructure/middleware layer:

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../test-cases/orphaned` (list) | 60 requests | per minute |
| `GET .../test-cases/orphaned/{id}` (detail) | 60 requests | per minute |
| `POST .../test-cases/orphaned` (create) | 30 requests | per minute |
| `PATCH .../test-cases/orphaned/{id}` (update/assign) | 30 requests | per minute |
| `DELETE .../test-cases/orphaned/{id}` (delete) | 30 requests | per minute |
| `GET .../test-cases/orphaned/{id}/shares` (list shares) | 60 requests | per minute |
| `POST .../test-cases/orphaned/{id}/shares` (add shares) | 30 requests | per minute |
| `DELETE .../test-cases/orphaned/{id}/shares/{userId}` (remove share) | 30 requests | per minute |

Rate limit state is communicated via `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset` response headers. Exceeding the limit returns `429 Too Many Requests`
with a `Retry-After` header.

### Orphan-to-Project Transition Security

When an orphan test case is assigned to a project:

1. All orphan sharing rows are deleted. The previous sharing grants no longer apply after
   the transition; project membership takes over.
2. The system does not automatically grant access to project members who were previously
   shared users or deny access to project members who were not shared. Project membership
   rules apply uniformly after assignment.
3. The creator's audit trail (`created_by`) is preserved.
4. If `category_id` and `priority_id` are set during assignment, they are validated against
   the target project within the same transaction to prevent cross-project FK injection.

---

## Out of Scope

- **UI views** (pages, forms, list views -- covered in `ui-*` specs)
- **Bulk share/unshare** (only single-test-case sharing via a list of user IDs)
- **Share expiration** (shares are permanent until explicitly removed)
- **Sharing roles** (shared users have read-only access; no editor/contributor sharing
  roles for orphans -- project-scoped sharing roles are covered in `sharing-*` specs)
- **Notification on share** (email/in-app notification when shared -- covered in
  `notifications-*` specs in Phase 3)
- **Convert project test case to orphan** (test cases cannot be "unlinked" from a project
  once assigned; the `project_id` cannot be set back to `NULL`)
- **Hard delete** (not implemented for any entity in Phase 1)
- **Test case restore** (clearing `deleted_at` / `deleted_by` -- deferred to a future phase)
- **Bulk import/export** (CSV, Excel -- separate feature)

---

## Dependencies

- **IAM Auth** -- Session-based authentication; all endpoints require an authenticated
  session via `AuthMiddleware`.
- **IAM Permissions** -- The permission codes `test_case:create`, `test_case:read_orphaned`,
  `test_case:update`, `test_case:delete`, `test_case:share` must be seeded in the
  `PERMISSIONS` table and assignable to roles.
- **IAM Users** -- `users.id` FK reference for `created_by`, `updated_by`, `deleted_by`
  audit columns, and `TEST_CASE_SHARES.user_id` and `TEST_CASE_SHARES.shared_by`.
- **Test Case CRUD** -- The `TEST_CASES` table must exist; orphan test cases use the same
  table with `project_id = NULL`. This feature adds a migration to allow `project_id` to be
  nullable.
- **Project CRUD** -- The `PROJECTS` table must exist; the assign-to-project operation
  validates the target project exists and is non-deleted.
- **Project Members** -- Project membership and roles are used for authorization during the
  assign-to-project operation.
- **Metadata Categories** -- Categories are validated during assign-to-project when a
  `category_id` is provided.
- **Metadata Priorities** -- Priorities are validated during assign-to-project when a
  `priority_id` is provided.
- **UI List Views** -- The orphan test cases list view follows the standard list-view
  pattern.
