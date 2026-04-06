use rand::Rng;
use std::collections::HashSet;

/// Generate `count` unique random ports in the range 10000-65535.
/// Checks that none of the generated ports are currently in use.
pub fn generate_ports(count: usize) -> Vec<u16> {
    let mut rng = rand::thread_rng();
    let mut ports = Vec::with_capacity(count);
    let mut used = HashSet::new();

    for _ in 0..count {
        loop {
            let port: u16 = rng.gen_range(10000..=65535);
            if !used.contains(&port) && !is_port_in_use(port) {
                used.insert(port);
                ports.push(port);
                break;
            }
        }
    }

    ports
}

#[cfg(unix)]
fn is_port_in_use(port: u16) -> bool {
    std::process::Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_port_in_use(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_err()
}
