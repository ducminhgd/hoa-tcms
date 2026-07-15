# Design: Automation Script Fields

## Architecture

This feature extends the existing Test Case CRUD with two new metadata columns. No
new layers, endpoints, or permission codes are introduced. The change is purely
additive: new columns on the `TEST_CASES` table, new fields on existing DTOs and
entities, and new validation rules in existing handlers and services.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                           │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handlers (modified -- same 6 endpoints):                         │   │
│  │  - POST / test-cases       -- accepts script, script_args             │   │
│  │  - PATCH / test-cases/{id} -- accepts script, script_args             │   │
│  │  - GET  / test-cases       -- includes script, script_args in list    │   │
│  │  - GET  / test-cases/{id}  -- includes script, script_args in detail  │   │
│  │  - DELETE, /select         -- no change                                │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  TestCaseService (modified):              │  │  - TestCase entity       │  │
│  │  - create_test_case (accepts script)      │  │    + script: Option<Str> │  │
│  │  - update_test_case (accepts script)      │  │    + script_args: ...    │  │
│  │  - list_test_cases (returns script)       │  └──────────────────────────┘  │
│  │  - get_test_case (returns script)         │                                │
│  │                                           │                                │
│  │  DTOs (modified):                         │                                │
│  │  - CreateTestCaseCommand +script fields   │                                │
│  │  - UpdateTestCaseCommand +script fields   │                                │
│  │  - TestCaseListItem +script fields        │                                │
│  │  - TestCaseDetailResponse +script fields  │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestCaseRepository (modified): read/write script columns         │   │
│  │  - Migration: ALTER TABLE test_cases ADD COLUMN script, script_args    │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. Script fields follow the exact same path through the system as `description` and
   `notes`:
   - Input sanitization at the HTTP handler layer (strip HTML tags).
   - Validation at the domain entity layer (max length).
   - Storage via the repository layer (parameterized SQL).
   - Output encoding at the presentation layer.
2. No new authorization decisions: the existing `test_case:create` and
   `test_case:update` permissions cover script fields.
3. The Contributor ownership check on update applies to script fields automatically
   (it checks `created_by` on the test case, not individual fields).
4. `script` and `script_args` are included in list responses (unlike `description`
   and `notes`) because automation systems poll the list endpoint to discover
   scripted test cases.

---

## API Contract

### Modifications to Existing Endpoints

The script fields are added to the request and response bodies of four existing
endpoints. No new endpoints are introduced. The response format and error handling
for the DELETE and select endpoints are unchanged.

---

### POST `/api/v1/projects/{projectId}/test-cases`

**Request Body** (adds two optional fields):

```json
{
  "summary": "User can log in with valid credentials",
  "category_id": 3,
  "priority_id": 1,
  "automated": true,
  "description": "Given a registered user with valid credentials...",
  "notes": "Critical path test.",
  "script": "tests/login/test_login.py",
  "script_args": "--browser chrome --headless"
}
```

| Field | Type | Required | Default | Constraints |
|-------|------|----------|---------|-------------|
| `script` | string | No | `null` | Max 2000 characters; sanitized on input (HTML stripped). Empty string `""` stored as-is. Explicit `null` stores `NULL`. Omitting defaults to `null`. |
| `script_args` | string | No | `null` | Max 5000 characters; sanitized on input (HTML stripped). Empty string `""` stored as-is. Explicit `null` stores `NULL`. Omitting defaults to `null`. |

All other fields unchanged from the test-case-crud spec.

**Success Response:** `201 Created`

```json
{
  "data": {
    "id": 128,
    "summary": "User can log in with valid credentials",
    "description": "Given a registered user with valid credentials...",
    "notes": "Critical path test.",
    "automated": true,
    "project_id": 42,
    "category_id": 3,
    "category_name": "Login",
    "priority_id": 1,
    "priority_name": "Critical",
    "script": "tests/login/test_login.py",
    "script_args": "--browser chrome --headless",
    "created_by": 15,
    "created_at": "2026-07-14T10:00:00Z",
    "updated_by": 15,
    "updated_at": "2026-07-14T10:00:00Z"
  }
}
```

**Additional Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `422` | `VALIDATION_ERROR` | `script` exceeds 2000 characters |
| `422` | `VALIDATION_ERROR` | `script_args` exceeds 5000 characters |

All other error responses unchanged from the test-case-crud spec.

---

### PATCH `/api/v1/projects/{projectId}/test-cases/{id}`

**Request Body** (adds two optional fields):

```json
{
  "script": "tests/login/test_login_v2.py",
  "script_args": null
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `script` | string | No | Max 2000 characters; sanitized on input (HTML stripped). Omitting preserves current. `null` clears. `""` stores empty string. |
| `script_args` | string | No | Max 5000 characters; sanitized on input (HTML stripped). Omitting preserves current. `null` clears. `""` stores empty string. |

All other fields unchanged from the test-case-crud spec.

**Success Response:** `200 OK`

Response body is the updated test case representation including `script` and
`script_args` (same shape as the POST success response).

**Additional Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `422` | `VALIDATION_ERROR` | `script` exceeds 2000 characters |
| `422` | `VALIDATION_ERROR` | `script_args` exceeds 5000 characters |

All other error responses unchanged from the test-case-crud spec.

**Notes:**
- Script fields use the same PATCH semantics as `description` and `notes`:
  omitted preserves current value; `null` clears to `NULL`; `""` stores empty string.
- The nested Option pattern applies: `None` (field absent, preserve),
  `Some(None)` (explicit null, clear), `Some(Some(value))` (set to value).

---

### GET `/api/v1/projects/{projectId}/test-cases`

**Success Response:** `200 OK`

Each item in the `data` array now includes `script` and `script_args`:

```json
{
  "data": [
    {
      "id": 42,
      "summary": "User can log in with valid credentials",
      "category_id": 3,
      "priority_id": 1,
      "automated": true,
      "script": "tests/login/test_login.py",
      "script_args": "--browser chrome --headless",
      "created_by": 15,
      "created_at": "2026-07-14T10:00:00Z",
      "updated_at": "2026-07-14T10:00:00Z"
    },
    {
      "id": 41,
      "summary": "Password reset email is sent within 60 seconds",
      "category_id": null,
      "priority_id": 2,
      "automated": false,
      "script": null,
      "script_args": null,
      "created_by": 15,
      "created_at": "2026-07-14T09:30:00Z",
      "updated_at": "2026-07-15T08:00:00Z"
    }
  ],
  "meta": {
    "total": 150,
    "page": 1,
    "limit": 25
  }
}
```

**Rationale for including script fields in list:**
The primary consumer of script fields is an external automation system that needs to
discover which test cases have automation configured. Requiring a separate detail
request per test case would be inefficient (N+1 problem). Script fields are short
reference strings (max 2000 + 5000 chars) and do not significantly inflate list
response sizes. This is a deliberate deviation from the pattern used for
`description` and `notes` (excluded from list for size), justified by the different
access pattern.

**Error Responses:** Unchanged from the test-case-crud spec.

---

### GET `/api/v1/projects/{projectId}/test-cases/{id}`

**Success Response:** `200 OK`

Response body now includes `script` and `script_args` (same shape as the POST success
response above).

**Error Responses:** Unchanged from the test-case-crud spec.

---

### DELETE and SELECT Endpoints

No changes. The DELETE endpoint returns `204 No Content` with no body. The SELECT
endpoint continues to return only `id` and `summary`.

---

## Data Model

### Migration: Add Columns to TEST_CASES

**Forward migration:**

```sql
-- Add script column: path or command to an external automation script
ALTER TABLE test_cases
  ADD COLUMN script TEXT;

-- Add script_args column: arguments/parameters for the automation script
ALTER TABLE test_cases
  ADD COLUMN script_args TEXT;

-- Length guard for script (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_cases
  ADD CONSTRAINT chk_test_cases_script
  CHECK (script IS NULL OR char_length(script) <= 2000);

-- Length guard for script_args (defence in depth; primary enforcement is at app layer)
ALTER TABLE test_cases
  ADD CONSTRAINT chk_test_cases_script_args
  CHECK (script_args IS NULL OR char_length(script_args) <= 5000);
```

**Rollback migration:**

```sql
ALTER TABLE test_cases DROP CONSTRAINT IF EXISTS chk_test_cases_script_args;
ALTER TABLE test_cases DROP CONSTRAINT IF EXISTS chk_test_cases_script;
ALTER TABLE test_cases DROP COLUMN IF EXISTS script_args;
ALTER TABLE test_cases DROP COLUMN IF EXISTS script;
```

### Updated Table: TEST_CASES

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `script` | `TEXT` | | Nullable; max 2000 chars enforced at app layer and by `CHECK` constraint. Stores a path or command that external automation systems use to locate and execute the test script. |
| `script_args` | `TEXT` | | Nullable; max 5000 chars enforced at app layer and by `CHECK` constraint. Stores arguments or parameters passed to the automation script at runtime. |

All other columns unchanged from the test-case-crud spec.

**Design notes:**

- Both columns are `TEXT` (unbounded native PostgreSQL type) with `CHECK` constraints
  that mirror the application-layer validation. This follows the same defence-in-depth
  pattern used for `description` and `notes`.
- Both columns are nullable, defaulting to `NULL`. A `NULL` value means "no automation
  configured." An empty string `""` is distinct and means "automation configured but
  with an empty script path/args" (though this is unlikely to be useful in practice).
- No separate indexes are needed for `script` or `script_args`. The primary access
  pattern is reading these fields as part of a test case row fetched by `id` or
  listing by `project_id` (already indexed). Filtering or searching by script values
  is not a requirement in Phase 2.
- No triggers are modified. The existing `BEFORE UPDATE` trigger
  `trg_test_cases_updated_at` already sets `updated_at = NOW()` on every `UPDATE`
  regardless of which columns changed, so `script` and `script_args` updates
  automatically update the timestamp.

---

## Sequence

### Create Test Case with Script Fields

Steps 1-6 are identical to the existing create flow (validate session, project,
permission, project role, begin transaction, validate summary/category/priority).

Additional steps within the transaction:
- 8f. Construct a `TestCase` entity including `script` and `script_args` from the
      command DTO. The entity constructor validates max lengths.
- 8g. Call `TestCaseRepository::save(test_case)`. The INSERT statement includes
      `script` and `script_args` columns.

All other steps (commit, Location header, 201 response) are unchanged.

### Update Test Case with Script Fields

Steps 1-9 are identical to the existing update flow.

Additional steps within the transaction:
- 10e. If `script` is provided: apply the new value (or clear to `NULL` or set to
       `""` following nested Option semantics). If omitted: preserve current value.
- 10f. If `script_args` is provided: same semantics as `script`.
- 10g. Apply all changes via `test_case.apply_update(cmd)` and save.

### List and Detail Flows

- **List:** The `SqlTestCaseRepository::find_by_project` query selects `script` and
  `script_args` columns in addition to the existing list columns. No additional
  JOINs are needed.
- **Detail:** The `SqlTestCaseRepository::find_by_id` query selects `script` and
  `script_args` columns. No additional JOINs are needed.
- **Select:** Unchanged (only `id` and `summary` are selected).

---

## Components

### New Components

None. This feature introduces no new components -- it extends existing ones.

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `TestCase` (Domain, Layer 1) | Add `script: Option<String>` and `script_args: Option<String>` fields. The factory method `create()` accepts these optional fields and validates max lengths (2000 and 5000 respectively). The `apply_update(cmd)` method applies changes following nested Option semantics. |
| `CreateTestCaseCommand` (Application DTO) | Add `script: Option<Option<String>>` and `script_args: Option<Option<String>>` fields. For create, these are single-level Options (provided or not) since there is no "preserve current" state. |
| `UpdateTestCaseCommand` (Application DTO) | Add `script: Option<Option<String>>` and `script_args: Option<Option<String>>` fields using the nested Option pattern: `None` = not provided (preserve), `Some(None)` = explicitly set to null, `Some(Some(value))` = set to value. |
| `TestCaseListItem` (Application DTO) | Add `script: Option<String>` and `script_args: Option<String>` fields for inclusion in list responses. |
| `TestCaseDetailResponse` (Application DTO) | Add `script: Option<String>` and `script_args: Option<String>` fields for inclusion in detail and create/update responses. |
| `TestCaseRepository` (Application, Layer 2) | The `save()` and `update()` methods now accept `script` and `script_args` values. The `find_by_project()` query selects these columns. The `find_by_id()` query selects these columns. No new interface methods are added. |
| `TestCaseService` (Application, Layer 2) | `create_test_case()` passes `script` and `script_args` from the command to the entity constructor. `update_test_case()` applies script field changes via `apply_update()`. List and detail methods include script fields in response DTOs. No new use case methods. |
| `TestCaseHandler` (Adapters, Layer 3) | Create and update handlers accept `script` and `script_args` in request bodies, apply HTML sanitization, and enforce strict mode (reject unrecognised fields -- `script` and `script_args` are recognized). List and detail handlers include the fields in serialized responses. |
| `SqlTestCaseRepository` (Infrastructure, Layer 4) | INSERT and UPDATE statements include `script` and `script_args` columns. SELECT statements for list and detail include these columns. No new query methods. |
| Database migration | A new migration file adds `script` and `script_args` columns with CHECK constraints to the `TEST_CASES` table. |

---

## New Permission Codes

None. The existing `test_case:create`, `test_case:update`, `test_case:read`, and
`test_case:read_list` permission codes cover all operations on script fields.

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| `script` exceeds 2000 characters | `422` | `VALIDATION_ERROR` | INFO | Checked at the domain entity and handler layers |
| `script_args` exceeds 5000 characters | `422` | `VALIDATION_ERROR` | INFO | Checked at the domain entity and handler layers |

All other error cases unchanged from the test-case-crud spec. Script fields do not
introduce new categories of error -- they are plain text metadata fields subject to
the same validation patterns as `description` and `notes`.

**Anti-patterns explicitly avoided:**

- **Do not validate script existence** -- the TCMS does not check whether `script`
  points to a real file or command.
- **Do not execute scripts** -- the TCMS never runs the contents of `script` or
  `script_args`.
- **Do not parse or interpret script values** -- the fields are opaque strings.
- **Do not create new endpoints** -- all changes are additive to existing endpoints.
- **Do not create new permission codes** -- existing `test_case:*` permissions
  suffice.
- **Do not add indexes on script columns** -- no filtering or searching by script
  values is required yet; indexes can be added later if needed without a migration
  breaking change.

---

## Route Registration

No new routes. The existing six test case routes serve script fields:

```text
GET    /api/v1/projects/{projectId}/test-cases           -- list (now includes script fields)
POST   /api/v1/projects/{projectId}/test-cases           -- create (now accepts script fields)
GET    /api/v1/projects/{projectId}/test-cases/select    -- select (unchanged)
GET    /api/v1/projects/{projectId}/test-cases/{id}      -- get (now includes script fields)
PATCH  /api/v1/projects/{projectId}/test-cases/{id}      -- update (now accepts script fields)
DELETE /api/v1/projects/{projectId}/test-cases/{id}      -- delete (unchanged)
```
