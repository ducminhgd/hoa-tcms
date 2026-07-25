//! Local filesystem implementation of the [`FileStorage`] trait.
//!
//! Files are stored under a configurable base directory with restricted
//! permissions:
//!
//! - Directories: `0750` (owner rwx, group r-x, others ---)
//! - Files: `0640` (owner rw-, group r--, others ---)
//!
//! Writes are atomic: data is written to a temporary file in the same
//! directory, then atomically renamed to the target path.

use std::fs;
use std::io::Read;
use std::path::PathBuf;

use async_trait::async_trait;

use crate::application::repositories::file_storage::FileStorage;
use crate::application::repositories::{RepositoryError, RepositoryResult};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Local-filesystem-backed [`FileStorage`].
pub struct LocalFileStorage {
    base_dir: PathBuf,
}

impl LocalFileStorage {
    /// Create a new `LocalFileStorage` rooted at `base_dir`.
    ///
    /// The base directory is created with `0750` permissions if it does not
    /// already exist.
    pub fn new(base_dir: &str) -> Self {
        let path = PathBuf::from(base_dir);
        if let Err(e) = fs::create_dir_all(&path) {
            tracing::warn!(
                base_dir = %base_dir,
                error = %e,
                "failed to create storage base directory; it may already exist"
            );
        }
        Self { base_dir: path }
    }

    /// Resolve a relative storage path to an absolute filesystem path.
    fn resolve(&self, relative_path: &str) -> PathBuf {
        self.base_dir.join(relative_path)
    }
}

#[async_trait]
impl FileStorage for LocalFileStorage {
    async fn store(&self, relative_path: &str, data: &[u8]) -> RepositoryResult<()> {
        let target = self.resolve(relative_path);

        // Create parent directories with 0750 permissions.
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                RepositoryError::Database(format!("failed to create directory: {}", e))
            })?;

            #[cfg(unix)]
            let _ = fs::set_permissions(parent, std::fs::Permissions::from_mode(0o750));
        }

        // Write to a temporary file in the same directory, then atomic rename.
        let tmp_ext = format!(".{}.tmp", std::process::id());
        let temp_path = target.with_extension(
            target
                .extension()
                .map(|e| {
                    let mut s = e.to_string_lossy().to_string();
                    s.push_str(&tmp_ext);
                    s
                })
                .unwrap_or_else(|| tmp_ext.trim_start_matches('.').to_string()),
        );

        fs::write(&temp_path, data)
            .map_err(|e| RepositoryError::Database(format!("failed to write temp file: {}", e)))?;

        // Set file permissions to 0640.
        #[cfg(unix)]
        let _ = fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o640));

        // Atomic rename.
        fs::rename(&temp_path, &target)
            .map_err(|e| RepositoryError::Database(format!("failed to rename file: {}", e)))?;

        Ok(())
    }

    async fn exists(&self, relative_path: &str) -> RepositoryResult<bool> {
        let path = self.resolve(relative_path);
        match fs::metadata(&path) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(RepositoryError::Database(format!(
                "failed to check file existence: {}",
                e
            ))),
        }
    }

    fn open_read(&self, relative_path: &str) -> RepositoryResult<Box<dyn Read + Send>> {
        let path = self.resolve(relative_path);
        let file = fs::File::open(&path).map_err(|e| {
            RepositoryError::Database(format!("failed to open file for reading: {}", e))
        })?;
        Ok(Box::new(file))
    }

    async fn delete(&self, relative_path: &str) -> RepositoryResult<()> {
        let path = self.resolve(relative_path);
        match fs::remove_file(&path) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(RepositoryError::Database(format!(
                "failed to delete file: {}",
                e
            ))),
        }
    }

    fn resolve_path(&self, relative_path: &str) -> String {
        self.resolve(relative_path).to_string_lossy().to_string()
    }
}
