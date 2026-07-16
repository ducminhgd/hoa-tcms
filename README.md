# Hoa — Test Cases Management System

A Test cases Management System.

## Prerequisites

- Rust (stable, edition 2024)
- PostgreSQL 15+
- Redis 7+
- Docker (for local development services)

## Quick start

```bash
# 1. Start infrastructure
docker compose up -d

# 2. Configure environment
cp .env.example .env
# Edit .env if needed — default values work for local development

# 3. Apply database migrations
PGPASSWORD=tcms_pass psql -h localhost -U tcms -d hoa_tcms \
  -f migrations/001_initial_schema.sql
PGPASSWORD=tcms_pass psql -h localhost -U tcms -d hoa_tcms \
  -f migrations/002_seed_permissions.sql

# 4. Run the server
cargo run

# 5. Verify
curl http://localhost:8080/api/v1/health
# {"status":"ok","version":"0.1.0","checks":{"database":"healthy","redis":"healthy"}}
```

## Development

```bash
# Format and lint
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test --all-features

# Build release
cargo build --release
```

## Configuration

| Variable | Default | Description |
|---|---|---|
| `HOST` | `127.0.0.1` | Listen address |
| `PORT` | `8080` | Listen port |
| `DATABASE_URL` | — | PostgreSQL connection string (**required**) |
| `REDIS_URL` | — | Redis connection string (**required**) |
| `SESSION_SECRET` | — | Session signing secret, min 32 chars (**required**) |
| `FILE_STORAGE_PATH` | `./storage/uploads` | Upload storage directory |
| `RUST_LOG` | `info` | Tracing verbosity (`trace`, `debug`, `info`, `warn`, `error`) |
| `LOG_MAX_LEVEL` | `TRACE` | Global tracing level cap |
| `DB_MAX_CONNECTIONS` | `10` | Database connection pool size |
| `PBKDF2_ITERATIONS` | `600000` | Password hashing iterations |

## Architecture

```
cmd/server/     Entry point — HTTP server startup, dependency wiring
internal/
  domain/       Entities, value objects, domain errors (innermost)
  application/  Use cases, repository interfaces, DTOs
  adapters/     HTTP handlers, middleware, error mapping
  infrastructure/ PostgreSQL repositories, Redis, config
pkg/            Shared utilities — errors, pagination, validation
migrations/     SQL migration files
```