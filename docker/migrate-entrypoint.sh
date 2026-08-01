#!/bin/sh
# HOA TCMS — one-shot migration + seed entrypoint.
#
# Used by the `migrate` compose service. Applies pending sqlx migrations and
# seeds reference data (permissions, roles, groups), then exits so the backend
# can start behind `condition: service_completed_successfully`.
set -e

# `sqlx migrate info` connects to the database, so it doubles as a readiness
# probe. (docker-compose already gates on postgres being healthy; this is
# defense-in-depth against a restart mid-run.)
echo "[migrate] waiting for PostgreSQL to accept connections..."
until sqlx migrate info --database-url "$DATABASE_URL" >/dev/null 2>&1; do
  sleep 1
done

echo "[migrate] applying migrations..."
sqlx migrate run --database-url "$DATABASE_URL"

echo "[migrate] seeding reference data..."
/usr/local/bin/seeder

echo "[migrate] complete."
