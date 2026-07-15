# Feature: Sharing UI

## Overview

The sharing UI feature adds a tabular sharing interface to the create, update, and
detail views of all shareable objects (test cases, test plans, test runs, test
executions). Users with Editor access on an object can view, add, and remove sharing
entries to grant or revoke per-object access for other users and groups.

The feature includes both the frontend UI component and its backing API endpoints.

---

## User Stories

### US-1: View Sharing Entries on Object Detail

As a user with Editor access on a shareable object, I want to see a table of current
sharing entries on the object's detail page, so that I know who has access to this
object and at what level.

**Acceptance Criteria (EARS)**

- WHEN a user opens the detail page of a shareable object, THE SYSTEM SHALL display a
  "Sharing" section containing a table of current sharing entries for that object.
- THE SYSTEM SHALL display the following columns for each sharing entry:
  - **User/Group** -- The display name of the user or group the object is shared with.
  - **Role** -- The sharing role (Editor, Contributor, Viewer), displayed as a badge
    or label.
  - **Shared By** -- The display name of the user who created the sharing entry.
  - **Shared At** -- The date and time the sharing entry was created.
  - **Actions** -- A remove button (trash icon) to revoke the sharing entry.
- IF the user's effective role on the object is Editor, THE SYSTEM SHALL display the
  remove action button for each entry.
- IF the user's effective role is Contributor or Viewer, THE SYSTEM SHALL display the
  sharing entries table in read-only mode (no remove buttons, no add button).
- IF no sharing entries exist for the object, THE SYSTEM SHALL display an empty state
  message: "This object has not been shared with anyone yet."
- IF the sharing entries fail to load, THE SYSTEM SHALL display an error message with
  a retry button.

### US-2: Add Sharing Entry on Create and Update Views

As a user with Editor access on a shareable object, I want to add new sharing entries
on the create and update forms, so that I can grant access to other users or groups at
the same time as creating or editing the object.

**Acceptance Criteria (EARS)**

- WHEN a user creates a new shareable object, THE SYSTEM SHALL include a "Sharing"
  section on the create form, allowing the user to add sharing entries before the
  object is created.
- WHEN a user edits an existing shareable object, THE SYSTEM SHALL include a
  "Sharing" section on the update form, pre-populated with current sharing entries
  and allowing addition of new entries and removal of existing ones.
- THE SYSTEM SHALL provide an "Add" button in the sharing section that opens an inline
  form row with:
  - A user/group search and select field (typeahead/autocomplete).
  - A role dropdown (Editor, Contributor, Viewer).
  - Confirm and Cancel buttons.
- THE SYSTEM SHALL search users and groups via dedicated endpoints
  (`GET /api/v1/users/select` and `GET /api/v1/groups/select`), filtering to users
  and groups within the same project scope.
- THE SYSTEM SHALL prevent adding the currently authenticated user as a sharing
  target. The typeahead must exclude the current user.
- THE SYSTEM SHALL prevent adding duplicate entries -- if the selected user/group
  already has a sharing entry for this object, the confirm action must be rejected
  with a validation error.
- WHEN the create/update form is submitted, THE SYSTEM SHALL persist the sharing
  entries as part of the object's save operation (or immediately via the sharing API
  if the object already exists).
- IF the user's effective role on the object is not Editor (Contributor or Viewer),
  THE SYSTEM SHALL hide the "Add" button and render the sharing section in read-only
  mode.

### US-3: Remove Sharing Entry

As a user with Editor access on a shareable object, I want to remove an existing
sharing entry, so that I can revoke a user's or group's access to the object.

**Acceptance Criteria (EARS)**

- WHEN a user clicks the remove (trash) icon on a sharing entry row, THE SYSTEM SHALL
  prompt for confirmation with a dialog showing the user/group name and current role.
- IF the user confirms, THE SYSTEM SHALL send a DELETE request to the sharing API and
  remove the row from the table upon success.
- IF the user cancels, THE SYSTEM SHALL close the confirmation dialog without changes.
- IF the delete request fails, THE SYSTEM SHALL display an error message and keep the
  row in the table.
- THE SYSTEM SHALL optimistically remove the row from the UI immediately on confirm,
  then revert if the API call fails.

### US-4: Sharing Entries API Endpoints

As a frontend developer, I want RESTful API endpoints to manage sharing entries for
any shareable object type, so that the sharing UI can interact with the backend in a
consistent way regardless of object type.

**Acceptance Criteria (EARS)**

- WHEN `GET /api/v1/sharing/{objectType}/{objectId}` is called with a valid session,
  THE SYSTEM SHALL return all sharing entries for that object, including user/group
  names and the sharing role.
- The GET response SHALL include for each entry:
  - `id` -- Sharing entry ID.
  - `user_id` / `group_id` -- Target user or group ID (one is null).
  - `user_name` / `group_name` -- Display name of the target (resolved via JOIN).
  - `role` -- Sharing role string (`"editor"`, `"contributor"`, `"viewer"`).
  - `created_by` -- User ID of who created the entry.
  - `created_by_name` -- Display name of who created the entry.
  - `created_at` -- ISO 8601 timestamp.
- WHEN `POST /api/v1/sharing/{objectType}/{objectId}` is called with a valid session
  and a body containing `user_id` (or `group_id`) and `role`, THE SYSTEM SHALL create
  a sharing entry and return `201 Created` with the entry representation and a
  `Location` header.
- WHEN `DELETE /api/v1/sharing/{objectType}/{objectId}/{entryId}` is called with a
  valid session, THE SYSTEM SHALL remove the sharing entry and return `204 No
  Content`.
- IF the user lacks Editor access on the object, THE SYSTEM SHALL return `403
  Forbidden` for POST and DELETE.
- IF the object does not exist or is soft-deleted, THE SYSTEM SHALL return `404 Not
  Found`.
- IF the target user is the authenticated user, THE SYSTEM SHALL return `422
  Unprocessable Entity` with error code `SELF_SHARE_NOT_ALLOWED`.
- IF a sharing entry already exists for the given user/group on this object, THE
  SYSTEM SHALL return `409 Conflict` with error code `DUPLICATE_SHARING_ENTRY`.
- IF the sharing entry to delete does not exist, THE SYSTEM SHALL return `404 Not
  Found`.
- THE SYSTEM SHALL validate that `user_id` and `group_id` are mutually exclusive
  (exactly one must be provided).
- THE SYSTEM SHALL validate that `role` is one of `"editor"`, `"contributor"`,
  `"viewer"`.

---

## Security Considerations

### Authentication

All sharing endpoints require a valid authenticated session. Requests without a valid
session cookie return `401 Unauthorized`. This is enforced by `AuthMiddleware`.

### Authorization

- GET endpoints: require effective role Viewer or higher on the object (any user with
  access to the object can see who it is shared with).
- POST and DELETE endpoints: require effective role Editor on the object. Only users
  with Editor access can manage sharing entries.
- System Admin bypasses all checks.

### Input Validation

- `objectType` must be validated against the server-side allowlist of shareable
  object types (`test_case`, `test_plan`, `test_run`, `test_execution`).
- `user_id` and `group_id` must reference existing, active users/groups.
- `role` must be a valid sharing role string.

### CSRF Protection

POST and DELETE endpoints must be protected against CSRF (consistent with project-wide
policy: `SameSite=Lax` cookies, `Content-Type` header verification).

### Rate Limiting

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `GET .../sharing/{objectType}/{objectId}` | 60 requests | per minute |
| `POST .../sharing/{objectType}/{objectId}` | 30 requests | per minute |
| `DELETE .../sharing/{objectType}/{objectId}/{entryId}` | 30 requests | per minute |

---

## Out of Scope

- **Bulk sharing** -- Adding multiple users/groups in a single request. Each POST
  creates one sharing entry.
- **Sharing via link / token** -- No public link sharing in Phase 1. Sharing is
  user/group-based only.
- **Sharing email notifications** -- Users are not notified when an object is shared
  with them. Notifications are deferred to Phase 3.
- **Sharing entry edit** -- Sharing entries cannot be edited (role changed). The
  workflow is: remove the existing entry and create a new one with the desired role.
- **Sharing audit log UI** -- The sharing history is not displayed in the UI in Phase
  1. Audit columns exist in the database for future use.

---

## Dependencies

- **IAM Auth** -- Session-based authentication for all API calls.
- **IAM Users** -- User search/select endpoint for the typeahead; user display names.
- **IAM Groups** -- Group search/select endpoint for the typeahead; group display
  names.
- **sharing-roles** -- Sharing role enumeration for the role dropdown.
- **sharing-override** -- Authorization layer that consumes the sharing entries
  created by this UI.
- **Project Members** -- Project-scoped user search for the typeahead.
- **UI List Views / UI Components** -- The sharing table reuses the project's
  standard table component and modal/dialog patterns.
