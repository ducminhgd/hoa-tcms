# Design: Test Case Template

## Architecture

The Test Case Template feature extends the existing Test Case CRUD feature. It adds a
`template_id` parameter to the existing create and update endpoints and introduces a
dependency from `TestCaseService` on `TemplateRepository` for template fetching and
validation. No new endpoints, no new tables, and no new permission codes are introduced.

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                              │
│  ┌───────────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers (modified):                                                 │   │
│  │  - create_test_case  POST   /api/v1/projects/{pid}/test-cases             │   │
│  │  - update_test_case  PATCH  /api/v1/projects/{pid}/test-cases/{id}        │   │
│  │                       ^                                                    │   │
│  │                       │ now accept optional template_id in request body    │   │
│  │                                                                             │   │
│  │  Template listing (reused, no changes):                                     │   │
│  │  - select_templates  GET    /api/v1/projects/{pid}/templates/select        │   │
│  └──────────────────────┬────────────────────────────────────────────────────┘   │
│                         │ calls                                                   │
│                         ▼                                                         │
│  Application (Layer 2)                                                            │
│  ┌──────────────────────────────────────────────────────────────────────────┐   │
│  │  TestCaseService (modified):                                               │   │
│  │  - create_test_case  -- now accepts optional template_id                   │   │
│  │  - update_test_case  -- now accepts optional template_id                   │   │
│  │                                                                             │   │
│  │  New dependency:                                                            │   │
│  │  - TemplateRepository (port, from metadata-templates)                       │   │
│  └──────────┬───────────────────────────────────────────────────────────────┘   │
│             │ delegates to                                                        │
│             ▼                                                                     │
│  Infrastructure (Layer 4)                                                         │
│  ┌──────────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestCaseRepository   (no changes)                                    │   │
│  │  - SqlTemplateRepository   (existing; used for template fetch + validate)  │   │
│  └──────────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. The test case create/update handlers accept an optional `template_id` integer field
   in the request body (alongside existing fields).
2. The handler passes `template_id` through the command DTO to `TestCaseService`.
3. If `template_id` is provided, `TestCaseService` calls
   `TemplateRepository::find_by_id(project_id, template_id)` within the same DB
   transaction as the test case insert/update.
4. If the template exists, is non-deleted, and belongs to the project, its
   `description` is used as the default for the test case's `description` field.
   An explicit `description` in the request always wins.
5. If the template is not found, soft-deleted, or belongs to a different project,
   the transaction is rolled back and `422 Unprocessable Entity` with
   `INVALID_TEMPLATE` is returned.
6. Template listing for the UI dropdown reuses the existing
   `GET /api/v1/projects/{projectId}/templates/select` endpoint (no changes).

---

## API Contract Changes

### POST `/api/v1/projects/{projectId}/test-cases` -- Extended

The existing create endpoint accepts one new optional field.

**Request Body** (all existing fields plus):

| Field | Type | Required | Default | Constraints |
|-------|------|----------|---------|-------------|
| `template_id` | integer | No | (none) | Must reference a non-deleted template in the same project. If provided but invalid, returns `422` with `INVALID_TEMPLATE`. If omitted, no template pre-fill occurs (backward compatible). |

Example request with template:

```json
{
  "summary": "User can log in with valid credentials",
  "template_id": 5,
  "category_id": 3,
  "priority_id": 1,
  "automated": true
}
```

In this example, the test case's `description` is set to the template's `description`
because no explicit `description` was provided.

Example request with template and explicit description override:

```json
{
  "summary": "User can log in with valid credentials",
  "template_id": 5,
  "description": "Custom description that overrides the template",
  "category_id": 3
}
```

In this example, the explicit `description` takes precedence over the template.

**Pre-fill resolution logic:**

| `template_id` | `description` in request | Resulting `description` |
|---------------|--------------------------|------------------------|
| Provided, valid | Provided (non-null) | Explicit `description` value |
| Provided, valid | Provided (`null`) | `null` (`null` explicitly overrides template) |
| Provided, valid | Omitted | Template's `description` (may be `null`) |
| Provided, invalid | (any) | `422 INVALID_TEMPLATE` |
| Omitted | (any) | Existing behaviour (no pre-fill) |

**Validation order:** Input sanitization is applied to the template's `description`
before it is used as the pre-fill value. The sanitized value then goes through the
same validation as any other `description` (max 10000 chars). If the template's
sanitized `description` exceeds 10000 characters, `422 VALIDATION_ERROR` is returned
with a field-level detail for `description` (referencing the template as the source).

**Error Responses** (additions to existing table):

| Status | Code | Condition |
|--------|------|-----------|
| `422` | `INVALID_TEMPLATE` | `template_id` does not exist, is soft-deleted, or belongs to a different project |
| `422` | `VALIDATION_ERROR` | `template_id` is not a valid integer (e.g., string, float, null) |

`422` response body for invalid template:

```json
{
  "error": {
    "code": "INVALID_TEMPLATE",
    "message": "The specified template does not exist or does not belong to this project.",
    "details": [
      { "field": "template_id", "message": "Template must exist and belong to the same project" }
    ]
  }
}
```

---

### PATCH `/api/v1/projects/{projectId}/test-cases/{id}` -- Extended

The existing update endpoint accepts one new optional field.

**Request Body** (all existing fields plus):

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `template_id` | integer | No | Must reference a non-deleted template in the same project. If provided but invalid, returns `422` with `INVALID_TEMPLATE`. If omitted, no template pre-fill occurs (backward compatible). |

Example request:

```json
{
  "template_id": 3
}
```

This replaces the test case's `description` with template 3's `description`, since no
explicit `description` was provided.

**Pre-fill resolution logic for update:**

| `template_id` | `description` in request | Resulting `description` |
|---------------|--------------------------|--------------------------|
| Provided, valid | Provided (non-null) | Explicit `description` value |
| Provided, valid | Provided (`null`) | `null` (clears the field) |
| Provided, valid | Omitted | Template's `description` (if non-null); if template's `description` is `null`, preserve current value |
| Provided, invalid | (any) | `422 INVALID_TEMPLATE` |
| Omitted | (any) | Existing behaviour (no pre-fill) |

Key difference from create: when `template_id` is provided, `description` is omitted,
and the template's `description` is `null`, the test case's current `description` is
preserved (not cleared). This prevents accidentally blanking a test case's description
by selecting a template that has no body content.

**Error Responses** (additions to existing table):

| Status | Code | Condition |
|--------|------|-----------|
| `422` | `INVALID_TEMPLATE` | `template_id` does not exist, is soft-deleted, or belongs to a different project |
| `422` | `VALIDATION_ERROR` | `template_id` is not a valid integer |

---

## Sequence

### Create Test Case with Template Flow

Steps 1-4 are identical to the existing create flow (validate session, project,
request body, permissions). The changes are in steps 5-8:

1. Client sends `POST /api/v1/projects/{projectId}/test-cases` with
   `{"summary": "...", "template_id": 5, "category_id": 3}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context.
3. Handler validates the project exists and is not soft-deleted.
4. Handler deserializes and validates the request body. If `template_id` is present
   but not a valid integer, return `422 VALIDATION_ERROR`.
5. Handler calls `TestCaseService::create_test_case(project_id, cmd, current_user_id)`.
6. `TestCaseService` checks `test_case:create` permission.
7. `TestCaseService` checks the user is Contributor/Editor/Owner of the project.
8. `TestCaseService` begins a database transaction.
9. Within the transaction:
   a. `SELECT ... FOR UPDATE` on the project row (existing).
   b. Check for duplicate summary (existing).
   c. Validate `category_id` FK (if provided, existing).
   d. Validate `priority_id` FK (if provided, existing).
   e. **NEW: If `template_id` is provided**: call
      `TemplateRepository::find_by_id_in_project(project_id, template_id)`.
      This query includes `WHERE deleted_at IS NULL`. If not found -> roll back
      and return `422 INVALID_TEMPLATE`.
   f. **NEW: Resolve description pre-fill**: determine the effective `description`
      using the resolution table in the API contract section above. If the resolved
      value is a template description, apply HTML sanitization (defence-in-depth).
   g. Construct `TestCase` entity with the resolved `description` and call
      `TestCaseRepository::save(test_case)` (existing).
10. Transaction commits.
11. Handler returns `201 Created` with `Location` header.

### Update Test Case with Template Flow

Steps 1-9 are identical to the existing update flow. Changes are in step 10:

1. Client sends `PATCH /api/v1/projects/{projectId}/test-cases/{id}` with
   `{"template_id": 3}` and session cookie.
2-7. (Existing) Validate session, project, test case existence, permissions,
     Contributor ownership check.
8. `TestCaseService` begins a database transaction.
9. Within the transaction:
   a. (Existing) Lock test case row, verify not soft-deleted.
   b. (Existing) If `summary` changing: check duplicate.
   c. (Existing) If `category_id` provided: validate FK.
   d. (Existing) If `priority_id` provided: validate FK.
   e. **NEW: If `template_id` is provided**: call
      `TemplateRepository::find_by_id_in_project(project_id, template_id)`.
      If not found -> roll back and return `422 INVALID_TEMPLATE`.
   f. **NEW: Resolve description pre-fill**: determine the effective `description`
      using the resolution table for update. If the resolved value is a template
      description, apply HTML sanitization. If the template description is `null`
      and no explicit `description` was provided, preserve the current value.
   g. (Existing) Apply updates and save via repository.
10. Transaction commits.
11. Handler returns `200 OK`.

---

## Components

### Modified Components

| Component | Layer | Change |
|-----------|-------|--------|
| `CreateTestCaseCommand` (DTO) | Application (2) | Add optional `template_id: Option<i64>` field. When `Some(id)`, the service pre-fills description from the template. When `None`, existing behaviour. |
| `UpdateTestCaseCommand` (DTO) | Application (2) | Add optional `template_id: Option<i64>` field. Same semantics as create. |
| `TestCaseService` | Application (2) | Accept `TemplateRepository` as a new constructor dependency. In `create_test_case` and `update_test_case`: if `template_id` is `Some`, fetch and validate the template, then resolve the effective description. Template validation occurs within the same DB transaction as the test case mutation. |
| `TestCaseHandler` | Adapters (3) | Accept `template_id` in request body deserialization for `create` and `update` handlers. Pass through to command DTOs. Add `INVALID_TEMPLATE` error mapping (domain error -> `422`). |
| DI container / wiring | Infrastructure (4) | Inject `SqlTemplateRepository` (or the `TemplateRepository` implementation) into `TestCaseService` constructor. |

### Template Repository Interface

The `TemplateRepository` interface (defined in metadata-templates) already provides
the method needed:

```
find_by_id(project_id: i64, template_id: i64) -> Option<TestCaseTemplate>
```

This method fetches a template by ID, scoped to a project, excluding soft-deleted
rows. The query is:

```sql
SELECT id, project_id, name, description, created_by, created_at, updated_by, updated_at
FROM test_case_templates
WHERE id = $1 AND project_id = $2 AND deleted_at IS NULL
```

If the template does not exist, is soft-deleted, or belongs to a different project,
the method returns `None`, and `TestCaseService` maps this to `INVALID_TEMPLATE`.

This is the only method from `TemplateRepository` that `TestCaseService` depends on.
A narrower interface (e.g., `TemplateResolver` with just `find_by_id_in_project`) may
be introduced for testability if the full `TemplateRepository` is too broad, but the
default implementation uses the existing `TemplateRepository`.

---

## Error Handling

Additions to the existing test-case-crud error handling table:

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| `template_id` is not a valid integer | `422` | `VALIDATION_ERROR` | INFO | Field-level detail for `template_id`. Includes cases where `template_id` is a string, float, boolean, array, object, or `null`. |
| Template not found, soft-deleted, or wrong project | `422` | `INVALID_TEMPLATE` | INFO | All three conditions produce the same error. Checked within the DB transaction. |
| Template description (after sanitization) exceeds 10000 chars | `422` | `VALIDATION_ERROR` | INFO | Field-level detail for `description`, noting the template source |

**Anti-patterns explicitly avoided:**

- **Do not return `404`** for an invalid `template_id` -- the template is not the
  primary resource; `422` correctly indicates a semantic problem with the request body.
- **Do not require `template:read` permission** -- `template_id` is an internal
  pre-fill parameter, not a direct template access. The caller's project membership
  (already verified by the test case endpoint) is sufficient.
- **Do not store `template_id` in `TEST_CASES`** -- the template content is copied,
  not referenced.
- **Do not silently ignore an invalid `template_id`** -- always return `422` to give
  the caller clear feedback.
- **Do not apply template pre-fill for `summary`, `notes`, or other fields** -- only
  `description` is affected.
