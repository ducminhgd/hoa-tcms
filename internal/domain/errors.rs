//! Domain errors — typed errors for business-rule violations.

use thiserror::Error;

/// Generic domain error.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// The requested entity was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// A uniqueness constraint was violated.
    #[error("duplicate value: {0}")]
    Duplicate(String),

    /// An operation was attempted on an entity in an invalid state.
    #[error("invalid state: {0}")]
    InvalidState(String),

    /// Validation failed.
    #[error("validation error: {0}")]
    Validation(String),

    /// The caller lacks permission.
    #[error("forbidden: {0}")]
    Forbidden(String),

    /// The operation is not supported.
    #[error("unsupported operation: {0}")]
    Unsupported(String),

    // ── IAM variants (Milestone 2) ──────────────────────────────────────
    /// Authentication failed; wrong username or password.
    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),

    /// User account is deactivated.
    #[error("user inactive: {0}")]
    UserInactive(String),

    /// Session not found or expired.
    #[error("session expired: {0}")]
    SessionExpired(String),

    /// Cannot modify a protected role (e.g. System Admin).
    #[error("role protected: {0}")]
    RoleProtected(String),

    // ── Project variants (Milestone 3) ────────────────────────────────────
    /// Duplicate project name.
    #[error("duplicate project name: {0}")]
    DuplicateProjectName(String),

    /// Project not found.
    #[error("project not found: {0}")]
    ProjectNotFound(String),

    /// Last owner cannot be removed or downgraded.
    #[error("cannot remove last owner: {0}")]
    LastOwnerCannotBeRemoved(String),

    /// User is not a member of the project.
    #[error("not a member of project: {0}")]
    NotProjectMember(String),

    // ── File variants (Milestone 5) ────────────────────────────────────────
    /// File not found.
    #[error("file not found: {0}")]
    FileNotFound(String),

    /// File exceeds the maximum allowed size.
    #[error("file too large: actual={0}, max={1}")]
    FileTooLarge(u64, u64),

    /// File MIME type is not in the configured allowlist.
    #[error("file type not allowed: {0}")]
    FileTypeNotAllowed(String),

    /// File storage backend error.
    #[error("file storage error: {0}")]
    FileStorageError(String),

    /// Invalid file name (empty, too long, or path traversal).
    #[error("invalid file name: {0}")]
    InvalidFileName(String),
}
