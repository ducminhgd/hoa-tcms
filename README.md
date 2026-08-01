# Hoa — Test Cases Management System

A Test Cases Management System with a Rust backend (Actix-Web) and a Leptos frontend (SSR + WASM).

## Prerequisites

- Rust (stable, edition 2024)
- PostgreSQL 15+
- Redis 7+
- Docker (for local development services)
- [sqlx-cli](https://crates.io/crates/sqlx-cli) (for migrations)
- [cargo-leptos](https://crates.io/crates/cargo-leptos) (for the frontend)

```bash
cargo install sqlx-cli --no-default-features --features postgres
cargo install cargo-leptos
```

## Quick start

```bash
# 1. Start infrastructure
docker compose up -d

# 2. Configure environment
cp .env.example .env
openssl rand -base64 32 | xargs -I{} sed -i "s/CHANGE_ME_TO_A_RANDOM_SECRET/{}/" .env

# 3. Apply database migrations and seed reference data
make migrate-all

# 4. Create a superuser
make createsuperuser USERNAME=admin EMAIL=admin@hoa.local FULLNAME="System Admin"
# (prompts for password — min 8 characters)

# 5. Backend — start the API server
make run
# → http://localhost:8080
curl http://localhost:8080/api/v1/health

# 6. Frontend — start the Leptos app (SSR + WASM with hot-reload)
make frontend
# → http://localhost:3000
```

## Front-End

The frontend is built with [Leptos](https://leptos.dev/) — a Rust full-stack web framework.
It uses **server-side rendering (SSR)** for fast initial loads and **WASM hydration** for client interactivity, all in one Rust codebase.

| Command | Description |
|---|---|
| `make frontend` | Start the Leptos dev server with hot-reload (SSR + WASM) |
| `make frontend-build` | Build the frontend for production |
| `cargo leptos serve` | Alias if run from the `frontend/` directory |
| `cargo leptos watch` | Watch mode (same as `serve` without the server) |

The `[package.metadata.leptos]` section in `frontend/Cargo.toml` controls the configuration:

```toml
[package.metadata.leptos]
name = "hoa-tcms-frontend"
output-name = "hoa-frontend"
site-root = "target/site"
site-pkg-dir = "pkg"
assets-dir = "public"
style-file = "output.css"
env = "DEV"
site-addr = "127.0.0.1:3000"
reload-port = 3001
```

## Database

| Command | Description |
|---|---|
| `make db-create` | Create the database |
| `make db-drop` | Drop the database |
| `make migrate` | Run pending migrations |
| `make migrate-revert` | Revert the last migration |
| `make migrate-info` | Show migration status (applied + pending) |
| `make seed` | Run seed data — permissions, roles, groups (idempotent) |
| `make migrate-all` | Run all migrations, then seed data |
| `make createsuperuser` | Create a user in the System Admin group |

The `DATABASE_URL` defaults to `postgres://tcms:tcms_pass@localhost:5432/hoa_tcms` (matches docker-compose).
Override it by setting the env var or passing it on the command line:

```bash
make migrate DATABASE_URL="postgres://user:pass@host:5432/dbname"
```

### Seed data

The seeder populates these reference tables at startup and via `make seed`:

| Entity | Count | Details |
|--------|-------|---------|
| Permissions | 55 | Full CRUD for all resources (`user:*`, `project:*`, `test_case:*`, …) |
| Groups | 2 | Default, System Admin |
| Roles | 4 | System Admin (55 perms), Project Manager (36), Tester (10), Viewer (11) |

### Adding permissions for a new model

```bash
make gen-perms MODELS=invoice,payment
# Paste the output into config/default-setup.yaml under 'permissions:',
# assign to roles under 'roles:', then run 'make seed'.
```

## Development

```bash
# Backend with hot‑reload (requires cargo-watch)
make dev

# Frontend with hot‑reload
make frontend

# Format and lint
make check

# Run tests
make test

# Build release
make build
```

## Configuration

| Variable | Default | Description |
|---|---|---|
| `HOST` | `127.0.0.1` | Listen address |
| `PORT` | `8080` | Listen port |
| `DATABASE_URL` | — | PostgreSQL connection string (**required**) |
| `REDIS_URL` | — | Redis connection string (**required**) |
| `SESSION_SECRET` | — | Session signing secret, min 32 chars (**required**) |
| `LOG_MAX_LEVEL` | `TRACE` | Global tracing level cap |
| `DB_MAX_CONNECTIONS` | `10` | Database connection pool size |
| `PBKDF2_ITERATIONS` | `600000` | Password hashing iterations |
| `UPLOAD_BASE_DIR` | `uploads` | File upload storage directory |
| `MAX_FILE_SIZE` | `10485760` | Max file upload size in bytes (10 MB) |
| `METADATA_CONFIG_PATH` | `config/default-metadata.yaml` | Pre-seeded metadata config |
| `SETUP_CONFIG_PATH` | `config/default-setup.yaml` | Permissions/roles/groups seed config |
| `CORS_ALLOWED_ORIGIN` | `http://localhost:3000` | Allowed CORS origin for the frontend |

The frontend reads `TCMS_API_BASE` (compile-time, defaults to `http://localhost:8080`) to reach the backend — set it in the cargo-leptos `.env` file when the API is hosted elsewhere.

## Architecture

```
cmd/
  server/              Entry point — HTTP server startup, dependency wiring
  seeder/              Standalone seed binary (make seed)
  createsuperuser/     CLI to create a bootstrap admin (make createsuperuser)
internal/
  domain/              Entities, value objects, domain errors (innermost)
  application/         Use cases, repository interfaces, DTOs
  adapters/            HTTP handlers, middleware, error mapping
  infrastructure/      PostgreSQL repositories, Redis, config
frontend/              Leptos SSR + WASM app (make frontend)
pkg/                   Shared utilities — errors, pagination, validation
migrations/            SQL migration files (sqlx-cli)
scripts/               Utility scripts (gen-permissions)
config/                YAML files for seed data
```
