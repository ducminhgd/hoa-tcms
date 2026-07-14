# Tasks: IAM Permissions

Tasks ordered by dependency. All tasks reference their source section in either
`requirements.md` or `design.md`.

---

- [ ] **1. Create domain entity and repository interface** — design.md#Components
  - Create `Permission` domain entity with fields: `id` (`i64`), `name` (`String`),
    `code` (`String`), `created_at` (`DateTime<Utc>`).
  - Create `PermissionRepository` trait in the application layer with methods:
    - `async fn find_all(&self, page: u32, limit: u32) -> Result<(Vec<Permission>, u64)>`
    - `async fn find_by_code(&self, code: &str) -> Result<Option<Permission>>`
  - Create response DTO structs: `PermissionDTO { id, name, code }` and the paginated
    wrapper `PermissionsResponse { data, meta }`.
  - References: requirements.md#US-2, requirements.md#US-4, design.md#Components

- [ ] **2. Create the seed migration** — design.md#Data-Model
  - Create migration file `V2__seed_permissions.sql` containing:
    - `CREATE TABLE permissions (...)` with `id`, `name`, `code`, `created_at`.
    - `CREATE UNIQUE INDEX uq_permissions_code ON permissions (code)`.
    - `INSERT INTO permissions (name, code)` statements for all 51 permission rows
      (ordered by `code`).
  - Ensure migration is idempotent or versioned (use `IF NOT EXISTS` on the table).
  - Ensure this migration runs **before** the `iam-roles` migration that creates
    `role_permissions` (the FK depends on `permissions.id`).
  - References: requirements.md#US-2, requirements.md#US-4, design.md#Data-Model

- [ ] **3. Implement `SqlPermissionRepository`** — design.md#Components
  - Implement `PermissionRepository` trait for `SqlPermissionRepository` using `sqlx`
    or `diesel`.
  - `find_all`: Execute `SELECT id, name, code, created_at FROM permissions ORDER BY code
    LIMIT $1 OFFSET $2` plus `SELECT COUNT(*) FROM permissions`.
  - `find_by_code`: Execute `SELECT id, name, code, created_at FROM permissions WHERE code = $1`.
  - Add compile-time trait implementation check.
  - References: design.md#Components, design.md#Sequence

- [ ] **4. Implement `ListPermissionsUseCase`** — design.md#Components
  - Create `ListPermissionsUseCase` struct with a `PermissionRepository` dependency.
  - `execute(page, limit)` method that:
    - Validates page/limit bounds (page >= 1, limit in 1..=100).
    - Calls `repository.find_all(page, limit)`.
    - Maps domain `Permission` entities to `PermissionDTO` values.
    - Returns `PermissionsResponse { data, meta }`.
  - References: requirements.md#US-1, design.md#Sequence, design.md#API-Contract

- [ ] **5. Create the HTTP handler and register the route** — design.md#Components
  - Create `ListPermissionsHandler` (Actix-Web handler):
    - Extract `page` and `limit` from query string (defaults: page=1, limit=25).
    - Require `permission:read_list` permission (auth middleware + permission guard).
    - Call `ListPermissionsUseCase::execute`.
    - Return `200 OK` with JSON body.
  - Register `GET /api/v1/permissions` in the HTTP router.
  - Ensure the router **does not** register POST/PUT/PATCH/DELETE for this path.
  - Add a catch-all handler that returns `405 Method Not Allowed` with `Allow: GET`
    header for any non-GET request to `/api/v1/permissions`.
  - References: requirements.md#US-1, requirements.md#US-3, design.md#API-Contract,
    design.md#Error-Handling

- [ ] **6. Write unit tests** — requirements.md#US-1 requirements.md#US-2
  - Test `Permission` entity construction and field accessors.
  - Test `ListPermissionsUseCase`:
    - Happy path: valid page/limit returns paginated results.
    - Edge cases: page=1, limit=max, single-page result, empty result.
    - Error cases: page=0 returns validation error, limit=0 returns validation error,
      limit=101 returns validation error.
  - Test `PermissionDTO` mapping from domain entity (fields are correctly mapped,
    `created_at` is excluded from the DTO).

- [ ] **7. Write integration tests** — requirements.md#US-1, design.md#API-Contract
  - Test seed migration against a test database:
    - Verify all 51 rows are inserted.
    - Verify unique constraint on `code` (duplicate insert fails).
    - Verify the table structure matches the schema.
  - Test `SqlPermissionRepository`:
    - `find_all` returns paginated results ordered by code.
    - `find_by_code` returns the correct permission for a known code.
    - `find_by_code` returns `None` for an unknown code.
  - Test API endpoint via HTTP:
    - `GET /api/v1/permissions` with valid session + `permission:read_list` returns
      `200 OK` with paginated data.
    - `GET /api/v1/permissions` without session returns `401 Unauthorized`.
    - `GET /api/v1/permissions` with session but without `permission:read_list` returns
      `403 Forbidden`.
    - `POST /api/v1/permissions` returns `405 Method Not Allowed`.
    - `PUT /api/v1/permissions` returns `405 Method Not Allowed`.
    - `DELETE /api/v1/permissions` returns `405 Method Not Allowed`.
    - Invalid pagination params return `422 Unprocessable Entity`.
