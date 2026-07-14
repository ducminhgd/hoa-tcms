# Design: IAM CLI Init

## Architecture

The `iam-cli-init` feature is a **standalone CLI binary** (`tcms init`) that shares domain and
infrastructure components with the main application. It does **not** go through the HTTP layer
or auth middleware — it operates directly on the database.

```
┌──────────────────────────────────────────────────────────────────┐
│  CLI Binary (cmd/init)                                           │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │  - clap command definition  ("tcms init")                  │  │
│  │  - flag parsing (--username, --email, --password, ...)     │  │
│  │  - interactive prompt fallback  (rpassword / dialoguer)    │  │
│  │  - output formatting  (success/error messages to stdout)   │  │
│  └──────────┬───────────────────────────────────────────────┘  │
│             │ calls                                             │
│             ▼                                                   │
│  Application (Layer 2)                                          │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │  InitUseCase                                                │  │
│  │  - Orchestrates: seed permissions → seed System Admin      │  │
│  │    role → create admin user → assign role                   │  │
│  │  - Runs inside a single transaction                         │  │
│  │  - Idempotent: checks existence before creating             │  │
│  └──────────┬───────────────────────────────────────────────┘  │
│             │ delegates to                                      │
│             ▼                                                   │
│  Infrastructure (Layer 4)                                       │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │  Shared components (reused from app):                      │  │
│  │  - PermissionRepository   (seed/find/find_all)             │  │
│  │  - RoleRepository         (create/find_by_name/assign)     │  │
│  │  - UserRepository         (create/find_by_username/email)  │  │
│  │  - Pbkdf2Hasher           (hash password)                  │  │
│  │                                                             │  │
│  │  Init-specific:                                              │  │
│  │  - ConnectionProvider     (reads DB config, creates pool)   │  │
│  └─────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

**Key design decisions:**

1. **Running alongside the main binary:** The init logic lives in a
   reusable library crate (`tcms-init`) under `src/`, exposed via a subcommand on the same
   `tcms` binary alongside the server command (using clap for CLI argument parsing).
   This shares the dependency tree and database configuration.

2. **Shared transaction:** All seeding and user creation happen within a single database
   transaction. If any step fails, everything is rolled back — no partial initialization.

3. **Idempotency by check-then-act:** Each seeding step checks existence first:
   - Permissions: Insert only those whose `code` does not already exist.
   - System Admin role: Check for role named "System Admin" (case-insensitive); skip creation
     if exists. Always re-sync permissions (ensure all existing permissions are assigned).
   - Admin user: Check for existing user with the provided username or email. If the
     credentials match an existing user who already has System Admin role, skip. If conflicting
     username/email exists for a different user, error out.

4. **No Redis dependency:** The init command does not use Redis. It only needs PostgreSQL.

5. **`created_by` = NULL and `updated_by` = NULL for bootstrap admin:** Per PRD FR-54a,
   `users.created_by` is nullable. The USERS table has both `created_by` and `updated_by`
   as nullable columns to accommodate the bootstrap case. The data model note in `iam-users`
   explains that `updated_by` is also nullable because the bootstrap admin has no referencing
   user. For all non-bootstrap users created via the HTTP API, the application layer sets both
   `created_by` and `updated_by` to the current user's ID.

---

## CLI Contract

### `tcms init`

Bootstrap a fresh HOA TCMS deployment by seeding permissions, creating the System Admin role,
and creating the first admin user.

**Usage:**

```text
tcms init [OPTIONS]
```

**Options (all optional — if a terminal is available, missing options are prompted interactively):**

| Flag | Env Var | Description | Default |
|------|---------|-------------|---------|
| `--username <USERNAME>` | `TCMS_INIT_USERNAME` | Admin username | Prompted |
| `--email <EMAIL>` | `TCMS_INIT_EMAIL` | Admin email address | Prompted |
| `--password <PASSWORD>` | `TCMS_INIT_PASSWORD` | Admin password (min 8 chars) | Prompted (hidden) |
| `--full-name <FULL_NAME>` | `TCMS_INIT_FULL_NAME` | Admin full name | Prompted |
| `--db-url <DATABASE_URL>` | `DATABASE_URL` | PostgreSQL connection string | From env |
| `--yes` / `-y` | — | Skip confirmation prompt | `false` |

**Password flag security note:** Passing `--password` on the command line exposes the password
in the shell history and process list. The command prints a warning when `--password` is used.
Interactive prompt (hidden input) is the recommended approach.

**Exit codes:**

| Code | Meaning |
|------|---------|
| `0` | Success (initialized or already initialized) |
| `1` | Validation error (bad input) |
| `2` | Database connection error or missing schema |
| `3` | Conflict (username/email taken by non-admin user) |
| `4` | Runtime error |

**Success output (first run):**

```text
HOA TCMS initialized successfully.

  Admin username: admin
  Admin email:   admin@example.com

You can now log in at the web interface or use the API.
```

**Success output (idempotent re-run):**

```text
HOA TCMS is already initialized. No changes made.
```

**Error output examples:**

```text
Error: Invalid input
  - password: must be at least 8 characters

Error: Database connection failed
  - Could not connect to PostgreSQL at "localhost:5432". Check that the
    database server is running and DATABASE_URL is set correctly.

Error: Conflict
  - A user with username "admin" already exists. Use a different username
    or check if the system has already been initialized.

Error: Database schema not found
  - The table "users" does not exist. Run database migrations before
    running "tcms init".
```

---

## Data Model

### No new tables

This feature introduces **no new database tables**. It writes to five existing tables:

| Table | Operation | Notes |
|-------|-----------|-------|
| `permissions` | INSERT (seed) | Read-only reference table. Only `created_at` column. |
| `roles` | INSERT (seed) | Creates "System Admin" role. |
| `role_permissions` | INSERT (seed) | Junction table: assigns all permissions to System Admin role. Composite PK (`role_id`, `permission_id`), `created_at` only, hard-delete. |
| `users` | INSERT | Creates the first admin user. Both `created_by` and `updated_by` are set to `NULL` (both columns are nullable to accommodate the bootstrap case). |
| `user_roles` | INSERT | Junction table: assigns System Admin role to the admin user. Composite PK (`user_id`, `role_id`), `created_at` only, hard-delete. |

### Permission catalog (seeded data)

The full permission catalog defines the granular access codes used throughout the system.
Each permission has a unique `code` and a human-readable `name`. The canonical source of
the permission list is `iam-permissions` (51 entries at launch); the seed constants defined
here must match that list exactly.

```rust
// Defined as a constant array in the shared crate.
// Codes use the pattern: <resource>:<action>
// Actions: create, read, read_list, update, delete, select

pub const SEED_PERMISSIONS: &[PermissionSeed] = &[
    // User management
    PermissionSeed { code: "user:create",     name: "Create User" },
    PermissionSeed { code: "user:read",       name: "Read User" },
    PermissionSeed { code: "user:read_list",  name: "List Users" },
    PermissionSeed { code: "user:update",     name: "Update User" },
    PermissionSeed { code: "user:delete",     name: "Delete User" },
    PermissionSeed { code: "user:select",     name: "Select User" },

    // Group management
    PermissionSeed { code: "group:create",    name: "Create Group" },
    PermissionSeed { code: "group:read",      name: "Read Group" },
    PermissionSeed { code: "group:read_list", name: "List Groups" },
    PermissionSeed { code: "group:update",    name: "Update Group" },
    PermissionSeed { code: "group:delete",    name: "Delete Group" },
    PermissionSeed { code: "group:select",    name: "Select Group" },

    // Role management
    PermissionSeed { code: "role:create",     name: "Create Role" },
    PermissionSeed { code: "role:read",       name: "Read Role" },
    PermissionSeed { code: "role:read_list",  name: "List Roles" },
    PermissionSeed { code: "role:update",     name: "Update Role" },
    PermissionSeed { code: "role:delete",     name: "Delete Role" },
    PermissionSeed { code: "role:select",     name: "Select Role" },

    // Permission catalog (read-only)
    PermissionSeed { code: "permission:read_list", name: "List Permissions" },

    // Project management
    PermissionSeed { code: "project:create",    name: "Create Project" },
    PermissionSeed { code: "project:read",      name: "Read Project" },
    PermissionSeed { code: "project:read_list", name: "List Projects" },
    PermissionSeed { code: "project:update",    name: "Update Project" },
    PermissionSeed { code: "project:delete",    name: "Delete Project" },
    PermissionSeed { code: "project:select",    name: "Select Project" },

    // Test Case
    PermissionSeed { code: "test_case:create",    name: "Create Test Case" },
    PermissionSeed { code: "test_case:read",      name: "Read Test Case" },
    PermissionSeed { code: "test_case:read_list", name: "List Test Cases" },
    PermissionSeed { code: "test_case:update",    name: "Update Test Case" },
    PermissionSeed { code: "test_case:delete",    name: "Delete Test Case" },
    PermissionSeed { code: "test_case:select",    name: "Select Test Case" },

    // Test Plan
    PermissionSeed { code: "test_plan:create",    name: "Create Test Plan" },
    PermissionSeed { code: "test_plan:read",      name: "Read Test Plan" },
    PermissionSeed { code: "test_plan:read_list", name: "List Test Plans" },
    PermissionSeed { code: "test_plan:update",    name: "Update Test Plan" },
    PermissionSeed { code: "test_plan:delete",    name: "Delete Test Plan" },
    PermissionSeed { code: "test_plan:select",    name: "Select Test Plan" },

    // Test Run
    PermissionSeed { code: "test_run:create",    name: "Create Test Run" },
    PermissionSeed { code: "test_run:read",      name: "Read Test Run" },
    PermissionSeed { code: "test_run:read_list", name: "List Test Runs" },
    PermissionSeed { code: "test_run:update",    name: "Update Test Run" },
    PermissionSeed { code: "test_run:delete",    name: "Delete Test Run" },
    PermissionSeed { code: "test_run:select",    name: "Select Test Run" },

    // Test Execution
    PermissionSeed { code: "test_execution:create",    name: "Create Test Execution" },
    PermissionSeed { code: "test_execution:read",      name: "Read Test Execution" },
    PermissionSeed { code: "test_execution:read_list", name: "List Test Executions" },
    PermissionSeed { code: "test_execution:update",    name: "Update Test Execution" },
    PermissionSeed { code: "test_execution:delete",    name: "Delete Test Execution" },
    PermissionSeed { code: "test_execution:select",    name: "Select Test Execution" },

    // Sharing
    PermissionSeed { code: "share:create", name: "Share Object" },
    PermissionSeed { code: "share:delete", name: "Unshare Object" },
];

**Note:** This catalog is the single source of truth and must be kept in sync with the
seed migration defined in `iam-permissions`. The `iam-permissions` feature defines the
canonical permission list (51 entries).
```

**Note:** The `permission:read_list` permission is the only permission on the permission
resource itself. Permissions are a read-only catalog — no create/update/delete on permissions.

### System Admin role

| Field | Value |
|-------|-------|
| `name` | `System Admin` |
| Permissions | All permissions from the catalog above |

The System Admin role is immutable through the API (handled by `iam-roles` feature). The init
command creates it if it does not exist, and re-syncs all permissions on every run (in case new
permissions were added in a deployment update).

---

## Sequence

### Initialization Flow (first run)

1. User runs `tcms init` (optionally with `--username`, `--email`, `--password`, `--full-name`).
2. CLI parses flags. For each missing credential, if a terminal is available, prompt
   interactively (use `rpassword` crate for hidden password prompt). If no terminal is available
   and flags are missing, exit with error.
3. Validate all inputs:
   - Username: non-empty, max 255 chars, trimmed.
   - Email: valid format via a proper email regex (e.g., the `email` crate or RFC 5322
     simplified regex) that verifies local-part, `@` symbol, domain with at least one dot,
     and a valid TLD. Reject inputs like `@example.com`, `user@`, `user@.com`, or
     `user@example` (missing TLD). The basic "contains @" check is insufficient.
   - Password: at least 8 characters. Additionally, reject against a common-password list
     (e.g., "password", "12345678", "admin123", "letmein", "qwerty123") to prevent weak
     passwords. Loading a file like `rockyou.txt` is not required, but checking a hardcoded
     list of 100+ common passwords provides significant protection against dictionary attacks.
   - Full name: non-empty, max 255 chars, trimmed. Strip HTML tags for XSS prevention
     (the fullname is a user-controlled display field).
4. Connect to PostgreSQL using `DATABASE_URL` or `--db-url`.
5. Verify that required tables exist (`users`, `roles`, `permissions`). If not, exit with error
   indicating migrations need to be run.
6. **Begin transaction.**
7. **Seed permissions:** Iterate `SEED_PERMISSIONS`. For each permission whose `code` does not
   exist in the `permissions` table, INSERT it. Collect all permission IDs (new + existing).
8. **Seed/update System Admin role:**
   - Find role by name "System Admin" (case-insensitive).
   - If not found: INSERT into `roles` table with `name = 'System Admin'`, `created_by = NULL`.
   - Delete all existing `role_permissions` rows for the System Admin role.
   - INSERT a `role_permissions` row for every permission ID from step 7.
9. **Create admin user:**
   - Check no existing user has the same `username` or `email` (case-insensitive).
   - Hash the password using `Pbkdf2Hasher::hash(password)`.
   - INSERT into `users` (`username`, `email`, `password_hash`, `fullname`, `status`,
     `created_by`, `updated_by`, `created_at`, `updated_at`)
     with `created_by = NULL`, `updated_by = NULL`.
   - Use `RETURNING id` to capture the DB-assigned ID.
10. **Assign role to user:**
    - INSERT into `user_roles` with the admin user's ID and the System Admin role's ID.
11. **Commit transaction.**
12. Output success message with username and email.
13. Exit with code `0`.

### Idempotent Re-run Flow

Steps 1–5 are the same. Then:

6. **Begin transaction.**
7. **Seed new permissions:** Only insert permissions whose `code` does not exist. Existing
   permissions are untouched.
8. **Update System Admin role permissions:** Re-sync all permissions — delete all existing
   `role_permissions` for the System Admin role, then insert all current permission IDs. This
   ensures any newly added permissions are granted.
9. **Check admin user(s):**
   - Query for any existing user with the System Admin role.
   - If at least one System Admin user exists:
     - If the provided credentials match that user exactly, output "already initialized" and
       commit (no duplicate user).
     - If the provided credentials conflict (same username/email but different user), error out.
   - If no System Admin user exists, create the admin user as in the first-run flow.
10. **Commit transaction.**
11. Output success or "already initialized" message.
12. Exit with code `0`.

---

## Components

### New Components (within this feature)

| Component | Layer | Role |
|-----------|-------|------|
| `InitCommand` (clap) | CLI / Entry point | Defines the `init` subcommand with its flags. Parses args, prompts for missing inputs, calls `InitUseCase`, formats output. |
| `InitUseCase` | Application (2) | Orchestrates the full init flow: permission seeding, role seeding/update, user creation, role assignment. Runs everything in a single transaction. Uses `PasswordHasher` (from `iam-auth`), `PermissionRepository`, `RoleRepository`, `UserRepository` (from `iam-users`). |
| `PermissionSeeder` | Infrastructure (4) | Reads the `SEED_PERMISSIONS` constant, checks which codes are missing from the DB, inserts new ones, returns all permission IDs. |
| `RoleSeeder` | Infrastructure (4) | Creates or finds the "System Admin" role. Deletes and re-inserts `role_permissions` for the role. |
| `SchemaVerifier` | Infrastructure (4) | Checks that required tables exist in the database before init proceeds. |
| `InitOutput` | CLI / Adapter | Formats success/error messages for the terminal. |

### Shared / Reused Components

| Component | Source Feature | How Used |
|-----------|---------------|----------|
| `Pbkdf2Hasher` | `iam-auth` | Hash the admin password before storage. Same PBKDF2 configuration. |
| `PasswordHash` (domain value object) | `iam-auth` | Parse and validate the password hash format. |
| `UserRepository` | `iam-users` | Create user, check uniqueness (find by username/email). |
| `RoleRepository` | `iam-roles` | Find/create roles, assign permissions. |
| `PermissionRepository` | `iam-permissions` | CRUD on permissions catalog — find by code, list all. |
| Database connection pool | Main app config | Reuse the same `sqlx` pool configuration from the app's config module. |

### Crate/Directory Structure

Since this project uses Rust with a Leptos/Actix-Web stack:

```
src/
├── application/
│   └── init_use_case.rs          # InitUseCase
├── infrastructure/
│   ├── db/
│   │   ├── permission_seeder.rs  # PermissionSeeder
│   │   ├── role_seeder.rs        # RoleSeeder
│   │   └── schema_verifier.rs    # SchemaVerifier
│   └── config.rs                 # (existing) — add DB connection helper
├── adapters/
│   └── cli/
│       └── init_command.rs       # InitCommand + InitOutput
└── cmd/
    └── hoa-tcms/
        └── main.rs               # (existing) — add "init" subcommand
```

---

## Error Handling

### Error Cases and Output

| Error Case | Exit Code | Message Pattern | Notes |
|------------|-----------|-----------------|-------|
| Missing required flags (non-interactive) | 1 | `Error: Missing required input\n  - username: must be provided via --username flag or TCMS_INIT_USERNAME env var` | Only when no TTY is available |
| Invalid username (empty/too long) | 1 | `Error: Invalid input\n  - username: must be between 1 and 255 characters` | |
| Invalid email format | 1 | `Error: Invalid input\n  - email: "foo" is not a valid email address` | |
| Password too short | 1 | `Error: Invalid input\n  - password: must be at least 8 characters` | |
| Database connection failed | 2 | `Error: Database connection failed\n  - Could not connect to PostgreSQL. Connection string: "postgres://..."` | Never log the password portion of the URL |
| Schema not found (tables missing) | 2 | `Error: Database schema not found\n  - Required table "users" does not exist. Run database migrations first.` | |
| Username/email conflict | 3 | `Error: Conflict\n  - A user with username "admin" already exists.` | Only when the conflicting user is NOT a System Admin |
| Unexpected runtime error | 4 | `Error: Initialization failed\n  - <descriptive message without stack trace>` | Log full error with trace to stderr |

### Design Constraints

- **Always use transactions.** Never write partial seed data.
- **Do not allow running init when the database has data but is partially initialized.** If
  permissions exist but the System Admin role is missing (unlikely but possible), complete the
  missing steps rather than erroring.
- **Do not expose database internals in CLI output.** SQL error messages are logged to stderr
  with full detail but the CLI output is user-friendly.
- **Do not allow `--password` without warning.** Print a warning to stderr when password is
  provided via flag: `WARNING: Password provided via command line flag — this may be visible in
  shell history. Consider using the interactive prompt instead.`
- **Do not overwrite an existing admin user's password** on re-run. If the admin user already
  exists, the re-run is a no-op for user data.
