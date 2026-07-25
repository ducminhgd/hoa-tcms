//! DTOs for object sharing.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateShareRequest {
    pub user_id: i64,
    pub resource_type: String,
    pub resource_id: i64,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateShareRequest {
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct ObjectSharingResponse {
    pub id: i64,
    pub user_id: i64,
    pub username: String,
    pub fullname: String,
    pub resource_type: String,
    pub resource_id: i64,
    pub role: String,
    pub created_by: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
