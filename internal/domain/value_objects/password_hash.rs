//! Password hash value object — wraps a PHC-format hash string.
//!
//! PHC (Password Hashing Competition) string format:
//! `$<id>$<params>$<salt>$<hash>`
//!
//! Example: `$pbkdf2-sha256$i=600000,l=32$<salt-b64>$<hash-b64>`

use crate::domain::errors::DomainError;
use std::fmt;

/// A validated PHC-format password hash string.
///
/// Provides PHC string parsing helpers to extract the algorithm identifier,
/// iteration count, salt, and hash value segments without pulling in an
/// external PHC parser.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct PasswordHash {
    /// The raw PHC-format string (guaranteed non-empty).
    inner: String,
}

impl PasswordHash {
    /// Create a new `PasswordHash`, validating that the string is non-empty.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::Validation` if `hash` is empty.
    pub fn new(hash: String) -> Result<Self, DomainError> {
        if hash.is_empty() {
            return Err(DomainError::Validation(
                "password hash must not be empty".into(),
            ));
        }
        Ok(Self { inner: hash })
    }

    /// Return the underlying hash string.
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// Parse the algorithm identifier from the PHC string.
    ///
    /// For example, `$pbkdf2-sha256$i=600000,l=32$<salt>$<hash>` returns
    /// `Some("pbkdf2-sha256")`.
    pub fn algorithm(&self) -> Option<&str> {
        let parts: Vec<&str> = self.inner.split('$').collect();
        // PHC format: $<id>$<params>$<salt>$<hash>
        // Splitting by '$' yields: ["", "<id>", "<params>", "<salt>", "<hash>"]
        parts.get(1).filter(|s| !s.is_empty()).copied()
    }

    /// Parse the iteration count from the params section.
    ///
    /// Supports `i=` (PBKDF2) and `t=` (Argon2 time cost) parameter names.
    pub fn iterations(&self) -> Option<u32> {
        let parts: Vec<&str> = self.inner.split('$').collect();
        let params = parts.get(2)?;
        for param in params.split(',') {
            let param = param.trim();
            if let Some(value) = param
                .strip_prefix("i=")
                .or_else(|| param.strip_prefix("t="))
            {
                return value.parse::<u32>().ok();
            }
        }
        None
    }

    /// Extract the base64-encoded salt segment.
    ///
    /// Returns the segment between the params section and the hash value.
    pub fn salt_b64(&self) -> Option<&str> {
        let parts: Vec<&str> = self.inner.split('$').collect();
        parts.get(3).filter(|s| !s.is_empty()).copied()
    }

    /// Extract the base64-encoded hash value segment.
    ///
    /// Returns the segment after the last `$` separator.
    pub fn hash_value_b64(&self) -> Option<&str> {
        let parts: Vec<&str> = self.inner.split('$').collect();
        parts.get(4).filter(|s| !s.is_empty()).copied()
    }
}

impl fmt::Display for PasswordHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_phc() -> String {
        "$pbkdf2-sha256$i=600000,l=32$c2FsdHlzYWx0$hashvaluebase64".into()
    }

    #[test]
    fn new_accepts_non_empty() {
        let phc = valid_phc();
        let hash = PasswordHash::new(phc).unwrap();
        assert_eq!(
            hash.as_str(),
            "$pbkdf2-sha256$i=600000,l=32$c2FsdHlzYWx0$hashvaluebase64"
        );
    }

    #[test]
    fn new_rejects_empty() {
        let err = PasswordHash::new(String::new()).unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn display_delegates_to_inner() {
        let phc = valid_phc();
        let hash = PasswordHash::new(phc.clone()).unwrap();
        assert_eq!(hash.to_string(), phc);
    }

    #[test]
    fn algorithm_parses_correctly() {
        let hash = PasswordHash::new(valid_phc()).unwrap();
        assert_eq!(hash.algorithm(), Some("pbkdf2-sha256"));
    }

    #[test]
    fn algorithm_returns_none_for_edge_cases() {
        let hash = PasswordHash::new("$".into()).unwrap();
        assert_eq!(hash.algorithm(), None);

        let hash = PasswordHash::new("$$".into()).unwrap();
        assert_eq!(hash.algorithm(), None);
    }

    #[test]
    fn iterations_parses_pbkdf2_i_param() {
        let hash = PasswordHash::new(valid_phc()).unwrap();
        assert_eq!(hash.iterations(), Some(600_000));
    }

    #[test]
    fn iterations_parses_argon2_t_param() {
        let hash = PasswordHash::new("$argon2id$v=19,m=65536,t=3,p=4$c2FsdA$hash".into()).unwrap();
        assert_eq!(hash.iterations(), Some(3));
    }

    #[test]
    fn iterations_returns_none_when_missing() {
        let hash = PasswordHash::new("$scrypt$ln=14$c2FsdA$hash".into()).unwrap();
        assert_eq!(hash.iterations(), None);
    }

    #[test]
    fn salt_b64_extracts_correctly() {
        let hash = PasswordHash::new(valid_phc()).unwrap();
        assert_eq!(hash.salt_b64(), Some("c2FsdHlzYWx0"));
    }

    #[test]
    fn salt_b64_returns_none_when_absent() {
        let hash = PasswordHash::new("$id$params".into()).unwrap();
        assert_eq!(hash.salt_b64(), None);
    }

    #[test]
    fn hash_value_b64_extracts_correctly() {
        let hash = PasswordHash::new(valid_phc()).unwrap();
        assert_eq!(hash.hash_value_b64(), Some("hashvaluebase64"));
    }

    #[test]
    fn hash_value_b64_returns_none_when_absent() {
        let hash = PasswordHash::new("$id$params$salt".into()).unwrap();
        assert_eq!(hash.hash_value_b64(), None);
    }

    #[test]
    fn clone_and_eq() {
        let a = PasswordHash::new(valid_phc()).unwrap();
        let b = PasswordHash::new(valid_phc()).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, a.clone());
    }

    #[test]
    fn serde_roundtrip() {
        let hash = PasswordHash::new(valid_phc()).unwrap();
        let json = serde_json::to_string(&hash).unwrap();
        assert_eq!(
            json,
            r#""$pbkdf2-sha256$i=600000,l=32$c2FsdHlzYWx0$hashvaluebase64""#
        );
        let deserialized: PasswordHash = serde_json::from_str(&json).unwrap();
        assert_eq!(hash, deserialized);
    }
}
