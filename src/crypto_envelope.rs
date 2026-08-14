//! HPKE-based protected message envelopes (RFC 9180).
//!
//! Framing is inspired by RFC 9458 Oblivious HTTP but is Darial-specific until
//! independent interoperability vectors pass. See docs/adr/0004-private-mode.md.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hpke::aead::AesGcm128;
use hpke::kdf::HkdfSha256;
use hpke::kem::X25519HkdfSha256;
use hpke::{Deserializable, Kem as KemTrait, OpModeR, OpModeS, Serializable};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::{DarialError, Result};
use crate::protocol::{Direction, PROTOCOL_VERSION};

type Kem = X25519HkdfSha256;
type Aead = AesGcm128;
type Kdf = HkdfSha256;

/// Info / application transcript label.
const HPKE_INFO: &[u8] = b"darial-ohttp-inspired/v1";

/// Gateway keypair for HPKE.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct GatewayKeypair {
    /// Key id.
    #[zeroize(skip)]
    pub kid: String,
    /// Public key bytes (32).
    #[zeroize(skip)]
    pub public: Vec<u8>,
    /// Private key bytes.
    private: Vec<u8>,
}

impl GatewayKeypair {
    /// Generate a new X25519 HPKE keypair.
    pub fn generate(kid: impl Into<String>) -> Result<Self> {
        let (private, public) = Kem::gen_keypair(&mut OsRng);
        Ok(Self {
            kid: kid.into(),
            public: public.to_bytes().to_vec(),
            private: private.to_bytes().to_vec(),
        })
    }

    /// Reconstruct from stored parts.
    #[must_use]
    pub fn from_parts(kid: impl Into<String>, public: Vec<u8>, private: Vec<u8>) -> Self {
        Self {
            kid: kid.into(),
            public,
            private,
        }
    }

    /// Export private key bytes for secure persistence.
    #[must_use]
    pub fn private_bytes(&self) -> &[u8] {
        &self.private
    }

    /// Public key document for discovery.
    #[must_use]
    pub fn public_document(&self) -> KeyDocument {
        KeyDocument {
            kid: self.kid.clone(),
            kem: "x25519_hkdf_sha256".into(),
            kdf: "hkdf_sha256".into(),
            aead: "aes_128_gcm".into(),
            public_key_b64: B64.encode(&self.public),
        }
    }

    /// Seal plaintext for this recipient (client side uses public only).
    pub fn seal_with_public(
        kid: &str,
        public: &[u8],
        plaintext: &[u8],
        direction: Direction,
        gateway_origin: &str,
    ) -> Result<Envelope> {
        let pk = <Kem as KemTrait>::PublicKey::from_bytes(public)
            .map_err(|_| DarialError::PrivacyNegotiationFailure("bad public key".into()))?;
        let aad = build_aad(direction, kid, gateway_origin);
        let (encapped, ciphertext) = hpke::single_shot_seal::<Aead, Kdf, Kem, _>(
            &OpModeS::Base,
            &pk,
            HPKE_INFO,
            plaintext,
            &aad,
            &mut OsRng,
        )
        .map_err(|_| DarialError::PrivacyNegotiationFailure("seal failed".into()))?;
        Ok(Envelope {
            protocol_version: PROTOCOL_VERSION,
            kid: kid.to_string(),
            direction: direction.as_str().to_string(),
            encapped_key_b64: B64.encode(encapped.to_bytes()),
            ciphertext_b64: B64.encode(ciphertext),
        })
    }

    /// Open an envelope with our private key.
    pub fn open(
        &self,
        env: &Envelope,
        expected: Direction,
        gateway_origin: &str,
    ) -> Result<Vec<u8>> {
        if env.protocol_version != PROTOCOL_VERSION {
            return Err(DarialError::PrivacyNegotiationFailure(
                "protocol version mismatch".into(),
            ));
        }
        if env.kid != self.kid {
            return Err(DarialError::PrivacyNegotiationFailure("unknown kid".into()));
        }
        if env.direction != expected.as_str() {
            return Err(DarialError::PrivacyNegotiationFailure(
                "direction mismatch".into(),
            ));
        }
        let sk = <Kem as KemTrait>::PrivateKey::from_bytes(&self.private)
            .map_err(|_| DarialError::PrivacyNegotiationFailure("bad private key".into()))?;
        let encapped_bytes = B64
            .decode(&env.encapped_key_b64)
            .map_err(|_| DarialError::PrivacyNegotiationFailure("bad encapped".into()))?;
        let ct = B64
            .decode(&env.ciphertext_b64)
            .map_err(|_| DarialError::PrivacyNegotiationFailure("bad ciphertext".into()))?;
        let encapped = <Kem as KemTrait>::EncappedKey::from_bytes(&encapped_bytes)
            .map_err(|_| DarialError::PrivacyNegotiationFailure("bad encapped key".into()))?;
        let aad = build_aad(expected, &env.kid, gateway_origin);
        hpke::single_shot_open::<Aead, Kdf, Kem>(
            &OpModeR::Base,
            &sk,
            &encapped,
            HPKE_INFO,
            &ct,
            &aad,
        )
        .map_err(|_| DarialError::PrivacyNegotiationFailure("open failed".into()))
    }
}

fn build_aad(direction: Direction, kid: &str, gateway_origin: &str) -> Vec<u8> {
    format!(
        "darial|{PROTOCOL_VERSION}|{}|{kid}|{gateway_origin}|application/json",
        direction.as_str()
    )
    .into_bytes()
}

/// Published public key document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDocument {
    /// Key id.
    pub kid: String,
    /// KEM name.
    pub kem: String,
    /// KDF name.
    pub kdf: String,
    /// AEAD name.
    pub aead: String,
    /// Public key base64.
    pub public_key_b64: String,
}

/// Wire envelope (JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// Protocol version.
    pub protocol_version: u32,
    /// Key id.
    pub kid: String,
    /// Direction label.
    pub direction: String,
    /// Encapsulated key.
    pub encapped_key_b64: String,
    /// Ciphertext.
    pub ciphertext_b64: String,
}

/// Key set returned by discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeySet {
    /// Keys.
    pub keys: Vec<KeyDocument>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_roundtrip() {
        let kp = GatewayKeypair::generate("k1").unwrap();
        let env = GatewayKeypair::seal_with_public(
            &kp.kid,
            &kp.public,
            b"hello-prompt",
            Direction::Request,
            "http://127.0.0.1:8443",
        )
        .unwrap();
        let pt = kp
            .open(&env, Direction::Request, "http://127.0.0.1:8443")
            .unwrap();
        assert_eq!(pt, b"hello-prompt");
    }

    #[test]
    fn rejects_wrong_direction() {
        let kp = GatewayKeypair::generate("k1").unwrap();
        let env = GatewayKeypair::seal_with_public(
            &kp.kid,
            &kp.public,
            b"hello",
            Direction::Request,
            "http://127.0.0.1:8443",
        )
        .unwrap();
        assert!(kp
            .open(&env, Direction::Response, "http://127.0.0.1:8443")
            .is_err());
    }

    #[test]
    fn rejects_corruption() {
        let kp = GatewayKeypair::generate("k1").unwrap();
        let mut env = GatewayKeypair::seal_with_public(
            &kp.kid,
            &kp.public,
            b"hello",
            Direction::Request,
            "http://127.0.0.1:8443",
        )
        .unwrap();
        env.ciphertext_b64.push('A');
        assert!(kp
            .open(&env, Direction::Request, "http://127.0.0.1:8443")
            .is_err());
    }
}
