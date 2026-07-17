//! HTTP handlers — one module per resource.
//!
//! Handlers parse requests, call use cases, and format responses. They
//! contain **no business logic** — only translation and validation.

pub mod project_handler;
