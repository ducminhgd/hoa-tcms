# Design: Sharing Override

## Architecture

The sharing override is an authorization-layer concern. It modifies how the effective
role is computed for a user on a shareable object. The logic is encapsulated in the
`AuthorizationService` and invoked by every endpoint that operates on a shareable
object.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                         Request Flow                                          │
│                                                                              │
│  Client Request                                                              │
│       │                                                                      │
│       ▼                                                                      │
│  AuthMiddleware: validates session, attaches user context (user_id, groups)   │
│       │                                                                      │
│       ▼                                                                      │
│  Handler: extracts object_type + object_id from path/body                    │
│       │                                                                      │
│       ▼                                                                      │
│  AuthorizationService::get_effective_role(user_id, groups, object_type,      │
│      object_id) -> EffectiveRole                                             │
│       │                                                                      │
│       │  1. If user is System Admin -> return Admin (bypass)                 │
│       │  2. Query sharing_entries WHERE (user_id = $1 OR group_id IN ($2))   │
│       │     AND object_type = $3 AND object_id = $4                          │
│       │  3. If sharing entry found -> return SharingRole from entry          │
│       │  4. If no sharing entry -> return ProjectMembershipRole              │
│       │                                                                      │
│       ▼                                                                      │
│  Handler/Service: checks effective_role against required role for operation  │
│       │                                                                      │
│       ▼                                                                      │
│  Proceed to use case logic or return 403                                     │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. `AuthMiddleware` authenticates the user and attaches `user_id` and `groups` (list
   of group IDs the user belongs to) to the request context.
2. The handler or service calls `AuthorizationService::get_effective_role` with the
   user ID, group IDs, object type, and object ID.
3. The method queries `SHARING_ENTRIES` for any direct user entry or group entry
   matching the user's groups.
4. If a sharing entry is found, its role is the effective role. If multiple entries
   match (direct + group, or multiple groups), the highest role wins.
5. If no sharing entry exists, the project membership role is the effective role.
6. System Admin bypasses all checks: `get_effective_role` returns a special `Admin`
   variant that satisfies all role requirements.

> **Canonical method:** `get_effective_role(user_id, group_ids, object_type, object_id)
> -> EffectiveRole` is the canonical sharing resolution method used by all shareable-object
> handlers. It is called via `auth-sharing-scope`'s `check_sharing_access`, which wraps
> `get_effective_role` with role-requirement comparison. Every handler for shareable
> objects (test cases, test plans, test runs, test executions) delegates to
> `check_sharing_access` for object-level authorization.

---

## Effective Role Resolution

### Algorithm

```text
function get_effective_role(user_id, group_ids, object_type, object_id) -> EffectiveRole:
    if is_system_admin(user_id):
        return Admin

    sharing_entries = query_sharing_entries(
        object_type, object_id, user_id, group_ids
    )

    if sharing_entries is not empty:
        return max(sharing_entries.map(|e| e.role))

    return get_project_membership_role(user_id, project_id_of(object_id))
```

### Role Hierarchy (Highest to Lowest)

```text
Admin > Editor > Contributor > Viewer > NoAccess
```

Where:
- `Admin` is the System Admin bypass (not a sharing role, not a project role).
- `Editor`, `Contributor`, `Viewer` are both sharing roles and project roles.
- `NoAccess` means the user has no project membership and no sharing entry.

### Multiple Entry Resolution

When a user has:
- A direct sharing entry with role `viewer`
- A group sharing entry (via a group they belong to) with role `editor`

The effective role is `editor` (the highest among all matching entries). This is the
permissive resolution strategy.

### Project Membership Required for Non-Shared Access

If no sharing entry exists for a user on an object, the project membership role is
used. If the user is not a project member and has no sharing entry, the effective role
is `NoAccess` and all operations are denied.

---

## Authorization Gate Integration

### Three-Layer Authorization with Sharing Override

The project's authorization model has three layers. Sharing override modifies layer 2
(object scope):

```
Layer 1: System Permission Check
  - "Does the user have the test_case:update permission?"
  - System Admin bypasses.

Layer 2: Object Scope Check (modified by sharing override)
  - "What is the user's effective role on this specific object?"
  - Query sharing_entries first; fall back to project membership role.
  - System Admin bypasses.

Layer 3: Ownership Check (unchanged)
  - "If the effective role is Contributor, is this the user's own object?"
  - Only applies to Contributor role, regardless of whether it came from
    sharing or project membership.
```

**Key design decision:** The ownership check (layer 3) uses the effective role, not
the project membership role. If a user's effective role is Contributor (via a sharing
entry), they are subject to the same ownership restrictions as a project Contributor:
they can only update/delete objects where `created_by` matches their user ID.

### Operation-to-Role Mapping

| Operation | Minimum Effective Role |
|-----------|----------------------|
| Read (GET detail, list) | Viewer |
| Select (dropdown) | Viewer |
| Create | Contributor (project-scoped; sharing does not grant create on unrelated objects) |
| Update (own) | Contributor |
| Update (any) | Editor |
| Delete (own) | Contributor |
| Delete (any) | Editor |
| Share (manage sharing entries) | Editor |

**Note on Create:** Sharing entries grant access to existing objects. Creating new
objects within a project requires project membership (Contributor or higher). A
sharing-only user (no project membership) cannot create new objects in the project,
even with an Editor sharing role on another object in that project.

---

## Database Query

### Get Effective Role Query

```sql
-- Single query to check both direct and group sharing entries
SELECT role FROM sharing_entries
WHERE object_type = $1
  AND object_id = $2
  AND (user_id = $3 OR group_id = ANY($4))
ORDER BY
  CASE role
    WHEN 'editor' THEN 3
    WHEN 'contributor' THEN 2
    WHEN 'viewer' THEN 1
  END DESC
LIMIT 1;
```

The query returns at most one row -- the highest role among all matching entries. If
no row is returned, the caller falls back to the project membership role.

The `$4` parameter (`group_ids`) is an array of BIGINT values representing all groups
the authenticated user belongs to.

### Performance Considerations

- The `idx_sharing_entries_user_lookup` index on `(user_id, object_type, object_id)`
  covers direct user lookups.
- Group lookups use the `group_id` FK index.
- For very large group memberships (100+ groups per user), the `ANY($4)` clause may
  degrade. A practical limit of 50 groups per user is expected in Phase 1.
- If performance becomes a concern, a covering index on `(object_type, object_id,
  user_id, role)` can be added.

---

## Effective Role Caching

**Design decision: No caching.** The sharing override check queries live data on every
request. Rationale:

- Sharing entries change infrequently, but when they do, the change must take effect
  immediately (e.g., revoking access).
- The query is a single indexed lookup, expected to complete in under 1ms.
- Caching would require an invalidation mechanism that adds complexity
  disproportionate to the performance gain.
- This is consistent with the project-wide policy of checking authorization against
  live data on every request (see `test-case-crud` design, Security Requirements).

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `EffectiveRole` | Domain (1) | Enum representing the resolved role for a user on an object: `Admin`, `Editor`, `Contributor`, `Viewer`, `NoAccess`. Implements methods `can_read()`, `can_write()`, `can_share()`, `can_delete_any()`. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (Application) | Add method `get_effective_role(user_id, group_ids, object_type, object_id) -> EffectiveRole`. Query sharing entries first; fall back to project membership. Add `resolve_object_project(object_type, object_id) -> project_id` helper for the fallback path. |
| `ProjectMemberRepository` (Application) | No change to interface. The `AuthorizationService` calls it only when no sharing entry exists. |
| All shareable object handlers | Update each handler (test case, test plan, test run, etc.) to call `AuthorizationService::get_effective_role` instead of directly checking project membership. The handler passes `object_type` and `object_id` to the authorization call. |

---

## Sequence

### Authorization Check with Sharing Override

```
1. Handler receives request for PATCH /api/v1/test-cases/42
2. Handler calls AuthorizationService::authorize(
       user_id, groups, "test_case", 42, RequiredRole::Contributor
   )
3. AuthorizationService checks system permission "test_case:update"
   -> if denied, return 403
4. AuthorizationService::get_effective_role(user_id, groups, "test_case", 42):
   a. Query: SELECT role FROM sharing_entries WHERE object_type='test_case'
      AND object_id=42 AND (user_id=$uid OR group_id = ANY($gids))
      ORDER BY role_rank DESC LIMIT 1
   b. If row returned -> role is the effective role
   c. If no row -> query project_id from test_cases WHERE id=42
      -> query project_members WHERE user_id=$uid AND project_id=$pid
      -> role from project_members is the effective role
5. Check effective_role >= RequiredRole::Contributor
   -> if denied, return 403
6. Proceed with operation
```

### Access via Sharing Without Project Membership

```
1. User (not a project member) has sharing entry with role=editor on test_case=42
2. User sends GET /api/v1/test-cases/42
3. get_effective_role returns Editor (from sharing entry)
4. Editor >= Viewer -> access granted
5. Response: 200 OK with test case detail
```

### Access Denied via Restrictive Sharing

```
1. User has project role=Editor but sharing entry with role=viewer on test_case=42
2. User sends PATCH /api/v1/test-cases/42 (attempts to update)
3. get_effective_role returns Viewer (sharing entry takes priority)
4. Viewer < Contributor -> access denied
5. Response: 403 Forbidden
```

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Effective role insufficient for operation | `403` | `FORBIDDEN` | INFO | Generic message; does not distinguish between sharing role and project role denial |
| User not authenticated | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| Object not found during project resolution | `404` | `NOT_FOUND` | INFO | Object does not exist or is soft-deleted |
| Database error during sharing lookup | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

---

## Shareable Object Types (Phase 1)

The sharing override applies to the following object types:

| object_type | Table | project_id Column |
|-------------|-------|-------------------|
| `test_case` | `test_cases` | `project_id` |
| `test_plan` | `test_plans` | `project_id` |
| `test_run` | `test_runs` | `project_id` |
| `test_execution` | `test_executions` | `project_id` |

Each object type requires:
- A sharing entries query (same `sharing_entries` table, filtered by `object_type`
  and `object_id`).
- A way to resolve the parent project ID for the fallback to project membership
  (simple lookup on the object's table).

The `object_type` string is a stable identifier matching the object's table or domain
name. It is validated against a server-side allowlist to prevent arbitrary string
injection into queries (defence in depth, even with parameterized queries).
