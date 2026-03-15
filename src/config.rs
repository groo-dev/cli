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

pub fn get_project_logs_dir(project: &str) -> PathBuf {
    get_logs_dir().join(project)
}

pub fn get_service_log_file(project: &str, service_name: &str) -> PathBuf {
    get_project_logs_dir(project).join(format!("{}.log", service_name))
}

#[allow(dead_code)]
pub fn ensure_project_logs_dir(project: &str) -> std::io::Result<()> {
    let logs_dir = get_project_logs_dir(project);
    if !logs_dir.exists() {
        std::fs::create_dir_all(&logs_dir)?;
    }
    Ok(())
}
