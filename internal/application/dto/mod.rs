//! Data Transfer Objects — request/response shapes for crossing layer boundaries.
//!
//! DTOs are plain structs with serde derives. They are **not** domain entities;
//! they represent input/output contracts.

pub mod auth;
pub mod group;
pub mod permission;
pub mod project;
pub mod role;
pub mod test_execution;
pub mod user;
