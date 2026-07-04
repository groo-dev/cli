//! Auth state storage.
//!
//! Tokens live in the OS credential store (macOS Keychain, Windows
//! Credential Manager, Linux Secret Service via the `keyring` crate) under
//! service `"groo-cli"` / user `"default"`. There is no local master
//! password any more — the OS keychain is the encryption boundary.
//!
//! Set `GROO_TOKEN_FILE=<path>` to opt out of the keychain entirely (e.g.
//! headless/CI boxes with no Secret Service running) and store the token
//! JSON in a plain file instead, `chmod 600`. That file is read/written for
//! *every* command when the env var is set, which is why login does not
//! have its own `--token-file` flag — see `commands::auth::login`.
//!
//! Older CLI versions stored an encrypted `~/.groo/auth.enc` protected by a
//! user-chosen master password. That format is no longer readable; if it's
//! the only thing on disk we tell the user to log in again rather than
//! prompt for a password we no longer use anywhere else.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::config;

const SERVICE_NAME: &str = "groo-cli";
const KEYRING_USER: &str = "default";
const GROO_TOKEN_FILE_ENV: &str = "GROO_TOKEN_FILE";

const LEGACY_AUTH_FILE: &str = "auth.enc";
const LEGACY_AUTH_SALT_FILE: &str = "auth.salt";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String, // "oauth" or "pat"
    pub expires_at: Option<i64>,
    pub user_email: Option<String>,
}

/// Load the stored auth state, if any.
pub fn load_auth() -> Result<Option<AuthState>> {
    if let Some(path) = token_file_override() {
        return load_from_token_file(&path);
    }

    let entry = keyring_entry()?;
    match entry.get_password() {
        Ok(json) => {
            let state: AuthState = serde_json::from_str(&json)
                .context("stored credentials are corrupted — run 'groo auth login' again")?;
            Ok(Some(state))
        }
        Err(keyring::Error::NoEntry) => {
            if legacy_files_exist() {
                bail!(legacy_format_message());
            }
            Ok(None)
        }
        Err(e) => bail!(keyring_failure_message("read", &e)),
    }
}

/// Persist the auth state, replacing whatever was previously stored.
///
/// On success, any leftover legacy `auth.enc`/`auth.salt` files are removed
/// so `load_auth` never has to reason about stale formats again.
pub fn save_auth(state: &AuthState) -> Result<()> {
    let json = serde_json::to_string(state).context("failed to serialize auth state")?;

    if let Some(path) = token_file_override() {
        write_token_file(&path, &json)?;
        println!(
            "! tokens stored in plaintext at {} ({GROO_TOKEN_FILE_ENV})",
            path.display()
        );
    } else {
        let entry = keyring_entry()?;
        entry
            .set_password(&json)
            .map_err(|e| anyhow!(keyring_failure_message("save", &e)))?;
    }

    remove_legacy_files()?;
    Ok(())
}

/// True if some form of credentials appears to be stored (keychain entry,
/// token file, or legacy encrypted file). Unlike `load_auth`, this never
/// bails on a legacy-only or unreadable store — it's meant for lightweight
/// "are we logged in" checks (`doctor`, `logout`) that shouldn't error out
/// just to answer a yes/no question.
pub fn has_stored_auth() -> bool {
    if let Some(path) = token_file_override() {
        return path.exists();
    }
    if let Ok(entry) = keyring_entry()
        && entry.get_password().is_ok()
    {
        return true;
    }
    legacy_files_exist()
}

/// Clear the stored auth state (keychain entry, token file, and any legacy
/// files), leaving the user fully logged out.
pub fn clear_auth() -> Result<()> {
    if let Some(path) = token_file_override() {
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove token file at {}", path.display()))?;
        }
    } else {
        let entry = keyring_entry()?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => bail!(keyring_failure_message("clear", &e)),
        }
    }

    remove_legacy_files()?;
    Ok(())
}

fn token_file_override() -> Option<PathBuf> {
    token_file_override_from(std::env::var_os(GROO_TOKEN_FILE_ENV))
}

/// Pure helper behind `token_file_override`, split out so tests can check
/// the "empty string doesn't count as set" rule without mutating the real
/// process environment (which isn't safe to do from parallel tests).
fn token_file_override_from(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    value.filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn load_from_token_file(path: &Path) -> Result<Option<AuthState>> {
    if !path.exists() {
        return Ok(None);
    }
    let json = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read token file at {}", path.display()))?;
    let state: AuthState = serde_json::from_str(&json)
        .with_context(|| format!("invalid token file at {}", path.display()))?;
    Ok(Some(state))
}

/// Write `contents` to `path` and lock it down to owner-only permissions on
/// unix. Split out from `save_auth` so the permission behavior is directly
/// unit-testable without touching the keychain or real config dir.
fn write_token_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory for {}", path.display()))?;
    }
    std::fs::write(path, contents)
        .with_context(|| format!("failed to write token file at {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    }

    Ok(())
}

fn keyring_entry() -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE_NAME, KEYRING_USER)
        .map_err(|e| anyhow!(keyring_failure_message("open", &e)))
}

/// Human name of the platform's keychain backend, for error messages.
fn keyring_backend_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS Keychain"
    } else if cfg!(target_os = "windows") {
        "Windows Credential Manager"
    } else if cfg!(target_os = "linux") {
        "Linux Secret Service"
    } else {
        "OS keychain"
    }
}

fn keyring_failure_message(action: &str, err: &keyring::Error) -> String {
    format!(
        "could not {action} credentials in the {backend} ({err}) — set {GROO_TOKEN_FILE_ENV}=<path> to store tokens in a plain file instead",
        backend = keyring_backend_name(),
    )
}

fn legacy_format_message() -> String {
    "stored credentials use the old encrypted-file format — run 'groo auth login' to sign in again (old files: ~/.groo/auth.enc)".to_string()
}

fn legacy_files_exist() -> bool {
    let dir = config::get_config_dir();
    dir.join(LEGACY_AUTH_FILE).exists() || dir.join(LEGACY_AUTH_SALT_FILE).exists()
}

fn remove_legacy_files() -> Result<()> {
    let dir = config::get_config_dir();
    let auth_path = dir.join(LEGACY_AUTH_FILE);
    let salt_path = dir.join(LEGACY_AUTH_SALT_FILE);

    if auth_path.exists() {
        std::fs::remove_file(&auth_path)?;
    }
    if salt_path.exists() {
        std::fs::remove_file(&salt_path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn token_file_gets_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "groo-cli-storage-test-{}-{}",
            std::process::id(),
            "token-perms"
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("token.json");

        write_token_file(&path, r#"{"access_token":"x"}"#).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn token_file_override_ignores_empty_value() {
        // Guard against `GROO_TOKEN_FILE=""` being treated as "set".
        assert!(token_file_override_from(Some("".into())).is_none());
    }

    #[test]
    fn token_file_override_uses_present_value() {
        assert_eq!(
            token_file_override_from(Some("/tmp/groo-token.json".into())),
            Some(PathBuf::from("/tmp/groo-token.json"))
        );
    }

    #[test]
    fn token_file_override_none_when_unset() {
        assert!(token_file_override_from(None).is_none());
    }

    #[test]
    fn load_from_missing_token_file_is_none() {
        let path = std::env::temp_dir().join(format!(
            "groo-cli-storage-test-{}-missing.json",
            std::process::id()
        ));
        std::fs::remove_file(&path).ok();
        assert!(load_from_token_file(&path).unwrap().is_none());
    }
}
