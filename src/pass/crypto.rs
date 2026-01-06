use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

const KEY_LENGTH: usize = 32;
const IV_LENGTH: usize = 12;

/// Derive encryption key from master password using PBKDF2
/// Uses same parameters as pass/web: SHA-256, configurable iterations
pub fn derive_key(password: &str, salt: &[u8], iterations: u32) -> [u8; KEY_LENGTH] {
    let mut key = [0u8; KEY_LENGTH];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
}

/// Decrypt vault data using AES-256-GCM
pub fn decrypt(ciphertext_b64: &str, iv_b64: &str, key: &[u8; KEY_LENGTH]) -> Result<String> {
    let ciphertext = BASE64
        .decode(ciphertext_b64)
        .map_err(|e| anyhow!("Invalid base64 ciphertext: {}", e))?;
    let iv = BASE64
        .decode(iv_b64)
        .map_err(|e| anyhow!("Invalid base64 IV: {}", e))?;

    if iv.len() != IV_LENGTH {
        return Err(anyhow!("Invalid IV length: expected {}, got {}", IV_LENGTH, iv.len()));
    }

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(&iv);

    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_slice())
        .map_err(|_| anyhow!("Decryption failed - incorrect master password?"))?;

    String::from_utf8(plaintext).map_err(|e| anyhow!("Invalid UTF-8 in decrypted data: {}", e))
}

/// Encrypt vault data using AES-256-GCM
pub fn encrypt(plaintext: &str, key: &[u8; KEY_LENGTH]) -> Result<(String, String)> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let iv: [u8; IV_LENGTH] = rand::random();
    let nonce = Nonce::from_slice(&iv);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow!("Encryption failed: {}", e))?;

    Ok((BASE64.encode(&ciphertext), BASE64.encode(iv)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let password = "test-password";
        let salt = b"test-salt-1234567890123456"; // 26 bytes
        let iterations = 1000; // Low for testing

        let key = derive_key(password, salt, iterations);
        let original = "Hello, World!";

        let (ciphertext, iv) = encrypt(original, &key).unwrap();
        let decrypted = decrypt(&ciphertext, &iv, &key).unwrap();

        assert_eq!(original, decrypted);
    }

    #[test]
    fn test_wrong_password_fails() {
        let salt = b"test-salt-1234567890123456";
        let iterations = 1000;

        let key1 = derive_key("password1", salt, iterations);
        let key2 = derive_key("password2", salt, iterations);

        let (ciphertext, iv) = encrypt("secret", &key1).unwrap();
        let result = decrypt(&ciphertext, &iv, &key2);

        assert!(result.is_err());
    }
}
