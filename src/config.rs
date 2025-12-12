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
