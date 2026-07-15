# Tasks: IAM CLI Init

- [ ] 1. Define the `SEED_PERMISSIONS` constant array in a shared module — `design.md` (Data Model: Permission catalog)
- [ ] 2. Implement `SchemaVerifier` — checks that required tables (`users`, `roles`, `permissions`) exist before init — `design.md#Sequence`, `requirements.md#US-04`
- [ ] 3. Implement `PermissionSeeder` — reads `SEED_PERMISSIONS`, inserts missing permissions, returns all IDs — `design.md#Components`, `design.md#Sequence` (step 7)
- [ ] 4. Implement `RoleSeeder` — creates or finds "System Admin" role, syncs all permissions — `design.md#Components`, `design.md#Sequence` (step 8)
- [ ] 5. Define `InitUseCase` interface/struct in the application layer — `design.md#Components`
- [ ] 6. Implement `InitUseCase::execute()` — orchestrates full init flow inside a single DB transaction — `design.md#Sequence`, `requirements.md#US-01`
- [ ] 7. Implement idempotency logic in `InitUseCase` — skip/restore checks for permissions, role, and admin user — `design.md#Sequence` (idempotent re-run), `requirements.md#US-02`
- [ ] 8. Implement `InitCommand` CLI definition with clap — flags: `--username`, `--email`, `--password`, `--full-name`, `--db-url`, `--yes` — `design.md#CLI Contract`
- [ ] 9. Implement interactive prompting for missing credentials when a TTY is available (use `dialoguer` / `rpassword` crates) — `design.md#CLI Contract`, `requirements.md#US-01`
- [ ] 10. Implement `InitOutput` — format success and error messages for CLI output — `design.md#CLI Contract`, `design.md#Error Handling`
- [ ] 11. Implement input validation for username, email, password, full name — `design.md#Sequence` (step 3), `requirements.md#US-03`
       - Username: non-empty, max 255 chars, trimmed.
       - Email: use a proper RFC 5322-compatible validation crate (`validator` or `email`).
         Reject `@example.com`, `user@`, `user@.com`, `user@example` (missing TLD).
       - Password: minimum 8 chars AND check against a hardcoded list of 100+ common/weak
         passwords (e.g., "password", "12345678", "admin123"). Load from a constant array.
       - Full name: non-empty, max 255 chars, trimmed. Strip HTML tags to prevent XSS.
- [ ] 12. Register the `init` subcommand on the `tcms` binary in `main.rs` — `design.md#Architecture`
- [ ] 13. Wire database connection pool creation for the init command (reuse app config) — `design.md#Architecture`, `requirements.md#US-04`
- [ ] 14. Implement init flow error handling: DB connection failures, missing schema, conflicts, unexpected errors with appropriate exit codes — `design.md#Error Handling`, `requirements.md#US-04`
- [ ] 15. Write unit tests for `InitUseCase` (mocked repositories) covering: first-run success, idempotent re-run, input validation edge cases, conflict detection — `requirements.md#US-01`, `US-02`, `US-03`
- [ ] 16. Write integration tests for `PermissionSeeder` and `RoleSeeder` against a real PostgreSQL test database — `design.md#Components`
- [ ] 17. Write end-to-end test: run `tcms init` against an empty database, verify permissions table, System Admin role, admin user existence with correct `created_by=NULL` — `requirements.md#US-01`
- [ ] 18. Write end-to-end test: run `tcms init` twice, verify idempotent behavior (no duplicate rows, exit code 0) — `requirements.md#US-02`
- [ ] 19. Write end-to-end test: verify `created_by` is `NULL` on the bootstrap admin user row — `requirements.md#US-01`, `design.md#Data Model`
- [ ] 20. Write end-to-end test: verify password hash format matches `pbkdf2$<algorithm>$<salt>$<iterations>$<hash>` — `requirements.md#US-01`
