//! Authentication and secret storage abstractions.

use std::env;

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::error::{DarialError, Result};

/// Secret store interface (env/file for MVP; secret managers later).
pub trait SecretStore: Send + Sync {
    /// Fetch a secret by logical name / env key.
    fn get(&self, key: &str) -> Result<Option<String>>;
}

/// Environment-backed secret store.
#[derive(Debug, Default, Clone)]
pub struct EnvSecretStore;

impl SecretStore for EnvSecretStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(env::var(key).ok())
    }
}

/// Hash an opaque token with SHA-256 (high entropy tokens).
#[must_use]
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Constant-time compare of hex hashes.
#[must_use]
pub fn hash_eq(a: &str, b: &str) -> bool {
    let aa = a.as_bytes();
    let bb = b.as_bytes();
    if aa.len() != bb.len() {
        return false;
    }
    bool::from(aa.ct_eq(bb))
}

/// Verify bearer token against configured SHA-256 hashes and/or env plaintext.
pub fn verify_bearer(
    authorization: Option<&str>,
    accepted_hashes: &[String],
    token_env: Option<&str>,
    store: &dyn SecretStore,
) -> Result<()> {
    let Some(header) = authorization else {
        return Err(DarialError::Unauthenticated);
    };
    let token = header
        .strip_prefix("Bearer ")
        .or_else(|| header.strip_prefix("bearer "))
        .ok_or(DarialError::Unauthenticated)?
        .trim();
    if token.is_empty() {
        return Err(DarialError::Unauthenticated);
    }
    let presented = hash_token(token);
    for h in accepted_hashes {
        if hash_eq(&presented, h) {
            return Ok(());
        }
    }
    if let Some(env_key) = token_env {
        if let Some(expected) = store.get(env_key)? {
            if bool::from(token.as_bytes().ct_eq(expected.as_bytes())) {
                return Ok(());
            }
        }
    }
    if accepted_hashes.is_empty() && token_env.is_none() {
        // Dev convenience: if no tokens configured, deny in this function —
        // callers may skip auth when profile allows.
        return Err(DarialError::Unauthenticated);
    }
    Err(DarialError::Unauthenticated)
}

/// Extract bearer token string.
#[must_use]
pub fn extract_bearer(authorization: Option<&str>) -> Option<&str> {
    authorization
        .and_then(|h| {
            h.strip_prefix("Bearer ")
                .or_else(|| h.strip_prefix("bearer "))
        })
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_roundtrip_verify() {
        let t = "test-token-value";
        let h = hash_token(t);
        assert!(hash_eq(&h, &hash_token(t)));
        assert!(!hash_eq(&h, &hash_token("other")));
    }
}
