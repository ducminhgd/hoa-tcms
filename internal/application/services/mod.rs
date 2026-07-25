//! Application services — domain-agnostic infrastructure abstractions.
//!
//! These traits define **ports** for cross-cutting concerns that the
//! application layer needs but does not implement:
//!
//! * `password_hasher` — password hashing and verification.
//! * `session_store` — session persistence (Redis, in-memory, etc.).
//! * `permission_resolver` — aggregation of direct and inherited permissions.
//! * `authorization` — high-level permission checking with admin bypass.
//!
//! Implementations of these traits live in the infrastructure layer
//! (e.g. `infrastructure::auth`, `infrastructure::cache`).

pub mod authorization;
pub mod errors;
pub mod password_hasher;
pub mod permission_resolver;
pub mod project_service;
pub mod session_store;
pub mod test_case_file_service;
pub mod test_execution_service;
