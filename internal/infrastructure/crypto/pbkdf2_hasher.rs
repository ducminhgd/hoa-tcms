//! PBKDF2-HMAC-SHA256 implementation of the `PasswordHasher` trait.
//!
//! Passwords are hashed using PBKDF2 with a 16-byte random salt and
//! a configurable iteration count (default 600 000). The resulting
//! hash is encoded as a PHC string:
//!
//! ```text
//! $pbkdf2-sha256$i=600000,l=32$<base64_salt>$<base64_hash>
//! ```
//!
//! See [Password Hashing Competition (PHC) string format](https://github.com/P-H-C/phc-string-format/blob/master/phc-sf-spec.md).

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use rand::rngs::OsRng;
use sha2::Sha256;

use async_trait::async_trait;

use crate::application::services::password_hasher::PasswordHasher;

/// PBKDF2-based password hasher using HMAC-SHA256.
///
/// # Defaults
///
/// - Iterations: 600 000 (OWASP 2023 recommended minimum for PBKDF2-HMAC-SHA256).
/// - Salt length: 16 bytes (128 bits).
/// - Derived key length: 32 bytes (256 bits).
///
/// # PHC string format
///
/// The encoded hash follows the PHC string format:
///
/// ```text
/// $pbkdf2-sha256$i=600000,l=32$ABcDeF...$ZxYwVu...
/// ```
///
/// where:
/// - `pbkdf2-sha256` is the algorithm identifier.
/// - `i` is the iteration count.
/// - `l` is the derived key length in bytes.
/// - The two base64 segments are the salt and the derived key respectively.
pub struct Pbkdf2Hasher {
    iterations: u32,
}

impl Pbkdf2Hasher {
    /// Create a new `Pbkdf2Hasher` with the given iteration count.
    pub fn new(iterations: u32) -> Self {
        Self { iterations }
    }
}

impl Default for Pbkdf2Hasher {
    fn default() -> Self {
        Self {
            iterations: 600_000,
        }
    }
}

#[async_trait]
impl PasswordHasher for Pbkdf2Hasher {
    async fn hash(&self, password: &str) -> Result<String, String> {
        let iterations = self.iterations;
        let password = password.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut salt = [0u8; 16];
            OsRng.fill_bytes(&mut salt);

            let mut output = [0u8; 32];
            pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, iterations, &mut output);

            let salt_b64 = STANDARD_NO_PAD.encode(salt);
            let hash_b64 = STANDARD_NO_PAD.encode(output);

            Ok(format!(
                "$pbkdf2-sha256$i={},l=32${}${}",
                iterations, salt_b64, hash_b64,
            ))
        })
        .await
        .map_err(|e| format!("password hashing failed: {}", e))?
    }

    fn verify(&self, password: &str, encoded: &str) -> bool {
        // Split on '$'. Expected format:
        //   [0] ""  [1] "pbkdf2-sha256"  [2] "i=N,l=32"  [3] salt_b64  [4] hash_b64
        let parts: Vec<&str> = encoded.split('$').collect();
        if parts.len() != 5 {
            return false;
        }

        // Validate algorithm identifier.
        if parts[1] != "pbkdf2-sha256" {
            return false;
        }

        // Parse iterations from the parameters segment.
        const MIN_ITERATIONS: u32 = 100_000;

        let iterations: u32 = parts[2]
            .split(',')
            .find_map(|param| param.strip_prefix("i="))
            .and_then(|val| val.parse::<u32>().ok())
            .unwrap_or(self.iterations);

        // Reject hashes with fewer than the minimum iterations to prevent
        // trivial brute-forcing if an attacker has injected a weak hash
        // (e.g. i=1) into the database.
        if iterations < MIN_ITERATIONS {
            return false;
        }

        // Decode salt.
        let salt_bytes = match STANDARD_NO_PAD.decode(parts[3]) {
            Ok(b) => b,
            Err(_) => return false,
        };

        // Decode expected hash.
        let expected_hash = match STANDARD_NO_PAD.decode(parts[4]) {
            Ok(b) => b,
            Err(_) => return false,
        };

        // Re-derive key with the extracted parameters.
        let output_len = expected_hash.len();
        let mut computed_hash = vec![0u8; output_len];
        pbkdf2_hmac::<Sha256>(
            password.as_bytes(),
            &salt_bytes,
            iterations,
            &mut computed_hash,
        );

        // Constant-time comparison to prevent timing attacks.
        constant_time_password_eq(&computed_hash, &expected_hash)
    }
}

/// Compare two byte slices in constant time for password verification.
///
/// Returns `true` if the slices are equal. Short-circuits on length
/// mismatch (lengths are not secret), but every byte of the shorter
/// slice is still compared if lengths differ.
///
/// # Warning
///
/// This function is only suitable for password / derived-key comparison
/// where the length of the expected value is known to the attacker from
/// the hash format anyway. **Do not** use it for comparing secrets whose
/// length is itself confidential.
fn constant_time_password_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hash_and_verify_valid_password() {
        let hasher = Pbkdf2Hasher::new(100_000);
        let hash = hasher.hash("correct-horse-battery-staple").await.unwrap();

        assert!(hasher.verify("correct-horse-battery-staple", &hash));
    }

    #[tokio::test]
    async fn reject_wrong_password() {
        let hasher = Pbkdf2Hasher::new(100_000);
        let hash = hasher.hash("real-password").await.unwrap();

        assert!(!hasher.verify("wrong-password", &hash));
    }

    #[tokio::test]
    async fn reject_malformed_hash() {
        let hasher = Pbkdf2Hasher::default();
        assert!(!hasher.verify("anything", "not-a-valid-phc-string"));
    }

    #[test]
    fn constant_time_eq_same() {
        assert!(constant_time_password_eq(b"hello", b"hello"));
    }

    #[test]
    fn constant_time_eq_different() {
        assert!(!constant_time_password_eq(b"hello", b"world"));
    }

    #[test]
    fn constant_time_eq_diff_len() {
        assert!(!constant_time_password_eq(b"short", b"longer"));
    }

    #[test]
    fn phc_string_roundtrip() {
        let hasher = Pbkdf2Hasher::new(100_000);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let hash = runtime.block_on(hasher.hash("test-password")).unwrap();

        // PHC string should start with the expected prefix.
        assert!(hash.starts_with("$pbkdf2-sha256$i=100000,l=32$"));

        // Should have exactly 4 dollar signs (5 segments).
        assert_eq!(hash.matches('$').count(), 4);

        // Base64 segments should not contain newlines.
        assert!(!hash.contains('\n'));
    }
}
