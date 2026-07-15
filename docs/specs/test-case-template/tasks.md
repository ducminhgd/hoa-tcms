# Tasks: Test Case Template

Tasks are ordered by dependency. Each task corresponds to one atomic, mergeable unit
of work that maps to one or more requirements or design sections. No new database table
or migration is required -- this feature extends existing components.

---

## Layer 2 -- Application

- [ ] 1. Add `template_id` field to `CreateTestCaseCommand` DTO --
       `requirements.md#US-1`, `design.md#Components`
  - Add `template_id: Option<i64>` field to the existing `CreateTestCaseCommand` struct
  - When `Some(id)`, indicates the caller wants template pre-fill
  - When `None` (omit from JSON body), existing behaviour is preserved (backward compatible)
  - No validation beyond "must be an integer if present" (non-integer values rejected
    by the deserializer/validator at the handler layer)

- [ ] 2. Add `template_id` field to `UpdateTestCaseCommand` DTO --
       `requirements.md#US-2`, `design.md#Components`
  - Add `template_id: Option<i64>` field to the existing `UpdateTestCaseCommand` struct
  - Semantics match create: `Some(id)` enables template pre-fill, `None` preserves
    existing behaviour
  - The nested Option pattern used by other fields (`Option<Option<T>>` for "provided
    vs omitted") does NOT apply to `template_id` -- it is a flat `Option<i64>` because
    `null` is not a meaningful value for a template reference (the handler rejects
    `"template_id": null` as a type error)

- [ ] 3. Add `TemplateRepository` dependency to `TestCaseService` --
       `design.md#Components`
  - Add `TemplateRepository` (or a narrower `TemplateResolver` interface) as a
    constructor parameter of `TestCaseService`
  - The dependency is used to fetch and validate templates by `(project_id, template_id)`
  - Update the DI wiring (in `main` or container) to inject the concrete
    `SqlTemplateRepository` implementation
  - If `TestCaseService` is unit-tested with mocks, update the mock setup to include
    the new dependency

- [ ] 4. Implement template pre-fill logic in `TestCaseService::create_test_case` --
       `requirements.md#US-1`, `design.md#Sequence`
  - After the existing FK validation steps (category, priority) and within the same
    DB transaction:
    a. If `cmd.template_id` is `Some(id)`: call
       `template_repo.find_by_id_in_project(project_id, id)`. If `None` -> return
       `InvalidTemplateError` (which maps to `422 INVALID_TEMPLATE`)
    b. Resolve effective `description`:
       - If `cmd.description` is explicitly provided (non-None, regardless of inner
         value): use the explicit value (the template is a pre-fill default)
       - If `cmd.description` is omitted (`None`): use the template's `description`
         (may be `null`, which maps to `None`)
    c. If the resolved description came from the template: apply HTML sanitization
       as a defence-in-depth measure (template content is already sanitized on input,
       but this guards against stale data)
    d. Pass the resolved description to the `TestCase` entity factory
  - If `cmd.template_id` is `None`: skip all template logic, use existing behaviour
    (description from `cmd.description`, default `null`)
  - Template fetch and validation executes within the same transaction to prevent
    TOCTOU (template deleted between check and insert)

- [ ] 5. Implement template pre-fill logic in `TestCaseService::update_test_case` --
       `requirements.md#US-2`, `design.md#Sequence`
  - After the existing FK validation steps and within the same DB transaction:
    a. If `cmd.template_id` is `Some(id)`: call
       `template_repo.find_by_id_in_project(project_id, id)`. If `None` -> return
       `InvalidTemplateError`
    b. Resolve effective `description`:
       - If `cmd.description` is explicitly provided (`Some(Some(val))` or
         `Some(None)`): use the explicit value (template is a default; explicit
         `null` clears the field)
       - If `cmd.description` is omitted (`None`): use the template's `description`.
         If the template's `description` is `null`, preserve the test case's current
         `description` (do NOT clear it)
    c. If the resolved description came from the template: apply HTML sanitization
    d. Use the resolved description as the new value for `description`
  - If `cmd.template_id` is `None`: skip all template logic, use existing behaviour
  - Contributor ownership check remains in effect (unchanged)

- [ ] 6. Write unit tests for `TestCaseService` template pre-fill logic --
       `requirements.md#US-1`, `requirements.md#US-2`
  - Table-driven tests with mock `TemplateRepository` (in addition to existing mocks)
  - Create with `template_id`:
    - Happy path: template exists, no explicit description -> test case gets template's
      description
    - Explicit description overrides template -> explicit value stored
    - Explicit `null` description overrides template -> `null` stored
    - Template description is `null`, no explicit description -> test case description
      is `null`
    - Template belongs to different project -> `InvalidTemplateError`
    - Template soft-deleted -> `InvalidTemplateError`
    - Template not found -> `InvalidTemplateError`
    - No `template_id` provided -> existing behaviour (no pre-fill)
  - Update with `template_id`:
    - Happy path: template exists, no explicit description -> test case description
      replaced with template's
    - Explicit description overrides template -> explicit value stored
    - Explicit `null` description overrides template -> `null` stored
    - Template description is `null`, no explicit description -> preserve current
      description
    - Template belongs to different project -> `InvalidTemplateError`
    - Template soft-deleted -> `InvalidTemplateError`
    - No `template_id` provided -> existing behaviour
  - Transaction rollback: template validation failure rolls back the entire operation
  - Template pre-fill does not affect other fields (summary, notes, etc.)
  - System Admin bypass still works (template_id is just a parameter, not a permission gate)

---

## Layer 3 -- Adapters (HTTP)

- [ ] 7. Update `TestCaseHandler` create and update handlers to accept `template_id` --
       `design.md#API Contract Changes`, `requirements.md#US-1`, `requirements.md#US-2`
  - In the `create` handler: deserialize `template_id` from the JSON request body as
    `Option<i64>`. If present but not a valid integer (string, float, boolean, array,
    object, `null`), return `422 VALIDATION_ERROR` with a field-level detail for
    `template_id`
  - In the `update` handler: same deserialization and validation for `template_id`
  - Pass `template_id` through to `CreateTestCaseCommand` / `UpdateTestCaseCommand`
  - `template_id` is NOT listed as an unrecognised field (it is a recognized field for
    both create and update endpoints). Add it to the allowed-field whitelist in strict
    mode validation
  - Map `InvalidTemplateError` from the service layer to `422 Unprocessable Entity`
    with error code `INVALID_TEMPLATE` and the standard error body format

- [ ] 8. Write integration tests for template pre-fill via HTTP --
       `requirements.md#US-1`, `requirements.md#US-2`
  - Test create with valid `template_id`:
    - No explicit description -> verify response includes template's description
    - With explicit description -> verify explicit value used
    - With `"description": null` -> verify `null` stored
    - Verify `Location` header still present
  - Test create with invalid `template_id`:
    - Non-existent template -> `422 INVALID_TEMPLATE`
    - Soft-deleted template -> `422 INVALID_TEMPLATE`
    - Template from different project -> `422 INVALID_TEMPLATE`
    - Template ID as string -> `422 VALIDATION_ERROR` with `template_id` detail
    - Template ID as float -> `422 VALIDATION_ERROR`
    - `"template_id": null` -> `422 VALIDATION_ERROR`
  - Test update with valid `template_id`:
    - No explicit description -> verify description updated from template
    - With explicit description -> verify explicit value used
    - With `"description": null` -> verify cleared
    - Template description is `null` -> verify current description preserved
  - Test update with invalid `template_id`:
    - Non-existent template -> `422 INVALID_TEMPLATE`
    - Soft-deleted template -> `422 INVALID_TEMPLATE`
  - Test backward compatibility:
    - Create without `template_id` -> existing behaviour
    - Update without `template_id` -> existing behaviour
  - Test combined with other fields:
    - `template_id` + `category_id` + `summary` -> all applied correctly
    - `template_id` + field validation error (e.g., summary too long) -> `422`
      (template is NOT pre-filled if the request fails overall validation)
  - Test Contributor ownership: `template_id` does not bypass ownership check
  - Test strict mode: `template_id` is a recognized field, not rejected

---

## Cross-Cutting Tasks

- [ ] 9. Update API documentation for test case endpoints --
       `design.md#API Contract Changes`
  - Update the OpenAPI 3.x spec for `POST /api/v1/projects/{projectId}/test-cases`:
    add `template_id` as an optional integer field in the request body schema, with
    description of its behaviour
  - Update the OpenAPI 3.x spec for `PATCH /api/v1/projects/{projectId}/test-cases/{id}`:
    add `template_id` as an optional integer field
  - Document the `422 INVALID_TEMPLATE` error response
  - Include examples: create with template, create with template + explicit
    description, update with template

- [ ] 10. Update test case create/edit form documentation for UI consumers --
        `requirements.md#US-3`
  - Document that the template dropdown on the test case form should call
    `GET /api/v1/projects/{projectId}/templates/select` to populate options
  - Document that the selected template ID is sent as `template_id` in the
    create/update request body
  - Document the pre-fill resolution behaviour (explicit description overrides
    template)

- [ ] 11. Verify template description sanitization in the pre-fill path --
        `requirements.md#Security Considerations`, `design.md#Sequence`
  - Confirm the HTML sanitizer used for test case `description` is applied to the
    template's `description` before storage
  - If the sanitizer is applied at the handler layer (before passing to service),
    ensure `TestCaseService` applies it after resolving the template source
  - If the sanitizer is applied at the repository layer, ensure it is called with the
    resolved description (not just the raw command field)
  - Add a unit test: template description contains `<script>alert(1)</script>` ->
    sanitized before storage

- [ ] 12. Manual QA checklist for template pre-fill --
       `requirements.md#US-1` through `US-4`
  - [ ] Create test case with template (no explicit description) -- verify description
        is pre-filled from template
  - [ ] Create test case with template + explicit description -- verify explicit
        description is used
  - [ ] Create test case with template + `"description": null` -- verify description
        is null
  - [ ] Create test case with template that has null description, no explicit
        description -- verify description is null
  - [ ] Create test case with invalid template_id (non-existent) -- verify `422
        INVALID_TEMPLATE`
  - [ ] Create test case with template_id from different project -- verify `422
        INVALID_TEMPLATE`
  - [ ] Create test case with template_id as a string -- verify `422 VALIDATION_ERROR`
  - [ ] Create test case without template_id -- verify backward compatible (no pre-fill)
  - [ ] Update test case with template (no explicit description) -- verify description
        updated from template
  - [ ] Update test case with template + explicit description -- verify explicit used
  - [ ] Update test case with template + `"description": null` -- verify cleared
  - [ ] Update test case with template that has null description, no explicit
        description -- verify description preserved (not cleared)
  - [ ] Update test case with invalid template_id -- verify `422 INVALID_TEMPLATE`
  - [ ] Update test case without template_id -- verify backward compatible
  - [ ] Contributor creates with template -- verify success
  - [ ] Contributor updates own test case with template -- verify success
  - [ ] Contributor attempts to update another's test case with template -- verify
        `403` (ownership check not bypassed)
  - [ ] Template listing dropdown loads correctly (reuses templates/select endpoint)
  - [ ] Soft-delete the template used by an existing test case -- verify the test
        case's description is unchanged (no FK relationship)
  - [ ] Edit the template used by an existing test case -- verify the test case's
        description is unchanged (copy, not reference)
  - [ ] Verify XSS sanitization on template description pre-fill
  - [ ] Verify strict mode still rejects unrecognised fields (but accepts
        `template_id`)
