# Design: Metadata Templates

## Architecture

The Metadata Templates feature follows Clean Architecture layering. Templates are
per-project metadata entities. All endpoints are nested under the project resource path
(`/api/v1/projects/{projectId}/templates`) and require both system RBAC permission and
project membership scope checks.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers:                                                        │   │
│  │  - list_templates     GET    /api/v1/projects/{pid}/templates         │   │
│  │  - create_template    POST   /api/v1/projects/{pid}/templates         │   │
│  │  - get_template       GET    /api/v1/projects/{pid}/templates/{id}    │   │
│  │  - update_template    PATCH  /api/v1/projects/{pid}/templates/{id}    │   │
│  │  - delete_template    DELETE /api/v1/projects/{pid}/templates/{id}    │   │
│  │  - select_templates   GET    /api/v1/projects/{pid}/templates/select  │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  TemplateService:                         │  │  TestCaseTemplate entity│  │
│  │  - create_template                        │  └──────────────────────────┘  │
│  │  - list_templates                         │                                │
│  │  - get_template                           │                                │
│  │  - update_template                        │                                │
│  │  - delete_template                        │                                │
│  │  - select_templates                       │                                │
│  │                                           │                                │
│  │  Interfaces:                              │                                │
│  │  - TemplateRepository (port)              │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTemplateRepository  (implements TemplateRepository)              │   │
│  │  - Template seeder        (part of ConfigFileSeeder; seeds from YAML)  │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. All template endpoints require an authenticated session (checked by `AuthMiddleware`).
2. `POST`, `PATCH`, and `DELETE` endpoints additionally require that the caller be an
   Owner or Editor of the project (or a System Admin), on top of the system permission
   check. `GET` endpoints require any project membership (or System Admin).
3. Create validates unique name and inserts within a single database transaction to
   prevent TOCTOU races between the duplicate check and the insert.
4. Update validates unique name (if name is being changed) and applies the update within a
   single database transaction to prevent TOCTOU races. Rejects updates on soft-deleted records.
5. Soft-delete has no referential-integrity gate. Templates are pre-fill sources; they are
   not referenced by FK from any other table. A template can be deleted freely.
6. The `/select` endpoint returns a flat, unpaginated list of `{id, name, description}`
   for dropdown components (including the description so the UI can show a preview/snippet
   on hover or selection).

### Security Requirements

**Input sanitization:** Template `name` and `description` fields must be sanitized on
input (strip disallowed HTML tags) before storage. Output-encoding must be applied at the
presentation layer. See PRD 5.3 for the project-wide XSS prevention policy.

**CSRF protection:** All state-changing endpoints (`POST`, `PATCH`, `DELETE`) must be
protected against CSRF. Session cookies must carry `SameSite=Lax` (or stricter).
`POST`/`PATCH`/`DELETE` requests with `Content-Type: application/json` must verify the
`Content-Type` header to block simple form-based CSRF attacks. Additional anti-CSRF
tokens SHOULD be implemented for all state-changing endpoints as an additional
defence-in-depth layer beyond SameSite cookies and Content-Type verification.

**Authorization layering:** The system permission check and the project membership role
check are independent gates. Both must pass (or the caller must be a System Admin, who
implicitly holds all system permissions and bypasses all project membership checks).
Authorization checks are performed against live data on every request -- permissions and
project membership are never cached in the session. If a user's role is changed
mid-session, the new role takes effect on their next request. A `403 Forbidden` response
must use a generic message that does not distinguish between "missing system permission"
and "wrong project role".

**Audit field visibility:** The `created_by` field is included in list and detail API
responses; `updated_by` is included in detail and create responses only (not in list or
select). Both fields are visible to all project members (including Viewers). This is
intentional: within a project, audit transparency (who created/modified a template) aids
collaboration. The fields carry numeric user IDs, not emails or names, limiting direct PII
exposure.

**Rate limiting:** All endpoints are protected by rate limiting configured at the
middleware layer. State-changing endpoints (POST/PATCH/DELETE) allow 30 req/min; read
endpoints (GET list/detail) allow 60 req/min; the select endpoint allows 120 req/min
due to frequent UI usage. See requirements.md Security Considerations for the full table.

**No referential integrity on delete:** Templates are not referenced by FK from any table.
The `test-case-template` feature (separate spec) copies template content into the test
case description at creation/edit time; no ongoing FK relationship exists. Therefore,
template soft-delete does not require a referential-integrity check and will always
succeed (assuming the template exists and is not already deleted).

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

### GET `/api/v1/projects/{projectId}/templates`

List templates in a project with pagination, filtering, and sorting.

**Required Permission:** `template:read_list`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Query Parameters:**

| Param | Type | Default | Max | Description |
|-------|------|---------|-----|-------------|
| `page` | integer | 1 | -- | Page number (1-based) |
| `limit` | integer | 25 | 100 | Items per page |
| `search` | string | -- | 255 | Case-insensitive substring match on `name` and `description` |
| `sort` | string | `name` | -- | Sort field: `name`, `-name`, `created_at`, `-created_at` |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 1,
      "name": "Login Test",
      "description": "1. Navigate to login page\n2. Enter valid credentials\n3. Verify redirect to dashboard",
      "created_by": 42,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_at": "2026-07-14T10:00:00Z"
    },
    {
      "id": 2,
      "name": "API Test",
      "description": "1. Send request to endpoint\n2. Verify 200 response\n3. Validate response body schema",
      "created_by": 42,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_at": "2026-07-15T08:30:00Z"
    }
  ],
  "meta": {
    "total": 12,
    "page": 1,
    "limit": 25
  }
}
```

**Notes:**
- Results are scoped to the given project and exclude soft-deleted templates
  (`WHERE project_id = $1 AND deleted_at IS NULL`).
- Default sort is `name` ascending (case-insensitive).
- `search` applies `ILIKE` on both `name` and `description` when provided;
  when absent, no filter is applied. The characters `%`, `_`, and `\` in the search value
  are escaped before building the ILIKE pattern (the user is searching for literal
  text, not writing SQL wildcards). Backslashes are doubled (`\\`) to prevent them
  from being consumed as PostgreSQL escape prefixes. The search value must not exceed 255 characters.
- The `sort` parameter accepts: `name` (ascending, default), `-name` (descending),
  `created_at` (oldest first), `-created_at` (newest first).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `template:read_list` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `422` | `VALIDATION_ERROR` | Invalid pagination, sort, or search parameter |

---

### POST `/api/v1/projects/{projectId}/templates`

Create a new template within a project.

**Required Permission:** `template:create` AND project role Owner or Editor

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Request Body:**

```json
{
  "name": "Login Test",
  "description": "1. Navigate to login page\n2. Enter valid credentials\n3. Verify redirect to dashboard"
}
```

| Field | Type | Required | Default | Constraints |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | -- | 1-255 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input (HTML stripped); unique per project (case-insensitive) |
| `description` | string | No | `null` | Max 10000 characters; sanitized on input (HTML stripped). Empty string `""` stored as-is. Explicit `null` stores `NULL`. Omitting the field defaults to `null`. |

**Success Response:** `201 Created`

Headers: `Location: /api/v1/projects/42/templates/15`

```json
{
  "data": {
    "id": 15,
    "name": "Login Test",
    "description": "1. Navigate to login page\n2. Enter valid credentials\n3. Verify redirect to dashboard",
    "project_id": 42,
    "created_by": 1,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 1,
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- `project_id` in the response mirrors the path parameter.
- `created_by` and `updated_by` are both set to the authenticated user's ID.
- The unique name check uses `LOWER(name) = LOWER($1)` and includes
  `WHERE project_id = $2 AND deleted_at IS NULL`.
- Soft-deleted templates with the same name do not block creation.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `template:create` permission or is not Owner/Editor of the project |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `409` | `DUPLICATE_TEMPLATE_NAME` | Template name already exists in this project (case-insensitive) |
| `422` | `VALIDATION_ERROR` | Name empty/whitespace-only/exceeds max length, description exceeds max length, unrecognised fields |

`409 Conflict` response body for duplicate name:

```json
{
  "error": {
    "code": "DUPLICATE_TEMPLATE_NAME",
    "message": "A template with the name 'Login Test' already exists in this project.",
    "details": [
      { "field": "name", "message": "Template name must be unique within the project" }
    ]
  }
}
```

---

### GET `/api/v1/projects/{projectId}/templates/{id}`

Get full detail of a single template.

**Required Permission:** `template:read`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Template ID |

**Success Response:** `200 OK`

```json
{
  "data": {
    "id": 15,
    "name": "Login Test",
    "description": "1. Navigate to login page\n2. Enter valid credentials\n3. Verify redirect to dashboard",
    "project_id": 42,
    "created_by": 1,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 1,
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Notes:**
- `deleted_at` and `deleted_by` are never returned to the client.
- The template must belong to the specified project; if the template exists but belongs
  to a different project, `404 Not Found` is returned (same as non-existent).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `template:read` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist, is soft-deleted, or template does not exist / is soft-deleted / belongs to a different project |

---

### PATCH `/api/v1/projects/{projectId}/templates/{id}`

Update a template's name and/or body content.

**Required Permission:** `template:update` AND project role Owner or Editor

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Template ID |

**Request Body** (all fields optional; at least one required):

```json
{
  "name": "Updated Login Test",
  "description": "1. Navigate to login page\n2. Enter invalid credentials\n3. Verify error message"
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `name` | string | No | 1-255 characters, must contain at least one non-whitespace character; leading/trailing whitespace trimmed; sanitized on input (HTML stripped); unique per project (case-insensitive) |
| `description` | string | No | Max 10000 characters; sanitized on input (HTML stripped). Omitting the field preserves the current value. Sending `null` explicitly clears it (sets `NULL`). Sending `""` stores empty string. |

**Success Response:** `200 OK`

Response body is the updated template representation (same shape as GET detail).

**Notes:**
- At least one field must be provided (empty body returns `422`).
- Only `name` and `description` are recognized fields. All other fields (`id`,
  `project_id`, `created_by`, `created_at`, `updated_by`, `updated_at`, `deleted_by`,
  `deleted_at`) are treated as unrecognized and return `422`.
- `id`, `project_id`, `created_at`, and `created_by` are immutable.
- `deleted_at` and `deleted_by` cannot be modified through this endpoint.
- `updated_at` and `updated_by` are set automatically on change.
- The unique name check is only performed if `name` is provided and differs from the
  current value (after trimming). The duplicate check query MUST exclude the current
  template's ID (`AND id != $currentId`) to allow case-only name changes (e.g.,
  "Login Test" to "LOGIN TEST"). If the trimmed name matches the current trimmed name,
  the name update is a no-op (duplicate check skipped, no DB write for name).
- Soft-deleted templates cannot be updated (per FR-54c).
- Unrecognised fields in the request body cause a `422 Unprocessable Entity` response
  (strict mode -- rejects typos and unknown fields for defence-in-depth).
- The template must belong to the specified project; if it belongs to a different
  project, `404 Not Found` is returned.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `template:update` permission or is not Owner/Editor of the project |
| `404` | `NOT_FOUND` | Project or template does not exist, is soft-deleted, or template belongs to a different project |
| `409` | `DUPLICATE_TEMPLATE_NAME` | Updated name conflicts with another template in the same project |
| `422` | `VALIDATION_ERROR` | No fields provided, name whitespace-only/exceeds max length, description exceeds max length, unrecognised fields |

---

### DELETE `/api/v1/projects/{projectId}/templates/{id}`

Soft-delete a template.

**Required Permission:** `template:delete` AND project role Owner or Editor

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `id` | integer | Template ID |

**Request Body:** None

**Success Response:** `204 No Content`

**Notes:**
- Sets `deleted_at = NOW()` and `deleted_by = <current_user_id>`.
- Unlike categories, templates have **no referential-integrity gate**. Templates are
  pre-fill sources copied into test case descriptions at creation/edit time; no FK from
  `TEST_CASES` references `TEST_CASE_TEMPLATES`. Deletion always succeeds (assuming the
  template exists and is not already soft-deleted).
- The soft-delete executes within a database transaction for consistency.
- Repeated DELETE on an already soft-deleted template returns `404`.
- The template must belong to the specified project.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `template:delete` permission or is not Owner/Editor of the project |
| `404` | `NOT_FOUND` | Project or template does not exist, is soft-deleted, or belongs to a different project |

---

### GET `/api/v1/projects/{projectId}/templates/select`

Return a compact list of all non-deleted templates for dropdown/selection UI components.

**Required Permission:** `template:select`

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |

**Success Response:** `200 OK`

```json
{
  "data": [
    {
      "id": 1,
      "name": "API Test",
      "description": "1. Send request to endpoint\n2. Verify 200 response\n3. Validate response body schema"
    },
    {
      "id": 5,
      "name": "Login Test",
      "description": "1. Navigate to login page\n2. Enter valid credentials\n3. Verify redirect to dashboard"
    },
    {
      "id": 2,
      "name": "Regression Check",
      "description": "1. Identify affected modules\n2. Run regression suite\n3. Compare baseline results"
    }
  ]
}
```

**Notes:**
- Unpaginated: returns all non-deleted templates for the project. The SQL query uses
  `LIMIT 500`; results are ordered by `LOWER(name)` ascending. If the result set has
  500 rows, a warning is logged (the project may have more templates not returned)
  and an `X-Result-Truncated: true` response header is set so the UI can surface this.
- Each entry contains `id`, `name`, and `description`. The `description` field is included
  so the UI can display a preview/snippet when the user hovers over or selects a template
  in the dropdown (e.g., first 100 characters as a tooltip).
- Results are ordered by `name` ascending (case-insensitive).
- Soft-deleted templates are excluded.
- Route registration order matters: the `/select` path must be registered **before**
  the `/{id}` path to prevent `select` from being interpreted as an ID.

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `template:select` permission or is not a project member |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |

---

## Data Model

### New Table: TEST_CASE_TEMPLATES

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `project_id` | `BIGINT` | `NOT NULL`, `REFERENCES projects(id) ON DELETE RESTRICT` | Each template belongs to exactly one project |
| `name` | `VARCHAR(255)` | `NOT NULL` | Template name; unique per project (case-insensitive); see constraint below |
| `description` | `TEXT` | | Nullable; max 10000 chars enforced at app layer and by `CHECK` constraint below |
| `created_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who created the template |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Last user who modified the template |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by `BEFORE UPDATE` trigger |
| `deleted_by` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Nullable; set on soft-delete |
| `deleted_at` | `TIMESTAMPTZ` | | Nullable; set on soft-delete |

**Constraints:**

```sql
-- Description length guard (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_case_templates
  ADD CONSTRAINT chk_test_case_templates_description
  CHECK (description IS NULL OR char_length(description) <= 10000);

-- Case-insensitive unique name per project (PostgreSQL)
-- Only enforced for non-deleted rows, allowing a soft-deleted row and a new row
-- with the same name to coexist.
CREATE UNIQUE INDEX uq_test_case_templates_name_project
  ON test_case_templates (project_id, LOWER(name))
  WHERE deleted_at IS NULL;

-- Foreign key indexes
CREATE INDEX idx_test_case_templates_project_id ON test_case_templates (project_id);
CREATE INDEX idx_test_case_templates_created_by ON test_case_templates (created_by);
CREATE INDEX idx_test_case_templates_updated_by ON test_case_templates (updated_by);
CREATE INDEX idx_test_case_templates_deleted_by ON test_case_templates (deleted_by);

-- Partial index for active templates (most queries filter out soft-deleted)
CREATE INDEX idx_test_case_templates_active ON test_case_templates (project_id, LOWER(name))
  WHERE deleted_at IS NULL;
```

**Design notes:**

- **Partial unique index** `uq_test_case_templates_name_project` enforces name uniqueness
  only among non-deleted rows. This allows creating a new template with the same name
  as a previously soft-deleted one.
- **`project_id` FK with `RESTRICT`** prevents deleting a project that has templates.
  Project soft-delete does not cascade (templates remain with their original
  `deleted_at IS NULL`; they are hidden because queries filter on
  `WHERE project.deleted_at IS NULL`).
- **`ON DELETE RESTRICT` on user FKs** (`created_by`, `updated_by`, `deleted_by`)
  prevents deleting a user who has created, updated, or deleted templates. This
  preserves audit trail integrity. Note that this means a user who performed any
  template operation can never be hard-deleted; offboarding must handle this gracefully.
- **`deleted_at` and `deleted_by`** follow the project-wide soft-delete convention:
  both set together on soft-delete, both `NULL` for active records.
- **Semantic note on `description`:** The `description` field on templates holds the
  template body/content (multi-step test instructions, up to 10000 chars), which is
  semantically different from the `description` field on categories (a short annotation,
  up to 2000 chars). The field name is reused for API surface consistency, not semantic
  equivalence.
- **No FK from `TEST_CASES`** --- templates are pre-fill sources whose content is
  copied into test cases at creation/edit time. There is no ongoing referential
  relationship. This means template deletion is always allowed and does not require
  checking for referencing test cases.
- **No `status` column** -- templates do not have an ACTIVE/INACTIVE state. They are
  either present (non-deleted) or soft-deleted. The project `status` field controls
  visibility of all child metadata.

### BEFORE UPDATE Trigger

```sql
CREATE OR REPLACE FUNCTION trg_test_case_templates_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_test_case_templates_updated_at
  BEFORE UPDATE ON test_case_templates
  FOR EACH ROW
  EXECUTE FUNCTION trg_test_case_templates_updated_at();
```

The trigger ensures `updated_at` is always set to the current timestamp on every `UPDATE`,
regardless of which code path performs the update. The application layer does not need to
set `updated_at` explicitly.

### Relationship to Other Tables

```
PROJECTS ──< TEST_CASE_TEMPLATES
```

- `TEST_CASE_TEMPLATES.project_id` → `PROJECTS.id` (each template belongs to one project)
- **No FK from `TEST_CASES`** to `TEST_CASE_TEMPLATES`. Templates are used at test-case
  creation/edit time to pre-fill the `description` field; the content is copied, and no
  persistent reference is maintained. This is intentionally different from categories
  (which have a `category_id` FK on `TEST_CASES`).

---

## YAML Config Seed Format

Default templates are defined in the YAML configuration file loaded at application
startup. The file path is set via environment variable:

| Env Var | Default | Description |
|---------|---------|-------------|
| `TEMPLATE_CONFIG_PATH` | `config/defaults.yaml` | Path to the YAML file containing default templates |

The `templates` section is a list of objects, each with `name` and optional
`description` (the template body/content).

```yaml
# config/defaults.yaml
templates:
  - name: "Login Test"
    description: |
      1. Navigate to the login page
      2. Enter valid username and password
      3. Click the "Sign In" button
      4. Verify redirection to the dashboard
      5. Confirm the user's name appears in the header

  - name: "API Smoke Test"
    description: |
      1. Send a GET request to the health check endpoint
      2. Verify the response status code is 200
      3. Confirm the response body contains {"status": "ok"}

  - name: "CRUD Happy Path"
    description: |
      1. Create a new resource via POST
      2. Verify 201 Created and Location header
      3. Retrieve the resource via GET
      4. Verify all fields match the creation input
      5. Update the resource via PATCH
      6. Verify 200 OK and updated fields
      7. Delete the resource via DELETE
      8. Verify 204 No Content
      9. Confirm GET returns 404

  - name: "Form Validation"
    description: |
      1. Submit the form with all fields empty
      2. Verify validation error messages for each required field
      3. Submit with invalid email format
      4. Verify email-specific error message
      5. Submit with password shorter than minimum length
      6. Verify password-specific error message
```

**Seeding behaviour:**
- The `ConfigFileSeeder` (defined in `project-crud` design) loads the YAML file at
  startup.
- On project creation, the seeder inserts each template in the `templates` list into
  the `TEST_CASE_TEMPLATES` table with the new `project_id`, `created_by` set to the project
  creator, and `created_at`/`updated_at` set to `NOW()`.
- Templates are inserted in the order they appear in the YAML file.
- If the `templates` section is missing or empty, no templates are seeded (this is not
  an error -- projects can exist without default templates).
- If a YAML entry is missing a `name`, or the `name` is not a string, the seeder skips
  that entry with a warning log (does not fail the project creation transaction).
- If a YAML entry has a duplicate `name` within the same config file, only the first
  occurrence is inserted; duplicates are skipped with a warning log.
- If a YAML entry has a `description` that is not a string, it is treated as `null`.
- If a YAML entry has a `description` exceeding 10000 characters, it is truncated to 10000
  characters using character-count truncation (not byte truncation), ensuring multi-byte
  UTF-8 characters are preserved intact. A warning is logged.

---

## Sequence

### Create Template Flow

1. Client sends `POST /api/v1/projects/{projectId}/templates` with
   `{"name": "Login Test", "description": "1. Navigate to login page\n..."}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. HTTP handler validates the project exists and is not soft-deleted
   (call `ProjectRepository::find_by_id`). If not found → `404`.
4. Handler deserializes and validates the request body (name required, lengths).
5. Handler calls `TemplateService::create_template(project_id, cmd)`.
6. `TemplateService` checks the user has `template:create` system permission
   (via `AuthorizationService`).
7. `TemplateService` checks the user is an Owner or Editor of the project
   (via `ProjectMemberRepository`). If not, and not System Admin → `403`.
8. `TemplateService` begins a database transaction.
9. Within the transaction:
   a. Check for duplicate name: `TemplateRepository::find_by_name_in_project(project_id, name)`.
      If a non-deleted duplicate exists → roll back and return `409 Conflict`.
   b. Construct a `TestCaseTemplate` entity and call `TemplateRepository::save(template)`.
10. Transaction commits.
11. Handler constructs the `Location` header from the new template ID and returns
    `201 Created`.

### Soft-Delete Template Flow

1. Client sends `DELETE /api/v1/projects/{projectId}/templates/{id}` with session
   cookie.
2. `AuthMiddleware` validates the session and attaches user context.
3. Handler validates the project exists and is not soft-deleted.
4. Handler calls `TemplateService::delete_template(project_id, template_id, user_id)`.
5. `TemplateService` checks `template:delete` system permission.
6. `TemplateService` checks the user is Owner or Editor of the project.
7. `TemplateService` begins a database transaction.
8. Within the transaction:
   a. `SELECT ... FOR UPDATE` on the template row to lock it and verify it exists,
      belongs to the project, and is not already soft-deleted.
      If not found or soft-deleted → roll back and return `404 Not Found`.
   b. Call `TemplateRepository::soft_delete(template_id, user_id)` which sets
      `deleted_at = NOW()`, `deleted_by = current_user_id`.
   c. No referential-integrity check is needed (templates have no FK references).
9. Transaction commits.
10. Handler returns `204 No Content`.

### Update Template Flow

1. Client sends `PATCH /api/v1/projects/{projectId}/templates/{id}` with
   `{"name": "New Name", "description": "..."}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context.
3. Handler validates the project exists and is not soft-deleted.
4. Handler deserializes and validates the request body (at least one field, field lengths,
   no unrecognised fields).
5. Handler calls `TemplateService::update_template(project_id, template_id, cmd)`.
6. `TemplateService` checks `template:update` system permission.
7. `TemplateService` checks the user is Owner or Editor of the project.
8. `TemplateService` begins a database transaction.
9. Within the transaction:
   a. `SELECT ... FOR UPDATE` on the template row to lock it and verify it exists,
      belongs to the project, and is not soft-deleted.
      If not found or soft-deleted → roll back and return `404 Not Found`.
   b. If `name` is provided and differs from the current name (after trimming):
      check for duplicate name: `SELECT 1 FROM test_case_templates WHERE project_id = $1
      AND LOWER(name) = LOWER($2) AND deleted_at IS NULL AND id != $3`.
      If a conflict exists → roll back and return `409 Conflict`.
   c. Apply the updates via `TemplateRepository::update(template)`.
10. Transaction commits.
11. Handler returns `200 OK` with the updated template representation.

### List Templates Flow

1. Client sends `GET /api/v1/projects/{projectId}/templates?page=1&limit=25&search=login&sort=name`
   with session cookie.
2. `AuthMiddleware` validates the session.
3. Handler validates the project exists and is not soft-deleted.
4. Handler calls `TemplateService::list_templates(project_id, query)`.
5. `TemplateService` checks `template:read_list` system permission.
6. `TemplateService` checks the user is a member of the project (any role) or System Admin.
7. `TemplateService` calls `TemplateRepository::find_by_project(project_id, page, limit, search, sort)`.
8. Repository executes a parameterized query with `WHERE project_id = $1 AND deleted_at IS NULL`,
   `ILIKE` filters on name/description if `search` is provided, and `ORDER BY` based on `sort`.
9. Repository returns the paginated results and total count.
10. `TemplateService` returns the response DTO with `data` and `meta`.
11. Handler returns `200 OK`.

### Select Templates Flow

1. Client sends `GET /api/v1/projects/{projectId}/templates/select` with session cookie.
2-3. Same as List: validate session and project.
4. Handler calls `TemplateService::select_templates(project_id)`.
5. `TemplateService` checks `template:select` system permission.
6. `TemplateService` checks the user is a project member (any role) or System Admin.
7. `TemplateService` calls `TemplateRepository::find_all_active_by_project(project_id)`
   which selects `id`, `name`, and `description`, ordered by `LOWER(name)`.
8. Handler returns `200 OK` with the flat array.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestCaseTemplate` | Domain (1) | Entity: `id`, `project_id`, `name`, `description`, audit fields. Factory method `create(project_id, name, description, created_by)` performs domain validation (name not empty, description length). No ORM or framework imports. |
| `TemplateService` | Application (2) | Orchestrates all template use cases: `create_template`, `list_templates`, `get_template`, `update_template`, `delete_template`, `select_templates`. Each method checks the required system permission and project membership, then delegates to the repository. |
| `TemplateRepository` | Application (2) | Interface (port): `find_by_id(project_id, template_id)`, `find_by_name_in_project(project_id, name)`, `find_by_project(project_id, page, limit, search, sort)`, `save(template)`, `update(template)`, `soft_delete(template_id, user_id)`, `find_all_active_by_project(project_id)`. |
| `TemplateHandler` | Adapters (3) | HTTP handler with six methods (`list`, `create`, `get`, `update`, `delete`, `select`). Deserializes requests, calls `TemplateService`, serializes responses. The select handler is registered before the `/{id}` handler. |
| `SqlTemplateRepository` | Infrastructure (4) | Implements `TemplateRepository` using PostgreSQL. All queries include `WHERE deleted_at IS NULL` for active record filtering. Uses parameterized queries exclusively. |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `ConfigFileSeeder` (Infrastructure, from `project-crud`) | Add `seed_templates(project_id, created_by, tx)` method that reads the `templates` section from the YAML config and inserts rows into `TEST_CASE_TEMPLATES`. |
| `AuthorizationService` (Application) | Add permission codes `template:create`, `template:read`, `template:read_list`, `template:update`, `template:delete`, `template:select` to the permission registry. Add role-based checks: Owner and Editor can create/update/delete; all members can read/read_list/select. |
| Seed migration (Infrastructure, from `iam-permissions`) | Add 6 new permission rows for `template:*` codes. |
| HTTP router registration | Register six new routes under `/api/v1/projects/{projectId}/templates/` -- all require session auth. Route order: `/select` before `/{id}`. |

---

## Route Registration

```text
# Order matters: /select must be registered before /{id}
GET    /api/v1/projects/{projectId}/templates           → list
POST   /api/v1/projects/{projectId}/templates           → create
GET    /api/v1/projects/{projectId}/templates/select    → select   (static path)
GET    /api/v1/projects/{projectId}/templates/{id}      → get      (dynamic path)
PATCH  /api/v1/projects/{projectId}/templates/{id}      → update
DELETE /api/v1/projects/{projectId}/templates/{id}      → delete
```

All routes require:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- System permission check (respective `template:*` code)
- Project membership check (Owner/Editor for mutations; any role for reads)

---

## New Permission Codes

These must be added to the `PERMISSIONS` table seed data. Use `INSERT ... ON CONFLICT (code)
DO NOTHING` and let the database assign IDs -- do not hardcode numeric IDs in the migration:

| code | name |
|------|------|
| `template:create` | Create Template |
| `template:read` | Read Template |
| `template:read_list` | Read Template List |
| `template:update` | Update Template |
| `template:delete` | Delete Template |
| `template:select` | Select Template |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks required system permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks required project role | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before any template operation |
| Template not found, soft-deleted, or wrong project | `404` | `NOT_FOUND` | INFO | Same message regardless of cause |
| Duplicate template name in project | `409` | `DUPLICATE_TEMPLATE_NAME` | INFO | Case-insensitive; only among non-deleted rows |
| Validation error (bad request body) | `422` | `VALIDATION_ERROR` | INFO | Includes field-level details; unrecognised fields rejected in strict mode |
| Invalid pagination or sort parameter | `422` | `VALIDATION_ERROR` | INFO | page < 1 or page > 1000, limit < 1 or limit > 100, invalid sort value, search > 255 chars |
| Whitespace-only name | `422` | `VALIDATION_ERROR` | INFO | Name must contain at least one non-whitespace character |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |

**Anti-patterns explicitly avoided:**

- **Do not reveal** whether a `404` is caused by a non-existent project, non-existent
  template, soft-deleted template, or wrong-project template (same message for all).
- **Do not return `400`** for business logic errors like duplicate names --
  use `409 Conflict` as specified.
- **Do not hard-delete** any record -- all deletions are soft-deletes.
- **Do not allow UPDATE on soft-deleted rows** -- the repository layer checks
  `deleted_at IS NULL` before applying mutations.
- **Do not omit `deleted_by`** -- every soft-delete sets both `deleted_at` and
  `deleted_by`.
- **Do not check for referencing test cases on delete** -- templates have no FK
  references. Unlike categories, there is no `TEMPLATE_IN_USE` error.
- **Do not use `GET` with a body** -- all state changes use `POST`, `PATCH`, or `DELETE`.
- **Do not expose `deleted_at` or `deleted_by`** in API responses.
