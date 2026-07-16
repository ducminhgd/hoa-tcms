//! Project DTOs — request/response shapes for project CRUD and member management.
//!
//! Request DTOs define the expected JSON shape for project endpoints.
//! Response DTOs define the public representation of project data.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------
// Request DTOs
// ---------------------------------------------------------------

/// Create a new project.
#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_status")]
    pub status: String,
}

fn default_status() -> String {
    "ACTIVE".to_string()
}

impl CreateProjectRequest {
    /// Validate all fields, returning a list of field-level error messages.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let name = self.name.trim();
        if name.is_empty() {
            errors.push("name is required".to_string());
        } else if name.len() > 255 {
            errors.push("name must not exceed 255 characters".to_string());
        }
        if let Some(ref desc) = self.description
            && desc.len() > 2000
        {
            errors.push("description must not exceed 2000 characters".to_string());
        }
        if !matches!(self.status.to_uppercase().as_str(), "ACTIVE" | "INACTIVE") {
            errors.push("status must be ACTIVE or INACTIVE".to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Update an existing project.
///
/// All fields are optional — only provided fields will be updated.
/// At least one field must be provided.
#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}

impl UpdateProjectRequest {
    /// Validate the optional fields if they are present.
    ///
    /// Returns `true` if at least one field was provided.
    pub fn validate(&self) -> Result<bool, Vec<String>> {
        let mut errors = Vec::new();
        let mut has_field = false;

        if let Some(ref name) = self.name {
            has_field = true;
            if name.trim().is_empty() {
                errors.push("name must not be empty".to_string());
            } else if name.len() > 255 {
                errors.push("name must not exceed 255 characters".to_string());
            }
        }
        if let Some(ref desc) = self.description {
            has_field = true;
            if desc.len() > 2000 {
                errors.push("description must not exceed 2000 characters".to_string());
            }
        }
        if let Some(ref status) = self.status {
            has_field = true;
            if !matches!(status.to_uppercase().as_str(), "ACTIVE" | "INACTIVE") {
                errors.push("status must be ACTIVE or INACTIVE".to_string());
            }
        }

        if !has_field {
            errors.push("at least one field must be provided".to_string());
        }
        if errors.is_empty() {
            Ok(true)
        } else {
            Err(errors)
        }
    }
}

/// Query parameters for listing projects.
#[derive(Debug, Deserialize)]
pub struct ListProjectsQuery {
    #[serde(default)]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
    pub status: Option<String>,
}

fn default_limit() -> u32 {
    25
}

/// Bulk member management request — add and/or remove members from a project.
#[derive(Debug, Deserialize)]
pub struct ManageMembersRequest {
    #[serde(default)]
    pub add: Vec<AddMemberEntry>,
    #[serde(default)]
    pub remove: Vec<i64>,
}

/// A single member to add with a specified role.
#[derive(Debug, Deserialize)]
pub struct AddMemberEntry {
    pub user_id: i64,
    pub role: String,
}

impl ManageMembersRequest {
    /// Validate the add entries have valid roles.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        for (i, entry) in self.add.iter().enumerate() {
            if !matches!(
                entry.role.as_str(),
                "Owner" | "Editor" | "Contributor" | "Viewer"
            ) {
                errors.push(format!(
                    "add[{}].role must be one of: Owner, Editor, Contributor, Viewer",
                    i
                ));
            }
        }
        if self.add.is_empty() && self.remove.is_empty() {
            errors.push("at least one of 'add' or 'remove' must be provided".to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Change a single member's role.
#[derive(Debug, Deserialize)]
pub struct ChangeMemberRoleRequest {
    pub role: String,
}

impl ChangeMemberRoleRequest {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        if !matches!(
            self.role.as_str(),
            "Owner" | "Editor" | "Contributor" | "Viewer"
        ) {
            return Err(vec![
                "role must be one of: Owner, Editor, Contributor, Viewer".to_string(),
            ]);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------
// Response DTOs
// ---------------------------------------------------------------

/// Project data returned in list responses.
#[derive(Debug, Serialize)]
pub struct ProjectListItem {
    pub id: i64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: String,
    pub member_count: u64,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Full project data returned in single-resource responses.
#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    pub id: i64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: String,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_by: i64,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub members: Option<Vec<MemberResponse>>,
}

/// A single project member as exposed in the API.
#[derive(Debug, Serialize)]
pub struct MemberResponse {
    pub user_id: i64,
    pub username: String,
    pub fullname: String,
    pub role: String,
}

/// Summary returned after bulk member management.
#[allow(dead_code)]
pub struct ManageMembersResponse {
    pub added: Vec<MemberResponse>,
    pub removed: Vec<i64>,
}
