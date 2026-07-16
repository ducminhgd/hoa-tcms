# Phase 1 Project Plan — HOA TCMS

**Based on:** [PRD-Phase-01.md](PRD-Phase-01.md) v3.2
**Stack:** Rust + Leptos + Actix-Web + PostgreSQL + Redis
**Architecture:** Clean Architecture (Domain → Application → Adapters → Infrastructure)
**Last Updated:** 2026-07-11

---

## Build Order

Tasks are sequential within each milestone. Parallel opportunities noted inline.

---

### Milestone 1: Foundation

| # | Task | Status |
|---|------|--------|
| 1 | Project scaffold — `cargo init`, Clean Architecture layout, `.env`, `Makefile` | Done |
| 2 | Database migrations — all 14 entities + 9 junction tables, audit column triggers | Done |
| 3 | Seed data — permissions table, config file schema (YAML) | Done |
| 4 | CI/CD pipeline — `cargo test`, `cargo clippy`, `cargo fmt --check` | Done |
| 5 | Dev environment — Docker Compose (PostgreSQL + Redis), hot-reload | Done |

### Milestone 2: IAM

| # | Task | Status |
|---|------|--------|
| 6 | Domain layer — User, Group, Role, Permission entities, value objects, errors, repo interfaces | To do |
| 7 | Infrastructure — PostgreSQL repositories (users, groups, roles, permissions) | To do |
| 8 | Auth service — login (username-or-email + password, PBKDF2), session creation, logout, SessionMiddleware | To do |
| 9 | RBAC service — effective permissions (direct roles ∪ group roles) | To do |
| 10 | IAM use cases — RegisterUser, UpdateUser, DeactivateUser, ManageGroup, ManageRole, self-profile | To do |
| 11 | IAM API routes — `/api/v1/auth/*`, `/api/v1/users/*`, `/api/v1/groups/*`, `/api/v1/roles/*`, `/api/v1/permissions` | To do |
| 12 | IAM UI — Login, Users/Groups/Roles lists + CRUD, Permissions list, User Profile | To do |
| 13 | CLI init-admin command — `tcms-cli init-admin`, first System Admin with `created_by = NULL` | To do |

### Milestone 3: Projects + Metadata

| # | Task | Status |
|---|------|--------|
| 14 | Project domain — Project entity, ProjectMember, repo interface | To do |
| 15 | Metadata domain — TestCategory, TestCaseTemplate, priority config, test plan types | To do |
| 16 | Project infrastructure — repositories, YAML config loader, per-project seeding on create | To do |
| 17 | Project use cases — CreateProject (auto-seed), UpdateProject, ManageProjectMembers | To do |
| 18 | Project API routes — `/api/v1/projects/*`, categories, templates sub-routes | To do |
| 19 | Project UI — list, create/update/detail with membership tab | To do |
| 20 | Project context switcher — sidebar component, filters metadata views by active project | To do |

### Milestone 4: Test Cases

| # | Task | Status |
|---|------|--------|
| 21 | TestCase domain — entity, repo interface | To do |
| 22 | TestCase infrastructure — repository with category multi-select, template population | To do |
| 23 | TestCase use cases — Create (template → description), Update, orphan visibility rules | To do |
| 24 | TestCase API routes — `/api/v1/test-cases` CRUD, file attach | To do |
| 25 | TestCase UI — list (paginated, per-project), create/update (template, category, priority, file), detail | To do |

### Milestone 5: File Upload

| # | Task | Status |
|---|------|--------|
| 26 | File domain — entity, polymorphic owner reference | To do |
| 27 | File infrastructure — local disk adapter, MIME validation, size enforcement, download/serve | To do |
| 28 | File API routes — upload, download, soft-delete | To do |
| 29 | File upload UI — reusable drag-and-drop component | To do |

### Milestone 6: Test Plans

| # | Task | Status |
|---|------|--------|
| 30 | TestPlan domain — entity (multi-project, JSONB types, status state machine), repo interface | To do |
| 31 | TestPlan infrastructure — repository, status transition validation (DONE blocked by IN PROGRESS results) | To do |
| 32 | TestPlan use cases — Create, Update, TransitionStatus (permission priority chain) | To do |
| 33 | TestPlan API routes — `/api/v1/test-plans` CRUD | To do |
| 34 | TestPlan UI — list, create/update (multi-project, multi-type, status), detail with linked Test Runs table | To do |

### Milestone 7: Test Runs

| # | Task | Status |
|---|------|--------|
| 35 | TestRun domain — entity (FK to Plan/Project, Tester, Planned dates), TestRunCase junction, repo interface | To do |
| 36 | TestRun infrastructure — repository, statistics aggregation (counts by result status) | To do |
| 37 | TestRun use cases — Create, AddTestCases, RemoveTestCases, ComputeStatistics | To do |
| 38 | TestRun API routes — `/api/v1/test-runs` CRUD, list cases for import dialog | To do |
| 39 | TestRun UI — list, create/update (case assignment, plan/project selector), detail (cases, executions, statistics) | To do |

### Milestone 8: Test Executions & Results

| # | Task | Status |
|---|------|--------|
| 40 | TestExecution domain — entity (mandatory FK to Run, multi-tester), TestCaseResult (snapshot + result), repo interface | To do |
| 41 | TestExecution infrastructure — repositories, import logic (INSERT snapshot), re-import (UPDATE snapshot, PRESERVE results) | To do |
| 42 | TestExecution use cases — Create, ImportCases, ReimportCases, UpdateResult (set tested_by on first transition) | To do |
| 43 | TestExecution API routes — CRUD, import-cases, update result | To do |
| 44 | TestExecution UI — list, create (run selector, tester select), detail (results table, inline update, file upload, import dialog) | To do |

### Milestone 9: Three-Layer Authorization

| # | Task | Status |
|---|------|--------|
| 45 | Auth middleware — extract session → load user → check ACTIVE → load permissions → attach to request | To do |
| 46 | Action check guard — per-endpoint permission, System Admin bypass, 403 on failure | To do |
| 47 | Scope check guard — project membership OR object sharing, orphaned visibility rules | To do |
| 48 | Permission inheritance — Execution → Result (same level) | To do |
| 49 | Retrofit all routes — wire action + scope checks into milestones 2–8 endpoints | To do |

### Milestone 10: Sharing

| # | Task | Status |
|---|------|--------|
| 50 | Sharing domain — ObjectSharing entity (polymorphic resource_type + resource_id + role), repo interface | To do |
| 51 | Sharing infrastructure — repository, sharing-overrides-project-membership logic | To do |
| 52 | Sharing use cases — ShareObject, RemoveShare, GetEffectiveRole | To do |
| 53 | Sharing API routes — create, delete share | To do |
| 54 | Sharing UI — sharing tab on create/update/detail of all shareable objects | To do |

### Milestone 11: Soft Delete & Data Lifecycle

| # | Task | Status |
|---|------|--------|
| 55 | Soft-delete enforcement — `WHERE deleted_at IS NULL` on all queries, set deleted_at + deleted_by on delete | To do |
| 56 | Audit column enforcement — reject writes to created_at/created_by after insert, trigger for updated_at/updated_by | To do |
| 57 | Deactivated/hidden rules — INACTIVE hides children, soft-deleted hides children, orphan visibility | To do |
| 58 | UI for soft-delete — filter deleted rows, delete confirmations, INACTIVE toggle | To do |

### Milestone 12: General UI & Polish

| # | Task | Status |
|---|------|--------|
| 59 | List view standard — checkbox first column, action icons last, click ID/Name → detail/update | To do |
| 60 | Pagination — server-side, page size selector (10/25/50/100, default 25), cookie preference | To do |
| 61 | Filter + Add New — on every list page | To do |
| 62 | Sidebar menu — full layout: IAM, Settings, Test Management, Profile, Logout | To do |

### Milestone 13: Testing & Documentation

| # | Task | Status |
|---|------|--------|
| 63 | Unit tests — domain logic: status transitions, permission union, snapshot cloning, re-import, audit rules | To do |
| 64 | Integration tests — API: CRUD per entity, auth flow, import/re-import, statistics | To do |
| 65 | Permission matrix tests — each role × object × action, sharing override, admin bypass, orphan visibility | To do |
| 66 | E2E tests — Journeys 1–4 (execution workflow, user provisioning, case authoring, re-import) | To do |
| 67 | API documentation — OpenAPI 3.x spec covering all endpoints | To do |
| 68 | README — setup, Docker Compose, config reference, make targets | To do |

---

## Dependency Graph

```
Foundation (1–5)
  └── IAM (6–13)
        ├── Projects + Metadata (14–20)
        │     ├── Test Cases (21–25)
        │     │     └── File Upload (26–29)
        │     ├── Test Plans (30–34)
        │     │     └── Test Runs (35–39)
        │     │           └── Test Executions + Results (40–44)
        │     └── ...
        ├── Three-Layer Auth (45–49) ← retrofits all routes above
        ├── Sharing (50–54)
        └── Soft Delete (55–58)
              └── General UI + Polish (59–62)
                    └── Testing + Docs (63–68)
```

**Critical path:** 1 → 2 → 6 → 7 → 8 → 9 → 10 → 45 → 46 → 47 (auth gates everything downstream)
**Parallel after IAM:** Auth middleware (45) + Projects (14) can start in parallel
**Deferrable:** File Upload (26–29) alongside Test Cases; Polish (59–62) independent once pages exist

---

## Summary

| Metric | Value |
|--------|-------|
| Total Tasks | 68 |
| Milestones | 13 |
| Build order | Sequential 1 → 68 |
| Critical path | ~22 sequential tasks |
