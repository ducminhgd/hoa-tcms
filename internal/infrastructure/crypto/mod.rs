//! Cryptographic infrastructure — password hashing, key derivation.
//!
//! This module provides concrete implementations of application-layer
//! cryptographic port interfaces (e.g. `PasswordHasher`).

pub mod pbkdf2_hasher;
