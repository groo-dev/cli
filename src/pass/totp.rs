use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

use super::types::{TotpAlgorithm, TotpConfig};

/// Generate a TOTP code from the given config (RFC 6238)
pub fn generate(config: &TotpConfig) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| anyhow!("System time error: {}", e))?;

    let counter = now.as_secs() / config.period as u64;
    generate_at(config, counter)
}

/// Generate TOTP code for a specific counter value
fn generate_at(config: &TotpConfig, counter: u64) -> Result<String> {
    // Decode base32 secret (normalize: uppercase, remove spaces)
    let secret_normalized = config
        .secret
        .to_uppercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();

    let secret_bytes = BASE32_NOPAD
        .decode(secret_normalized.as_bytes())
        .map_err(|e| anyhow!("Invalid TOTP secret (base32 decode failed): {}", e))?;

    // Counter as big-endian 8 bytes
    let counter_bytes = counter.to_be_bytes();

    // HMAC based on algorithm
    let hmac_result = match config.algorithm {
        TotpAlgorithm::SHA1 => {
            let mut mac =
                Hmac::<Sha1>::new_from_slice(&secret_bytes).map_err(|e| anyhow!("{}", e))?;
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
        TotpAlgorithm::SHA256 => {
            let mut mac =
                Hmac::<Sha256>::new_from_slice(&secret_bytes).map_err(|e| anyhow!("{}", e))?;
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
        TotpAlgorithm::SHA512 => {
            let mut mac =
                Hmac::<Sha512>::new_from_slice(&secret_bytes).map_err(|e| anyhow!("{}", e))?;
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
    };

    // Dynamic truncation (RFC 4226)
    let offset = (hmac_result[hmac_result.len() - 1] & 0x0f) as usize;
    let binary = ((hmac_result[offset] & 0x7f) as u32) << 24
        | (hmac_result[offset + 1] as u32) << 16
        | (hmac_result[offset + 2] as u32) << 8
        | (hmac_result[offset + 3] as u32);

    // Modulo to get desired digits
    let modulo = 10u32.pow(config.digits as u32);
    let code = binary % modulo;

    // Zero-pad to required digits
    Ok(format!("{:0>width$}", code, width = config.digits as usize))
}

/// Get seconds remaining until next TOTP code
pub fn seconds_remaining(period: u32) -> u32 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let elapsed = now.as_secs() % period as u64;
    period - elapsed as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_totp_sha1() {
        // Test vector from RFC 6238 appendix B
        // Secret: "12345678901234567890" (ASCII) = base32 "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"
        let config = TotpConfig {
            secret: "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ".to_string(),
            algorithm: TotpAlgorithm::SHA1,
            digits: 8,
            period: 30,
        };

        // Time = 59 seconds, counter = 1
        assert_eq!(generate_at(&config, 1).unwrap(), "94287082");

        // Time = 1111111109, counter = 37037036
        assert_eq!(generate_at(&config, 37037036).unwrap(), "07081804");
    }

    #[test]
    fn test_totp_6_digits() {
        let config = TotpConfig {
            secret: "JBSWY3DPEHPK3PXP".to_string(), // Common test secret
            algorithm: TotpAlgorithm::SHA1,
            digits: 6,
            period: 30,
        };

        // Just verify it generates a 6-digit code
        let code = generate_at(&config, 0).unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }
}
