//! Domain entities — core business objects.
//!
//! Each entity represents a first-class concept in the HOA TCMS domain.
//! Entities are plain Rust structs with **no ORM or framework imports**.

pub mod group;
pub mod permission;
pub mod project;
pub mod role;
pub mod session;
pub mod user;
