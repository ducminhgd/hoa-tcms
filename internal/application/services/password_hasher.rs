//! `PasswordHasher` trait — hashing and verification of passwords.
//!
//! This trait abstracts over the password hashing algorithm (e.g. PBKDF2,
//! bcrypt, argon2) so that the application layer does not depend on a
//! specific implementation.

use async_trait::async_trait;

/// Hashing and verification of user passwords.
///
/// Both operations are async because key derivation is CPU-bound and must run
/// on the blocking thread pool (`tokio::task::spawn_blocking`) so that the
/// async worker threads are never blocked by a slow PBKDF2 computation.
#[async_trait]
pub trait PasswordHasher: Send + Sync {
    /// Hash a plaintext password.
    ///
    /// Returns the encoded hash string on success. The hash includes the
    /// algorithm, salt, and parameters so that `verify` can extract them.
    ///
    /// # Errors
    ///
    /// Returns `Err(String)` if hashing fails (e.g. invalid algorithm config
    /// or internal library error).
    async fn hash(&self, password: &str) -> Result<String, String>;

    /// Verify a plaintext password against an encoded hash.
    ///
    /// Returns `true` if the password matches the hash, `false` otherwise.
    /// This method does not distinguish between "wrong password" and "malformed
    /// hash" — both return `false`.
    async fn verify(&self, password: &str, hash: &str) -> bool;
}
