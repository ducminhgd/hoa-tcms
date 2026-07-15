# Design: Auth RBAC

## Architecture

The Auth RBAC feature is a cross-cutting application-layer component. It has no HTTP endpoints
of its own -- it provides the `AuthorizationService` that all other features consume for
system permission checking. It lives entirely in the Application layer but delegates queries
to Infrastructure through existing repository interfaces.

```
+----------------------------------------------------------------------+
|  Adapters (Layer 3)                                                  |
|  +---------------------------------------------------------------+  |
|  |  Authorization Middleware / Guard                              |  |
|  |  - Invoked before every protected route handler               |  |
|  |  - Calls AuthorizationService::check_permission               |  |
|  +-----------------------------------+---------------------------+  |
|                                     | calls                        |
|                                     v                              |
|  Application (Layer 2)                    +--------------------+  |
|  +-------------------------------------+  |  Domain (L1)      |  |
|  |  AuthorizationService               |  |  (no new types)   |  |
|  |  - check_permission(user_id, code)  |  +--------------------+  |
|  |                                     |                           |
|  |  PermissionResolver (interface)     |                           |
|  +-------------------+-----------------+                           |
|                      | delegates to                                |
|                      v                                             |
|  Infrastructure (Layer 4)                                         |
|  +--------------------------------------------------------------+ |
|  |  SqlPermissionResolver (implements PermissionResolver)       | |
|  |  - resolve_effective_permissions(user_id) -> HashSet<i64>    | |
|  +--------------------------------------------------------------+ |
+----------------------------------------------------------------------+
```

**Flow summary:**

1. A request arrives at a protected endpoint handler. The authorization middleware/guard
   intercepts the request after authentication is confirmed.
2. The middleware calls `AuthorizationService::check_permission(user_id, permission_code)`.
3. `AuthorizationService` first delegates to `auth-admin-bypass` to check if the user is a
   System Admin. If so, return `true` immediately.
4. If not System Admin, `AuthorizationService` resolves the permission code to its numeric ID
   via `PermissionRepository::find_by_code`.
5. `AuthorizationService` calls `PermissionResolver::resolve_effective_permissions(user_id)`
   to get the union set of permission IDs from direct roles and group-inherited roles.
6. The result is checked for containment. If the permission ID is present, return `true`;
   otherwise, return `false`.

---

## Data Model

### No new tables

This feature introduces **no new database tables**. It is a pure application-layer feature
that queries existing IAM tables:

| Table | Used For |
|-------|----------|
| `permissions` | Resolving a permission code string to its numeric ID |
| `user_roles` | Direct role assignments for a user |
| `role_permissions` | Permission-to-role mapping for each assigned role |
| `user_groups` | Group memberships for a user |
| `group_roles` | Role assignments inherited through groups |

### Permission Resolution Query

The effective permission set for a user is computed by a single SQL query that unions
direct roles and group-inherited roles:

```sql
SELECT DISTINCT rp.permission_id
FROM user_roles ur
JOIN role_permissions rp ON rp.role_id = ur.role_id
WHERE ur.user_id = $1

UNION

SELECT DISTINCT rp.permission_id
FROM user_groups ug
JOIN group_roles gr ON gr.group_id = ug.group_id
JOIN role_permissions rp ON rp.role_id = gr.role_id
WHERE ug.user_id = $1;
```

**Design notes:**

- The `UNION` (not `UNION ALL`) deduplicates permission IDs across the two sources.
- If a user has the same permission through both a direct role and a group-inherited role,
  it appears only once in the result set.
- The query does **not** join against `roles.deleted_at` because roles are soft-deleted
  and `role_permissions` rows for deleted roles remain physically present but should be
  excluded from the effective set. The repository must add a join condition:
  `JOIN roles r ON r.id = ur.role_id AND r.deleted_at IS NULL` (and similarly for
  group roles).
- The query does **not** join against `groups.deleted_at` because groups are soft-deleted
  and `group_roles` + `user_groups` rows remain physically present. The repository must
  add join conditions: `JOIN groups g ON g.id = ug.group_id AND g.deleted_at IS NULL`
  and `JOIN groups g2 ON g2.id = gr.group_id AND g2.deleted_at IS NULL`.

### Complete Production Query

```sql
-- Direct roles
SELECT DISTINCT rp.permission_id
FROM user_roles ur
JOIN roles r ON r.id = ur.role_id AND r.deleted_at IS NULL
JOIN role_permissions rp ON rp.role_id = ur.role_id
WHERE ur.user_id = $1

UNION

-- Group-inherited roles
SELECT DISTINCT rp.permission_id
FROM user_groups ug
JOIN groups g ON g.id = ug.group_id AND g.deleted_at IS NULL
JOIN group_roles gr ON gr.group_id = ug.group_id
JOIN roles r ON r.id = gr.role_id AND r.deleted_at IS NULL
JOIN role_permissions rp ON rp.role_id = gr.role_id
WHERE ug.user_id = $1;
```

---

## Sequence

### Permission Check Flow

1. Authorization middleware/guard extracts `user_id` from the request context (set by
   `AuthMiddleware` during authentication).
2. Middleware calls `AuthorizationService::check_permission(user_id, "test_case:update")`.
3. `AuthorizationService` calls `AdminBypassService::is_system_admin(user_id)` (from
   `auth-admin-bypass`). If `true`, return `true` immediately.
4. `AuthorizationService` calls `PermissionRepository::find_by_code("test_case:update")`
   to resolve the code to a `Permission` entity.
   - If not found (unknown permission code), log a warning and return `false`.
5. `AuthorizationService` calls `PermissionResolver::resolve_effective_permissions(user_id)`
   which executes the union query above and returns a `HashSet<i64>` of permission IDs.
6. `AuthorizationService` checks if the resolved permission ID is in the set.
7. Middleware receives `true` or `false`:
   - `true`: allow the request to proceed to the handler.
   - `false`: return `403 Forbidden` with generic message.

### Denial Logging

When a permission check returns `false`, the service logs:

```
[INFO] Authorization denied: user_id=42, permission="test_case:update"
```

The log does **not** include: the user's roles, the endpoint path, or the request body.
This limits sensitive information in logs while preserving audit trail capability.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `AuthorizationService` | Application (2) | Central authorization facade. Provides `check_permission(user_id: i64, permission_code: &str) -> bool`. Delegates to `AdminBypassService`, `PermissionRepository`, and `PermissionResolver`. This is the single entry point that all features call for system permission checks. |
| `PermissionResolver` | Application (2) | Interface (port) defining `resolve_effective_permissions(user_id: i64) -> HashSet<i64>`. Returns the union set of permission IDs from direct roles and group-inherited roles. |
| `SqlPermissionResolver` | Infrastructure (4) | Implements `PermissionResolver` using the complete production query (with soft-delete filtering). Returns a `HashSet<i64>` of permission IDs. |
| `PermissionDeniedError` | Application (2) | Error type for permission check failures. Carries `user_id` and `permission_code` for logging but serializes to the generic `FORBIDDEN` response. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `PermissionRepository` (from `iam-permissions`) | Already provides `find_by_code(code: &str) -> Option<Permission>`. No changes needed. |
| `AuthMiddleware` (from `iam-auth`) | After populating `user_id` in the request context, no changes needed -- the authorization middleware is a separate component that runs after `AuthMiddleware`. |
| Authorization middleware / route guard | This new component is registered in the middleware chain after `AuthMiddleware`. It reads the required permission code from route metadata and calls `AuthorizationService::check_permission`. The integration is detailed in the `tasks.md` of this feature. |
| Every feature handler | Each protected handler must annotate its required permission code. The authorization middleware reads this annotation at request time. The annotation mechanism depends on the web framework (e.g., a custom derive macro or route attribute in Actix-Web, or a wrapper function in Axum). |
| IAM Roles (from `iam-roles`) | The permission inheritance computation that was documented as "in auth-rbac" is implemented here. The `iam-roles` feature's design.md already references this as a consumer. No schema changes to `iam-roles`. |

### Compile-time interface checks (Rust convention)

```rust
// - `SqlPermissionResolver` implements `PermissionResolver`
// - The trait system enforces method signature matching at compile time.
```

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| User is not authenticated | `401` | `NOT_AUTHENTICATED` | INFO | From `AuthMiddleware`; never reaches `AuthorizationService` |
| Permission check returns `false` | `403` | `FORBIDDEN` | INFO | Generic message: `"Insufficient permissions"`. Logs user_id and permission_code. |
| Permission code not found in catalog | `403` | `FORBIDDEN` | WARN | Unknown permission code -- treated as "not held". Indicates misconfiguration or typo in route annotation. |
| Database unreachable during permission query | `503` | `DATABASE_UNAVAILABLE` | ERROR | All authorization checks fail closed. |
| Unexpected query failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation. |

**Anti-patterns explicitly avoided:**

- **Do not cache** the effective permission set -- compute fresh on every request.
- **Do not return different messages** for "missing permission" vs "unknown permission code"
  -- both produce the same generic `403` response.
- **Do not perform permission checks in individual handlers** -- use a centralized
  middleware/guard pattern so no endpoint accidentally omits authorization.
- **Do not log the user's full permission set** in denial messages -- log only the specific
  permission code that was required.
- **Do not fail open** -- if the database is unreachable, all permission checks fail with
  `503`, never with `200`.
- **Do not store permissions in the session cookie or Redis session payload** -- permissions
  are computed from live database data on every request.
