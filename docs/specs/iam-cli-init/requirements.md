# Feature: IAM CLI Init

## Overview

Provide a CLI bootstrap command (`tcms init`) that seeds the initial permission catalog, creates
the built-in "System Admin" role with all permissions, and creates the first System Admin user.
This is the entry point for any fresh deployment — the system cannot be used until initialization
has completed.

## User Stories

### US-01: Bootstrap System Admin from CLI

As a system administrator deploying the HOA TCMS for the first time, I want to run a CLI command
that creates the first System Admin user, so that I can log in and provision the rest of the
system.

**Acceptance Criteria (EARS)**

- WHEN the `tcms init` command is run against an empty database, THE SYSTEM SHALL seed the
  full permission catalog, create the "System Admin" role with all permissions, prompt or accept
  credentials for the first admin user, create that user with the System Admin role, and exit with
  a `0` exit code and a success message.
- WHEN the admin user is created, THE SYSTEM SHALL set the user's `created_by` and `updated_by`
  columns to `NULL` — the bootstrap admin has no creator (per PRD SS6.4).
- WHEN the admin user is created, THE SYSTEM SHALL hash the password using PBKDF2 (the same
  `PasswordHasher` used by `iam-auth`) and store the hash in the `password_hash` column in
  the format `pbkdf2$<algorithm>$<salt>$<iterations>$<hash>`.
- WHEN the `tcms init` command completes successfully, THE SYSTEM SHALL output a success message
  that includes the username and a hint to log in.
- IF the `tcms init` command is run without any required credential flags AND no interactive
  terminal is available, THE SYSTEM SHALL exit with a non-zero code and an error message
  explaining that credentials must be provided via flags in non-interactive mode.

### US-02: Idempotent Re-runs

As a system administrator, I want to re-run the `tcms init` command without causing errors or
duplicate data, so that automated deployment scripts can safely call it multiple times.

**Acceptance Criteria (EARS)**

- WHEN `tcms init` is run a second time AND all seed data (permissions, System Admin role) and
  at least one System Admin user already exist, THE SYSTEM SHALL exit with a `0` exit code and
  output a message indicating that the system is already initialized — no changes made.
- WHEN `tcms init` is run again AND the permission catalog is missing new permissions that were
  added in a later deployment, THE SYSTEM SHALL insert only the new permissions and grant them
  to the System Admin role — no duplicate entries and no removal of existing permissions.
- WHEN `tcms init` is run again AND the System Admin role exists but has been modified (e.g.,
  some permissions removed), THE SYSTEM SHALL restore all permissions to the System Admin role.

### US-03: Input Validation

As a system administrator, I want the `tcms init` command to validate all credential input, so
that the first admin user has valid data and a strong password.

**Acceptance Criteria (EARS)**

- IF the provided `username` is empty, contains only whitespace, or exceeds 255 characters,
  THE SYSTEM SHALL reject the input and output a clear validation error message.
- IF the provided `email` is not a syntactically valid email address (checked via a proper
  email validation regex that verifies local-part, `@` symbol, domain with at least one dot,
  and a valid TLD), THE SYSTEM SHALL reject the input and output a clear validation error
  message. The basic "contains @" check is insufficient — it must reject values like
  `user@`, `@domain`, and `user@.com`.
- IF the provided `password` is fewer than 8 characters, THE SYSTEM SHALL reject the input
  and output a clear validation error message.
- IF the provided `password` is a common or weak password (e.g., "password", "12345678",
  "admin123"), THE SYSTEM SHALL reject the input with a message indicating the password is
  too weak — a minimum length check alone is insufficient against dictionary-based attacks.
- IF the provided `full-name` is empty or exceeds 255 characters, THE SYSTEM SHALL reject the
  input and output a clear validation error message.
- IF `username` or `email` conflicts with an existing user in the database, THE SYSTEM SHALL
  output a clear error message indicating the conflict — this can happen if a user was manually
  inserted and the init command creates the admin afterward.
- WHEN all input passes validation, THE SYSTEM SHALL proceed with the user creation.

### US-04: Connection and Startup Failures

As a system administrator, I want clear error messages when the CLI cannot connect to the
database or encounters an unexpected failure, so that I can diagnose and resolve the issue.

**Acceptance Criteria (EARS)**

- IF the CLI cannot connect to the PostgreSQL database (connection string missing, unreachable
  host, invalid credentials, database does not exist), THE SYSTEM SHALL exit with a non-zero
  exit code and output a descriptive error message identifying the cause.
- IF a database migration is required before initialization (e.g., the USERS or ROLES tables
  do not exist), THE SYSTEM SHALL exit with a non-zero exit code and output a message indicating
  that migrations must be run first.
- IF an unexpected runtime error occurs during seeding or user creation, THE SYSTEM SHALL exit
  with a non-zero exit code and output the error message without exposing stack traces or
  sensitive data.

## Out of Scope

- **Interactive terminal UI beyond basic prompts:** The command accepts either flags or prompts
  for credentials. No TUI framework, no colored wizard.
- **Creating additional users beyond the first admin:** Only one admin is created by the init
  command. Additional users are created via the HTTP API (`iam-users`).
- **Seeding per-project metadata (categories, priorities, templates):** Per-project metadata is
  seeded when the first project is created via the API, not at system init time.
- **Password rotation policies or expiry:** The init command does not set password expiry.
- **Email verification or welcome messages:** No SMTP dependency — the init command does not
  send any email.
- **Running database migrations:** The init command assumes migrations have been applied
  already. It does not create tables or run `CREATE DATABASE`.
- **Credentials in command-line history (plaintext):** The init command should warn if the
  password is provided via `--password` flag, as it will be visible in the shell history.
  Interactive prompt (hidden input) is preferred.
- **Azure/Key Vault or external secret store integration:** No external secret manager for
  the bootstrap admin credentials.

## Dependencies

- **Database schema (migrations):** The USERS, ROLES, PERMISSIONS, ROLE_PERMISSIONS, and
  USER_ROLES tables must exist before `tcms init` runs.
- **`PasswordHasher` interface** from `iam-auth` — reused for PBKDF2 password hashing with the
  same hash format.
- **`PermissionRepository` / permission seeding logic** from `iam-permissions` — the permission
  catalog must be defined (ideally as a shared module) so the init command can seed it.
- **`RoleRepository`** from `iam-roles` — used to create the System Admin role and assign
  all permissions to it.
- **`UserRepository`** from `iam-users` — used to create the admin user.
- **Database connection configuration** — shared with the main application (same connection
  string or config file).
