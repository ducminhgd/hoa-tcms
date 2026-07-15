# Design: Test Execution -- Selective Import

## Architecture

The Test Execution Import feature is an action endpoint under the execution resource. It
copies selected Test Cases from a Test Run into the execution's result set, creating a
snapshot that is independent of subsequent changes to the original Test Cases.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Adapters (Layer 3)                                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  HTTP Handler:                                                         │   │
│  │  - import_test_cases  POST /api/v1/projects/{pid}/test-runs/{rid}/    │   │
│  │                               executions/{eid}/import                  │   │
│  └──────────────────────┬────────────────────────────────────────────────┘   │
│                         │ calls                                               │
│                         ▼                                                     │
│  Application (Layer 2)                         ┌──────────────────────────┐  │
│  ┌──────────────────────────────────────────┐  │  Domain (Layer 1)        │  │
│  │  TestExecutionImportService:              │  │  - TestCaseResult        │  │
│  │  - import_test_cases                      │  │    (entity)              │  │
│  │                                           │  │  - ImportOutcome         │  │
│  │  Interfaces:                              │  │    (value object)        │  │
│  │  - TestCaseResultRepository (port)        │  └──────────────────────────┘  │
│  │  - TestRunRepository (port, read-only)    │                                │
│  │  - TestExecutionRepository (port,         │                                │
│  │    read-only)                             │                                │
│  └──────────┬───────────────────────────────┘                                │
│             │ delegates to                                                    │
│             ▼                                                                 │
│  Infrastructure (Layer 4)                                                     │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │  - SqlTestCaseResultRepository (implements TestCaseResultRepository)   │   │
│  │  - SqlTestRunRepository (implements TestRunRepository)                 │   │
│  │  - SqlTestExecutionRepository (implements TestExecutionRepository)     │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Flow summary:**

1. `AuthMiddleware` validates the session and attaches user context (user ID).
2. Handler deserializes the request body and validates the `test_case_ids` array.
3. Handler calls `TestExecutionImportService::import_test_cases(project_id, test_run_id,
   execution_id, test_case_ids, current_user_id)`.
4. Service checks `test_execution:import` system permission (via `AuthorizationService`).
5. Service checks the user is a Contributor, Editor, or Owner of the project (via
   `ProjectMemberRepository`). Viewer gets `403`.
6. Service begins a database transaction.
7. Within the transaction:
   a. Validates project exists and is not soft-deleted.
   b. Validates test run exists, is not soft-deleted, and belongs to the project.
   c. Validates execution exists, is not soft-deleted, and is linked to the given test run
      (execution's `test_run_id` matches the path param). Project scope is validated through
      the test run: `test_run.project_id` must match the path `projectId`.
   d. For each `test_case_id`:
      - Validates the test case exists, is not soft-deleted, and is a member of the
        test run (checked via a join or application-level lookup).
      - Checks if a `TEST_CASE_RESULTS` row already exists for `(execution_id,
        test_case_id)`. If yes, marks it as skipped.
      - If not present: reads the test case's `summary`, `description`, and resolves
        its priority name from `test_priorities`. Inserts a new `TEST_CASE_RESULTS` row
        with these snapshot values and `status = 'NOT_TESTED'`.
8. Transaction commits.
9. Handler returns `200 OK` with `imported` and `skipped` arrays.

### Security Requirements

**Authorization layering:** Two authorization gates apply:

1. System permission check (`test_execution:import`)
2. Project membership and role check (Contributor, Editor, or Owner for import; Viewer
   excluded)

Both gates must pass (or the caller must be a System Admin, who implicitly holds all
permissions and bypasses project membership checks).

**Input validation:** Every `test_case_id` is validated for existence, soft-delete status,
and membership in the linked test run. Invalid IDs are rejected with specific details in
the error response.

**Same-project enforcement:** The test run must belong to the project (via
`test_runs.project_id`), the execution must be linked to that test run (via
`execution.test_run_id`), and the test cases must be members of the test run. Project
scope is validated through the chain: execution -> test_run -> project.

**CSRF protection:** Same requirements as all state-changing endpoints (`SameSite=Lax`
cookies, `Content-Type: application/json` verification).

---

## API Contract

### Common Error Response Format

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

### POST `/api/v1/projects/{projectId}/test-runs/{testRunId}/executions/{executionId}/import`

Import selected Test Cases from the linked Test Run into the Execution as a snapshot.

**Required Permission:** `test_execution:import` AND project role Contributor, Editor,
or Owner

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `projectId` | integer | Project ID |
| `testRunId` | integer | Test Run ID (must be linked to the execution) |
| `executionId` | integer | Execution ID |

**Request Body:**

```json
{
  "test_case_ids": [42, 85, 128]
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `test_case_ids` | array of integers | Yes | Non-empty; each element must be a positive integer. Duplicates are tolerated (first occurrence processes; subsequent are silently skipped as already-present). |

**Success Response:** `200 OK`

```json
{
  "data": {
    "imported": [
      {
        "id": 501,
        "execution_id": 10,
        "test_case_id": 42,
        "summary": "User can log in with valid credentials",
        "description": "Given a registered user with valid credentials, when they submit the login form, then they are redirected to the dashboard.",
        "priority": "Critical",
        "status": "NOT_TESTED",
        "created_by": 15,
        "created_at": "2026-07-15T10:00:00Z",
        "updated_by": 15,
        "updated_at": "2026-07-15T10:00:00Z"
      }
    ],
    "skipped": [
      {
        "test_case_id": 85,
        "reason": "already_imported"
      }
    ]
  }
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `data.imported` | array | Test case results that were newly created in this request. Each entry contains the full `TEST_CASE_RESULTS` row (id, execution_id, test_case_id, summary, description, priority, status, created_by, created_at, updated_by, updated_at). `result_logs` is omitted from the response (always null on import). |
| `data.skipped` | array | Test case IDs that were skipped because they were already present in the execution. Each entry contains `test_case_id` and `reason` (`"already_imported"`). |

**Notes:**

- The import is a snapshot operation: `summary`, `description`, and `priority` are copied
  from the source Test Case at import time and are never updated by changes to the original
  Test Case.
- Priority is stored as the resolved name string (e.g., `"Critical"`), not the priority ID.
  This is intentional -- the snapshot captures the human-readable label at the time of
  import, and priority names are immutable within a project (priorities are seeded from
  config and cannot be renamed).
- `status` always starts as `NOT_TESTED` on import.
- `result_logs` starts as `NULL` on import.
- The order of `test_case_ids` in the request does not determine the order of insertion
  or the order of results in the response.
- The database `UNIQUE` constraint on `(execution_id, test_case_id)` provides
  defence-in-depth against duplicate inserts in race conditions.
- The import does not validate or care about the execution's current state (e.g., whether
  it is "in progress" or "completed") -- that is a business rule enforced by the
  execution state machine, which is a separate concern (in `test-execution-crud`).

**Error Responses:**

| Status | Code | Condition |
|--------|------|-----------|
| `401` | `NOT_AUTHENTICATED` | Session missing or invalid |
| `403` | `FORBIDDEN` | User lacks `test_execution:import` permission or is a Viewer of the project |
| `404` | `NOT_FOUND` | Project does not exist or is soft-deleted |
| `404` | `NOT_FOUND` | Test run does not exist, is soft-deleted, belongs to a different project, or is not linked to the execution |
| `404` | `NOT_FOUND` | Execution does not exist, is soft-deleted, or its parent test run belongs to a different project |
| `422` | `VALIDATION_ERROR` | `test_case_ids` is missing, empty, or contains non-integer values |
| `422` | `TEST_CASE_NOT_IN_TEST_RUN` | One or more `test_case_ids` do not exist, are soft-deleted, or are not members of the linked test run |

`422` response body for invalid test case IDs:

```json
{
  "error": {
    "code": "TEST_CASE_NOT_IN_TEST_RUN",
    "message": "One or more test cases are not part of this test run.",
    "details": [
      { "field": "test_case_ids[1]", "message": "Test case 999 does not exist", "test_case_id": 999 },
      { "field": "test_case_ids[3]", "message": "Test case 42 is not linked to test run 5", "test_case_id": 42 }
    ]
  }
}
```

---

## Data Model

### New Table: TEST_CASE_RESULTS

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | `BIGINT` | `PK`, `GENERATED ALWAYS AS IDENTITY` | Surrogate primary key |
| `execution_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_executions(id) ON DELETE RESTRICT` | Each result belongs to exactly one execution |
| `test_case_id` | `BIGINT` | `NOT NULL`, `REFERENCES test_cases(id) ON DELETE RESTRICT` | Reference to the original Test Case |
| `summary` | `VARCHAR(500)` | `NOT NULL` | Snapshotted from `test_cases.summary` at import time |
| `description` | `TEXT` | | Snapshotted from `test_cases.description` at import time; nullable |
| `priority` | `VARCHAR(50)` | | Snapshotted priority **name** resolved from `test_priorities.name` at import time; nullable. NOT a FK -- this is a denormalized string snapshot. |
| `status` | `VARCHAR(50)` | `NOT NULL`, `DEFAULT 'NOT_TESTED'` | Current result status. Valid values enforced by CHECK constraint: `NOT_TESTED`, `IN_PROGRESS`, `PASS`, `FAIL`, `WARNING`, `IGNORE` |
| `result_logs` | `TEXT` | | Free-form test execution logs; nullable; empty on import, populated later by result update |
| `tested_by` | `BIGINT` | `REFERENCES users(id) ON DELETE RESTRICT` | Set once on first transition away from NOT_TESTED; not modified thereafter; nullable |
| `created_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | User who imported the test case |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Immutable after insert |
| `updated_by` | `BIGINT` | `NOT NULL`, `REFERENCES users(id) ON DELETE RESTRICT` | Last user who modified the result (initially same as `created_by`) |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()` | Updated by `BEFORE UPDATE` trigger |

**Constraints:**

```sql
-- Status domain guard (defence in depth)
ALTER TABLE test_case_results
  ADD CONSTRAINT chk_test_case_results_status
  CHECK (status IN ('NOT_TESTED', 'IN_PROGRESS', 'PASS', 'FAIL', 'WARNING', 'IGNORE'));

-- One result per test case per execution (prevents duplicate imports)
CREATE UNIQUE INDEX uq_test_case_results_execution_test_case
  ON test_case_results (execution_id, test_case_id);

-- Foreign key indexes
CREATE INDEX idx_test_case_results_execution_id ON test_case_results (execution_id);
CREATE INDEX idx_test_case_results_test_case_id ON test_case_results (test_case_id);
CREATE INDEX idx_test_case_results_created_by ON test_case_results (created_by);
CREATE INDEX idx_test_case_results_updated_by ON test_case_results (updated_by);
CREATE INDEX idx_test_case_results_tested_by ON test_case_results (tested_by);

-- Composite index for listing results by execution (common query pattern)
CREATE INDEX idx_test_case_results_execution_status
  ON test_case_results (execution_id, status);
```

**Design notes:**

- **Unique constraint on `(execution_id, test_case_id)`** prevents importing the same
  test case twice into the same execution. This is the database-level defence-in-depth
  for idempotency.
- **`priority` as a denormalized string** rather than a FK to `test_priorities`. This is
  intentional snapshot semantics: the priority name at import time is frozen. If the
  priority is later renamed or deleted, execution results are unaffected. The priority
  name is resolved at import time via a JOIN on `test_priorities`.
- **`description` is nullable** because the source test case's `description` can be null.
- **`result_logs` starts as `NULL`** on import and is populated later by the
  `test-case-result-update` feature.
- **No soft-delete columns** on `TEST_CASE_RESULTS`. Results are hard-deleted (if ever)
  as part of execution management. The current design treats results as an integral part
  of the execution -- they live and die with the execution. If soft-delete is needed, it
  will be added in a future phase.
- **`ON DELETE RESTRICT` on user FKs** (`created_by`, `updated_by`) prevents deleting
  a user who has imported test case results. This preserves audit trail integrity.
- **`ON DELETE RESTRICT` on `execution_id` FK** prevents hard-deleting an execution that
  has results. Execution soft-delete is the supported path.
- **`ON DELETE RESTRICT` on `test_case_id` FK** prevents hard-deleting a test case that
  has been imported into any execution. Test case soft-delete does not cascade to results
  (results retain their snapshot).

### BEFORE UPDATE Trigger

```sql
CREATE OR REPLACE FUNCTION trg_test_case_results_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_test_case_results_updated_at
  BEFORE UPDATE ON test_case_results
  FOR EACH ROW
  EXECUTE FUNCTION trg_test_case_results_updated_at();
```

### Relationship to Other Tables

```
TEST_EXECUTIONS ──< TEST_CASE_RESULTS >── TEST_CASES
                         │
                    [snapshot of]
                         │
                 test_cases.summary
                 test_cases.description
                 test_priorities.name
```

- `TEST_CASE_RESULTS.execution_id` -> `TEST_EXECUTIONS.id`
- `TEST_CASE_RESULTS.test_case_id` -> `TEST_CASES.id`
- `TEST_CASE_RESULTS.priority` is a denormalized snapshot of `test_priorities.name`
  resolved at import time (no FK).

### Assumed Existing Tables

The import feature reads from (but does not modify) these tables:

**TEST_EXECUTIONS** (from `test-execution-crud`):

| Column | Notes |
|--------|-------|
| `id` | PK |
| `test_run_id` | `NOT NULL`, FK to test runs |
| `name` | Execution name |
| `deleted_at` | Soft-delete |

**Project scope resolution:** `TEST_EXECUTIONS` does NOT have a `project_id` column. The project
scope is resolved via the 4-hop chain: result -> execution -> test_run -> project (through
`test_runs.project_id`). When authorizing an import, validate that
`execution.test_run_id -> test_runs.id -> test_runs.project_id` matches the path parameter.

**TEST_RUNS** (from `test-run-crud`):

| Column | Notes |
|--------|-------|
| `id` | PK |
| `project_id` | `NOT NULL`, FK to projects; used for project scope validation |

**TEST_RUN_TEST_CASES** (join table from `test-run-cases`):

| Column | Notes |
|--------|-------|
| `test_run_id` | FK to test runs |
| `test_case_id` | FK to test cases |

---

## Sequence

### Import Test Cases Flow

1. Client sends `POST /api/v1/projects/{projectId}/test-runs/{testRunId}/executions/
   {executionId}/import` with `{"test_case_ids": [42, 85, 128]}` and session cookie.
2. `AuthMiddleware` validates the session and attaches user context (user ID).
3. Handler deserializes and validates the request body:
   - `test_case_ids` must be present, must be a non-empty array, each element must be a
     positive integer.
4. Handler calls `TestExecutionImportService::import_test_cases(project_id, test_run_id,
   execution_id, test_case_ids, current_user_id)`.
5. Service checks `test_execution:import` system permission (via `AuthorizationService`).
6. Service checks the user is a Contributor, Editor, or Owner of the project (via
   `ProjectMemberRepository`). Viewer or non-member and not System Admin -> `403`.
7. Service begins a database transaction.
8. Within the transaction:
   a. `SELECT ... FOR UPDATE` on the project row to verify it exists and is not
      soft-deleted. If not found or soft-deleted -> roll back and return `404`.
   b. Validate test run: `SELECT id FROM test_runs WHERE id = $1 AND project_id = $2
      AND deleted_at IS NULL`. If not found -> roll back and return `404`.
   c. Validate execution (project scope resolved via test_run chain):
      `SELECT te.id, te.test_run_id FROM test_executions te JOIN test_runs tr ON
      te.test_run_id = tr.id WHERE te.id = $1 AND tr.project_id = $2 AND
      te.deleted_at IS NULL AND tr.deleted_at IS NULL`. If not found -> roll back and
      return `404`. If found but `test_run_id != test_run_id` path param -> roll back and
      return `404` (execution is not linked to this test run).
   d. Validate test case IDs against the test run's members:
      `SELECT test_case_id FROM test_run_test_cases WHERE test_run_id = $1 AND
      test_case_id = ANY($2)`.
      Additionally, verify each test case exists and is not soft-deleted:
      `SELECT id FROM test_cases WHERE id = ANY($1) AND deleted_at IS NULL`.
      Combine both checks to produce a list of valid IDs and a list of invalid IDs. If any
      ID is invalid -> roll back and return `422` with `TEST_CASE_NOT_IN_TEST_RUN`,
      listing each invalid ID and the reason (not found, soft-deleted, or not in test run).
   e. Check which of the valid test case IDs already exist in `TEST_CASE_RESULTS`:
      `SELECT test_case_id FROM test_case_results WHERE execution_id = $1 AND
      test_case_id = ANY($2)`.
      These are the "already imported" IDs -> they go into the `skipped` array.
   f. For the remaining (not-yet-imported) test case IDs:
      - Fetch snapshot data:
        ```sql
        SELECT tc.id, tc.summary, tc.description, tp.name AS priority
        FROM test_cases tc
        LEFT JOIN test_priorities tp
          ON tc.priority_id = tp.id AND tp.deleted_at IS NULL
        WHERE tc.id = ANY($1) AND tc.deleted_at IS NULL
        ```
      - For each row from the result set, construct a `TestCaseResult` entity and insert
        via `TestCaseResultRepository::save_batch(results)`. Each row gets
        `status = 'NOT_TESTED'`, `result_logs = NULL`, and audit fields set to the
        current user and timestamp.
      - The `save_batch` method uses a bulk INSERT:
        ```sql
        INSERT INTO test_case_results
          (execution_id, test_case_id, summary, description, priority,
           status, created_by, updated_by)
        VALUES
          ($1, $2, $3, $4, $5, 'NOT_TESTED', $6, $6),
          ...
        RETURNING id, execution_id, test_case_id, summary, description,
                  priority, status, created_by, created_at, updated_by, updated_at
        ```
9. Transaction commits.
10. Handler constructs the response with `imported` and `skipped` arrays and returns
    `200 OK`.

---

## Components

### New Components

| Component | Layer | Role |
|-----------|-------|------|
| `TestCaseResult` | Domain (1) | Entity: `id`, `execution_id`, `test_case_id`, `summary`, `description`, `priority`, `status`, `result_logs`, audit fields. Factory method `create_snapshot(execution_id, test_case_id, summary, description, priority, created_by)` creates a result from snapshot data with `status = NOT_TESTED`. No ORM or framework imports. |
| `ImportOutcome` | Domain (1) | Value object: `imported: Vec<TestCaseResult>`, `skipped: Vec<SkippedTestCase>`. Returned by the import use case to communicate what was imported vs skipped. |
| `SkippedTestCase` | Domain (1) | Value object: `test_case_id: i64`, `reason: String` (always `"already_imported"` in this feature). |
| `ResultStatus` | Domain (1) | Enum: `NotTested`, `InProgress`, `Pass`, `Fail`, `Warning`, `Ignore`. Default is `NotTested`. Converts to/from the string representation for storage. |
| `TestExecutionImportService` | Application (2) | Orchestrates the import use case: `import_test_cases(project_id, test_run_id, execution_id, test_case_ids, current_user_id)`. Checks permission, membership, validates all entities, snapshots test case data, inserts results, returns `ImportOutcome`. |
| `TestCaseResultRepository` | Application (2) | Interface (port): `find_by_execution_and_test_case_ids(execution_id, test_case_ids) -> Vec<i64>` (returns the subset of `test_case_ids` that already exist as results for this execution), `save_batch(results: Vec<TestCaseResult>) -> Vec<TestCaseResult>` (bulk inserts with RETURNING, returns rows with generated IDs and timestamps). |
| `TestExecutionImportHandler` | Adapters (3) | HTTP handler with single method `import`. Deserializes request, validates `test_case_ids` array, calls `TestExecutionImportService`, serializes response with `imported` and `skipped` arrays. |
| `SqlTestCaseResultRepository` | Infrastructure (4) | Implements `TestCaseResultRepository` using PostgreSQL. Uses parameterized queries exclusively. The `save_batch` method builds a multi-row INSERT with `RETURNING` for efficiency. |

### Existing Components (Read-Only)

The import feature **reads from** but does **not modify** these components:

| Component | Usage |
|-----------|-------|
| `TestRunRepository` (Application port) | `find_by_id_and_project(id, project_id) -> Option<TestRun>` -- validates the test run exists, is not soft-deleted, and belongs to the project. Must also validate the test run is linked to the execution (can be done by checking `execution.test_run_id` against the path param). |
| `TestExecutionRepository` (Application port) | `find_by_id_and_project(id, project_id) -> Option<TestExecution>` -- validates the execution exists, is not soft-deleted, and its parent test run belongs to the project (JOIN through `test_runs.project_id`). Must return `test_run_id` for the link check. |
| `TestCaseRepository` (Application port) | Used indirectly via the snapshot query. The import service may call `find_by_ids_with_priority(ids) -> Vec<TestCaseSnapshot>` to batch-fetch summary, description, and resolved priority name. |
| `AuthorizationService` (Application) | `has_permission(user_id, "test_execution:import") -> bool` |
| `ProjectMemberRepository` (Application port) | `find_membership(user_id, project_id) -> Option<ProjectRole>` -- returns the user's project role (or None if not a member). |

### Modified Existing Components

| Component | Change |
|-----------|--------|
| `TestCaseRepository` (Application port) | Add method `find_snapshots_by_ids(ids: Vec<i64>) -> Vec<TestCaseSnapshot>` that returns `id`, `summary`, `description`, and resolved `priority_name` for a batch of test case IDs (excluding soft-deleted). This method performs a LEFT JOIN on `test_priorities` and filters `WHERE deleted_at IS NULL`. |
| `AuthorizationService` (Application) | Add permission code `test_execution:import` to the permission registry. |
| Seed migration (Infrastructure, from `iam-permissions`) | Add 1 new permission row for `test_execution:import`. |
| HTTP router registration | Register one new route under the execution resource path -- requires session auth. |

---

## Route Registration

```text
POST /api/v1/projects/{projectId}/test-runs/{testRunId}/executions/{executionId}/import
  -> import_test_cases
```

The route requires:
- Session-based authentication (`AuthMiddleware`)
- Project existence validation (project not soft-deleted)
- System permission check (`test_execution:import`)
- Project membership check: Contributor, Editor, or Owner (Viewer excluded)

---

## New Permission Code

This must be added to the `PERMISSIONS` table seed data. Use `INSERT ... ON CONFLICT (code)
DO NOTHING` and let the database assign IDs -- do not hardcode numeric IDs in the migration:

| code | name |
|------|------|
| `test_execution:import` | Import Test Cases into Execution |

---

## Error Handling

| Error Case | HTTP Status | Error Code | Log Level | Notes |
|------------|-------------|------------|-----------|-------|
| Missing or invalid session | `401` | `NOT_AUTHENTICATED` | INFO | From AuthMiddleware |
| User lacks required system permission | `403` | `FORBIDDEN` | INFO | Generic message |
| User lacks required project role | `403` | `FORBIDDEN` | INFO | Same generic message; indistinguishable from missing permission |
| Project not found or soft-deleted | `404` | `NOT_FOUND` | INFO | Checked before any import operation |
| Test run not found, soft-deleted, wrong project, or not linked to execution | `404` | `NOT_FOUND` | INFO | Same message for all cases |
| Execution not found, soft-deleted, or parent test run wrong project | `404` | `NOT_FOUND` | INFO | Same message |
| `test_case_ids` missing or empty | `422` | `VALIDATION_ERROR` | INFO | Array must be present and non-empty |
| `test_case_ids` contains non-integer values | `422` | `VALIDATION_ERROR` | INFO | Each element must be a positive integer |
| Test case ID not in test run / not found / soft-deleted | `422` | `TEST_CASE_NOT_IN_TEST_RUN` | INFO | Lists each invalid ID with specific reason |
| Rate limit exceeded | `429` | `RATE_LIMITED` | INFO | Returns `Retry-After` header |
| Database connection failure | `503` | `DATABASE_UNAVAILABLE` | ERROR | All operations fail |
| Unexpected internal failure | `500` | `INTERNAL_ERROR` | ERROR | Should not happen in normal operation |
| DB unique violation on `(execution_id, test_case_id)` (race condition) | `200` (skip) | -- | WARN | Caught from PostgreSQL error 23505; mapped to skip instead of error because another concurrent request just imported the same test case. This is a race between the SELECT check and INSERT; the unique index guarantees correctness. |

**Anti-patterns explicitly avoided:**

- **Do not** return `409 Conflict` for already-imported test cases -- silent skip with
  `200 OK` is the idempotency contract.
- **Do not** return `201 Created` -- this is not a resource creation endpoint. The imported
  results are a side effect of the action; the action itself returns `200 OK`.
- **Do not** allow importing test cases that are not members of the linked test run --
  validate every ID against the test-run-to-test-case join table.
- **Do not** import all test cases automatically -- the client must explicitly provide the
  list of IDs. A "select all" can be achieved by providing all test case IDs from the test
  run.
- **Do not** allow cross-project data injection -- every entity in the chain is validated
  to belong to the same project.
- **Do not** cascade updates -- snapshot fields (summary, description, priority) are frozen
  at import time and never automatically updated.
- **Do not** use `GET` for import -- always `POST` with a JSON body.
- **Do not** import into a non-existent or soft-deleted execution.
