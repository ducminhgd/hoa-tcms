# Design: Auth Project Scope

## Architecture

The Auth Project Scope feature extends `AuthorizationService` with project membership checks.
It has no HTTP endpoints of its own -- it provides methods consumed by every project-scoped
feature. It lives in the Application layer and delegates to Infrastructure through the
existing `ProjectMemberRepository` interface.

```
+----------------------------------------------------------------------+
|  Adapters (Layer 3)                                                  |
|  +---------------------------------------------------------------+  |
|  |  Project-Scoped Route Handlers                                 |  |
|  |  - After system permission pass, calls check_project_membership|  |
|  +-----------------------------------+---------------------------+  |
|                                     | calls                        |
|                                     v                              |
|  Application (Layer 2)                    +--------------------+  |
|  +------------------------------+         |  Domain (L1)      |  |
|  |  AuthorizationService        |         |  - ProjectRole    |  |
|  |  + check_project_membership( |         |    (enum)         |  |
|  |    user_id, project_id,      |         +--------------------+  |
|  |    required_roles)           |                                  |
|  +-------------+----------------+                                  |
|                | delegates to                                      |
|                v                                                   |
|  Infrastructure (Layer 4)                                         |
|  +--------------------------------------------------------------+ |
|  |  ProjectMemberRepository (existing, from project-members)    | |
|  |  - find_by_id(project_id, user_id) -> Option<ProjectMember>  | |
|  |  - exists(project_id, user_id) -> bool                       | |
|  +--------------------------------------------------------------+ |
|  |  ProjectRepository (existing, from project-crud)             | |
|  |  - find_by_id(project_id) -> Option<Project>                 | |
|  +--------------------------------------------------------------+ |
+----------------------------------------------------------------------+
```

**Authorization layering reminder (from design guideline):**

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
Project Membership Check (this feature) <-- Layer 2
  |-- fail --> 403 Forbidden (same generic message)
  v
Handler executes
```

The system permission check and the project membership role check are **independent gates**.
Both must pass (or the caller must be a System Admin, who implicitly holds all system
permissions and bypasses all membership checks).

---

## `ProjectRole` Domain Enum

A shared domain enum used by both `project-members` and `auth-project-scope`:

```rust
pub enum ProjectRole {
    Owner,
    Editor,
    Contributor,
    Viewer,
}
```

Role hierarchy for authorization purposes (higher roles include lower roles):

| Role | Can manage members | Can modify project & objects | Can modify objects only | Can share | Read-only |
|------|-------------------|------------------------------|------------------------|-----------|-----------|
| Owner | Yes | Yes | Yes | Yes | -- |
| Editor | No | Yes | Yes | Yes | -- |
| Contributor | No | No | Yes | No | -- |
| Viewer | No | No | No | No | Yes |

When `required_roles` is `[Owner, Editor]`, a user with the Owner role passes (their
role is in the set). A user with the Contributor role does not. There is no implicit
hierarchy -- a check for `[Editor]` does **not** pass for an Owner unless Owner is
explicitly included in `required_roles`.

---

## Sequence

### Project Membership Check Flow

1. A project-scoped endpoint handler (e.g., `POST /api/v1/projects/{projectId}/categories`)
   has already passed the system permission check (`category:create`).
2. The handler (or a project-scope guard) calls
   `AuthorizationService::check_project_membership(user_id, project_id, &[ProjectRole::Owner, ProjectRole::Editor])`.
3. `AuthorizationService` calls `AdminBypassService::is_system_admin(user_id)` (from
   `auth-admin-bypass`). If `true`, return `true` immediately.
4. `AuthorizationService` validates the project exists and is not soft-deleted via
   `ProjectRepository::find_by_id(project_id)`. If not found or soft-deleted, return `false`.
5. `AuthorizationService` calls `ProjectMemberRepository::find_by_id(project_id, user_id)`
   to get the user's membership record.
6. If no membership record found, return `false`.
7. If the membership record's role is in `required_roles` (or `required_roles` is empty),
   return `true`. Otherwise, return `false`.
8. On `false`, the handler returns `403 Forbidden` with the generic message.
   The handler does **not** distinguish between "project not found", "not a member", and
   "wrong role" -- all produce the same `403` response to prevent project existence
   enumeration.

### Typical Usage Pattern (in a project-scoped feature handler)

```rust
// In a project-scoped handler (e.g., CreateCategoryHandler):
fn handle(request: HttpRequest, path: Path<(i64, i64)>, body: Json<CreateCategoryRequest>) {
    let (project_id, _) = path.into_inner();
    let user_id = request.extensions().get::<UserId>().unwrap();

    // Gate 1: System permission (checked by middleware/guard before reaching handler)
    // Required: category:create

    // Gate 2: Project membership with role requirement
    if !auth_service.check_project_membership(
        user_id, project_id,
        &[ProjectRole::Owner, ProjectRole::Editor]
    ) {
        return HttpResponse::Forbidden().json(ErrorResponse::forbidden());
    }

    // Proceed with category creation
}
```

The system permission check (Gate 1) and the project membership check (Gate 2) use the same
generic `403 Forbidden` response body. An attacker cannot determine whether the denial was
caused by a missing system permission or a wrong project role.

### Denial Logging

When a project membership check returns `false`, the service logs:

```
[INFO] Project scope denied: user_id=42, project_id=7,
       required_roles=[Owner, Editor], actual_role=Some(Viewer)
```

Or for a non-member:

```
[INFO] Project scope denied: user_id=42, project_id=7,
       required_roles=[Owner, Editor], actual_role=None
```

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `ProjectRole` | Domain (1) | Enum: `Owner`, `Editor`, `Contributor`, `Viewer`. May already exist in `project-members` domain -- if so, reuse that definition. |
| `check_project_membership` method on `AuthorizationService` | Application (2) | Extends the existing `AuthorizationService` struct. Method signature: `fn check_project_membership(&self, user_id: i64, project_id: i64, required_roles: &[ProjectRole]) -> bool`. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` (from `auth-rbac`) | Add `check_project_membership` method with dependencies on `AdminBypassService`, `ProjectRepository`, and `ProjectMemberRepository`. |
| `ProjectMemberRepository` (from `project-members`) | Already provides `find_by_id(project_id, user_id) -> Option<ProjectMember>`. No changes needed. The `ProjectMember` entity already carries the `role` field. |
| `ProjectRepository` (from `project-crud`) | Already provides `find_by_id(project_id) -> Option<Project>`. No changes needed. The service only needs to check existence and `deleted_at IS NULL`. |
| Every project-scoped feature handler | Each handler must call `check_project_membership` with the appropriate `required_roles` for the operation. See the integration table below. |

### Required Roles per Operation Type

| Operation | Required Roles | Examples |
|-----------|---------------|----------|
| Read list / detail | Any project member (or empty `required_roles`) | GET categories, GET test cases list |
| Create / Update / Delete | Owner, Editor | POST/PATCH/DELETE categories, test cases |
| Manage members | Owner | POST/PATCH project members |
| Share objects | Owner, Editor | POST sharing endpoints |

### Compile-time interface checks (Rust convention)

No new trait implementations. This feature extends an existing struct with additional
methods. The `ProjectMemberRepository` and `ProjectRepository` traits are already verified
at compile time in their respective features.

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| User is System Admin | -- | -- | -- | Returns `true` immediately; no log |
| User is project member with required role | -- | -- | -- | Returns `true`; no log |
| Project not found or soft-deleted | `403` | `FORBIDDEN` | INFO | Same generic message as missing membership |
| User not a project member | `403` | `FORBIDDEN` | INFO | Same generic message |
| User is member but wrong role | `403` | `FORBIDDEN` | INFO | Same generic message; log includes actual role |
| Database unreachable | `503` | `DATABASE_UNAVAILABLE` | ERROR | All checks fail closed |

**Anti-patterns explicitly avoided:**

- **Do not return `404` for project membership failure** -- it reveals that the project
  exists. Always return `403` for authorization denials.
- **Do not return different error codes** for "not a member" vs "wrong role" vs "project
  not found". All produce the same generic `403`.
- **Do not cache membership** -- compute from live data on every request.
- **Do not implement role hierarchy** in the check -- `[Editor]` does not implicitly
  include `Owner`. The caller must include all roles that are allowed.
- **Do not skip the admin bypass check** -- System Admin must bypass project membership
  checks just as they bypass system permission checks.
