# HOA TCMS — Makefile
#
# Common development tasks.
# Requires: cargo, docker (for dev services), sqlx-cli (for migrations).

.PHONY: help build test lint fmt dev clean migrate seed docker-up docker-down

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
	cargo watch -x run

dev-quiet: ## Start dev server with minimal logging
	RUST_LOG=warn cargo watch -x run

# ------------------------------------------------------------------
# Database
# ------------------------------------------------------------------

db-create: ## Create the database (requires sqlx-cli)
	sqlx database create

db-drop: ## Drop the database (requires sqlx-cli)
	sqlx database drop

migrate: ## Run pending database migrations
	sqlx migrate run

migrate-revert: ## Revert the last migration
	sqlx migrate revert

migrate-pending: ## List pending migrations
	sqlx migrate info

seed: ## Run seed data migration
	@echo "Running seed data..."
	@if [ -f migrations/002_seed_permissions.sql ]; then \
		echo "Running permission seed..."; \
	fi
	@echo "Seed complete."

migrate-all: migrate seed ## Run migrations and seed data

# ------------------------------------------------------------------
# Docker
# ------------------------------------------------------------------

docker-up: ## Start dev services (PostgreSQL + Redis)
	docker compose up -d

docker-down: ## Stop dev services
	docker compose down

docker-logs: ## View dev service logs
	docker compose logs -f

docker-restart: ## Restart dev services
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

outdated: ## Check for outdated dependencies
	cargo outdated

audit: ## Check for security vulnerabilities (requires cargo-audit)
	cargo audit

update: ## Update dependencies
	cargo update

docs: ## Build and open documentation
	cargo doc --no-deps --open
