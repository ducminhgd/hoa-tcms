-- Migration: 006_add_projects_name_unique_index
-- Description: Add case-insensitive unique constraint on projects.name
-- This prevents TOCTOU race conditions in project creation where two concurrent
-- requests with the same project name both pass the application-level check.

BEGIN;

CREATE UNIQUE INDEX uq_projects_name_lower ON projects (LOWER(name));

COMMIT;
