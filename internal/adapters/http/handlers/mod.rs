//! HTTP handlers — one module per resource.
//!
//! Handlers parse requests, call use cases, and format responses. They
//! contain **no business logic** — only translation and validation.

pub mod project_handler;
pub mod test_case_file_handler;
pub mod test_execution_handler;
pub mod test_plan_handler;
