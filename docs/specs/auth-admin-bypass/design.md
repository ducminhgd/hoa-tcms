# Design: Auth Admin Bypass

## Architecture

The Admin Bypass feature is the simplest cross-cutting authorization component. It provides a
single predicate -- `is_system_admin(user_id)` -- that is called at the top of every
`AuthorizationService` method. If `true`, the method returns immediately without performing
any further checks.

```
+----------------------------------------------------------------------+
|  Application (Layer 2)                                               |
|  +---------------------------------------------------------------+  |
|  |  AuthorizationService                                          |  |
|  |  + check_permission()     --> first: admin_bypass              |  |
|  |  + check_project_membership() --> first: admin_bypass          |  |
|  |  + check_sharing_access()  --> first: admin_bypass             |  |
|  +-----------------------------------+---------------------------+  |
|                                     | calls                        |
|                                     v                              |
|  +---------------------------------------------------------------+  |
|  |  AdminBypassService                                            |  |
|  |  + is_system_admin(user_id) -> bool                            |  |
|  +-----------------------------------+---------------------------+  |
|                                     | delegates to                 |
|                                     v                              |
|  Infrastructure (Layer 4)                                         |
|  +---------------------------------------------------------------+ |
|  |  UserRoleRepository (existing, from iam-users)                | |
|  |  RoleRepository (existing, from iam-roles)                    | |
|  |  Query: SELECT 1 FROM user_roles ur                           | |
|  |    JOIN roles r ON r.id = ur.role_id                          | |
|  |    WHERE ur.user_id = $1 AND r.name = 'System Admin'          | |
|  |      AND r.deleted_at IS NULL                                 | |
|  +---------------------------------------------------------------+ |
+----------------------------------------------------------------------+
```

**Flow summary:**

1. Any `AuthorizationService` method receives `user_id`.
2. The method calls `AdminBypassService::is_system_admin(user_id)`.
3. The service executes a single query: does the user have the "System Admin" role
   assigned (directly or via a group)?
4. If yes, return `true` -- the AuthorizationService method returns `true` immediately.
5. If no, the AuthorizationService method proceeds with its normal checks.

The bypass check is intentionally trivial: it is the same query pattern used everywhere.
The key distinction is that it runs **first**, before any other authorization logic,
and short-circuits all subsequent checks.

### Why a separate service?

The `AdminBypassService` is a thin wrapper around a single query. It exists as a separate
component because:

1. **Testability**: Unit tests for `AuthorizationService` can mock `AdminBypassService`
   independently, testing admin bypass and normal authorization in isolation.
2. **Single responsibility**: The "System Admin" role name is a magic string (`"System
   Admin"`) that is used in the seed migration, the CLI init command, and the bypass
   check. Encapsulating it in one service prevents duplication of this constant.
3. **Future extension**: If System Admin bypass needs additional behaviour (e.g.,
   audit logging for admin actions, rate limiting of admin endpoints, or a "sudo mode"
   confirmation step), the service can be extended without touching the other
   authorization methods.

---

## Data Model

### No new tables

This feature introduces **no new database tables**. It queries:

| Table | Used For |
|-------|----------|
| `user_roles` | Direct role assignments (checking if the user has the System Admin role) |
| `roles` | Role name lookup (identifying the System Admin role by name) |
| `user_groups` | Group memberships |
| `group_roles` | Group-inherited role assignments (System Admin can also be inherited) |

### Admin Check Query

```sql
-- Check if user has System Admin role directly
SELECT EXISTS (
    SELECT 1
    FROM user_roles ur
    JOIN roles r ON r.id = ur.role_id
    WHERE ur.user_id = $1
      AND r.name = 'System Admin'
      AND r.deleted_at IS NULL
)

OR EXISTS (
    -- Check if user has System Admin role via group inheritance
    SELECT 1
    FROM user_groups ug
    JOIN groups g ON g.id = ug.group_id AND g.deleted_at IS NULL
    JOIN group_roles gr ON gr.group_id = ug.group_id
    JOIN roles r ON r.id = gr.role_id
    WHERE ug.user_id = $1
      AND r.name = 'System Admin'
      AND r.deleted_at IS NULL
);
```

**Design notes:**

- The query checks both direct role assignment and group-inherited role assignment. A
  System Admin role can be assigned via a group (e.g., an "Administrators" group that has
  the System Admin role), and the bypass applies.
- The query uses `EXISTS` for efficiency -- the database can stop scanning after finding
  the first match.
- The `roles.deleted_at IS NULL` filter prevents bypass from a soft-deleted role (should
  not happen in practice since the System Admin role is protected from deletion, but
  defence in depth).
- The constant `"System Admin"` is the role name seeded by the `iam-roles` migration.
  It must match exactly.

### Alternative: Use a flag or reserved role ID

An alternative design would store a `is_system_admin BOOLEAN` column on the `users` table
or use a reserved `role_id` (e.g., `id = 1` for System Admin). This is rejected because:

- A boolean flag creates a denormalized field that must be kept in sync with role
  assignments.
- A reserved role ID couples the application to a specific migration-generated ID, which
  is fragile across environments.
- The query-based approach uses the same pattern as all other authorization checks,
  keeping the system consistent and predictable.

---

## Sequence

### Admin Bypass in `check_permission`

1. `AuthorizationService::check_permission(user_id, "test_case:update")` is called.
2. **Immediately** call `AdminBypassService::is_system_admin(user_id)`.
3. If `true`, return `true` and skip steps 4-6 (permission resolution, effective
   permissions query, containment check).
4. If `false`, proceed with normal permission check.

### Admin Bypass in `check_project_membership`

1. `AuthorizationService::check_project_membership(user_id, 42, &[Owner, Editor])` is
   called.
2. **Immediately** call `AdminBypassService::is_system_admin(user_id)`.
3. If `true`, return `true` and skip steps 4-7 (project existence check, membership
   lookup, role comparison).
4. If `false`, proceed with normal membership check.

### Admin Bypass in `check_sharing_access`

1. `AuthorizationService::check_sharing_access(user_id, "test_case", 153, Some(42),
   &[Editor], &[Owner, Editor])` is called.
2. **Immediately** call `AdminBypassService::is_system_admin(user_id)`.
3. If `true`, return `true` and skip steps 4-6 (share lookup, role comparison, project
   membership fallback).
4. If `false`, proceed with normal sharing check.

### The pattern is identical in all three methods

```
fn authorization_method(&self, user_id: i64, ...) -> bool {
    if self.admin_bypass.is_system_admin(user_id) {
        return true;
    }
    // ... normal checks
}
```

This consistency makes the bypass behaviour easy to audit and reason about. There is
exactly one place where a System Admin bypasses authorization, and it is always the first
line of every authorization method.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `AdminBypassService` | Application (2) | Wraps the admin check query. Method: `fn is_system_admin(&self, user_id: i64) -> bool`. Uses `UserRoleRepository` and `GroupRoleRepository` (or a combined query via `RoleRepository`). |
| `AdminBypassRepository` | Application (2) | Interface (port): `fn is_system_admin(&self, user_id: i64) -> Result<bool>`. Encapsulates the admin check query. |
| `SqlAdminBypassRepository` | Infrastructure (4) | Implements `AdminBypassRepository` with the EXISTS query above. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (from `auth-rbac`) | Inject `AdminBypassService` as a dependency. Add `self.admin_bypass.is_system_admin(user_id)` as the first line in `check_permission`. |
| `AuthorizationService` (from `auth-project-scope`) | Add the same bypass line as the first line in `check_project_membership`. |
| `AuthorizationService` (from `auth-sharing-scope`) | Add the same bypass line as the first line in `check_sharing_access`. |

### Compile-time interface checks (Rust convention)

```rust
// - `SqlAdminBypassRepository` implements `AdminBypassRepository`
// - The trait system enforces method signature matching at compile time.
```

---

## Error Handling

The admin bypass feature does not produce error responses directly. It is a predicate
that returns `true` or `false`:

| Result | Meaning | Effect |
|--------|---------|--------|
| `true` | User is System Admin | Authorization method returns `true` immediately |
| `false` | User is not System Admin | Authorization method proceeds with normal checks |
| Database error | Query failed | Propagate the error to the caller; the authorization method returns a `503` error (same as any other database failure in the authorization layer) |

**Anti-patterns explicitly avoided:**

- **Do not log admin bypass events** in Phase 1 -- logging every admin action would
  produce excessive noise. Audit logging is deferred to Phase 2.
- **Do not set a flag in the session** -- admin status is checked from live data on
  every request, not cached.
- **Do not perform the admin check after other checks** -- it must be the first check
  to avoid unnecessary queries for System Admins.
- **Do not duplicate the query** across the three authorization methods -- use a single
  `AdminBypassService` called from all methods.
- **Do not store the System Admin role ID as a constant** -- use the role name string
  so that the query is resilient to different database instances. The name `"System
  Admin"` is guaranteed unique by the seed migration.
