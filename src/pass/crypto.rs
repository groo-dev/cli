use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit},
};
use anyhow::{Result, anyhow};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
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
        return Err(anyhow!(
            "Invalid IV length: expected {}, got {}",
            IV_LENGTH,
            iv.len()
        ));
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

pub fn unwrap_key(
    ciphertext: &str,
    iv: &str,
    wrapping_key: &[u8; KEY_LENGTH],
) -> Result<[u8; KEY_LENGTH]> {
    let encoded = decrypt(ciphertext, iv, wrapping_key)?;
    let bytes = BASE64
        .decode(encoded)
        .map_err(|e| anyhow!("Invalid wrapped key: {}", e))?;
    bytes.try_into().map_err(|v: Vec<u8>| {
        anyhow!(
            "Invalid unwrapped key length: expected {}, got {}",
            KEY_LENGTH,
            v.len()
        )
    })
}

pub fn encrypt_record(
    id: &str,
    kind: &str,
    data: &serde_json::Value,
    vault_key: &[u8; KEY_LENGTH],
) -> Result<crate::pass::types::RecordWriteRequest> {
    let record_key: [u8; KEY_LENGTH] = rand::random();
    let envelope = serde_json::to_string(&serde_json::json!({ "kind": kind, "data": data }))?;
    let (encrypted_data, iv) = encrypt(&envelope, &record_key)?;
    let encoded_key = BASE64.encode(record_key);
    let (wrapped_record_key, wrap_iv) = encrypt(&encoded_key, vault_key)?;
    Ok(crate::pass::types::RecordWriteRequest {
        id: id.to_owned(),
        encrypted_data,
        iv,
        wrapped_record_key,
        wrap_iv,
        expected_version: None,
    })
}

pub fn decrypt_record(
    record: &crate::pass::types::ServerRecord,
    vault_key: &[u8; KEY_LENGTH],
) -> Result<(String, serde_json::Value)> {
    let encrypted_data = record
        .encrypted_data
        .as_deref()
        .ok_or_else(|| anyhow!("Record {} has no encrypted data", record.id))?;
    let iv = record
        .iv
        .as_deref()
        .ok_or_else(|| anyhow!("Record {} has no IV", record.id))?;
    let wrapped = record
        .wrapped_record_key
        .as_deref()
        .ok_or_else(|| anyhow!("Record {} has no wrapped key", record.id))?;
    let wrap_iv = record
        .wrap_iv
        .as_deref()
        .ok_or_else(|| anyhow!("Record {} has no key IV", record.id))?;
    let record_key = unwrap_key(wrapped, wrap_iv, vault_key)?;
    let plaintext = decrypt(encrypted_data, iv, &record_key)?;
    let envelope: serde_json::Value = serde_json::from_str(&plaintext)?;
    let kind = envelope
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Record {} has no kind", record.id))?;
    if kind != "item" && kind != "folder" {
        return Err(anyhow!("Record {} has unknown kind {}", record.id, kind));
    }
    let data = envelope
        .get("data")
        .cloned()
        .ok_or_else(|| anyhow!("Record {} has no data", record.id))?;
    Ok((kind.to_owned(), data))
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

    #[test]
    fn decrypts_canonical_cross_platform_record_vector() {
        let vault_key: [u8; 32] = BASE64
            .decode("73vRYTK0AwdYE4ytf+AlthK3tI1IA2UmA4Ijq+2mJ7w=")
            .unwrap()
            .try_into()
            .unwrap();
        let record = crate::pass::types::ServerRecord {
            id: "vector-folder".into(),
            encrypted_data: Some("1WMpPhuRExuLUTYHu3iLJnYxQUq/sKjnhWe8NtL6eg2PI9XESy7o+2Ih4jCeNHlAbG91KmyoWeFUDxTGZA/EQp+xTfHo+fRnKwdSbp0=".into()),
            iv: Some("HAoamBQ3XNwFr6ls".into()),
            wrapped_record_key: Some("v2kaE8FoltGvKmCIAu2M5YFhHcbVnGEbhuGKJq4PYtFMFTmErjZphSExVNkvVJ0VwrzcHJVNds+PHx9h".into()),
            wrap_iv: Some("N4bw2jF6WNGJaOC6".into()),
            version: 1,
            seq: 1,
            is_deleted: false,
        };
        let (kind, payload) = decrypt_record(&record, &vault_key).unwrap();
        assert_eq!(kind, "folder");
        assert_eq!(
            payload,
            serde_json::json!({ "id": "vector-folder", "name": "Work" })
        );
    }
}
