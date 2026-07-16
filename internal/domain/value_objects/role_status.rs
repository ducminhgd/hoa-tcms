//! `RoleStatus` value object — the lifecycle state of a role.
//!
//! Maps to the `roles.status` column (`VARCHAR(20)` with CHECK constraint
//! `IN ('ACTIVE', 'INACTIVE')`).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The lifecycle status of a [`Role`](crate::domain::entities::role::Role).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleStatus {
    /// Role is active and its permissions can be granted to users/groups.
    Active,

    /// Role is inactive — it still exists but its permissions are not
    /// included in effective permission resolution.
    Inactive,
}

impl fmt::Display for RoleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoleStatus::Active => write!(f, "ACTIVE"),
            RoleStatus::Inactive => write!(f, "INACTIVE"),
        }
    }
}

impl FromStr for RoleStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "ACTIVE" => Ok(RoleStatus::Active),
            "INACTIVE" => Ok(RoleStatus::Inactive),
            other => Err(format!("invalid role status: {other}")),
        }
    }
}

impl Serialize for RoleStatus {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for RoleStatus {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        RoleStatus::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_active() {
        assert_eq!(RoleStatus::Active.to_string(), "ACTIVE");
    }

    #[test]
    fn display_inactive() {
        assert_eq!(RoleStatus::Inactive.to_string(), "INACTIVE");
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!("ACTIVE".parse::<RoleStatus>().unwrap(), RoleStatus::Active);
        assert_eq!("active".parse::<RoleStatus>().unwrap(), RoleStatus::Active);
        assert_eq!("Active".parse::<RoleStatus>().unwrap(), RoleStatus::Active);
        assert_eq!(
            "INACTIVE".parse::<RoleStatus>().unwrap(),
            RoleStatus::Inactive
        );
        assert_eq!(
            "inactive".parse::<RoleStatus>().unwrap(),
            RoleStatus::Inactive
        );
    }

    #[test]
    fn parse_invalid() {
        assert!("unknown".parse::<RoleStatus>().is_err());
        assert!("".parse::<RoleStatus>().is_err());
    }

    #[test]
    fn serde_round_trip() {
        let original = RoleStatus::Active;
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, "\"ACTIVE\"");
        let deserialized: RoleStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, original);
    }

    #[test]
    fn partial_eq() {
        assert_eq!(RoleStatus::Active, RoleStatus::Active);
        assert_ne!(RoleStatus::Active, RoleStatus::Inactive);
    }
}
