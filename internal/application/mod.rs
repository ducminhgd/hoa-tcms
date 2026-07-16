//! Application layer — use cases, repository interfaces, and DTOs.
//!
//! This layer contains **orchestration logic** (use cases) and defines
//! **ports** (repository interfaces) that the infrastructure layer implements.
//! It depends only on the domain layer.

pub mod dto;
pub mod repositories;
pub mod services;
pub mod use_cases;
