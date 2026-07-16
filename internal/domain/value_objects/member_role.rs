//! Project member role value object.
//!
//! Defines the four project membership roles that determine what actions
//! a user can perform on project-scoped objects.

use std::fmt;
use std::str::FromStr;

/// The role a user has on a project.
///
/// Roles are ordered by permission level: Owner > Editor > Contributor > Viewer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemberRole {
    /// Read-only access. Cannot modify or share.
    Viewer,
    /// Can modify project objects but cannot share.
    Contributor,
    /// Can modify project objects and share them.
    Editor,
    /// Full control — can manage members, change settings, delete the project.
    Owner,
}

impl MemberRole {
    /// Parse a string into a `MemberRole`.
    ///
    /// Matching is case-insensitive. Returns `None` for unrecognised values.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "viewer" => Some(Self::Viewer),
            "contributor" => Some(Self::Contributor),
            "editor" => Some(Self::Editor),
            "owner" => Some(Self::Owner),
            _ => None,
        }
    }

    /// Return the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Viewer => "Viewer",
            Self::Contributor => "Contributor",
            Self::Editor => "Editor",
            Self::Owner => "Owner",
        }
    }

    /// Returns `true` if this role grants edit permissions on project objects.
    pub fn can_edit(&self) -> bool {
        matches!(self, Self::Owner | Self::Editor | Self::Contributor)
    }

    /// Returns `true` if this role grants sharing permissions.
    pub fn can_share(&self) -> bool {
        matches!(self, Self::Owner | Self::Editor)
    }

    /// Returns `true` if this role can manage project members.
    pub fn can_manage_members(&self) -> bool {
        matches!(self, Self::Owner)
    }
}

impl fmt::Display for MemberRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Error returned when parsing an invalid role string.
#[derive(Debug, Clone)]
pub struct InvalidMemberRole(pub String);

impl fmt::Display for InvalidMemberRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid member role: {}", self.0)
    }
}

impl std::error::Error for InvalidMemberRole {}

impl FromStr for MemberRole {
    type Err = InvalidMemberRole;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| InvalidMemberRole(s.to_string()))
    }
}
