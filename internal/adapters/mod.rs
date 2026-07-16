//! Interface adapters — HTTP handlers, middleware, and messaging.
//!
//! This layer translates between external concerns (HTTP requests/responses)
//! and the application layer. Handlers call use cases; they do not contain
//! business logic.

pub mod http;
pub mod messaging;
