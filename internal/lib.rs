//! HOA TCMS — Library root (core crate).
//!
//! This crate implements the Clean Architecture layers:
//!
//! - **Domain**      — Entities, value objects, domain errors (innermost).
//! - **Application** — Use cases, repository interfaces, DTOs.
//! - **Adapters**    — HTTP handlers, middleware, messaging.
//! - **Infrastructure** — PostgreSQL repositories, Redis, config loading.
//!
//! Shared utilities live in the `hoa-tcms-pkg` crate.

pub mod adapters;
pub mod application;
pub mod domain;
pub mod infrastructure;

use actix_web::web;

/// Register all application routes on the given `ServiceConfig`.
///
/// Called by `cmd/server/main.rs` when building the `HttpServer`.
pub fn configure_app(cfg: &mut web::ServiceConfig) {
    adapters::http::router::configure(cfg);
}
