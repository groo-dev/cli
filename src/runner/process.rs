use anyhow::Result;
use console::Style;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, Mutex};

use super::output::{print_service_error, print_service_log};

pub struct ProcessHandle {
    pub name: String,
    pub child: Child,
    pub color: Style,
}

impl ProcessHandle {
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }
}

pub async fn spawn_service(
    name: &str,
    path: &Path,
    _command: &str,
    color: Style,
    log_file: PathBuf,
) -> Result<ProcessHandle> {
    // Ensure logs directory exists and truncate log file
    if let Some(parent) = log_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_file)
        .await?;
    let log_writer = Arc::new(Mutex::new(file));

    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(format!("cd {} && npm run dev", path.display()))
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let name_clone = name.to_string();
    let color_clone = color.clone();

    // Spawn stdout reader
    if let Some(stdout) = stdout {
        let name = name_clone.clone();
        let color = color_clone.clone();
        let log_writer = Arc::clone(&log_writer);
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                print_service_log(&name, &line, &color);
                // Write to log file
                let mut file = log_writer.lock().await;
                let _ = file.write_all(format!("[{}] {}\n", name, line).as_bytes()).await;
                let _ = file.flush().await;
            }
        });
    }

    // Spawn stderr reader
    if let Some(stderr) = stderr {
        let name = name_clone.clone();
        let color = color_clone.clone();
        let log_writer = Arc::clone(&log_writer);
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                print_service_error(&name, &line, &color);
                // Write to log file
                let mut file = log_writer.lock().await;
                let _ = file.write_all(format!("[{}] {}\n", name, line).as_bytes()).await;
                let _ = file.flush().await;
            }
        });
    }

    Ok(ProcessHandle {
        name: name.to_string(),
        child,
        color,
    })
}

pub async fn wait_for_processes(
    mut handles: Vec<ProcessHandle>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                // Shutdown signal received, kill all processes and wait for them
                for handle in &mut handles {
                    let _ = handle.child.start_kill();
                }
                for handle in &mut handles {
                    let _ = handle.child.wait().await;
                }
                break;
            }
            // Check if any process has exited
            result = async {
                for (i, handle) in handles.iter_mut().enumerate() {
                    if let Ok(Some(status)) = handle.child.try_wait() {
                        return Some((i, status));
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                None
            } => {
                if let Some((index, status)) = result {
                    let handle = &handles[index];
                    let color = &handle.color;
                    if status.success() {
                        print_service_log(&handle.name, "Process exited", color);
                    } else {
                        print_service_error(
                            &handle.name,
                            &format!("Process exited with status: {}", status),
                            color,
                        );
                    }
                    handles.remove(index);

                    if handles.is_empty() {
                        break;
                    }
                }
            }
        }
    }
}
