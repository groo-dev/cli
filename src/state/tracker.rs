use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    pub pid: u32,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectState {
    pub path: PathBuf,
    pub services: HashMap<String, ServiceState>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub projects: HashMap<String, ProjectState>,
}

impl State {
    pub fn load() -> Result<Self> {
        let state_file = config::get_state_file();
        if !state_file.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&state_file)?;
        let state: State = serde_json::from_str(&content)?;
        Ok(state)
    }

    pub fn save(&self) -> Result<()> {
        config::ensure_config_dir()?;
        let state_file = config::get_state_file();
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&state_file, content)?;
        Ok(())
    }

    pub fn add_service(
        &mut self,
        project_name: &str,
        project_path: PathBuf,
        service_name: &str,
        pid: u32,
        port: Option<u16>,
    ) {
        let project = self
            .projects
            .entry(project_name.to_string())
            .or_insert_with(|| ProjectState {
                path: project_path,
                services: HashMap::new(),
            });

        project.services.insert(
            service_name.to_string(),
            ServiceState { pid, port },
        );
    }

    pub fn remove_project(&mut self, project_name: &str) {
        self.projects.remove(project_name);
    }

    #[allow(dead_code)]
    pub fn remove_service(&mut self, project_name: &str, service_name: &str) {
        if let Some(project) = self.projects.get_mut(project_name) {
            project.services.remove(service_name);
            if project.services.is_empty() {
                self.projects.remove(project_name);
            }
        }
    }

    pub fn get_project(&self, project_name: &str) -> Option<&ProjectState> {
        self.projects.get(project_name)
    }

    pub fn clean_stale_pids(&mut self) {
        for project in self.projects.values_mut() {
            project.services.retain(|_, service| {
                is_service_running(service.port, service.pid)
            });
        }
        self.projects.retain(|_, project| !project.services.is_empty());
    }
}

/// Check if a service is running by port (preferred) or PID fallback
pub fn is_service_running(port: Option<u16>, pid: u32) -> bool {
    // If we have a port, check if it's in use (more reliable)
    if let Some(p) = port {
        return is_port_in_use(p);
    }
    // Fall back to PID check
    is_pid_running(pid)
}

/// Check if a port is in use (using lsof for reliability)
#[cfg(unix)]
pub fn is_port_in_use(port: u16) -> bool {
    use std::process::Command;
    Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

#[cfg(not(unix))]
pub fn is_port_in_use(port: u16) -> bool {
    use std::net::TcpListener;
    TcpListener::bind(("127.0.0.1", port)).is_err()
}

#[cfg(unix)]
fn is_pid_running(pid: u32) -> bool {
    use std::process::Command;
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_pid_running(pid: u32) -> bool {
    true
}
