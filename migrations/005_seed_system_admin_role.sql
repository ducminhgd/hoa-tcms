-- ==========================================================================
-- Migration: 005_seed_system_admin_role (NO-OP)
-- ==========================================================================
-- The System Admin role and its permission assignments are now seeded at
-- application startup by the Rust `ConfigFileSetupSeeder`
-- (see `config/default-setup.yaml`).
--
-- This migration file is preserved as a no-op to maintain the migration
-- chain for existing databases.
--
-- See `internal/infrastructure/config/setup_seeder.rs` for the
-- current implementation.

SELECT 1;
