# Design: Auth Sharing Scope

## Architecture

The Auth Sharing Scope feature extends `AuthorizationService` with object-level sharing
checks. It has no HTTP endpoints of its own -- it provides methods consumed by every feature
with shareable objects (test cases, test plans, test runs, test executions).

```
+-----------------------------------------------------------------------+
|  Adapters (Layer 3)                                                    |
|  +------------------------------------------------------------------+ |
|  |  Object-Specific Route Handlers                                   | |
|  |  - After system permission + project membership pass,            | |
|  |    calls check_sharing_access for sharing-eligible operations    | |
|  +----------------------------------+-------------------------------+ |
|                                    | calls                           |
|                                    v                                 |
|  Application (Layer 2)                     +---------------------+  |
|  +-------------------------------+         |  Domain (L1)        |  |
|  |  AuthorizationService         |         |  - SharingRole      |  |
|  |  + check_sharing_access(      |         |    (enum)           |  |
|  |    user_id,                   |         +---------------------+  |
|  |    object_type,               |                                   |
|  |    object_id,                 |                                   |
|  |    required_roles             |                                   |
|  |  )                            |                                   |
|  +---------------+----------------+                                   |
|                  | delegates to                                      |
|                  v                                                   |
|  Infrastructure (Layer 4)                                           |
|  +----------------------------------------------------------------+ |
|  |  SharingRepository (from sharing-override)                     | |
|  |  - find_active_share(object_type, object_id, user_id)          | |
|  |    -> Option<ShareRecord>                                      | |
|  +----------------------------------------------------------------+ |
|  |  ProjectMemberRepository (from project-members)                | |
|  |  - find_by_id(project_id, user_id) -> Option<ProjectMember>    | |
|  +----------------------------------------------------------------+ |
+-----------------------------------------------------------------------+
```

**Authorization layering (all three gates):**

```
Request
  |
  v
AuthMiddleware (session check)
  |-- fail --> 401 Unauthorized
  v
System Permission Check (auth-rbac)  <-- Layer 1
  |-- fail --> 403 Forbidden (generic)
  v
Sharing Scope Check (this feature) <-- Layer 3 (evaluates first)
  |-- object is shared with sufficient role --> pass
  |-- object is not shared --> fall through to Layer 2
  v
Project Membership Check (auth-project-scope) <-- Layer 2 (fallback)
  |-- fail --> 403 Forbidden (generic)
  v
Handler executes
```

The sharing check is performed **before** the project membership check. If the object is
shared with the user, the sharing role becomes the effective role and the project role is
never consulted. If the object is not shared, the project membership check runs as normal.

> **Implementation note:** The sharing resolution logic is delegated to `sharing-override`.
> The canonical method for resolving a user's effective role on a shareable object is
> `get_effective_role(user_id, group_ids, object_type, object_id) -> EffectiveRole`,
> defined in the `sharing-override` feature. `check_sharing_access` wraps
> `get_effective_role` with role-requirement comparison against the required roles for the
> current operation.

---

## `SharingRole` Domain Enum

A shared domain enum defined in the `sharing-roles` feature, consumed here:

```rust
pub enum SharingRole {
    Editor,      // Can edit + share the object
    Contributor, // Can edit the object (no sharing)
    Viewer,      // Read-only access
}
```

| Role | Can edit object | Can share object |
|------|----------------|-----------------|
| Editor | Yes | Yes |
| Contributor | Yes | No |
| Viewer | No | No |

---

## Sequence

### Sharing Access Check Flow

1. A shareable-object endpoint handler (e.g., `GET /api/v1/test-cases/{id}`) has already
   passed the system permission check (`test_case:read`).
2. The handler calls `AuthorizationService::check_sharing_access(user_id, "test_case",
   test_case_id, &[SharingRole::Viewer, SharingRole::Contributor, SharingRole::Editor])`.
3. `AuthorizationService` calls `AdminBypassService::is_system_admin(user_id)` (from
   `auth-admin-bypass`). If `true`, return `true` immediately.
4. `AuthorizationService` validates the object exists and is not soft-deleted via the
   appropriate repository (`TestCaseRepository::find_by_id`). If not found or soft-deleted,
   return `false`. (This validation is usually already performed by the handler before the
   authorization check, but the service validates defensively.)
5. `AuthorizationService` calls `get_effective_role(user_id, group_ids, object_type,
   object_id)` from `sharing-override`. This single call replaces the direct sharing lookup
   and project membership fallback -- it resolves the canonical effective role (sharing role
   if a share exists, project membership role otherwise, or `NoAccess` if neither).
6. `check_sharing_access` compares the returned `EffectiveRole` against
   `required_sharing_roles`: if `required_sharing_roles` contains the effective role (or a
   lower role in the hierarchy), return `true`; otherwise return `false`.
7. The project membership fallback is handled entirely inside `get_effective_role` --
   `check_sharing_access` does not call `check_project_membership` directly.

### Sharing Role Prioritization (the "override" semantics)

The sharing role **takes priority** over the project role -- this is the core of the
sharing-override feature. This means:

- If a user with project role Viewer is granted a share role of Editor on a specific test
  case, they can edit that test case (but not other test cases in the project).
- If a user with project role Editor is granted a share role of Viewer on a specific test
  case, they can only view that test case (the share role overrides the project role,
  which would normally allow editing).
- If the share is revoked, the user's access reverts to their project role.

This is translated into `check_sharing_access` logic as:
```
if shared:
    return required_roles.contains(share_role)  // sharing role = effective role
else:
    return check_project_membership(...)         // fall through to project role
```

### Orphaned Objects (no project)

For orphaned objects (e.g., test cases with `project_id IS NULL`), there is no project
membership to fall through to. The `check_sharing_access` method handles this naturally:
when `project_id` is `None`, `check_project_membership` is not called. Only the sharing
path can grant access. The creator of an orphaned object has implicit full access (the
object's `created_by` field). This creator-access check is performed by the handler
before calling `check_sharing_access`, or by a dedicated `is_creator` check within
`AuthorizationService`.

### Denial Logging

When a sharing check returns `false`, the service logs:

```
[INFO] Sharing scope denied: user_id=42, object_type="test_case",
       object_id=153, share_role=Some(Viewer), required_roles=[Editor, Contributor],
       reason="share_role_insufficient"
```

Or for an unshared object with insufficient project role:

```
[INFO] Sharing scope denied: user_id=42, object_type="test_case",
       object_id=153, share_role=None, required_roles=[Editor],
       reason="project_role_insufficient"
```

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `SharingRole` | Domain (1) | Enum: `Editor`, `Contributor`, `Viewer`. Defined in `sharing-roles` feature. If not yet defined, create it here and coordinate with `sharing-roles`. |
| `check_sharing_access` method on `AuthorizationService` | Application (2) | Extends `AuthorizationService`. Method signature: `fn check_sharing_access(&self, user_id: i64, object_type: &str, object_id: i64, required_roles: &[SharingRole]) -> Option<bool>`. Returns `None` when the object is not shared (caller falls through to `check_project_membership`). Returns `Some(true)` when sharing grants access. Returns `Some(false)` when sharing exists but the role is insufficient. |

**Alternative design consideration:** Instead of `Option<bool>`, the method encapsulates
the full check including the project membership fallback:

```rust
fn check_sharing_access(
    &self,
    user_id: i64,
    object_type: &str,
    object_id: i64,
    project_id: Option<i64>,
    required_sharing_roles: &[SharingRole],
    required_project_roles: &[ProjectRole],
) -> bool
```

This self-contained version is preferred because it prevents the caller from accidentally
forgetting the project membership fallback. It accepts both `required_sharing_roles` and
`required_project_roles` because the project role enum (`Owner/Editor/Contributor/Viewer`)
differs from the sharing role enum (`Editor/Contributor/Viewer`). The method maps between
them internally.

**Final design (preferred):** The self-contained signature above. The method body:

1. Admin bypass check.
2. Look up active share record.
3. If shared: return `required_sharing_roles.contains(share_role)`.
4. If not shared and `project_id` is `Some`:
   map `required_sharing_roles` to `required_project_roles` and call
   `check_project_membership`.
5. If not shared and `project_id` is `None` (orphaned object): return `false`.

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (from `auth-rbac` + `auth-project-scope`) | Add `check_sharing_access` method with the self-contained signature. Add dependency on `SharingRepository`. |
| `SharingRepository` (from `sharing-override`) | Must provide `find_active_share(object_type: &str, object_id: i64, user_id: i64) -> Option<ShareRecord>` where `ShareRecord` includes the `sharing_role` field. If `sharing-override` has not yet defined this repository, define the interface here as an application-layer port and implement it in `sharing-override`. |
| Every feature handler for shareable objects | Handlers that need sharing checks must call `check_sharing_access` instead of (or in addition to) `check_project_membership`. The migration path is: replace `check_project_membership` calls with `check_sharing_access` calls that include the fallback. |

### Sharing-Enabled Object Types

| Object Type | string identifier | Project-Scoped | Has Sharing |
|-------------|-------------------|----------------|-------------|
| Test Case | `"test_case"` | Yes (optional -- can be orphaned) | Yes |
| Test Plan | `"test_plan"` | Yes | Yes |
| Test Run | `"test_run"` | Yes | Yes |
| Test Execution | `"test_execution"` | Yes | Yes |

Objects without sharing (e.g., categories, templates, priorities) do not use
`check_sharing_access` -- they only use `check_project_membership`.

### Role Mapping (SharingRole to ProjectRole)

When falling through from sharing to project membership, the sharing roles map to project
roles as follows:

| SharingRole | Equivalent ProjectRole |
|-------------|----------------------|
| Editor | Owner, Editor |
| Contributor | Contributor |
| Viewer | Viewer |

This mapping is used when a feature specifies `required_sharing_roles` and the object is
not shared -- the method converts them to the equivalent `required_project_roles` before
calling `check_project_membership`.

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| User is System Admin | -- | -- | -- | Returns `true` immediately |
| Object shared with sufficient role | -- | -- | -- | Returns `true` |
| Object shared but role insufficient | `403` | `FORBIDDEN` | INFO | Sharing role overrides project role; no fallback |
| Object not shared, project membership sufficient | -- | -- | -- | Returns `true` (via fallback) |
| Object not shared, project membership insufficient | `403` | `FORBIDDEN` | INFO | Same generic message |
| Object not found or soft-deleted | `403` | `FORBIDDEN` | INFO | Same generic message (do not reveal existence) |
| Orphaned object, no sharing, not creator | `403` | `FORBIDDEN` | INFO | Orphaned objects have no project membership fallback |
| Database unreachable | `503` | `DATABASE_UNAVAILABLE` | ERROR | All checks fail closed |

**Anti-patterns explicitly avoided:**

- **Do not check project membership first** and then sharing -- sharing must be checked
  first because it overrides the project role.
- **Do not allow fallback to project role when sharing exists** -- if the object is shared
  with the user, the sharing role is the effective role, even if the project role is
  higher.
- **Do not return `404` for sharing denials** -- this reveals that the object exists.
  Always return `403` for authorization failures.
- **Do not cache sharing records** -- compute from live data on every request.
- **Do not hardcode the sharing role hierarchy** -- use the explicit mapping table so that
  role semantics are defined in one place.
