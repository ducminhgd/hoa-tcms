# HOA TCMS — Test Case Management System

A full-stack, role-controlled test case management system built with Rust
(Clean Architecture), PostgreSQL, Redis, and vanilla JavaScript.

## Prerequisites

- **Rust** stable (edition 2024)
- **PostgreSQL** 15+
- **Redis** 7+
- **Docker** (for local development services)

## Quick Start

```bash
# 1. Start infrastructure (PostgreSQL + Redis)
docker compose up -d

# 2. Configure environment
cp .env.example .env
# Default values work for local development

# 3. Apply all database migrations
for f in migrations/[0-9]*.sql; do
  PGPASSWORD=tcms_pass psql -h localhost -U tcms -d hoa_tcms -f "$f"
done

# 4. Run the server (API + frontend)
cargo run

# 5. Open in browser
open http://localhost:8080
# Login with the System Admin credentials created via the CLI init command
```

## CLI — System Admin Bootstrap

```bash
# Create the first System Admin user (required before login)
cargo run --bin server -- init-admin --username admin --email admin@example.com --password "secure-password-here"
```

## Verify

```bash
# Health check
curl http://localhost:8080/api/v1/health
# {"status":"ok","version":"0.1.0","checks":{"database":"healthy","redis":"healthy"}}

# Login (get session cookie)
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username_or_email":"admin","password":"secure-password-here"}' \
  -c cookies.txt

# List projects (with session)
curl http://localhost:8080/api/v1/projects -b cookies.txt
```

## Development

```bash
# Check, lint, format, test
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo test --all-features

# Or use make targets
make check    # fmt-check + clippy
make test     # all tests
make lint     # clippy only
make fmt      # format code
```

## Configuration

| Variable | Default | Description |
|---|---|---|
| `HOST` | `127.0.0.1` | Listen address |
| `PORT` | `8080` | Listen port |
| `DATABASE_URL` | — | PostgreSQL connection string (**required**) |
| `REDIS_URL` | — | Redis connection string (**required**) |
| `SESSION_SECRET` | — | Session signing secret, ≥ 32 chars (**required**) |
| `UPLOAD_BASE_DIR` | `uploads` | File upload storage directory |
| `MAX_FILE_SIZE` | `10485760` | Max file upload size in bytes (10 MB) |
| `FRONTEND_DIST` | `../frontend` | Path to frontend static files |
| `METADATA_CONFIG_PATH` | `config/default-metadata.yaml` | Pre-seeded metadata config |
| `LOG_MAX_LEVEL` | `TRACE` | Global tracing level cap |
| `DB_MAX_CONNECTIONS` | `10` | Database connection pool size |

## Migrations

| File | Description |
|---|---|
| `001_initial_schema.sql` | All tables, constraints, indexes, triggers |
| `002_seed_permissions.sql` | 32 permission codes for IAM, projects, test entities, sharing |
| `003_fix_roles_nullable.sql` | Schema fix |
| `004_seed_system_admin_role.sql` | System Admin role with all permissions |
| `005_add_is_system_to_roles.sql` | `is_system` flag on roles |
| `006_add_projects_name_unique_index.sql` | Unique name constraint |
| `007_add_unique_execution_test_case.sql` | Unique constraint on execution+test case |
| `008_create_test_case_files.sql` | Test case file attachments table |

Apply all in numbered order:
```bash
for f in migrations/[0-9]*.sql; do
  PGPASSWORD=tcms_pass psql -h localhost -U tcms -d hoa_tcms -f "$f"
done
```

## API Overview (35 routes)

### Auth
| Method | Endpoint | Permission |
|--------|----------|------------|
| POST | `/api/v1/auth/login` | None |
| POST | `/api/v1/auth/logout` | Session |

### IAM
| Method | Endpoint | Permission |
|--------|----------|------------|
| GET | `/api/v1/users` | `user:read_list` |
| POST | `/api/v1/users` | `user:create` |
| GET | `/api/v1/users/me` | Session |
| PATCH | `/api/v1/users/me` | Session |
| GET | `/api/v1/users/{id}` | `user:read` |
| PATCH | `/api/v1/users/{id}` | `user:update` |
| DELETE | `/api/v1/users/{id}` | `user:delete` |
| GET | `/api/v1/groups` | `group:read_list` |
| POST | `/api/v1/groups` | `group:create` |
| GET | `/api/v1/groups/{id}` | `group:read` |
| PATCH | `/api/v1/groups/{id}` | `group:update` |
| GET | `/api/v1/roles` | `role:read_list` |
| POST | `/api/v1/roles` | `role:create` |
| GET | `/api/v1/roles/{id}` | `role:read` |
| PATCH | `/api/v1/roles/{id}` | `role:update` |
| GET | `/api/v1/permissions` | `permission:read_list` |

### Projects
| Method | Endpoint | Permission |
|--------|----------|------------|
| GET | `/api/v1/projects` | `project:read_list` |
| POST | `/api/v1/projects` | `project:create` |
| GET | `/api/v1/projects/{id}` | `project:read` |
| PATCH | `/api/v1/projects/{id}` | `project:update` |
| DELETE | `/api/v1/projects/{id}` | `project:delete` |
| GET | `/api/v1/projects/{id}/members` | `project:read` |
| POST | `/api/v1/projects/{id}/members` | `project:update` |
| PATCH | `/api/v1/projects/{p_id}/members/{u_id}` | `project:update` |

### Test Plans
| Method | Endpoint | Permission |
|--------|----------|------------|
| GET | `/api/v1/test-plans` | `test_plan:read_list` |
| POST | `/api/v1/test-plans` | `test_plan:create` |
| GET | `/api/v1/test-plans/select` | `test_plan:select` |
| GET | `/api/v1/test-plans/{id}` | `test_plan:read` |
| PATCH | `/api/v1/test-plans/{id}` | `test_plan:update` |
| DELETE | `/api/v1/test-plans/{id}` | `test_plan:delete` |
| POST | `/api/v1/test-plans/{id}/transition-status` | `test_plan:update` |

### Test Runs
| Method | Endpoint | Permission |
|--------|----------|------------|
| GET | `/api/v1/projects/{pid}/test-runs` | `test_run:read_list` |
| POST | `/api/v1/projects/{pid}/test-runs` | `test_run:create` |
| GET | `/api/v1/projects/{pid}/test-runs/{id}` | `test_run:read` |
| PATCH | `/api/v1/projects/{pid}/test-runs/{id}` | `test_run:update` |
| DELETE | `/api/v1/projects/{pid}/test-runs/{id}` | `test_run:delete` |
| GET | `/api/v1/test-runs/{id}/cases` | `test_run:read` |
| POST | `/api/v1/test-runs/{id}/cases` | `test_run:update` |
| GET | `/api/v1/test-runs/{id}/statistics` | `test_run:read` |

### Test Executions & Results
| Method | Endpoint | Permission |
|--------|----------|------------|
| GET | `/api/v1/test-executions` | `test_execution:read_list` |
| POST | `/api/v1/test-executions` | `test_execution:create` |
| GET | `/api/v1/test-executions/{id}` | `test_execution:read` |
| PATCH | `/api/v1/test-executions/{id}` | `test_execution:update` |
| DELETE | `/api/v1/test-executions/{id}` | `test_execution:delete` |
| POST | `/api/v1/test-executions/{id}/import-cases` | `test_execution:update` |
| PATCH | `/api/v1/test-case-results/{id}` | `test_execution:update` |

### Test Case Files
| Method | Endpoint | Permission |
|--------|----------|------------|
| GET | `/api/v1/projects/{pid}/test-cases/{id}/files` | `test_case_file:read` |
| POST | `/api/v1/projects/{pid}/test-cases/{id}/files` | `test_case_file:upload` |
| GET | `/api/v1/projects/{pid}/test-cases/{id}/files/{fid}` | `test_case_file:read` |
| DELETE | `/api/v1/projects/{pid}/test-cases/{id}/files/{fid}` | `test_case_file:delete` |

### Sharing
| Method | Endpoint | Permission |
|--------|----------|------------|
| GET | `/api/v1/share/{type}/{id}` | `share:read` |
| POST | `/api/v1/share` | `share:create` |
| PATCH | `/api/v1/share/{id}` | `share:create` |
| DELETE | `/api/v1/share/{id}` | `share:delete` |

## Architecture

```
cmd/server/          Entry point — HTTP server startup, dependency wiring
internal/
  domain/            Entities, value objects, domain errors (innermost)
  application/       Use cases, repository interfaces, DTOs
  adapters/          HTTP handlers, middleware, error mapping
  infrastructure/    PostgreSQL repositories, Redis, config, file storage
pkg/                 Shared utilities — errors, pagination, validation
frontend/            HTML + CSS + vanilla JS SPA (served by Actix)
migrations/          SQL migration files (8 total)
docs/
  api/               Bruno sample requests (67 .bru files)
  specs/             52 feature specifications (requirements + design + tasks)
config/              YAML metadata config (categories, templates, priorities)
```

## Request Collections

Bruno (open-source API client) sample requests are in `docs/api/`.
Open the collection with:

```bash
npx @usebruno/cli open docs/api
```

## Docker Compose Services

```yaml
# docker-compose.yml provides:
# - PostgreSQL 15 on port 5432 (user: tcms, password: tcms_pass, db: hoa_tcms)
# - Redis 7 on port 6379
```

Start with `docker compose up -d`, stop with `docker compose down`.

## Project Structure (Clean Architecture)

```
┌─────────────────────────────────────┐
│           Frameworks & Drivers      │  ← infrastructure (DB, Redis, FS)
│  ┌───────────────────────────────┐  │
│  │     Interface Adapters        │  │  ← adapters (HTTP handlers)
│  │  ┌─────────────────────────┐  │  │
│  │  │    Application / Use    │  │  │  ← application (services, repos)
│  │  │    Cases                │  │  │
│  │  │  ┌───────────────────┐  │  │  │
│  │  │  │      Domain       │  │  │  │  ← domain (entities, value objects)
│  │  │  └───────────────────┘  │  │  │
│  │  └─────────────────────────┘  │  │
│  └───────────────────────────────┘  │
└─────────────────────────────────────┘
```

Dependencies point inward. Domain knows nothing about databases, HTTP, or frameworks.

## License

Proprietary — all rights reserved.
