//! Domain entities — core business objects.
//!
//! Each entity represents a first-class concept in the HOA TCMS domain.
//! Entities are plain Rust structs with **no ORM or framework imports**.

pub mod group;
pub mod permission;
pub mod project;
pub mod role;
pub mod session;
pub mod test_case_file;
pub mod test_case_result;
pub mod test_execution;
pub mod user;
