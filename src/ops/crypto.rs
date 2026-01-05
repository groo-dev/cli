use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use keyring::Entry;
use rand::RngCore;
use rsa::{
    pkcs8::{DecodePrivateKey, EncodePrivateKey},
    traits::PublicKeyParts,
    Oaep, RsaPrivateKey, RsaPublicKey,
};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

const SERVICE_NAME: &str = "groo-cli";
const RSA_BITS: usize = 2048;
const AES_KEY_SIZE: usize = 32;
const NONCE_SIZE: usize = 12;

/// Key pair for secrets encryption
pub struct KeyPair {
    pub public_key_jwk: String,
    pub private_key_base64: String,
}

/// Encrypted data format (matches ops web)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedData {
    pub iv: String,
    pub encrypted_key: String,
    pub encrypted_value: String,
}

/// Generate a new RSA key pair for secrets encryption
pub fn generate_key_pair() -> Result<KeyPair> {
    let mut rng = rand::thread_rng();
    let private_key =
        RsaPrivateKey::new(&mut rng, RSA_BITS).context("Failed to generate RSA key")?;
    let public_key = RsaPublicKey::from(&private_key);

    // Export public key as JWK
    let public_key_jwk = public_key_to_jwk(&public_key)?;

    // Export private key as base64 PKCS8
    let private_key_der = private_key
        .to_pkcs8_der()
        .context("Failed to encode private key")?;
    let private_key_base64 = BASE64.encode(private_key_der.as_bytes());

    Ok(KeyPair {
        public_key_jwk,
        private_key_base64,
    })
}

/// Convert RSA public key to JWK format
fn public_key_to_jwk(public_key: &RsaPublicKey) -> Result<String> {
    // Get the public key components
    let n = public_key.n();
    let e = public_key.e();

    // Convert to base64url encoding (no padding)
    let n_bytes = n.to_bytes_be();
    let e_bytes = e.to_bytes_be();

    let n_b64 = base64_url_encode(&n_bytes);
    let e_b64 = base64_url_encode(&e_bytes);

    // Build JWK
    let jwk = serde_json::json!({
        "kty": "RSA",
        "n": n_b64,
        "e": e_b64,
        "alg": "RSA-OAEP-256",
        "use": "enc"
    });

    Ok(serde_json::to_string(&jwk)?)
}

/// Parse JWK to RSA public key
fn jwk_to_public_key(jwk_str: &str) -> Result<RsaPublicKey> {
    let jwk: serde_json::Value = serde_json::from_str(jwk_str)?;

    let n_b64 = jwk["n"].as_str().context("Missing 'n' in JWK")?;
    let e_b64 = jwk["e"].as_str().context("Missing 'e' in JWK")?;

    let n_bytes = base64_url_decode(n_b64)?;
    let e_bytes = base64_url_decode(e_b64)?;

    let n = rsa::BigUint::from_bytes_be(&n_bytes);
    let e = rsa::BigUint::from_bytes_be(&e_bytes);

    RsaPublicKey::new(n, e).context("Invalid RSA public key")
}

/// Encrypt a secret value using hybrid encryption (RSA-OAEP + AES-256-GCM)
pub fn encrypt_secret(value: &str, public_key_jwk: &str) -> Result<String> {
    let public_key = jwk_to_public_key(public_key_jwk)?;

    // Generate random AES key
    let mut aes_key = [0u8; AES_KEY_SIZE];
    rand::thread_rng().fill_bytes(&mut aes_key);

    // Generate random IV/nonce
    let mut iv = [0u8; NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut iv);

    // Encrypt value with AES-GCM
    let cipher = Aes256Gcm::new_from_slice(&aes_key).context("Failed to create AES cipher")?;
    let nonce = Nonce::from_slice(&iv);
    let encrypted_value = cipher
        .encrypt(nonce, value.as_bytes())
        .map_err(|_| anyhow::anyhow!("AES encryption failed"))?;

    // Encrypt AES key with RSA-OAEP
    let padding = Oaep::new::<Sha256>();
    let mut rng = rand::thread_rng();
    let encrypted_key = public_key
        .encrypt(&mut rng, padding, &aes_key)
        .context("RSA encryption failed")?;

    // Build encrypted data structure
    let data = EncryptedData {
        iv: BASE64.encode(iv),
        encrypted_key: BASE64.encode(encrypted_key),
        encrypted_value: BASE64.encode(encrypted_value),
    };

    Ok(serde_json::to_string(&data)?)
}

/// Decrypt a secret value using hybrid decryption
pub fn decrypt_secret(encrypted_json: &str, private_key_base64: &str) -> Result<String> {
    let data: EncryptedData = serde_json::from_str(encrypted_json)?;

    // Decode base64 values
    let iv = BASE64.decode(&data.iv)?;
    let encrypted_key = BASE64.decode(&data.encrypted_key)?;
    let encrypted_value = BASE64.decode(&data.encrypted_value)?;

    // Decode and parse private key
    let private_key_der = BASE64.decode(private_key_base64)?;
    let private_key =
        RsaPrivateKey::from_pkcs8_der(&private_key_der).context("Invalid private key")?;

    // Decrypt AES key with RSA-OAEP
    let padding = Oaep::new::<Sha256>();
    let aes_key = private_key
        .decrypt(padding, &encrypted_key)
        .context("RSA decryption failed")?;

    // Decrypt value with AES-GCM
    let cipher = Aes256Gcm::new_from_slice(&aes_key).context("Failed to create AES cipher")?;
    let nonce = Nonce::from_slice(&iv);
    let decrypted = cipher
        .decrypt(nonce, encrypted_value.as_ref())
        .map_err(|_| anyhow::anyhow!("AES decryption failed"))?;

    String::from_utf8(decrypted).context("Decrypted value is not valid UTF-8")
}

// Keychain storage for private keys

/// Store private key in OS keychain
pub fn store_private_key(app_id: &str, private_key: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, &format!("ops-{}", app_id))
        .context("Failed to create keychain entry")?;
    entry
        .set_password(private_key)
        .context("Failed to store private key in keychain")?;
    Ok(())
}

/// Get private key from OS keychain
pub fn get_private_key(app_id: &str) -> Result<String> {
    let entry = Entry::new(SERVICE_NAME, &format!("ops-{}", app_id))
        .context("Failed to create keychain entry")?;
    entry
        .get_password()
        .context("Private key not found in keychain")
}

/// Check if private key exists in keychain
pub fn has_private_key(app_id: &str) -> bool {
    Entry::new(SERVICE_NAME, &format!("ops-{}", app_id))
        .and_then(|e| e.get_password())
        .is_ok()
}

/// Delete private key from OS keychain
pub fn delete_private_key(app_id: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, &format!("ops-{}", app_id))
        .context("Failed to create keychain entry")?;
    entry
        .delete_credential()
        .context("Failed to delete private key from keychain")?;
    Ok(())
}

// Base64 URL encoding helpers (for JWK)

fn base64_url_encode(data: &[u8]) -> String {
    BASE64
        .encode(data)
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string()
}

fn base64_url_decode(data: &str) -> Result<Vec<u8>> {
    // Add padding if needed
    let padded = match data.len() % 4 {
        2 => format!("{}==", data),
        3 => format!("{}=", data),
        _ => data.to_string(),
    };
    // Convert from URL-safe to standard base64
    let standard = padded.replace('-', "+").replace('_', "/");
    BASE64.decode(standard).context("Invalid base64")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key_pair = generate_key_pair().unwrap();
        let original = "my secret value";

        let encrypted = encrypt_secret(original, &key_pair.public_key_jwk).unwrap();
        let decrypted = decrypt_secret(&encrypted, &key_pair.private_key_base64).unwrap();

        assert_eq!(original, decrypted);
    }

    #[test]
    fn test_jwk_roundtrip() {
        let key_pair = generate_key_pair().unwrap();
        let public_key = jwk_to_public_key(&key_pair.public_key_jwk).unwrap();

        // Re-encode and compare
        let jwk2 = public_key_to_jwk(&public_key).unwrap();
        let parsed1: serde_json::Value = serde_json::from_str(&key_pair.public_key_jwk).unwrap();
        let parsed2: serde_json::Value = serde_json::from_str(&jwk2).unwrap();

        assert_eq!(parsed1["n"], parsed2["n"]);
        assert_eq!(parsed1["e"], parsed2["e"]);
    }
}
