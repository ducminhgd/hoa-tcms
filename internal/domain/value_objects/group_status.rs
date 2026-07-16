//! `GroupStatus` value object — the lifecycle state of a group.
//!
//! Maps to the `groups.status` column (`VARCHAR(20)` with CHECK constraint
//! `IN ('ACTIVE', 'INACTIVE')`).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The lifecycle status of a [`Group`](crate::domain::entities::group::Group).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupStatus {
    /// Group is active and its members can be assigned to roles, projects, etc.
    Active,

    /// Group is inactive — it still exists but no new assignments are allowed.
    Inactive,
}

impl fmt::Display for GroupStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GroupStatus::Active => write!(f, "ACTIVE"),
            GroupStatus::Inactive => write!(f, "INACTIVE"),
        }
    }
}

impl FromStr for GroupStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "ACTIVE" => Ok(GroupStatus::Active),
            "INACTIVE" => Ok(GroupStatus::Inactive),
            other => Err(format!("invalid group status: {other}")),
        }
    }
}

impl Serialize for GroupStatus {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for GroupStatus {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        GroupStatus::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_active() {
        assert_eq!(GroupStatus::Active.to_string(), "ACTIVE");
    }

    #[test]
    fn display_inactive() {
        assert_eq!(GroupStatus::Inactive.to_string(), "INACTIVE");
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!(
            "ACTIVE".parse::<GroupStatus>().unwrap(),
            GroupStatus::Active
        );
        assert_eq!(
            "active".parse::<GroupStatus>().unwrap(),
            GroupStatus::Active
        );
        assert_eq!(
            "Active".parse::<GroupStatus>().unwrap(),
            GroupStatus::Active
        );
        assert_eq!(
            "INACTIVE".parse::<GroupStatus>().unwrap(),
            GroupStatus::Inactive
        );
        assert_eq!(
            "inactive".parse::<GroupStatus>().unwrap(),
            GroupStatus::Inactive
        );
    }

    #[test]
    fn parse_invalid() {
        assert!("unknown".parse::<GroupStatus>().is_err());
        assert!("".parse::<GroupStatus>().is_err());
    }

    #[test]
    fn serde_round_trip() {
        let original = GroupStatus::Active;
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, "\"ACTIVE\"");
        let deserialized: GroupStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, original);
    }

    #[test]
    fn partial_eq() {
        assert_eq!(GroupStatus::Active, GroupStatus::Active);
        assert_ne!(GroupStatus::Active, GroupStatus::Inactive);
    }
}
