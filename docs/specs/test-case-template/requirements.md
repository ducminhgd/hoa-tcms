# Feature: Test Case Template

## Overview

When creating or editing a test case, users can select a template to pre-fill the test
case description field with standardised content. Templates are managed through the
metadata-templates feature and are scoped to a single project. The template content is
copied into the test case at creation/edit time -- there is no persistent FK relationship
between test cases and templates after the operation completes.

This feature extends the existing test case create and update endpoints to accept an
optional `template_id` parameter. When provided, the template's `description` is used as
the default value for the test case's `description` field. The user can override it by
providing their own `description` in the same request. Template listing for the UI
dropdown reuses the existing `GET /api/v1/projects/{projectId}/templates/select` endpoint
from the metadata-templates feature.

No new database table, no new HTTP endpoints, and no new permission codes are required.

---

## User Stories

### US-1: Create Test Case from Template

As a project member with `test_case:create` permission, I want to select a template when
creating a test case so that the description field is pre-filled with the template's
standardised content, saving me from typing repetitive test steps.

**Acceptance Criteria (EARS)**

- WHEN a user sends `POST /api/v1/projects/{projectId}/test-cases` with a valid
  `template_id` in the request body, THE SYSTEM SHALL fetch the referenced template,
  verify it exists, is not soft-deleted, and belongs to the same project. THE SYSTEM
  SHALL then use the template's `description` as the default value for the test case's
  `description` field:
  - IF the request also includes an explicit `description`, THE SYSTEM SHALL use the
    explicit value (the template is a pre-fill default, not an override).
  - IF the request does not include `description` and the template's `description` is
    non-null, THE SYSTEM SHALL set the test case's `description` to the template's value.
  - IF the request does not include `description` and the template's `description` is
    `null`, THE SYSTEM SHALL leave the test case's `description` as `null`.
- IF `template_id` is omitted from the request body, THE SYSTEM SHALL behave exactly
  as before (no template pre-fill; `description` defaults to `null` unless explicitly
  provided). This preserves backward compatibility.
- IF `template_id` is provided but the template does not exist, is soft-deleted, or
  belongs to a different project, THE SYSTEM SHALL return `422 Unprocessable Entity`
  with error code `INVALID_TEMPLATE` and a message indicating the template is invalid
  for this project.
- IF `template_id` is provided but is not a valid integer, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with `VALIDATION_ERROR` and a field-level detail for
  `template_id`.
- The template pre-fill only affects the `description` field. All other test case
  fields (`summary`, `category_id`, `priority_id`, `automated`, `notes`) are
  unaffected by the template.
- All existing create validation rules (summary required, unique summary, FK
  validation, field lengths) remain in effect and are applied after template pre-fill.
- No additional system permission beyond `test_case:create` is required. The template
  validation is an internal operation and does not check `template:read` permission.

### US-2: Update Test Case with Template

As a project member with `test_case:update` permission, I want to select a template when
editing a test case so that the description field is replaced with the template's
standardised content, allowing me to reset or change the test steps to a known template.

**Acceptance Criteria (EARS)**

- WHEN a user sends `PATCH /api/v1/projects/{projectId}/test-cases/{id}` with a valid
  `template_id` in the request body, THE SYSTEM SHALL fetch the referenced template,
  verify it exists, is not soft-deleted, and belongs to the same project. THE SYSTEM
  SHALL then use the template's `description` as the new value for the test case's
  `description` field:
  - IF the request also includes an explicit `description`, THE SYSTEM SHALL use the
    explicit value (the template is a pre-fill default, not an override).
  - IF the request does not include `description` and the template's `description` is
    non-null, THE SYSTEM SHALL set the test case's `description` to the template's value.
  - IF the request does not include `description` and the template's `description` is
    `null`, THE SYSTEM SHALL NOT change the test case's `description` (the field
    preserves its current value).
- IF `template_id` is omitted from the request body, THE SYSTEM SHALL behave exactly
  as before (no template pre-fill). This preserves backward compatibility.
- IF `template_id` is provided but the template does not exist, is soft-deleted, or
  belongs to a different project, THE SYSTEM SHALL return `422 Unprocessable Entity`
  with error code `INVALID_TEMPLATE`.
- IF `template_id` is provided but is not a valid integer, THE SYSTEM SHALL return
  `422 Unprocessable Entity` with `VALIDATION_ERROR` and a field-level detail for
  `template_id`.
- The template pre-fill only affects the `description` field. All other test case
  fields are unaffected by the template.
- All existing update validation rules (at least one field, unique summary if changing,
  FK validation, field lengths) remain in effect and are applied after template pre-fill.
- The Contributor ownership check remains in effect -- providing `template_id` does
  not bypass it.
- No additional system permission beyond `test_case:update` is required.

### US-3: Browse Available Templates for Test Case Form

As a project member creating or editing a test case, I want to see a list of available
templates in a dropdown so that I can choose one to pre-fill the description field.

**Acceptance Criteria (EARS)**

- WHEN the UI needs to populate the template selection dropdown on the test case
  create/edit form, THE SYSTEM SHALL use the existing `GET /api/v1/projects/{projectId}/templates/select`
  endpoint (defined in metadata-templates spec) to retrieve all non-deleted templates
  in the project, each containing `id`, `name`, and `description`.
- The UI SHALL display the template `name` in the dropdown and may show a preview of
  the `description` (e.g., first 100 characters as a tooltip or inline snippet) when
  the user hovers or focuses on a template option.
- No new endpoint is created for this purpose -- the existing templates/select endpoint
  already serves this need.

### US-4: Template Reference is Not Stored

As a system designer, I want to confirm that selecting a template is a one-time copy
operation and does not create a persistent link between the test case and the template,
so that templates can be edited or deleted independently without affecting existing
test cases.

**Acceptance Criteria (EARS)**

- WHEN a test case is created or updated with a `template_id`, THE SYSTEM SHALL copy
  the template's `description` value into the test case's `description` column and
  SHALL NOT store the `template_id` in the `TEST_CASES` table.
- IF a template is later edited, THE SYSTEM SHALL NOT retroactively update test cases
  that were created from that template.
- IF a template is later soft-deleted, THE SYSTEM SHALL NOT affect test cases that
  were created from that template (the test case retains its `description` as copied
  at creation/edit time).

---

## Security Considerations

### Authorization

No new permission codes are introduced. The `template_id` parameter is validated as
part of the existing `test_case:create` and `test_case:update` flows. The template
fetch is an internal server-side read that does not require the caller to hold
`template:read` permission -- the template data is used only for pre-filling and is
never returned directly to the caller through this flow. The caller must already hold
the appropriate test case permission and project membership, which ensures they are
authorised to access project-scoped data.

### Template Validation (Same-Project Constraint)

The `template_id` validation is a security boundary: it prevents cross-project data
leakage where a malicious client could provide a `template_id` from project B when
creating a test case in project A to extract template content. The validation must
verify that the referenced template exists, is not soft-deleted, and its `project_id`
matches the test case's `project_id`. This check is performed within the same database
transaction as the test case insert/update to prevent TOCTOU races.

### Input Sanitization

Template `description` content is already sanitized on input when the template is
created/updated. However, as a defence-in-depth measure, the template description
must pass through the same HTML sanitization that is applied to the test case
`description` field before being stored. This prevents stale unsanitized data (e.g.,
from a migration or direct DB manipulation) from being written into test cases.

### Rate Limiting

No changes to rate limits. The existing test case create (30 req/min) and update
(30 req/min) limits apply. The template select endpoint already has its own limit
(120 req/min).

---

## Out of Scope

- **Dedicated "create from template" endpoint** -- A separate
  `POST /api/v1/projects/{projectId}/test-cases/from-template/{templateId}` endpoint
  is not implemented. The `template_id` parameter on the existing create/update
  endpoints provides equivalent functionality with a simpler API surface.
- **Template pre-fill for fields other than description** -- Only the `description`
  field is pre-filled. Templates do not affect `summary`, `notes`, or metadata fields.
- **Multi-step template wizard** -- No step-by-step template application UI.
- **Template preview endpoint** -- The existing templates/select endpoint already
  returns `description` content for UI preview.
- **Bulk apply template** -- Applying a template to multiple test cases at once.
- **Template version tracking** -- No record of which template version was used.
- **UI views** -- Covered in `ui-*` specs.

---

## Dependencies

- **Metadata Templates** -- The `TEST_CASE_TEMPLATES` table and its `/select`
  endpoint provide the template data. The `TemplateRepository` interface is used
  by `TestCaseService` to fetch and validate templates.
- **Test Case CRUD** -- The existing `POST` and `PATCH` endpoints for test cases
  are extended to accept `template_id`. The `TestCaseService` and `TestCaseHandler`
  are modified.
- **IAM Auth** -- Session-based authentication via `AuthMiddleware` (no changes).
- **Project CRUD** -- Project existence validation (no changes).
- **Project Members** -- Project membership checks (no changes from test-case-crud).
