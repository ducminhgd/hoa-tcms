# Design: Sharing UI

## Architecture

The sharing UI feature spans both frontend (UI component) and backend (API endpoints).
The backend exposes RESTful endpoints under `/api/v1/sharing/{objectType}/{objectId}`
that the frontend consumes via a dedicated sharing component embedded in object
create, update, and detail pages.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Frontend (UI Layer)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  SharingPanel Component:                                               │   │
│  │  - Embedded in create, update, and detail views of shareable objects   │   │
│  │  - Table of sharing entries with role badges and remove actions         │   │
│  │  - Inline add form: user/group typeahead + role dropdown                │   │
│  │  - Optimistic UI updates for add/remove operations                      │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ HTTP calls                                          │
│  Backend (API Layer)    │                                                     │
│  ┌──────────────────────┴────────────────────────────────────────────────┐   │
│  │  SharingHandler (Adapters):                                            │   │
│  │  - GET    /api/v1/sharing/{objectType}/{objectId}          -> list     │   │
│  │  - POST   /api/v1/sharing/{objectType}/{objectId}          -> create   │   │
│  │  - DELETE /api/v1/sharing/{objectType}/{objectId}/{entryId} -> delete  │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│  Application (Layer 2) │                                                     │
│  ┌──────────────────────┴────────────────────────────────────────────────┐   │
│  │  SharingService:                                                       │   │
│  │  - list_sharing_entries(object_type, object_id, user_id)               │   │
│  │  - create_sharing_entry(object_type, object_id, cmd, created_by)       │   │
│  │  - delete_sharing_entry(entry_id, deleted_by)                          │   │
│  │                                                                        │   │
│  │  Interfaces: SharingRepository                                         │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ delegates to                                        │
│  Infrastructure (Layer 4)                                                     │
│  ┌──────────────────────┴────────────────────────────────────────────────┐   │
│  │  SqlSharingRepository (implements SharingRepository)                   │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## API Contract

### Common Error Response Format

All errors follow the standard project format:

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

### GET `/api/v1/sharing/{objectType}/{objectId}`

List all sharing entries for a shareable object.

**Authentication:** Required (session cookie)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `objectType` | string | One of: `test_case`, `test_plan`, `test_run`, `test_execution` |
| `objectId` | integer | ID of the object |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 15,
      "user_id": 42,
      "user_name": "Jane Smith",
      "group_id": null,
      "group_name": null,
      "role": "editor",
      "created_by": 15,
      "created_by_name": "John Doe",
      "created_at": "2026-07-14T10:00:00Z"
    },
    {
      "id": 16,
      "user_id": null,
      "user_name": null,
      "group_id": 7,
      "group_name": "QA Team",
      "role": "viewer",
      "created_by": 15,
      "created_by_name": "John Doe",
      "created_at": "2026-07-15T08:30:00Z"
    }
  ]
}
```

**Notes:**
- Results are ordered by `created_at` descending (newest first).
- `user_id` is null for group-targeted entries; `group_id` is null for
  user-targeted entries.
- `user_name` and `group_name` are resolved via LEFT JOIN on the respective tables.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks access to the object (effective role lower than Viewer) |
| `404` | `NOT_FOUND` | Object does not exist, is soft-deleted, or `objectType` is invalid |
| `422` | `VALIDATION_ERROR` | `objectType` is not a recognised shareable type |

---

### POST `/api/v1/sharing/{objectType}/{objectId}`

Create a new sharing entry for a user or group on an object.

**Authentication:** Required (session cookie)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `objectType` | string | One of: `test_case`, `test_plan`, `test_run`, `test_execution` |
| `objectId` | integer | ID of the object |

**Request Body:**

```json
{
  "user_id": 42,
  "role": "editor"
}
```

Or for a group:

```json
{
  "group_id": 7,
  "role": "viewer"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `user_id` | integer | Conditional | Required if `group_id` absent. Must reference an existing, active user. Must not be the authenticated user. |
| `group_id` | integer | Conditional | Required if `user_id` absent. Must reference an existing, active group. |
| `role` | string | Yes | One of: `"editor"`, `"contributor"`, `"viewer"` |

**Success Response:** `201 Created`

Headers: `Location: /api/v1/sharing/test_case/128/17`

```json
{
  "data": {
    "id": 17,
    "user_id": 42,
    "user_name": "Jane Smith",
    "group_id": null,
    "group_name": null,
    "role": "editor",
    "created_by": 15,
    "created_by_name": "John Doe",
    "created_at": "2026-07-15T09:00:00Z"
  }
}
```

**Notes:**
- The authenticated user's effective role on the object must be Editor (or the user
  must be a System Admin). Otherwise `403`.
- The max-role constraint applies: the sharing role in the request must not exceed the
  authenticated user's effective role on the object.
- `created_by` is set to the authenticated user. `created_by_name` is resolved from
  the users table.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks Editor access on the object (effective role lower than Editor) |
| `403` | `FORBIDDEN` | User attempted to assign a role higher than their own effective role |
| `404` | `NOT_FOUND` | Object does not exist, is soft-deleted, or `objectType` is invalid |
| `409` | `DUPLICATE_SHARING_ENTRY` | A sharing entry already exists for this user/group on this object |
| `422` | `VALIDATION_ERROR` | Invalid `objectType`, missing `role`, invalid `role` value |
| `422` | `SELF_SHARE_NOT_ALLOWED` | The target `user_id` is the authenticated user |
| `422` | `VALIDATION_ERROR` | Neither `user_id` nor `group_id` provided, or both provided |
| `422` | `VALIDATION_ERROR` | Referenced `user_id` or `group_id` does not exist or is inactive |

---

### DELETE `/api/v1/sharing/{objectType}/{objectId}/{entryId}`

Remove a sharing entry.

**Authentication:** Required (session cookie)

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `objectType` | string | One of: `test_case`, `test_plan`, `test_run`, `test_execution` |
| `objectId` | integer | ID of the object |
| `entryId` | integer | ID of the sharing entry to remove |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- The authenticated user's effective role on the object must be Editor (or System
  Admin). Otherwise `403`.
- The sharing entry must exist and belong to the specified object. If the entry exists
  but belongs to a different object, `404 Not Found` is returned.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks Editor access on the object |
| `404` | `NOT_FOUND` | Object does not exist, is soft-deleted, `objectType` invalid, or sharing entry does not exist / belongs to a different object |

---

## Frontend Component Design

### SharingPanel Component

The `SharingPanel` is a reusable component embedded in the create, update, and detail
views of all shareable objects.

**Props:**

| Prop | Type | Description |
|------|------|-------------|
| `objectType` | `string` | The type of the object (`"test_case"`, etc.) |
| `objectId` | `number \| null` | The object ID. `null` when the object has not been created yet (create form). |
| `effectiveRole` | `SharingRole` | The current user's effective role on the object. Controls read-only vs. editable mode. |
| `onEntriesChange` | `(entries: SharingEntry[]) => void` | Callback when entries are modified (for create/update forms to include sharing state in form submission). |

**States:**

| State | Condition | UI |
|-------|-----------|----|
| Loading | API call in progress | Skeleton rows in the table |
| Empty | No sharing entries exist | "This object has not been shared with anyone yet." |
| Read-only | `effectiveRole` is Contributor or Viewer | Table with entries but no "Add" button or remove actions |
| Editable | `effectiveRole` is Editor | Full table with "Add" button and remove actions on each row |
| Error | API call failed | Error message with retry button |
| Inline add | User clicked "Add" | Inline form row at the top of the table |

### Table Columns

```
┌──────────────────┬──────────────┬──────────────┬────────────────────┬─────────┐
│  User / Group     │  Role        │  Shared By   │  Shared At         │  Actions │
├──────────────────┼──────────────┼──────────────┼────────────────────┼─────────┤
│  Jane Smith       │  [Editor]    │  John Doe    │  2026-07-14 10:00  │  🗑     │
│  QA Team (group)  │  [Viewer]    │  John Doe    │  2026-07-15 08:30  │  🗑     │
└──────────────────┴──────────────┴──────────────┴────────────────────┴─────────┘
```

- **User/Group column:** Display name with an icon distinguishing users from groups
  (person icon for users, people icon for groups).
- **Role column:** Color-coded badge: Editor (blue), Contributor (green), Viewer
  (gray).
- **Shared By column:** Display name of the user who created the sharing entry.
- **Shared At column:** Formatted date (relative for recent: "2 hours ago"; absolute
  for older: "Jul 14, 2026").
- **Actions column:** Trash icon button. Only shown when `effectiveRole === Editor`.

### Inline Add Form

When the "Add" button is clicked, an inline form row appears at the top of the table:

```
┌──────────────────────────────────────────────────────────────────────────┐
│  [User/Group Typeahead ▼]  [Role Dropdown ▼]  [✓ Add] [✕ Cancel]       │
└──────────────────────────────────────────────────────────────────────────┘
```

**User/Group Typeahead:**
- Debounced search (300ms) against `GET /api/v1/users/select?search={query}` and
  `GET /api/v1/groups/select?search={query}`.
- Results merged into a single dropdown with sections: "Users" and "Groups".
- Each result shows the display name and type icon.
- The authenticated user is excluded from user results.
- Users/groups already in the sharing entries list are excluded.

**Role Dropdown:**
- Options: Editor, Contributor, Viewer.
- Only roles less than or equal to the current user's effective role are enabled.
  Higher roles are greyed out with a tooltip: "You cannot grant a role higher than
  your own."

**Add Button:**
- Disabled until both a target and a role are selected.
- On click: calls `POST /api/v1/sharing/{objectType}/{objectId}`, optimistically adds
  the entry to the table, then reverts on failure.

### Workflow for Create Form

When the sharing panel is on a create form (objectId is `null`):

1. The user adds sharing entries in the panel (stored in local component state).
2. When the create form is submitted, the sharing entries are sent as part of the
   create payload (or in a separate batch request after the object is created).
3. **Recommended approach for Phase 1:** The frontend first creates the object (POST),
   reads the new object's `id` from the `Location` header, then creates each sharing
   entry sequentially via POST to the sharing endpoint. This avoids coupling the
   object creation endpoint to sharing logic.
4. If any sharing entry creation fails, the error is surfaced to the user, but the
   object has already been created. The user can retry sharing from the detail page.

### Workflow for Update Form

When the sharing panel is on an update form (objectId is known):

1. On mount, load existing sharing entries via `GET /api/v1/sharing/{objectType}/{objectId}`.
2. The user can add new entries (POST) and remove existing entries (DELETE).
3. Add operations are applied immediately (not deferred to form submission) so the
   sharing state is always live.
4. Remove operations are applied immediately with optimistic UI.
5. The update form submission handles only the object's own fields. Sharing is managed
   independently.

---

## Data Model

The `SHARING_ENTRIES` table is defined in the `sharing-roles` design. This design
references it and adds the resolver queries for user/group names.

### Resolving Display Names

```sql
-- For list endpoint (GET sharing entries for an object)
SELECT
    se.id,
    se.user_id,
    u.display_name AS user_name,
    se.group_id,
    g.name AS group_name,
    se.role,
    se.created_by,
    cb.display_name AS created_by_name,
    se.created_at
FROM sharing_entries se
LEFT JOIN users u ON se.user_id = u.id
LEFT JOIN groups g ON se.group_id = g.id
LEFT JOIN users cb ON se.created_by = cb.id
WHERE se.object_type = $1
  AND se.object_id = $2
ORDER BY se.created_at DESC;
```

---

## Sequence

### View Sharing Entries (Detail Page Load)

```
1. User navigates to /test-cases/42
2. Frontend loads test case detail via GET /api/v1/test-cases/42
3. Frontend renders the page with the SharingPanel component
4. SharingPanel mounts, calls GET /api/v1/sharing/test_case/42
5. Backend handler:
   a. Validates session via AuthMiddleware
   b. Validates objectType against allowlist
   c. Checks effective role >= Viewer on object (via AuthorizationService)
   d. Queries sharing_entries with JOINs for display names
   e. Returns 200 with the list
6. Frontend renders the sharing table
```

### Add Sharing Entry

```
1. User clicks "Add" in SharingPanel on test case 42
2. User searches for "Jane" in typeahead
3. Frontend calls GET /api/v1/users/select?search=Jane
4. User selects Jane Smith, chooses role "Editor" from dropdown
5. User clicks "Add"
6. Frontend calls POST /api/v1/sharing/test_case/42 with {"user_id": 55, "role": "editor"}
7. Backend handler:
   a. Validates session
   b. Validates effective role is Editor on test_case:42 (via AuthorizationService)
   c. Validates max-role: requesting user's effective role >= "editor"
   d. Checks user_id != authenticated user (self-share prevention)
   e. Checks no duplicate entry for user 55 on test_case:42
   f. Validates user 55 exists and is active
   g. Inserts sharing entry
   h. Returns 201 Created with the entry representation
8. Frontend optimistically adds the row to the table (already visible)
9. On success: row stays, toast confirms "Shared with Jane Smith as Editor"
10. On failure: row removed, error message displayed
```

### Remove Sharing Entry

```
1. User clicks trash icon on sharing entry 17 for Jane Smith
2. Frontend shows confirmation dialog: "Remove sharing access for Jane Smith?"
3. User confirms
4. Frontend calls DELETE /api/v1/sharing/test_case/42/17
5. Backend handler:
   a. Validates session
   b. Validates effective role is Editor on test_case:42
   c. Verifies entry 17 exists and belongs to test_case:42
   d. Deletes the entry
   e. Returns 204 No Content
6. Frontend optimistically removes the row (already removed on confirm)
7. On success: toast confirms "Sharing access removed for Jane Smith"
8. On failure: row restored, error message displayed
```

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `SharingService` | Application (2) | Orchestrates sharing CRUD: `list_entries`, `create_entry`, `delete_entry`. Validates max-role constraint, self-share prevention, and duplicate prevention. Checks effective role is Editor for mutations. |
| `SharingHandler` | Adapters (3) | HTTP handler: `list`, `create`, `delete`. Deserializes requests, calls `SharingService`, serializes responses. Validates `objectType` against allowlist. |
| `SharingPanel` | Frontend | Reusable React component for the sharing UI. Embeds in create, update, and detail pages of shareable objects. Handles typeahead search, inline add, remove confirmation, and optimistic updates. |
| `SharingRepository` | Application (2) | Interface (port): `find_by_object`, `find_by_user_and_object`, `save`, `delete`, `exists`. Defined in `sharing-roles`, consumed here. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| Object detail pages (test case, test plan, test run, test execution) | Add `SharingPanel` component to the detail view. |
| Object create forms | Add `SharingPanel` component with `objectId=null`. |
| Object update forms | Add `SharingPanel` component with live add/remove (no deferral). |
| HTTP router | Register 3 new routes under `/api/v1/sharing/{objectType}/{objectId}`. |

---

## Route Registration

```text
GET    /api/v1/sharing/{objectType}/{objectId}             -> list
POST   /api/v1/sharing/{objectType}/{objectId}             -> create
DELETE /api/v1/sharing/{objectType}/{objectId}/{entryId}   -> delete
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- `objectType` validation against the server-side allowlist
- Object existence check (object not soft-deleted)
- Effective role check:
  - `GET`: effective role Viewer or higher
  - `POST`, `DELETE`: effective role Editor or higher
  - System Admin bypasses all role checks

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| Invalid objectType | `422` | `VALIDATION_ERROR` | INFO | Not in the server-side allowlist |
| Object not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Generic message |
| User lacks effective role for operation | `403` | `FORBIDDEN` | INFO | Generic message; GET needs Viewer+, mutations need Editor+ |
| Max-role constraint violated | `403` | `FORBIDDEN` | INFO | "You cannot grant a role higher than your own" |
| Self-share attempted | `422` | `SELF_SHARE_NOT_ALLOWED` | INFO | "Cannot create a sharing entry for yourself" |
| Duplicate sharing entry | `409` | `DUPLICATE_SHARING_ENTRY` | INFO | Entry already exists for this user/group on this object |
| Invalid role value | `422` | `VALIDATION_ERROR` | INFO | "role must be one of: editor, contributor, viewer" |
| Missing user_id and group_id | `422` | `VALIDATION_ERROR` | INFO | "Either user_id or group_id must be provided" |
| Both user_id and group_id provided | `422` | `VALIDATION_ERROR` | INFO | "Provide either user_id or group_id, not both" |
| Referenced user does not exist | `422` | `VALIDATION_ERROR` | INFO | Invalid user_id |
| Referenced group does not exist | `422` | `VALIDATION_ERROR` | INFO | Invalid group_id |
| Sharing entry to delete not found | `404` | `NOT_FOUND` | INFO | Entry does not exist or belongs to a different object |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Per-endpoint limits with `Retry-After` header |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | |

**Anti-patterns explicitly avoided:**

- **Do not use GET with a body** -- state changes use POST and DELETE.
- **Do not expose the existence of sharing entries to unauthorized users** -- if the
  user lacks Viewer access to the object, return `403` (not `404`, which would reveal
  the object exists).
- **Do not allow PUT for sharing entry updates** -- sharing entries cannot be edited.
  The workflow is DELETE + POST.
- **Do not accept `objectType` directly from user input without server-side
  validation** -- always check against the allowlist before any query.
