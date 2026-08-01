# HOA TCMS — Makefile
#
# Common development tasks.
# Requires: cargo, docker (for dev services), sqlx-cli (for migrations).

# Default DATABASE_URL for local development (matches docker-compose.yml).
# Override with: make migrate DATABASE_URL="postgres://..."
DATABASE_URL ?= postgres://tcms:tcms_pass@localhost:5432/hoa_tcms

.PHONY: help build test lint fmt dev run frontend frontend-build clean migrate migrate-revert migrate-info seed migrate-all createsuperuser gen-perms docker-up docker-build docker-down

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# ------------------------------------------------------------------
# Build
# ------------------------------------------------------------------

build: ## Build the project in release mode
	cargo build --release

build-dev: ## Build the project in debug mode
	cargo build

# ------------------------------------------------------------------
# Test
# ------------------------------------------------------------------

test: ## Run all tests (unit + integration)
	cargo test --all-features

test-unit: ## Run unit tests only
	cargo test --lib

test-integration: ## Run integration tests (requires DB + Redis)
	cargo test --test '*' -- --test-threads=1

test-race: ## Run tests with race detector
	cargo test --all-features -- --test-threads=4

coverage: ## Generate test coverage report (requires cargo-tarpaulin)
	cargo tarpaulin --out Html --output-dir target/coverage

# ------------------------------------------------------------------
# Lint & Format
# ------------------------------------------------------------------

lint: ## Run clippy lints
	cargo clippy --all-targets --all-features -- -D warnings

fmt: ## Format code with rustfmt
	cargo fmt --all

fmt-check: ## Check formatting without modifying
	cargo fmt --all -- --check

clippy: ## Alias for lint
	cargo clippy --all-targets --all-features -- -D warnings

check: ## Run both fmt-check and clippy
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings

# ------------------------------------------------------------------
# Development
# ------------------------------------------------------------------

dev: ## Start development server with hot-reload (requires cargo-watch)
	cargo watch -x "run --bin server"

dev-quiet: ## Start dev server with minimal logging
	RUST_LOG=warn cargo watch -x "run --bin server"

run: ## Start the server
	cargo run --bin server

frontend: ## Start the Leptos frontend with hot-reload (requires cargo-leptos)
	cd frontend && cargo leptos serve

# NOTE: --bin-features ssr --lib-features hydrate are required. cargo-leptos
# does not enable them by default, which produces a stub binary (empty main)
# and a WASM bundle with no hydrate entry point.
frontend-build: ## Build the Leptos frontend for production (SSR + WASM)
	cd frontend && cargo leptos build --release --bin-features ssr --lib-features hydrate

# ------------------------------------------------------------------
# Database
# ------------------------------------------------------------------

db-create: ## Create the database (requires sqlx-cli)
	@DATABASE_URL=$(DATABASE_URL) sqlx database create

db-drop: ## Drop the database (requires sqlx-cli)
	@DATABASE_URL=$(DATABASE_URL) sqlx database drop

migrate: ## Run pending database migrations
	@DATABASE_URL=$(DATABASE_URL) sqlx migrate run

migrate-revert: ## Revert the last migration
	@DATABASE_URL=$(DATABASE_URL) sqlx migrate revert

migrate-info: ## Show migration status (applied + pending)
	@DATABASE_URL=$(DATABASE_URL) sqlx migrate info

seed: ## Run seed data via Rust seeder (idempotent; safe to re-run)
	@echo "Running seed data..."
	@DATABASE_URL=$(DATABASE_URL) cargo run --bin seeder
	@echo "Seed complete."

migrate-all: migrate seed ## Run all migrations and seed data

createsuperuser: ## Create a superuser (added to System Admin group)
	@DATABASE_URL=$(DATABASE_URL) cargo run --bin createsuperuser -- \
		--username $(USERNAME) --email $(EMAIL) \
		--fullname "$(FULLNAME)"
	@# Usage: make createsuperuser USERNAME=admin EMAIL=admin@example.com FULLNAME="System Admin"

# ------------------------------------------------------------------
# Docker
# ------------------------------------------------------------------

docker-up: ## Build and start the full stack (infra + backend + frontend)
	docker compose up -d --build

docker-build: ## Build all Docker images (backend + frontend)
	docker compose build

docker-down: ## Stop all services
	docker compose down

docker-logs: ## View all service logs
	docker compose logs -f

docker-restart: ## Restart all services
	docker compose restart

# ------------------------------------------------------------------
# Clean
# ------------------------------------------------------------------

clean: ## Clean build artifacts
	cargo clean
	rm -rf target

clean-all: clean ## Clean build artifacts and node/data directories
	rm -rf storage/uploads/*

# ------------------------------------------------------------------
# Utilities
# ------------------------------------------------------------------

gen-perms: ## Generate CRUD permission YAML for a model. Usage: make gen-perms MODELS=invoice,payment
	@./scripts/gen-permissions $(shell echo $(MODELS) | tr ',' ' ')

outdated: ## Check for outdated dependencies
	cargo outdated

audit: ## Check for security vulnerabilities (requires cargo-audit)
	cargo audit

update: ## Update dependencies
	cargo update

docs: ## Build and open documentation
	cargo doc --no-deps --open
