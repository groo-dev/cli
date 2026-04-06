#![allow(dead_code)]

use anyhow::Result;
use console::{style, Term};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const MAX_LOG_LINES: usize = 5;

pub struct Ui {
    term: Term,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            term: Term::stderr(),
        }
    }

    pub fn header(&self) {
        let _ = self.term.write_line(&format!(
            "\n  {} {}\n",
            "Launchpad",
            style("🚀").dim()
        ));
    }

    pub fn section(&self, title: &str) {
        let _ = self.term.write_line(&format!("  {}", style(title).bold()));
    }

    pub fn success(&self, message: &str) {
        let _ = self.term.write_line(&format!(
            "  {} {}",
            style("✓").green().bold(),
            message
        ));
    }

    pub fn skipped(&self, message: &str) {
        let _ = self.term.write_line(&format!(
            "  {} {} {}",
            style("✓").green().bold(),
            message,
            style("— skipped").dim()
        ));
    }

    pub fn failure(&self, message: &str) {
        let _ = self.term.write_line(&format!(
            "  {} {}",
            style("✗").red().bold(),
            style(message).red()
        ));
    }

    pub fn log_line(&self, line: &str) {
        let _ = self.term.write_line(&format!(
            "    {} {}",
            style(">").dim(),
            style(line).dim()
        ));
    }

    pub fn done(&self) {
        let _ = self.term.write_line(&format!(
            "\n  {} {}\n",
            style("Done!").green().bold(),
            "Run \"groo dev\" to start building."
        ));
    }

    pub fn newline(&self) {
        let _ = self.term.write_line("");
    }

    /// Run a shell command with live spinner and streaming output.
    /// On success: clears log lines, shows checkmark with summary.
    /// On failure: keeps log lines visible, shows error.
    pub async fn run_command(
        &self,
        description: &str,
        command: &str,
        working_dir: &std::path::Path,
    ) -> Result<String> {
        let stop = Arc::new(AtomicBool::new(false));
        let log_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let lines_displayed = Arc::new(Mutex::new(0usize));

        // Start spinner
        let stop_clone = stop.clone();
        let term_clone = self.term.clone();
        let desc = description.to_string();
        let log_lines_clone = log_lines.clone();
        let lines_displayed_clone = lines_displayed.clone();
        let spinner_handle = tokio::spawn(async move {
            let mut frame_idx = 0;
            loop {
                if stop_clone.load(Ordering::Relaxed) {
                    break;
                }

                // Clear previous spinner + log lines
                let displayed = *lines_displayed_clone.lock().await;
                for _ in 0..displayed + 1 {
                    let _ = term_clone.clear_line();
                    let _ = term_clone.move_cursor_up(1);
                }
                let _ = term_clone.clear_line();

                // Write spinner line
                let spinner_char = SPINNER_FRAMES[frame_idx % SPINNER_FRAMES.len()];
                let _ = write!(
                    &term_clone,
                    "  {} {}",
                    style(spinner_char).cyan().bold(),
                    &desc
                );
                let _ = term_clone.write_line("");

                // Write recent log lines
                let lines = log_lines_clone.lock().await;
                let start = if lines.len() > MAX_LOG_LINES {
                    lines.len() - MAX_LOG_LINES
                } else {
                    0
                };
                let visible_lines = &lines[start..];
                for line in visible_lines {
                    let _ = term_clone.write_line(&format!(
                        "    {} {}",
                        style(">").dim(),
                        style(line).dim()
                    ));
                }
                *lines_displayed_clone.lock().await = visible_lines.len();

                frame_idx += 1;
                tokio::time::sleep(Duration::from_millis(80)).await;
            }
        });

        // Write initial spinner line (so first clear has something to clear)
        let _ = self.term.write_line(&format!(
            "  {} {}",
            style(SPINNER_FRAMES[0]).cyan().bold(),
            description
        ));

        // Spawn the process
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(working_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to run '{}': {}", command, e))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let all_output: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        // Read stdout
        let log_lines_clone = log_lines.clone();
        let all_output_clone = all_output.clone();
        let stdout_handle = tokio::spawn(async move {
            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    log_lines_clone.lock().await.push(line.clone());
                    all_output_clone.lock().await.push(line);
                }
            }
        });

        // Read stderr
        let log_lines_clone = log_lines.clone();
        let all_output_clone = all_output.clone();
        let stderr_handle = tokio::spawn(async move {
            if let Some(stderr) = stderr {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    log_lines_clone.lock().await.push(line.clone());
                    all_output_clone.lock().await.push(line);
                }
            }
        });

        // Wait for process
        let status = child.wait().await?;

        // Wait for readers
        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        // Stop spinner
        stop.store(true, Ordering::Relaxed);
        let _ = spinner_handle.await;

        // Clear spinner + log lines
        let displayed = *lines_displayed.lock().await;
        for _ in 0..displayed + 1 {
            let _ = self.term.clear_line();
            let _ = self.term.move_cursor_up(1);
        }
        let _ = self.term.clear_line();

        let output = all_output.lock().await.join("\n");

        if status.success() {
            self.success(description);
            Ok(output)
        } else {
            self.failure(description);
            // Show last few lines of output on failure
            let lines = log_lines.lock().await;
            let start = if lines.len() > MAX_LOG_LINES {
                lines.len() - MAX_LOG_LINES
            } else {
                0
            };
            for line in &lines[start..] {
                self.log_line(line);
            }
            anyhow::bail!(
                "Command failed: {}\n{}",
                command,
                output
            );
        }
    }
}
