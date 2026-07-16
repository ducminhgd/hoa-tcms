//! Project status value object.
//!
//! Represents the lifecycle state of a project: `Active` (visible and
//! usable) or `Inactive` (hidden from views, all child objects hidden).

use std::fmt;

/// The lifecycle state of a project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectStatus {
    /// Project is active — visible in lists and selection fields.
    Active,
    /// Project is inactive — hidden from lists and selection fields,
    /// but data and references are preserved.
    Inactive,
}

impl ProjectStatus {
    /// Parse a string into a `ProjectStatus`.
    ///
    /// Matching is case-insensitive. Returns `None` for unrecognised values.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "ACTIVE" => Some(Self::Active),
            "INACTIVE" => Some(Self::Inactive),
            _ => None,
        }
    }

    /// Return the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Inactive => "INACTIVE",
        }
    }
}

impl fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
