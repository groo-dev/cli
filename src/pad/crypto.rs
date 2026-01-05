use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

use super::types::EncryptedPayload;

const PBKDF2_ITERATIONS: u32 = 600_000;
const KEY_LENGTH: usize = 32;
const IV_LENGTH: usize = 12;

pub fn derive_key(password: &str, salt: &[u8]) -> [u8; KEY_LENGTH] {
    let mut key = [0u8; KEY_LENGTH];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

pub fn encrypt(plaintext: &str, key: &[u8; KEY_LENGTH]) -> Result<EncryptedPayload> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let iv: [u8; IV_LENGTH] = rand::random();
    let nonce = Nonce::from_slice(&iv);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow!("Encryption failed: {}", e))?;

    Ok(EncryptedPayload {
        ciphertext: BASE64.encode(&ciphertext),
        iv: BASE64.encode(iv),
        version: 1,
    })
}

pub fn decrypt(payload: &EncryptedPayload, key: &[u8; KEY_LENGTH]) -> Result<String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let ciphertext = BASE64.decode(&payload.ciphertext)?;
    let iv = BASE64.decode(&payload.iv)?;
    let nonce = Nonce::from_slice(&iv);

    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_slice())
        .map_err(|_| anyhow!("Decryption failed - incorrect password?"))?;

    String::from_utf8(plaintext).map_err(|e| anyhow!("Invalid UTF-8: {}", e))
}

pub fn verify_key(test_payload: &EncryptedPayload, key: &[u8; KEY_LENGTH]) -> bool {
    decrypt(test_payload, key).is_ok()
}

pub fn encrypt_file(data: &[u8], key: &[u8; KEY_LENGTH]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let iv: [u8; IV_LENGTH] = rand::random();
    let nonce = Nonce::from_slice(&iv);

    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| anyhow!("File encryption failed: {}", e))?;

    // Prepend IV to ciphertext
    let mut result = Vec::with_capacity(IV_LENGTH + ciphertext.len());
    result.extend_from_slice(&iv);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}
