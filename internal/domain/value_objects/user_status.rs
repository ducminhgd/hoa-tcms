//! `UserStatus` value object — domain-level user account status.
//!
//! Maps to the `status` column in the `users` table, constrained to
//! `'ACTIVE'` or `'INACTIVE'`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The status of a user account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStatus {
    /// Account is active and usable.
    #[serde(rename = "ACTIVE")]
    Active,
    /// Account is inactive (disabled); login is blocked.
    #[serde(rename = "INACTIVE")]
    Inactive,
}

impl fmt::Display for UserStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "ACTIVE"),
            Self::Inactive => write!(f, "INACTIVE"),
        }
    }
}

impl FromStr for UserStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "ACTIVE" => Ok(Self::Active),
            "INACTIVE" => Ok(Self::Inactive),
            _ => Err(format!(
                "invalid UserStatus: '{s}' — expected 'ACTIVE' or 'INACTIVE'"
            )),
        }
    }
}
