//! PostgreSQL repository implementations.
//!
//! Each module implements the corresponding repository interface
//! defined in `application::repositories`.

pub mod admin_bypass_repository;
pub mod group_repository;
pub mod group_role_repository;
pub mod member_repository;
pub mod permission_repository;
pub mod permission_resolver;
pub mod project_member_repository;
pub mod project_repository;
pub mod role_repository;
pub mod test_case_result_repository;
pub mod test_execution_repository;
pub mod user_repository;
