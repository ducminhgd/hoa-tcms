# Tasks: Sharing UI

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work. Frontend tasks depend on the backend API being available.

---

## Layer 1 -- Domain

- [ ] 1. Define sharing-related DTOs -- `design.md#API Contract`
  - `CreateSharingEntryCommand` -- `user_id` (optional), `group_id` (optional),
    `role` (SharingRole). Validates exactly one of `user_id` or `group_id` is
    present.
  - `SharingEntryResponse` -- `id`, `user_id`, `user_name`, `group_id`, `group_name`,
    `role` (string), `created_by`, `created_by_name`, `created_at` (ISO 8601 string)
  - `SharingEntryListResponse` -- `data: Vec<SharingEntryResponse>`
  - `SharingEntryTarget` enum -- `User(user_id)` or `Group(group_id)`, used in
    duplicate/exists checks

---

## Layer 2 -- Application

- [ ] 2. Define `SharingRepository` interface extensions for UI --
      `design.md#Components`
  - In addition to the methods defined in `sharing-roles`:
    - `find_by_object_with_names(object_type, object_id) ->
      Vec<SharingEntryResponse>` -- same as `find_by_object` but includes resolved
      `user_name`, `group_name`, and `created_by_name` via JOINs
    - `find_by_id(entry_id) -> Option<SharingEntry>` -- for delete validation
    - `delete_by_id(entry_id) -> bool` -- returns true if a row was deleted

- [ ] 3. Implement `SharingService` -- `requirements.md#US-4`, `design.md#API Contract`
  - `list_entries(object_type, object_id, current_user_id) ->
    Vec<SharingEntryResponse>`:
    - Validates `object_type` against the server-side allowlist
    - Checks effective role >= Viewer on the object (via `AuthorizationService`)
    - Queries repository for all sharing entries with resolved names
    - Returns the list ordered by `created_at` DESC
  - `create_entry(object_type, object_id, cmd, current_user_id) ->
    SharingEntryResponse`:
    - Validates `object_type` against allowlist
    - Checks effective role >= Editor on the object (via `AuthorizationService`)
    - Validates max-role constraint: the user's effective role must be >= the
      requested role (via `SharingRole::can_assign`)
    - Validates `user_id != current_user_id` (self-share prevention)
    - Validates exactly one of `user_id` or `group_id` is present and non-null
    - Validates referenced user/group exists and is active
    - Checks no duplicate entry exists for the same target on this object
    - Constructs `SharingEntry` entity, sets `created_by = current_user_id`
    - Saves via repository, maps to `SharingEntryResponse` with resolved names
    - Returns the response
  - `delete_entry(entry_id, object_type, object_id, current_user_id)`:
    - Validates `object_type` against allowlist
    - Checks effective role >= Editor on the object (via `AuthorizationService`)
    - Verifies the sharing entry exists and belongs to the specified object
    - Deletes via repository
  - System Admin bypasses all effective role checks
  - Write unit tests with mocks:
    - List entries for an object (with both user and group entries)
    - List entries for an object with no entries (empty list)
    - Create entry for a user with valid role
    - Create entry for a group with valid role
    - Reject create when effective role < Editor (Contributor, Viewer)
    - Reject create when max-role constraint violated
    - Reject create when self-sharing
    - Reject create when both user_id and group_id provided
    - Reject create when neither user_id nor group_id provided
    - Reject create when duplicate entry exists
    - Reject create when referenced user/group does not exist
    - Reject create with invalid role string
    - Delete entry successfully
    - Reject delete when effective role < Editor
    - Reject delete when entry does not exist
    - Reject delete when entry belongs to a different object
    - Reject all operations with invalid object_type
    - System Admin bypasses role checks for all operations

---

## Layer 3 -- Adapters (HTTP)

- [ ] 4. Implement `SharingHandler` -- `design.md#API Contract`
  - Three handler methods: `list`, `create`, `delete`
  - `list`:
    - Parse `objectType` and `objectId` from path parameters
    - Validate `objectType` against allowlist (return 422 if invalid)
    - Call `SharingService::list_entries`
    - Map errors to HTTP status codes (see error handling table)
    - Return `200 OK` with `SharingEntryListResponse`
  - `create`:
    - Parse `objectType` and `objectId` from path
    - Parse and deserialize request body into `CreateSharingEntryCommand`
    - Validate the body (strict mode -- reject unrecognised fields)
    - Call `SharingService::create_entry`
    - On success: set `Location` header, return `201 Created`
    - On `DuplicateSharingEntryError`: return `409 Conflict`
    - On `SelfShareError`: return `422` with `SELF_SHARE_NOT_ALLOWED`
    - On validation errors: return `422` with field-level details
  - `delete`:
    - Parse `objectType`, `objectId`, and `entryId` from path
    - Validate `objectType` against allowlist
    - Call `SharingService::delete_entry`
    - Return `204 No Content`

- [ ] 5. Register sharing routes in HTTP router -- `design.md#Route Registration`
  - `GET    /api/v1/sharing/{objectType}/{objectId}`             -> `list`
  - `POST   /api/v1/sharing/{objectType}/{objectId}`             -> `create`
  - `DELETE /api/v1/sharing/{objectType}/{objectId}/{entryId}`   -> `delete`
  - All routes require session auth middleware
  - No route order constraints (all three paths are distinguishable)

- [ ] 6. Write integration tests for sharing API handlers -- `design.md#API Contract`
  - Full request/response cycle with a test database (transactional, rolled back)
  - Test GET list with no entries -> `200 OK`, empty `data: []`
  - Test GET list with entries -> verify each field: id, user_id/user_name,
    group_id/group_name, role, created_by/created_by_name, created_at
  - Test GET list ordered by `created_at` DESC
  - Test POST create for user target -> `201 Created` with `Location` header
  - Test POST create for group target -> `201 Created`
  - Test POST create with `role: "editor"`, `role: "contributor"`, `role: "viewer"`
  - Test POST body validation: missing role, invalid role, missing both user_id and
    group_id, providing both
  - Test POST with `user_id` matching authenticated user -> `422 SELF_SHARE_NOT_ALLOWED`
  - Test POST with duplicate user+object -> `409 DUPLICATE_SHARING_ENTRY`
  - Test POST with duplicate group+object -> `409 DUPLICATE_SHARING_ENTRY`
  - Test POST with non-existent user_id -> `422`
  - Test POST with non-existent group_id -> `422`
  - Test POST with inactive user/group -> `422`
  - Test POST with max-role constraint violated -> `403`
  - Test POST with unrecognised fields in body -> `422` (strict mode)
  - Test DELETE -> `204 No Content`
  - Test DELETE on non-existent entry -> `404`
  - Test DELETE on entry belonging to a different object -> `404`
  - Test all operations with invalid `objectType` -> `422`
  - Test `401` when no session
  - Test `403` when effective role < Editor on POST and DELETE
  - Test `403` when effective role < Viewer on GET
  - Test System Admin can perform all operations on any object
  - Test that GET works with Viewer effective role
  - Test that POST/DELETE work with Editor effective role

---

## Layer 4 -- Infrastructure

- [ ] 7. Implement `SqlSharingRepository` UI-specific methods -- `design.md#Data Model`
  - `find_by_object_with_names`: SQL query with LEFT JOINs on `users` (twice: for
    `user_id` and `created_by`) and `groups` (for `group_id`)
    ```sql
    SELECT se.id, se.user_id, u.display_name AS user_name,
           se.group_id, g.name AS group_name,
           se.role, se.created_by,
           cb.display_name AS created_by_name,
           se.created_at
    FROM sharing_entries se
    LEFT JOIN users u ON se.user_id = u.id
    LEFT JOIN groups g ON se.group_id = g.id
    LEFT JOIN users cb ON se.created_by = cb.id
    WHERE se.object_type = $1 AND se.object_id = $2
    ORDER BY se.created_at DESC
    ```
  - `find_by_id`: SELECT WHERE `id = $1`
  - `delete_by_id`: DELETE WHERE `id = $1`; return true if affected rows > 0
  - Write unit tests:
    - Retrieve entries with user names resolved (user_id scenario)
    - Retrieve entries with group names resolved (group_id scenario)
    - Retrieve entries with created_by_name resolved
    - Verify ordering by created_at DESC
    - Delete by id returns true when row exists
    - Delete by id returns false when row does not exist
    - Find by id returns None for non-existent entry

- [ ] 8. Wire dependency injection for sharing components -- `design.md#Components`
  - Register `SqlSharingRepository` as the implementation of `SharingRepository`
  - Register `SharingService` with `SharingRepository` and `AuthorizationService`
    dependencies
  - Register `SharingHandler` with `SharingService`
  - Ensure repository is scoped per-request (database transaction)

---

## Frontend Tasks

- [ ] 9. Build `SharingPanel` component -- `design.md#Frontend Component Design`
  - Props: `objectType`, `objectId`, `effectiveRole`, `onEntriesChange`
  - States: loading, empty, read-only, editable, error, inline-add
  - Fetch sharing entries on mount (via `GET` to sharing API)
  - Render table with columns: User/Group, Role, Shared By, Shared At, Actions
  - Role column uses color-coded badges:
    - Editor: blue background, white text
    - Contributor: green background, white text
    - Viewer: gray background, dark text
  - Actions column shows trash icon only when `effectiveRole === "editor"`
  - Empty state: centered message with muted text
  - Loading state: skeleton placeholder rows (3 rows)
  - Error state: error message with "Retry" button
  - Unit tests (React Testing Library):
    - Renders table with sharing entries
    - Renders empty state when no entries
    - Renders role badges with correct colors
    - Hides remove buttons when effectiveRole is "contributor"
    - Hides remove buttons when effectiveRole is "viewer"
    - Shows remove buttons when effectiveRole is "editor"
    - Hides "Add" button when not Editor
    - Shows "Add" button when Editor
    - Displays error state on fetch failure
    - Calls retry on button click in error state

- [ ] 10. Build inline add form in `SharingPanel` -- `design.md#Inline Add Form`
  - "Add" button toggles inline form row
  - User/Group typeahead:
    - Debounced input (300ms)
    - Fetches `GET /api/v1/users/select?search={query}` and
      `GET /api/v1/groups/select?search={query}`
    - Merges results into a single dropdown with "Users" and "Groups" sections
    - Excludes authenticated user from results
    - Excludes already-shared users/groups from results
    - Shows person icon for users, people icon for groups
  - Role dropdown:
    - Options: Editor, Contributor, Viewer
    - Roles higher than current `effectiveRole` are disabled with tooltip
    - Default: Viewer
  - Confirm button: calls `POST /api/v1/sharing/{objectType}/{objectId}`
  - Optimistic UI: adds row immediately, reverts on API error
  - Cancel button: hides inline form, clears selections
  - Unit tests:
    - Typeahead search triggers API calls after debounce
    - Typeahead excludes current user
    - Typeahead excludes already-shared users/groups
    - Role dropdown disables roles above effective role
    - Add button disabled until target and role selected
    - Successful add: row appears in table
    - Failed add: row removed, error message shown
    - Cancel clears form and hides it

- [ ] 11. Build remove confirmation flow in `SharingPanel` -- `design.md#Sequence`
  - Trash icon click opens a confirmation modal/dialog
  - Dialog shows: "Remove sharing access for {name}?" with current role displayed
  - Confirm: calls `DELETE /api/v1/sharing/{objectType}/{objectId}/{entryId}`
  - Optimistic UI: removes row immediately on confirm, reverts on API error
  - Cancel: closes dialog, no changes
  - Unit tests:
    - Clicking trash icon opens confirmation dialog
    - Confirming triggers DELETE API call
    - Row is removed optimistically
    - Row is restored on API error
    - Cancelling closes dialog without changes

- [ ] 12. Integrate `SharingPanel` into object pages -- `design.md#Components`
  - Add to test case detail page (`/test-cases/{id}`)
  - Add to test case create page (`/test-cases/new`)
  - Add to test case update page (`/test-cases/{id}/edit`)
  - Add to test plan detail, create, and update pages
  - Add to test run detail, create, and update pages
  - Add to test execution detail, create, and update pages
  - On detail/update pages: mount with `objectId` from URL, fetch entries on mount
  - On create page: mount with `objectId=null`, defer sharing entry creation to after
    object creation
  - Pass `effectiveRole` from the page's authorization context
  - Ensure the sharing section is visually consistent with other form sections
    (heading, spacing, border)

---

## Cross-Cutting Tasks

- [ ] 13. Add `objectType` validation and allowlist at the handler level --
      `design.md#API Contract`
  - Define a constant array of valid object types:
    `["test_case", "test_plan", "test_run", "test_execution"]`
  - In the handler, validate the `objectType` path parameter against this array
    before calling the service. If invalid, return `422` immediately (fail closed).

- [ ] 14. Verify user and group select endpoints for typeahead --
      `requirements.md#US-2`
  - Confirm `GET /api/v1/users/select?search={query}` exists and returns `{ "data":
    [{ "id": 1, "display_name": "Jane Smith" }] }` filtered by project scope
  - Confirm `GET /api/v1/groups/select?search={query}` exists and returns `{ "data":
    [{ "id": 1, "name": "QA Team" }] }`
  - If these endpoints do not yet exist, coordinate with the `iam-users` and
    `iam-groups` features to add them
  - The select endpoints must not return the authenticated user in results (or the
    frontend must filter them out)

- [ ] 15. Test sharing create flow on object creation (frontend integration) --
      `requirements.md#US-2`
  - Create a test case with sharing entries specified in the form
  - Verify: object is created, then sharing entries are created sequentially
  - Verify: if a sharing entry creation fails, the object is still created and the
    error is surfaced
  - Verify: sharing entries appear on the detail page after navigation

- [ ] 16. Manual QA checklist for sharing UI --
  - [ ] Open a test case detail page as Editor -- verify sharing section is visible
  - [ ] Add a sharing entry for a user with role "Viewer" -- verify row appears
  - [ ] Add a sharing entry for a group with role "Contributor" -- verify row appears
  - [ ] Verify role badges display correct colors
  - [ ] Verify user icon vs. group icon in the User/Group column
  - [ ] Attempt to add a sharing entry with role above current effective role --
    verify disabled in dropdown
  - [ ] Attempt to share with self -- verify typeahead excludes current user
  - [ ] Attempt to share with an already-shared user -- verify typeahead excludes
  - [ ] Open sharing panel as Contributor -- verify read-only (no Add button, no
    trash icons)
  - [ ] Open sharing panel as Viewer -- verify read-only
  - [ ] Remove a sharing entry -- verify confirmation dialog and optimistic removal
  - [ ] Refresh the page -- verify removed entry is gone, added entries persist
  - [ ] Open a test case create form -- verify sharing section is present
  - [ ] Create a test case with sharing entries -- verify entries persist after
    creation
  - [ ] Verify empty state message when no entries exist
  - [ ] Verify error state when API fails, and retry works
  - [ ] Verify sharing entries on test plan, test run, and test execution pages
