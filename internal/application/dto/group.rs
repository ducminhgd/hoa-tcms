//! Group DTOs — data transfer objects for group CRUD and membership management.
//!
//! Groups are collections of users with shared role assignments. These DTOs
//! define the API contract for group management endpoints.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------
// Request DTOs
// ---------------------------------------------------------------

/// Create a new group.
#[derive(Debug, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    pub description: Option<String>,
}

/// Update an existing group.
///
/// All fields are optional — only provided fields will be updated.
#[derive(Debug, Deserialize)]
pub struct UpdateGroupRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}

/// Replace the member list of a group (full replacement).
#[derive(Debug, Deserialize)]
pub struct ManageMembersRequest {
    pub user_ids: Vec<i64>,
}

/// Replace the role assignments of a group (full replacement).
#[derive(Debug, Deserialize)]
pub struct ManageRolesRequest {
    pub role_ids: Vec<i64>,
}

impl CreateGroupRequest {
    /// Validate that required fields are present and within allowed lengths.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.name.trim().is_empty() {
            errors.push("name is required".to_string());
        } else if self.name.len() > 255 {
            errors.push("name must not exceed 255 characters".to_string());
        }
        if let Some(ref description) = self.description
            && description.len() > 2000
        {
            errors.push("description must not exceed 2000 characters".to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl UpdateGroupRequest {
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
        if let Some(ref description) = self.description
            && description.len() > 2000
        {
            errors.push("description must not exceed 2000 characters".to_string());
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

/// Group information returned in list responses.
#[derive(Debug, Serialize)]
pub struct GroupResponse {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Detailed group information including members and role assignments.
#[derive(Debug, Serialize)]
pub struct GroupDetailResponse {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub members: Vec<MemberInfo>,
    pub roles: Vec<RoleInfo>,
}

/// Lightweight member information embedded in group details.
#[derive(Debug, Serialize)]
pub struct MemberInfo {
    pub user_id: i64,
    pub username: String,
}

/// Lightweight role information embedded in group details.
#[derive(Debug, Serialize)]
pub struct RoleInfo {
    pub role_id: i64,
    pub name: String,
}
