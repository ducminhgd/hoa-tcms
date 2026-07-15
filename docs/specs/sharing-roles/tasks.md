# Tasks: Sharing Roles

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work.

---

## Layer 1 -- Domain

- [ ] 1. Define `SharingRole` enum -- `design.md#Sharing Role Enumeration`
  - Three variants: `Editor`, `Contributor`, `Viewer`
  - `from_str(s: &str) -> Result<SharingRole, ValidationError>` for parsing from API
    input (case-insensitive match on `"editor"`, `"contributor"`, `"viewer"`)
  - `as_str() -> &str` for serialization (lowercase)
  - Implement `PartialOrd` for max-role constraint: `Editor > Contributor > Viewer`
  - Method `can_assign(&self, target: &SharingRole) -> bool` -- returns true if
    `self >= target` (the assigner's role is at least as high as the role being
    assigned)
  - Unit test: parse valid strings, reject invalid strings, verify ordering,
    verify `can_assign` for all combinations

- [ ] 2. Define domain exceptions for sharing roles -- `design.md#Error Handling`
  - `InvalidSharingRoleError` (carries the invalid role string)
  - `SharingRoleEscalationError` (carries the attempted role and the max allowed)
  - `DuplicateSharingEntryError` (carries object_type, object_id, user_id or group_id)
  - `SelfShareError` (carries the user_id that attempted to share with themselves)

---

## Layer 2 -- Application

- [ ] 3. Define `SharingRepository` interface (port) -- `design.md#Components`
  - `find_by_object(object_type, object_id) -> Vec<SharingEntry>` -- all sharing
    entries for an object
  - `find_by_user_and_object(user_id, object_type, object_id) -> Option<SharingEntry>`
    -- the sharing entry for a specific user on an object (used for auth check)
  - `find_by_group_and_object(group_id, object_type, object_id) -> Option<SharingEntry>`
  - `save(entry) -> SharingEntry` -- inserts a new sharing entry
  - `delete(entry_id)` -- removes a sharing entry by ID
  - `exists(object_type, object_id, user_id?, group_id?) -> bool` -- duplicate check

---

## Layer 4 -- Infrastructure

- [ ] 4. Create `sharing_entries` database migration -- `design.md#Data Model`
  - Table: `id` (BIGINT PK), `object_type` (VARCHAR(50) NOT NULL), `object_id`
    (BIGINT NOT NULL), `user_id` (BIGINT, nullable, FK to users), `group_id` (BIGINT,
    nullable, FK to groups), `role` (VARCHAR(20) NOT NULL), `created_by`, `created_at`,
    `updated_by`, `updated_at`
  - CHECK constraint: `role IN ('editor', 'contributor', 'viewer')`
  - CHECK constraint: exactly one of `user_id` or `group_id` is non-null
  - Unique partial indexes:
    - `uq_sharing_entries_user` on `(object_type, object_id, user_id)` WHERE
      `user_id IS NOT NULL`
    - `uq_sharing_entries_group` on `(object_type, object_id, group_id)` WHERE
      `group_id IS NOT NULL`
  - Foreign key indexes on `user_id`, `group_id`, `created_by`, `updated_by`
  - Composite index `idx_sharing_entries_object` on `(object_type, object_id)` for
    listing all sharing entries for an object
  - Composite index `idx_sharing_entries_user_lookup` on `(user_id, object_type,
    object_id)` for auth checks
  - `BEFORE UPDATE` trigger for `updated_at`

- [ ] 5. Implement `SqlSharingRepository` -- `design.md#Components`
  - All methods from `SharingRepository` interface
  - `find_by_object`: SELECT with JOIN on `users` (for display name) and `groups`
    (for group name) WHERE `object_type = $1 AND object_id = $2`
  - `find_by_user_and_object`: SELECT WHERE `object_type = $1 AND object_id = $2
    AND user_id = $3`
  - `save`: INSERT and return the new row. Catch unique violation (23505) and map to
    `DuplicateSharingEntryError`
  - `delete`: DELETE WHERE `id = $1`
  - All queries use parameterized statements
  - Write unit tests with test database:
    - Insert and retrieve sharing entries for user targets
    - Insert and retrieve sharing entries for group targets
    - Reject duplicate user+object sharing entry
    - Reject duplicate group+object sharing entry
    - Reject entries with both user_id and group_id (CHECK constraint violation)
    - Reject entries with neither user_id nor group_id (CHECK constraint violation)
    - Reject entries with invalid role (CHECK constraint violation)
    - Delete removes the entry
    - findAll returns all entries for an object sorted by created_at

- [ ] 6. Write unit tests for max-role constraint and role validation --
      `design.md#Role Assignment Rules`
  - `SharingRole::can_assign`: Editor can assign all three roles; Contributor can
    assign Contributor or Viewer; Viewer cannot assign any role
  - Role parsing: "editor" -> Editor, "EDITOR" -> Editor, "Contributor" ->
    Contributor, "viewer" -> Viewer, "admin" -> error, "" -> error
  - Role ordering: Editor > Contributor, Editor > Viewer, Contributor > Viewer
  - Verify that `can_assign` returns false when assigner role is lower than target
