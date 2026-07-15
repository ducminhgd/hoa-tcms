# Design: Sharing Roles

## Architecture

Sharing roles are a domain-level enumeration consumed by the sharing override
authorization logic and the sharing UI. They are stored alongside sharing entries and
validated at the application boundary.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  SharingHandler (see sharing-ui) validates role strings against the    │   │
│  │  SharingRole enum before passing to the service.                       │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  SharingService (see sharing-ui):         │  │  - SharingRole (enum)   │  │
│  │  - validates role assignment             │  │  - SharingEntry (entity)│  │
│  │  - enforces max-role constraint          │  └──────────────────────────┘  │
│  │  - delegates to SharingRepository        │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  share_roles lookup table (or CHECK constraint on sharing_entries)     │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. Sharing roles are defined as a domain enumeration with three variants.
2. The API layer accepts role strings (`"editor"`, `"contributor"`, `"viewer"`) and
   maps them to the enum.
3. The application layer validates that the requesting user is authorized to assign
   the given role (max-role constraint: cannot grant a role higher than one's own).
4. The role is stored as a string in the `SHARING_ENTRIES` table, constrained by a
   `CHECK` or a lookup table FK.
5. The authorization layer reads the sharing role at access-check time and applies the
   override logic (see `sharing-override`).

---

## Sharing Role Enumeration

### Domain Definition

```text
SharingRole ::= EDITOR | CONTRIBUTOR | VIEWER
```

| Role | Capabilities on Shared Object |
|------|------------------------------|
| `EDITOR` | Read, update, delete, **share** (manage sharing entries for this object) |
| `CONTRIBUTOR` | Read, update. Cannot share. Cannot delete (unless the underlying object rules allow it via project role). |
| `VIEWER` | Read only. No mutations, no sharing. |

### Comparison with Project Roles

| Aspect | Project Roles | Sharing Roles |
|--------|--------------|---------------|
| Scope | Project-wide | Per-object |
| Values | Owner, Editor, Contributor, Viewer | Editor, Contributor, Viewer |
| Storage | `PROJECT_MEMBERS` table | `SHARING_ENTRIES` table |
| Precedence | Baseline | Overrides project role on the shared object |
| Assignable by | Project Owner | Any user with Editor sharing role (or Editor/Owner project role) on the object |

The sharing role set is deliberately a subset of the project role set (no Owner
equivalent). Object ownership is always determined by project membership; sharing
cannot transfer ownership.

---

## Role Assignment Rules

### Max-Role Constraint

When user A creates or updates a sharing entry for user B on object O, the sharing
role assigned to B must not exceed the effective role of A on O. The effective role is
computed as:

1. If A has a sharing entry on O, use that sharing role.
2. Otherwise, use A's project membership role on O's project.

The hierarchy from highest to lowest: Editor > Contributor > Viewer.

**Examples:**

- A is a Viewer on the project but an Editor on the shared object (via a prior
  sharing entry). A can assign Editor, Contributor, or Viewer.
- A is a Contributor on the project and has no sharing entry on the object. A can
  assign Contributor or Viewer, but not Editor.
- A is a Viewer on the project and has no sharing entry on the object. A cannot
  create sharing entries at all (Viewers cannot share).

### Self-Share Prevention

A user cannot create a sharing entry targeting themselves. The system must reject such
requests with `422 Unprocessable Entity`.

### Duplicate Prevention

A user cannot have more than one sharing entry for the same object. If an entry
already exists for (object_type, object_id, user_id), the request to create another
must be rejected with `409 Conflict`. Group entries are identified by
(object_type, object_id, group_id).

---

## Data Model

### Option A: CHECK Constraint (Inline Enum)

```sql
CREATE TABLE sharing_entries (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    object_type VARCHAR(50) NOT NULL,
    object_id BIGINT NOT NULL,
    user_id BIGINT REFERENCES users(id) ON DELETE CASCADE,
    group_id BIGINT REFERENCES groups(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL,
    created_by BIGINT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by BIGINT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT chk_sharing_entries_target
        CHECK ((user_id IS NOT NULL AND group_id IS NULL)
            OR (user_id IS NULL AND group_id IS NOT NULL)),

    CONSTRAINT chk_sharing_entries_role
        CHECK (role IN ('editor', 'contributor', 'viewer'))
);
```

### Option B: Lookup Table + FK

```sql
CREATE TABLE share_roles (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    code VARCHAR(20) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL
);

INSERT INTO share_roles (code, name) VALUES
    ('editor', 'Editor'),
    ('contributor', 'Contributor'),
    ('viewer', 'Viewer');

CREATE TABLE sharing_entries (
    -- ... same columns as Option A ...
    role_id BIGINT NOT NULL REFERENCES share_roles(id) ON DELETE RESTRICT
);
```

**Design decision:** Option A (CHECK constraint) is preferred for Phase 1 because:

- The role set is small (3 values) and stable.
- No additional JOIN is needed at query time.
- The string value is self-describing in query results and logs.
- Option B would be warranted if roles needed metadata (description, display order) or
  if the set was expected to grow, but that is deferred to a future phase.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `SharingRole` | Domain (1) | Enum with three variants: `Editor`, `Contributor`, `Viewer`. Provides `from_str(s) -> Result<Self>` for parsing, `as_str() -> &str` for serialization, and a partial ordering (`can_assign(other) -> bool`) for the max-role constraint. |
| `SharingEntry` | Domain (1) | Entity: `id`, `object_type`, `object_id`, `user_id` (optional), `group_id` (optional), `role` (SharingRole), `created_by`, `created_at`, `updated_by`, `updated_at`. Validates that exactly one of `user_id` or `group_id` is present. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `AuthorizationService` | Add `get_effective_role(user_id, object_type, object_id) -> EffectiveRole` method that checks sharing entries first, then falls back to project membership. This is the core override logic (detailed in `sharing-override`). |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Notes |
|------------|-------------|------------|-------|
| Invalid role string in request | `422` | `VALIDATION_ERROR` | "role must be one of: editor, contributor, viewer" |
| User attempts to assign role exceeding their own | `403` | `FORBIDDEN` | "You cannot grant a role higher than your own" |
| User attempts to share with themselves | `422` | `VALIDATION_ERROR` | "Cannot create a sharing entry for yourself" |
| Duplicate sharing entry for same user + object | `409` | `DUPLICATE_SHARING_ENTRY` | A sharing entry already exists for this user on this object |
| Neither user_id nor group_id provided | `422` | `VALIDATION_ERROR` | "Either user_id or group_id must be provided" |
| Both user_id and group_id provided | `422` | `VALIDATION_ERROR` | "Provide either user_id or group_id, not both" |
