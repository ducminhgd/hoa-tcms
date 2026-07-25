//! `FileStorage` trait — abstract persistence for uploaded files.
//!
//! Defines the port for storing, reading, checking existence, and deleting
//! files on the storage backend. The initial implementation uses the local
//! filesystem (`LocalFileStorage`). Future backends (S3, GCS, etc.) can
//! implement this trait without changing application or domain code.

use async_trait::async_trait;

use crate::application::repositories::RepositoryResult;

/// Abstract file storage backend.
///
/// All paths are relative to the storage root and use `/` as the path
/// separator regardless of the underlying operating system.
#[async_trait]
pub trait FileStorage: Send + Sync {
    /// Store a file at the given relative path.
    ///
    /// Creates parent directories if they do not exist. The write is atomic
    /// on supported backends (temp file + rename).
    async fn store(&self, relative_path: &str, data: &[u8]) -> RepositoryResult<()>;

    /// Check whether a file exists at the given relative path.
    async fn exists(&self, relative_path: &str) -> RepositoryResult<bool>;

    /// Open a file for reading.
    ///
    /// Returns a boxed `Read + Send` handle. The caller is responsible for
    /// reading (or streaming) the contents.
    fn open_read(&self, relative_path: &str) -> RepositoryResult<Box<dyn std::io::Read + Send>>;

    /// Delete a file from the storage backend.
    ///
    /// Returns `Ok(())` if the file was deleted or did not exist.
    async fn delete(&self, relative_path: &str) -> RepositoryResult<()>;

    /// Resolve a relative storage path to an absolute filesystem path.
    ///
    /// Used for streaming downloads via `NamedFile` so the entire file is
    /// never buffered in memory. Only called internally — the resolved path
    /// is **never** returned to API clients.
    fn resolve_path(&self, relative_path: &str) -> String;
}
