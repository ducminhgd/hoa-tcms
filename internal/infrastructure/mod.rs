//! Infrastructure layer — framework and driver implementations.
//!
//! This layer contains concrete implementations of repository interfaces,
//! Redis session storage, configuration loading, and the database connection pool.

pub mod config;
pub mod crypto;
pub mod db;
pub mod postgres;
pub mod redis;
pub mod storage;
