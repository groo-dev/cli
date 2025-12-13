use std::path::PathBuf;

pub fn get_config_dir() -> PathBuf {
    dirs::config_dir()
        .map(|p| p.join("groo"))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|p| p.join(".groo"))
                .expect("Could not determine home directory")
        })
}

pub fn get_state_file() -> PathBuf {
    get_config_dir().join("state.json")
}

pub fn ensure_config_dir() -> std::io::Result<()> {
    let config_dir = get_config_dir();
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)?;
    }
    Ok(())
}

pub fn get_logs_dir() -> PathBuf {
    get_config_dir().join("logs")
}

pub fn get_service_log_file(service_path: &std::path::Path) -> PathBuf {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    service_path.hash(&mut hasher);
    let hash = format!("{:x}", hasher.finish());
    let short_hash = &hash[..8.min(hash.len())];

    get_logs_dir().join(format!("{}.log", short_hash))
}

#[allow(dead_code)]
pub fn ensure_logs_dir() -> std::io::Result<()> {
    let logs_dir = get_logs_dir();
    if !logs_dir.exists() {
        std::fs::create_dir_all(&logs_dir)?;
    }
    Ok(())
}
