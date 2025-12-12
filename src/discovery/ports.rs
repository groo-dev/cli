use regex::Regex;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum FrameworkType {
    NextJs,
    Vite,
    Wrangler,
    Unknown,
}

pub fn detect_port(framework: &FrameworkType, dev_command: &str, service_dir: &Path) -> Option<u16> {
    match framework {
        FrameworkType::NextJs => detect_nextjs_port(dev_command),
        FrameworkType::Vite => detect_vite_port(service_dir),
        FrameworkType::Wrangler => detect_wrangler_port(service_dir),
        FrameworkType::Unknown => detect_port_from_command(dev_command),
    }
}

fn detect_nextjs_port(dev_command: &str) -> Option<u16> {
    // Match -p 3001 or --port 3001 or -p=3001 or --port=3001
    let re = Regex::new(r"(?:-p|--port)[=\s]+(\d+)").ok()?;
    re.captures(dev_command)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse().ok())
        .or(Some(3000)) // Next.js default
}

fn detect_vite_port(service_dir: &Path) -> Option<u16> {
    // Try vite.config.ts first, then vite.config.js
    let config_files = ["vite.config.ts", "vite.config.js", "vite.config.mts", "vite.config.mjs"];

    for config_file in &config_files {
        let config_path = service_dir.join(config_file);
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                // Look for server.port or port: in the config
                let re = Regex::new(r"port\s*:\s*(\d+)").ok()?;
                if let Some(cap) = re.captures(&content) {
                    if let Some(m) = cap.get(1) {
                        if let Ok(port) = m.as_str().parse() {
                            return Some(port);
                        }
                    }
                }
            }
        }
    }

    Some(5173) // Vite default
}

fn detect_wrangler_port(service_dir: &Path) -> Option<u16> {
    // Try wrangler.jsonc first, then wrangler.toml
    let jsonc_path = service_dir.join("wrangler.jsonc");
    if jsonc_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&jsonc_path) {
            // Simple regex to find port in JSON (handles comments by just looking for pattern)
            let re = Regex::new(r#""port"\s*:\s*(\d+)"#).ok()?;
            if let Some(cap) = re.captures(&content) {
                if let Some(m) = cap.get(1) {
                    if let Ok(port) = m.as_str().parse() {
                        return Some(port);
                    }
                }
            }
        }
    }

    let toml_path = service_dir.join("wrangler.toml");
    if toml_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&toml_path) {
            // Parse TOML and look for dev.port
            if let Ok(value) = content.parse::<toml::Value>() {
                if let Some(port) = value
                    .get("dev")
                    .and_then(|d| d.get("port"))
                    .and_then(|p| p.as_integer())
                {
                    return Some(port as u16);
                }
            }
        }
    }

    Some(8787) // Wrangler default
}

fn detect_port_from_command(dev_command: &str) -> Option<u16> {
    // Generic port detection from command
    let re = Regex::new(r"(?:-p|--port)[=\s]+(\d+)").ok()?;
    re.captures(dev_command)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse().ok())
}
