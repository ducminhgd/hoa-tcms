# Specs — HOA TCMS

Feature specs for the HOA Test Case Management System, organized by delivery phase.
Each feature will get its own subdirectory with `requirements.md`, `design.md`, and `tasks.md`
when work begins.

> **Source PRD:** [docs/PRD-Phase-01.md](../docs/PRD-Phase-01.md)
> **Last updated:** 2026-07-14

---

## Phase 1 — MVP

### 1. Identity & Access Management (IAM)

- `iam-auth` — Login/logout (username/email + password), session-based auth, password hashing (PBKDF2).
- `iam-users` — User CRUD, status (ACTIVE/INACTIVE), unique username/email, self-profile update, password change.
- `iam-groups` — Group CRUD, user-group membership, group status.
- `iam-roles` — Role CRUD, role-permission assignment, role inheritance from groups.
- `iam-permissions` — Seeded permission catalog, object-level permission codes (create/read/read_list/update/select/delete).
- `iam-cli-init` — CLI bootstrap command to create the first System Admin user.

### 2. Project Management

- `project-crud` — Project CRUD (name, description, status), soft-delete, auto-seed metadata from config file on creation.
- `project-members` — Project membership with roles (Owner, Editor, Contributor, Viewer).

### 3. Metadata Management (Per-Project)

- `metadata-categories` — Test Category CRUD per project, seeded from YAML config.
- `metadata-templates` — Test Case Template CRUD per project, seeded from YAML config.
- `metadata-priorities` — Five priority levels (HIGHEST, HIGH, MEDIUM, LOW, LOWEST) per project, config-driven.
- `metadata-plan-types` — Test Plan types (ACCEPTANCE, FUNCTIONAL, API, INTEGRATION, PERFORMANCE, REGRESSION, SECURITY), seeded from config.

### 4. Test Case Management

- `test-case-crud` — Test Case CRUD (summary, project, category, priority, automated toggle, description, notes).
- `test-case-template` — Select template when creating/editing a Test Case to pre-fill description.
- `test-case-files` — File attachments on Test Cases (upload, download, delete).
- `test-case-orphan` — Orphaned Test Cases (no project) visible only to creator and shared users.

### 5. Test Plan Management

- `test-plan-crud` — Test Plan CRUD (name, multi-project, version, type, description, status TO DO/IN PROGRESS/DONE/CANCEL).
- `test-plan-status` — Status transition validation: cannot set DONE while any linked Test Run has IN PROGRESS results.
- `test-plan-runs` — Link/unlink Test Runs to a Test Plan; display runs on detail view.

### 6. Test Run Management

- `test-run-crud` — Test Run CRUD (summary, report-to, default tester, project, plan, version, notes, planned dates).
- `test-run-cases` — Add/remove Test Cases to a Test Run via tabular interface.
- `test-run-statistics` — Statistics aggregation (total, NOT TESTED, IN PROGRESS, PASS, FAIL, WARNING, IGNORE) on detail view.
- `test-run-executions` — Display linked Test Executions on detail view.

### 7. Test Execution Management

- `test-execution-crud` — Test Execution CRUD (name, testers, mandatory Test Run).
- `test-execution-import` — Selective import of Test Cases from linked Test Run into the execution (snapshot).
- `test-execution-reimport` — Re-import refreshes snapshot fields (summary, description, priority) but preserves result, logs, files.
- `test-case-result-update` — Update each Test Case Result independently (status, logs, attached files).
- `test-case-result-files` — File attachments on Test Case Results.

### 8. Sharing

- `sharing-ui` — Tabular sharing interface on create, update, and detail views of all shareable objects.
- `sharing-override` — Per-object sharing role overrides project membership role for that object.
- `sharing-roles` — Editor (edit + share), Contributor (edit, no share), Viewer (read-only) on shared objects.

### 9. Authorization (Three-Layer)

- `auth-rbac` — System RBAC: action check via permissions from direct roles + group-inherited roles.
- `auth-project-scope` — Project membership scope check on all project-scoped objects.
- `auth-sharing-scope` — Object sharing scope check; sharing role takes priority over project role.
- `auth-admin-bypass` — System Admin bypasses all permission and scope checks.

### 10. Soft Delete & Audit

- `soft-delete` — Soft-delete (`deleted_at` + `deleted_by`) on all entities, no cascade, no hard delete.
- `audit-columns` — Audit column pairs (`created_at`/`created_by`, `updated_at`/`updated_by`) on all mutable tables.
- `audit-triggers` — DB triggers for `updated_at`/`updated_by` auto-maintenance; app-layer immutability for `created_at`/`created_by`.

### 11. General UI

- `ui-list-views` — List views with checkbox column, action icons, ID/Name navigation, Add New button, filter controls.
- `ui-pagination` — Server-side pagination with configurable page sizes (10/25/50/100), cookie-persisted preference.
- `ui-sidebar` — Sidebar navigation: IAM, Settings (Metadata scoped to project), Test Management, User Profile, Logout.
- `ui-project-switcher` — Project context switcher to change active project scope.
- `ui-file-upload` — File upload with configurable size/type restrictions.

---

## Phase 2 — Integration

### 12. Jira Integration

- `jira-config` — Jira Personal Access Token configuration (encrypted storage).
- `jira-project-link` — Link TCMS Project to Jira Project(s).
- `jira-release-link` — Link Test Plan to Jira Releases.
- `jira-issue-link` — Link Test Case to Jira Issues (related + discovered bugs).
- `jira-create-bug` — Create Jira bug ticket from a failed Test Case Result.

### 13. Automation Integration

- `automation-script` — Use Script + Arguments fields on Test Cases to trigger/reference external automation runs.

---

## Phase 3 — Scale

### 14. Notifications

- `notifications-email` — Email/in-app notifications to Report To user on execution result changes.

### 15. Reporting & Dashboards

- `reporting-charts` — Charts, coverage reports, trend analysis dashboards.

### 16. Public REST API

- `public-api` — Documented, versioned public REST API for third-party integrations.

---

## Feature Spec Directory Structure

When a feature is ready for detailed specification, create its subdirectory:

```
specs/<feature-slug>/
├── requirements.md    # User stories with EARS acceptance criteria
├── design.md          # Architecture, API contract, data model, sequence
└── tasks.md           # Ordered, atomic implementation tasks
```

> **Rule:** No implementation starts until `requirements.md` and `design.md` exist and are confirmed.
