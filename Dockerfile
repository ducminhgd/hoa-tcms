# syntax=docker/dockerfile:1
#
# HOA TCMS — backend image.
#
# Builds the Actix-Web API server plus the `seeder` and `createsuperuser`
# CLI tools, and the `sqlx` CLI used by the one-shot `migrate` compose
# service. Produces a slim runtime image (no Rust toolchain).

# ---------------------------------------------------------------------------
# Stage 1 — Builder
# ---------------------------------------------------------------------------
FROM rust:1.97-bookworm AS builder

WORKDIR /app

# Cache dependency downloads across source edits by fetching with just the
# manifests present, then copying the full tree afterwards.
COPY Cargo.toml Cargo.lock ./
COPY cmd/ cmd/
COPY internal/ internal/
COPY pkg/ pkg/
# Frontend manifest + source so `cargo fetch` can resolve the full workspace.
COPY frontend/Cargo.toml frontend/Cargo.toml
COPY frontend/src/ frontend/src/
RUN cargo fetch

# Build the three backend binaries in release mode.
COPY . .
RUN cargo build --release --bin server --bin seeder --bin createsuperuser

# Install sqlx-cli (used by the `migrate` compose service to run migrations).
RUN cargo install sqlx-cli --no-default-features --features postgres --root /usr/local

# ---------------------------------------------------------------------------
# Stage 2 — Runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# ca-certificates for any HTTPS outbound calls; curl for the healthcheck.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/local/bin/sqlx /usr/local/bin/sqlx
COPY --from=builder /app/target/release/server /usr/local/bin/server
COPY --from=builder /app/target/release/seeder /usr/local/bin/seeder
COPY --from=builder /app/target/release/createsuperuser /usr/local/bin/createsuperuser

# Runtime config + migrations (paths default relative to WORKDIR /app).
COPY config/ /app/config/
COPY migrations/ /app/migrations/
COPY docker/migrate-entrypoint.sh /usr/local/bin/migrate-entrypoint.sh
RUN chmod +x /usr/local/bin/migrate-entrypoint.sh

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=5 \
    CMD curl -fsS http://localhost:8080/api/v1/health || exit 1

# Default command — start the API server.
CMD ["/usr/local/bin/server"]
