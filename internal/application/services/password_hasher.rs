//! `PasswordHasher` trait — hashing and verification of passwords.
//!
//! This trait abstracts over the password hashing algorithm (e.g. PBKDF2,
//! bcrypt, argon2) so that the application layer does not depend on a
//! specific implementation.

use async_trait::async_trait;

/// Hashing and verification of user passwords.
///
/// # Async vs sync
///
/// - `hash` is async because the underlying operation (key derivation) is
///   CPU-bound and should run on a blocking thread pool in production.
/// - `verify` is sync because it is frequently called on hot paths (login,
///   re-authentication) and the overhead of an async call is not justified.
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
    fn verify(&self, password: &str, hash: &str) -> bool;
}
