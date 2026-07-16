//! Permission DTOs — data transfer objects for permission-related operations.
//!
//! Permissions are a **read-only** reference table seeded by migrations.
//! There are no create/update request DTOs — permissions are never modified
//! through the application API.

use serde::Serialize;

/// Permission data returned in API responses.
#[derive(Debug, Serialize)]
pub struct PermissionDTO {
    pub id: i64,
    pub name: String,
    pub code: String,
}
