# Tasks: Automation Script Fields

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections.

---

## Layer 1 -- Domain

- [ ] 1. Extend `TestCase` entity with `script` and `script_args` fields --
       `requirements.md#US-1`, `design.md#Components`
  - Add `script: Option<String>` and `script_args: Option<String>` fields to the
    entity struct.
  - Update the `TestCase::create()` factory method to accept these optional fields
    and validate max lengths:
    - `script` must be null or <= 2000 characters.
    - `script_args` must be null or <= 5000 characters.
    - Empty strings `""` are accepted and stored as-is (distinct from `null`).
  - Update the `apply_update(cmd: UpdateTestCaseCommand)` method to handle script
    fields using the nested Option pattern:
    - `None` = field not provided, preserve current value.
    - `Some(None)` = explicitly set to null, clear the field.
    - `Some(Some(value))` = set to value, validate max lengths.
  - No framework imports; pure domain logic.

---

## Layer 2 -- Application

- [ ] 2. Update command/query/response DTOs with script fields --
       `design.md#Components`, `design.md#API Contract`
  - `CreateTestCaseCommand`: add `script: Option<String>` and
    `script_args: Option<String>` fields. On create, these are flat Options
    (omitted defaults to `null`, provided value is used).
  - `UpdateTestCaseCommand`: add `script: Option<Option<String>>` and
    `script_args: Option<Option<String>>` fields using the nested Option pattern
    (same as `description` and `notes`).
  - `TestCaseListItem`: add `script: Option<String>` and
    `script_args: Option<String>` fields for list endpoint responses.
  - `TestCaseDetailResponse`: add `script: Option<String>` and
    `script_args: Option<String>` fields for detail and create/update responses.

- [ ] 3. Update `TestCaseRepository` interface to carry script fields --
       `design.md#Components`
  - The `save(test_case)` method: the `TestCase` entity now carries script fields;
    the repository INSERT must include them.
  - The `update(test_case)` method: the entity carries updated script fields; the
    repository UPDATE must include them.
  - The `find_by_project()` method: the SELECT query must include `script` and
    `script_args` columns in the result set.
  - The `find_by_id()` method: the SELECT query must include `script` and
    `script_args` columns.
  - No new interface methods are required. The existing method signatures accept
    entities and return DTOs that now carry script fields.

- [ ] 4. Update `TestCaseService` to handle script fields --
       `requirements.md#US-1` through `US-3`, `design.md#Sequence`
  - `create_test_case()`: extract `script` and `script_args` from the command DTO,
    pass to the entity constructor, save via repository.
  - `update_test_case()`: after fetching the existing test case, call
    `apply_update(cmd)` which now handles script fields alongside existing fields.
  - `list_test_cases()`: map repository results to `TestCaseListItem` including
    script fields.
  - `get_test_case()`: map repository result to `TestCaseDetailResponse` including
    script fields.
  - Permission checks and project membership checks are unchanged -- script fields
    are covered by the existing `test_case:create`, `test_case:update`,
    `test_case:read`, and `test_case:read_list` permissions.
  - The Contributor ownership check on update automatically applies (it checks
    `created_by` on the test case entity, not individual fields).

- [ ] 5. Write/update unit tests for `TestCaseService` with script fields --
       `requirements.md#US-1` through `US-3`
  - Update existing test cases to include `script` and `script_args` in assertions
    (existing tests should not break -- new fields should be `null` when not
    explicitly provided).
  - Happy path: create with script and script_args populated, verify they appear
    in the response.
  - Happy path: create with script only, script_args omitted (verify defaults to
    null).
  - Happy path: create with script_args only, script omitted (verify defaults to
    null).
  - Happy path: update script to a new value, verify response reflects change.
  - Happy path: update script_args to a new value, verify response reflects change.
  - Happy path: explicitly set script to `null` on update (clears field), verify
    response has `null`.
  - Happy path: explicitly set script to empty string `""` on update, verify stored
    as empty string.
  - Happy path: omit script on update, verify current value preserved.
  - Happy path: omit script_args on update, verify current value preserved.
  - Validation: script exceeding 2000 characters rejected on create.
  - Validation: script exceeding 2000 characters rejected on update.
  - Validation: script_args exceeding 5000 characters rejected on create.
  - Validation: script_args exceeding 5000 characters rejected on update.
  - Boundary: script exactly 2000 chars accepted.
  - Boundary: script_args exactly 5000 chars accepted.
  - Boundary: script at 2001 chars rejected.
  - Boundary: script_args at 5001 chars rejected.
  - Null handling: script `null` on create stored as NULL and returned as `null`.
  - Empty string handling: script `""` on create stored as empty string and returned
    as `""`.
  - List: verify script and script_args appear in each list item.
  - Detail: verify script and script_args appear in the detail response.
  - Select: verify script and script_args are NOT included (select returns only id
    and summary).
  - Contributor ownership: verify Contributor cannot update script fields on another
    user's test case (existing test should already cover this for other fields;
    verify it still passes with script fields).
  - Owner/Editor: verify they can update script fields on any test case.
  - Nested Option semantics for update:
    - `"script": "new_value"` -> updates to "new_value".
    - `"script": null` -> clears to NULL.
    - script field absent -> preserves current value.
  - Unicode: script and script_args with multi-byte UTF-8 characters (e.g., file
    paths with Unicode directory names) round-trip correctly.

---

## Layer 3 -- Adapters (HTTP)

- [ ] 6. Update `TestCaseHandler` request/response handling for script fields --
       `design.md#API Contract`, `design.md#Components`
  - Create handler (`POST`): accept `script` and `script_args` in the request body
    deserialization. Apply HTML sanitization (strip disallowed tags) alongside
    `summary`, `description`, and `notes`. Validate max lengths (2000 and 5000).
  - Update handler (`PATCH`): accept `script` and `script_args` in the request body
    deserialization using the nested Option pattern (distinguish absent vs null vs
    value). Apply HTML sanitization. Validate max lengths.
  - List handler (`GET .../test-cases`): include `script` and `script_args` in
    each item of the serialized `data` array.
  - Detail handler (`GET .../test-cases/{id}`): include `script` and `script_args`
    in the serialized response body.
  - Select handler (`GET .../test-cases/select`): no change (continue returning
    only `id` and `summary`).
  - Delete handler: no change.
  - Strict mode: add `script` and `script_args` to the recognized field sets for
    create and update endpoints (so they pass the unrecognised-field rejection
    check).
  - Error mapping: max-length validation failures return `422 Unprocessable Entity`
    with field-level details (same pattern as `description` and `notes`).

- [ ] 7. Write/update integration tests for script fields in HTTP handlers --
       `design.md#API Contract`
  - Test `POST` with script and script_args populated: verify `201 Created` and
    response includes both fields.
  - Test `POST` with script only: verify script_args defaults to `null`.
  - Test `POST` with script_args only: verify script defaults to `null`.
  - Test `POST` with both omitted: verify both default to `null`.
  - Test `POST` with script exceeding 2000 chars: verify `422`.
  - Test `POST` with script_args exceeding 5000 chars: verify `422`.
  - Test `POST` with script exactly 2000 chars: verify `201`.
  - Test `POST` with script_args exactly 5000 chars: verify `201`.
  - Test `POST` with script as empty string `""`: verify stored and returned as
    `""`.
  - Test `POST` with script as explicit `null`: verify stored as NULL and returned
    as `null`.
  - Test `PATCH` to update script to a new value: verify `200` and response
    reflects change.
  - Test `PATCH` to update script_args to a new value: verify `200` and response
    reflects change.
  - Test `PATCH` to set script to `null` (explicit clear): verify field becomes
    `null`.
  - Test `PATCH` to set script to empty string `""`: verify field becomes `""`.
  - Test `PATCH` with script field absent: verify current value preserved.
  - Test `PATCH` with script_args field absent: verify current value preserved.
  - Test `PATCH` with script exceeding 2000 chars: verify `422`.
  - Test `PATCH` with script_args exceeding 5000 chars: verify `422`.
  - Test `GET .../test-cases` (list): verify each item includes `script` and
    `script_args` fields.
  - Test `GET .../test-cases/{id}` (detail): verify response includes `script` and
    `script_args`.
  - Test `GET .../test-cases/select`: verify items have only `id` and `summary`
    (no script fields).
  - Test `PATCH` as Contributor on another user's test case: verify `403` (ownership
    restriction still applies).
  - Test `PATCH` as Owner on any test case: verify `200` (no ownership restriction).
  - Test that unrecognised fields in the request body still return `422` (strict
    mode), but `script` and `script_args` are recognized as valid fields.
  - Test XSS sanitization: verify HTML tags are stripped from `script` and
    `script_args` before storage.
  - Test Unicode in script and script_args: verify round-trip with multi-byte UTF-8
    characters.
  - Test that System Admin can set script fields on any project's test cases
    (admin bypass still works).
  - Test that `updated_at` changes when only script fields are updated (verify the
    trigger fires).

---

## Layer 4 -- Infrastructure

- [ ] 8. Create database migration to add `script` and `script_args` columns --
       `design.md#Data Model`
  - Forward migration:
    ```sql
    ALTER TABLE test_cases ADD COLUMN script TEXT;
    ALTER TABLE test_cases ADD COLUMN script_args TEXT;
    ALTER TABLE test_cases
      ADD CONSTRAINT chk_test_cases_script
      CHECK (script IS NULL OR char_length(script) <= 2000);
    ALTER TABLE test_cases
      ADD CONSTRAINT chk_test_cases_script_args
      CHECK (script_args IS NULL OR char_length(script_args) <= 5000);
    ```
  - Rollback migration:
    ```sql
    ALTER TABLE test_cases DROP CONSTRAINT IF EXISTS chk_test_cases_script_args;
    ALTER TABLE test_cases DROP CONSTRAINT IF EXISTS chk_test_cases_script;
    ALTER TABLE test_cases DROP COLUMN IF EXISTS script_args;
    ALTER TABLE test_cases DROP COLUMN IF EXISTS script;
    ```
  - Update `SqlTestCaseRepository` in the same task:
    - INSERT statement: add `script` and `script_args` columns with parameterized
      values.
    - UPDATE statement: add `script = $N` and `script_args = $N` to the SET clause
      with parameterized values.
    - SELECT in `find_by_id`: add `tc.script, tc.script_args` to the column list.
    - SELECT in `find_by_project`: add `tc.script, tc.script_args` to the column
      list.
    - `find_all_active_by_project` (select endpoint): no change (only selects `id`
      and `summary`).
  - Write repository-level tests:
    - Insert a test case with script and script_args, retrieve via `find_by_id`,
      verify values round-trip.
    - Insert with null script and script_args, verify nulls round-trip.
    - Insert with empty string script, verify empty string round-trips (distinct
      from null).
    - Update script on an existing row, verify change persisted.
    - Update script_args on an existing row, verify change persisted.
    - Clear script by setting to null, verify NULL stored.
    - List query includes script and script_args in result rows.
    - Verify CHECK constraint rejects script > 2000 chars at the database level.
    - Verify CHECK constraint rejects script_args > 5000 chars at the database
      level.
    - Verify `BEFORE UPDATE` trigger updates `updated_at` when only script fields
      change.
