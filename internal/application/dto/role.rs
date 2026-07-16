//! Role DTOs — data transfer objects for role CRUD and permission assignment.
//!
//! Roles are named sets of permissions that can be assigned to groups or
//! directly to users. These DTOs define the API contract for role management.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::permission::PermissionDTO;

// ---------------------------------------------------------------
// Request DTOs
// ---------------------------------------------------------------

/// Create a new role with an initial set of permissions.
#[derive(Debug, Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub permission_ids: Vec<i64>,
}

/// Update an existing role's name and/or permission set.
///
/// All fields are optional — only provided fields will be updated.
/// Passing `Some(vec![])` for `permission_ids` clears all permissions.
#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub permission_ids: Option<Vec<i64>>,
}

impl CreateRoleRequest {
    /// Validate that required fields are present and within allowed lengths.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.name.trim().is_empty() {
            errors.push("name is required".to_string());
        } else if self.name.len() > 255 {
            errors.push("name must not exceed 255 characters".to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl UpdateRoleRequest {
    /// Validate optional fields if they are present.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if let Some(ref name) = self.name {
            if name.trim().is_empty() {
                errors.push("name must not be empty if provided".to_string());
            } else if name.len() > 255 {
                errors.push("name must not exceed 255 characters".to_string());
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ---------------------------------------------------------------
// Response DTOs
// ---------------------------------------------------------------

/// Role information returned in list responses.
#[derive(Debug, Serialize)]
pub struct RoleResponse {
    pub id: i64,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Detailed role information including associated permissions.
#[derive(Debug, Serialize)]
pub struct RoleDetailResponse {
    pub id: i64,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub permissions: Vec<PermissionDTO>,
}
