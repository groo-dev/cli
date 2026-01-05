use anyhow::Result;
use chrono::Local;
use ratatui::style::Color;
use std::path::PathBuf;
use tokio::process::Child;
use tokio::sync::{broadcast, mpsc};

use crate::config::get_service_log_file;
use crate::discovery::Service;
use crate::runner::get_color_for_index;

use super::logs::{LogBuffer, LogMessage};
use super::stats::StatsCollector;

/// Service status indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Starting,
    Error,
}

impl ServiceStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            ServiceStatus::Running => "●",
            ServiceStatus::Stopped => "○",
            ServiceStatus::Starting => "◐",
            ServiceStatus::Error => "●",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            ServiceStatus::Running => Color::Green,
            ServiceStatus::Stopped => Color::DarkGray,
            ServiceStatus::Starting => Color::Yellow,
            ServiceStatus::Error => Color::Red,
        }
    }
}

/// A service entry in the TUI
pub struct ServiceEntry {
    pub name: String,
    pub path: PathBuf,
    #[allow(dead_code)]
    pub dev_command: String,
    pub port: Option<u16>,
    pub status: ServiceStatus,
    pub child: Option<Child>,
    pub pid: Option<u32>,
    pub color: Color,
    pub selected: bool, // Whether logs are visible
}

impl ServiceEntry {
    pub fn from_service(service: &Service, index: usize) -> Self {
        let color = get_color_for_index(index);
        let ratatui_color = style_to_ratatui_color(&color);

        Self {
            name: service.name.clone(),
            path: service.path.clone(),
            dev_command: service.dev_command.clone(),
            port: service.port,
            status: ServiceStatus::Stopped,
            child: None,
            pid: None,
            color: ratatui_color,
            selected: true, // Select all by default
        }
    }
}

/// Convert console::Style color to ratatui Color
fn style_to_ratatui_color(_style: &console::Style) -> Color {
    // console::Style doesn't expose the color directly, so we use a mapping
    // based on the index from get_color_for_index
    // This is a workaround; in practice we just use predefined colors
    Color::Cyan // Default fallback
}

/// Search state
pub struct SearchState {
    pub query: String,
    pub matches: Vec<usize>, // Indexes of matching entries in filtered view
    pub current_match: usize,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            matches: Vec::new(),
            current_match: 0,
        }
    }
}

/// Application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Search,
    Help,
    Visual,
}

/// Which pane has focus for keyboard navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneFocus {
    Services,
    Logs,
}

/// Visual mode selection state
pub struct VisualSelection {
    pub anchor: usize,  // Starting line index (in filtered view)
    pub cursor: usize,  // Current line index (can be above or below anchor)
}

impl VisualSelection {
    pub fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.cursor), self.anchor.max(self.cursor))
    }

    pub fn line_count(&self) -> usize {
        let (start, end) = self.range();
        end - start + 1
    }
}

/// Main application state
pub struct App {
    pub project_name: String,
    #[allow(dead_code)]
    pub git_root: PathBuf,
    pub services: Vec<ServiceEntry>,
    pub cursor: usize,
    pub logs: LogBuffer,
    pub scroll_offset: usize,
    pub follow_mode: bool,
    pub mode: AppMode,
    pub focus: PaneFocus,
    pub log_cursor: Option<usize>, // Position in filtered logs (None = no cursor yet)
    pub search: SearchState,
    pub visual: Option<VisualSelection>,
    pub stats: StatsCollector,
    pub should_quit: bool,
    pub status_message: Option<String>,
    #[allow(dead_code)]
    shutdown_tx: broadcast::Sender<()>,
    log_tx: mpsc::UnboundedSender<LogMessage>,
}

impl App {
    pub fn new(
        project_name: String,
        git_root: PathBuf,
        services: Vec<Service>,
        shutdown_tx: broadcast::Sender<()>,
        log_tx: mpsc::UnboundedSender<LogMessage>,
    ) -> Self {
        let service_entries: Vec<ServiceEntry> = services
            .iter()
            .enumerate()
            .map(|(i, s)| ServiceEntry::from_service(s, i))
            .collect();

        // Assign distinct colors
        let colors = [
            Color::Cyan,
            Color::Magenta,
            Color::Yellow,
            Color::Green,
            Color::Blue,
            Color::Red,
            Color::LightCyan,
            Color::LightMagenta,
        ];

        let mut entries = service_entries;
        for (i, entry) in entries.iter_mut().enumerate() {
            entry.color = colors[i % colors.len()];
        }

        Self {
            project_name,
            git_root,
            services: entries,
            cursor: 0,
            logs: LogBuffer::new(10000),
            scroll_offset: 0,
            follow_mode: true,
            mode: AppMode::Normal,
            focus: PaneFocus::Services,
            log_cursor: None,
            search: SearchState::new(),
            visual: None,
            stats: StatsCollector::new(),
            should_quit: false,
            status_message: None,
            shutdown_tx,
            log_tx,
        }
    }

    /// Get the currently highlighted service
    #[allow(dead_code)]
    pub fn current_service(&self) -> Option<&ServiceEntry> {
        self.services.get(self.cursor)
    }

    /// Get mutable reference to current service
    #[allow(dead_code)]
    pub fn current_service_mut(&mut self) -> Option<&mut ServiceEntry> {
        self.services.get_mut(self.cursor)
    }

    /// Move cursor up
    pub fn cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor down
    pub fn cursor_down(&mut self) {
        if self.cursor < self.services.len().saturating_sub(1) {
            self.cursor += 1;
        }
    }

    /// Toggle selection of current service
    pub fn toggle_selection(&mut self) {
        if let Some(service) = self.services.get_mut(self.cursor) {
            service.selected = !service.selected;
        }
    }

    /// Select all services
    pub fn select_all(&mut self) {
        for service in &mut self.services {
            service.selected = true;
        }
    }

    /// Deselect all services
    pub fn select_none(&mut self) {
        for service in &mut self.services {
            service.selected = false;
        }
    }

    /// Get list of selected service names
    pub fn selected_services(&self) -> Vec<&str> {
        self.services
            .iter()
            .filter(|s| s.selected)
            .map(|s| s.name.as_str())
            .collect()
    }

    /// Scroll logs up
    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
        self.follow_mode = false;
    }

    /// Scroll logs down
    pub fn scroll_down(&mut self, amount: usize, max_lines: usize) {
        let filtered_len = self.logs.filtered_entries(&self.selected_services()).len();
        let max_offset = filtered_len.saturating_sub(max_lines);
        self.scroll_offset = (self.scroll_offset + amount).min(max_offset);

        // Re-enable follow mode if scrolled to bottom
        if self.scroll_offset >= max_offset {
            self.follow_mode = true;
        }
    }

    /// Scroll to top
    #[allow(dead_code)]
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
        self.follow_mode = false;
    }

    /// Scroll to bottom (enable follow mode)
    pub fn scroll_to_bottom(&mut self) {
        let filtered_len = self.logs.filtered_entries(&self.selected_services()).len();
        self.scroll_offset = filtered_len;
        self.follow_mode = true;
    }

    /// Start all services
    pub async fn start_all_services(&mut self) -> Result<()> {
        for i in 0..self.services.len() {
            self.start_service(i).await?;
        }
        Ok(())
    }

    /// Start a specific service
    pub async fn start_service(&mut self, index: usize) -> Result<()> {
        let service = &mut self.services[index];
        if service.status == ServiceStatus::Running {
            return Ok(());
        }

        service.status = ServiceStatus::Starting;

        let log_tx = self.log_tx.clone();
        let name = service.name.clone();
        let path = service.path.clone();
        let color = service.color;
        let log_file = get_service_log_file(&path);

        // Spawn the process
        match spawn_service_with_channel(&name, &path, log_file, color, log_tx).await {
            Ok((child, pid)) => {
                service.child = Some(child);
                service.pid = Some(pid);
                service.status = ServiceStatus::Running;

                // Log start message
                let _ = self.log_tx.send(LogMessage {
                    service: name,
                    content: "Service started".to_string(),
                    is_stderr: false,
                    timestamp: Local::now(),
                    color,
                });
            }
            Err(e) => {
                service.status = ServiceStatus::Error;
                let _ = self.log_tx.send(LogMessage {
                    service: name,
                    content: format!("Failed to start: {}", e),
                    is_stderr: true,
                    timestamp: Local::now(),
                    color,
                });
            }
        }

        Ok(())
    }

    /// Stop a specific service
    pub async fn stop_service(&mut self, index: usize) -> Result<()> {
        let service = &mut self.services[index];
        if service.status != ServiceStatus::Running {
            return Ok(());
        }

        // Kill the process group to ensure all child processes are killed
        if let Some(pid) = service.pid {
            #[cfg(unix)]
            {
                use nix::sys::signal::{killpg, Signal};
                use nix::unistd::Pid;
                // Kill the entire process group
                let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGTERM);
                // Give processes a moment to terminate gracefully
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                // Force kill if still running
                let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
            }

            #[cfg(not(unix))]
            {
                if let Some(ref mut child) = service.child {
                    let _ = child.start_kill();
                }
            }
        }

        if let Some(ref mut child) = service.child {
            let _ = child.wait().await;
        }

        let name = service.name.clone();
        let color = service.color;

        service.child = None;
        if let Some(pid) = service.pid.take() {
            self.stats.remove(pid);
        }
        service.status = ServiceStatus::Stopped;

        let _ = self.log_tx.send(LogMessage {
            service: name,
            content: "Service stopped".to_string(),
            is_stderr: false,
            timestamp: Local::now(),
            color,
        });

        Ok(())
    }

    /// Restart a specific service
    pub async fn restart_service(&mut self, index: usize) -> Result<()> {
        self.stop_service(index).await?;
        // Brief pause for port release
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        self.start_service(index).await?;
        Ok(())
    }

    /// Stop the current service
    pub async fn stop_current(&mut self) -> Result<()> {
        let index = self.cursor;
        self.stop_service(index).await
    }

    /// Restart the current service
    pub async fn restart_current(&mut self) -> Result<()> {
        let index = self.cursor;
        self.restart_service(index).await
    }

    /// Open the current service in browser
    pub fn open_current_in_browser(&mut self) {
        let Some(service) = self.services.get(self.cursor) else {
            return;
        };

        let Some(port) = service.port else {
            self.status_message = Some("No port configured".to_string());
            return;
        };

        let url = format!("http://localhost:{}", port);
        match open::that(&url) {
            Ok(_) => {
                self.status_message = Some(format!("Opened {}", url));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to open: {}", e));
            }
        }
    }

    /// Stop all services
    pub async fn stop_all(&mut self) -> Result<()> {
        for i in 0..self.services.len() {
            self.stop_service(i).await?;
        }
        Ok(())
    }

    /// Restart all running services
    pub async fn restart_all(&mut self) -> Result<()> {
        let running: Vec<usize> = self
            .services
            .iter()
            .enumerate()
            .filter(|(_, s)| s.status == ServiceStatus::Running)
            .map(|(i, _)| i)
            .collect();

        for i in running {
            self.restart_service(i).await?;
        }
        Ok(())
    }

    /// Check process statuses
    pub async fn check_processes(&mut self) {
        for service in &mut self.services {
            if service.status == ServiceStatus::Running
                && let Some(ref mut child) = service.child
            {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        // Process exited
                        let color = service.color;
                        let name = service.name.clone();

                        if status.success() {
                            service.status = ServiceStatus::Stopped;
                            let _ = self.log_tx.send(LogMessage {
                                service: name,
                                content: "Process exited".to_string(),
                                is_stderr: false,
                                timestamp: Local::now(),
                                color,
                            });
                        } else {
                            service.status = ServiceStatus::Error;
                            let _ = self.log_tx.send(LogMessage {
                                service: name,
                                content: format!("Process exited with status: {}", status),
                                is_stderr: true,
                                timestamp: Local::now(),
                                color,
                            });
                        }

                        if let Some(pid) = service.pid.take() {
                            self.stats.remove(pid);
                        }
                        service.child = None;
                    }
                    Ok(None) => {
                        // Still running
                    }
                    Err(_) => {
                        // Error checking status
                    }
                }
            }
        }
    }

    /// Update resource stats if needed
    pub fn maybe_update_stats(&mut self) {
        if self.stats.should_refresh() {
            let pids: Vec<u32> = self
                .services
                .iter()
                .filter_map(|s| s.pid)
                .collect();
            self.stats.refresh(&pids);
        }
    }

    /// Shutdown all services and cleanup
    pub async fn shutdown(&mut self) {
        let _ = self.stop_all().await;
    }

    /// Get number of running services
    pub fn running_count(&self) -> usize {
        self.services
            .iter()
            .filter(|s| s.status == ServiceStatus::Running)
            .count()
    }

    /// Enter search mode
    pub fn enter_search(&mut self) {
        self.mode = AppMode::Search;
        self.search = SearchState::new();
    }

    /// Exit search mode
    pub fn exit_search(&mut self) {
        self.mode = AppMode::Normal;
        self.search.query.clear();
        self.search.matches.clear();
    }

    /// Update search results
    pub fn update_search(&mut self) {
        if self.search.query.is_empty() {
            self.search.matches.clear();
            return;
        }

        let selected = self.selected_services();
        let results = self.logs.search(&self.search.query, &selected);
        self.search.matches = results.into_iter().map(|(i, _)| i).collect();
        self.search.current_match = 0;
    }

    /// Go to next search match
    pub fn next_match(&mut self) {
        if !self.search.matches.is_empty() {
            self.search.current_match = (self.search.current_match + 1) % self.search.matches.len();
            // Scroll to match
            if let Some(&idx) = self.search.matches.get(self.search.current_match) {
                self.scroll_offset = idx.saturating_sub(5);
                self.follow_mode = false;
            }
        }
    }

    /// Go to previous search match
    pub fn prev_match(&mut self) {
        if !self.search.matches.is_empty() {
            self.search.current_match = if self.search.current_match == 0 {
                self.search.matches.len() - 1
            } else {
                self.search.current_match - 1
            };
            // Scroll to match
            if let Some(&idx) = self.search.matches.get(self.search.current_match) {
                self.scroll_offset = idx.saturating_sub(5);
                self.follow_mode = false;
            }
        }
    }

    /// Toggle help overlay
    pub fn toggle_help(&mut self) {
        self.mode = if self.mode == AppMode::Help {
            AppMode::Normal
        } else {
            AppMode::Help
        };
    }

    /// Toggle focus between Services and Logs panes
    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            PaneFocus::Services => {
                // Initialize log cursor when switching to logs
                if self.log_cursor.is_none() {
                    let total = self.logs.filtered_entries(&self.selected_services()).len();
                    self.log_cursor = Some(total.saturating_sub(1)); // Start at bottom
                }
                self.follow_mode = false;
                PaneFocus::Logs
            }
            PaneFocus::Logs => PaneFocus::Services,
        };
    }

    /// Move log cursor up
    pub fn log_cursor_up(&mut self) {
        if let Some(ref mut cursor) = self.log_cursor
            && *cursor > 0
        {
            *cursor -= 1;
            // Auto-scroll if cursor goes above visible area
            if *cursor < self.scroll_offset {
                self.scroll_offset = *cursor;
            }
        }
    }

    /// Move log cursor down
    pub fn log_cursor_down(&mut self, visible_height: usize) {
        let max = self.logs.filtered_entries(&self.selected_services()).len();
        if let Some(ref mut cursor) = self.log_cursor
            && *cursor < max.saturating_sub(1)
        {
            *cursor += 1;
            // Auto-scroll if cursor goes below visible area
            let visible_end = self.scroll_offset + visible_height;
            if *cursor >= visible_end {
                self.scroll_offset = cursor.saturating_sub(visible_height) + 1;
            }
        }
    }

    /// Move log cursor to top
    pub fn log_cursor_top(&mut self) {
        self.log_cursor = Some(0);
        self.scroll_offset = 0;
        self.follow_mode = false;
    }

    /// Move log cursor to bottom
    pub fn log_cursor_bottom(&mut self) {
        let max = self.logs.filtered_entries(&self.selected_services()).len();
        self.log_cursor = Some(max.saturating_sub(1));
        self.follow_mode = true;
    }

    /// Copy current line at log cursor to clipboard
    pub fn copy_current_line(&mut self) -> Result<()> {
        use arboard::Clipboard;

        let Some(cursor) = self.log_cursor else {
            return Ok(());
        };

        let filtered = self.logs.filtered_entries(&self.selected_services());
        let Some(entry) = filtered.get(cursor) else {
            return Ok(());
        };

        let text = format!(
            "[{}] {} {}",
            entry.service,
            entry.timestamp.format("%H:%M:%S"),
            entry.content
        );

        match Clipboard::new() {
            Ok(mut clipboard) => {
                if let Err(e) = clipboard.set_text(&text) {
                    self.status_message = Some(format!("Copy failed: {}", e));
                } else {
                    self.status_message = Some("Copied line".to_string());
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Clipboard error: {}", e));
            }
        }

        Ok(())
    }

    /// Enter visual mode for log selection
    pub fn enter_visual_mode(&mut self) {
        let filtered = self.logs.filtered_entries(&self.selected_services());
        if filtered.is_empty() {
            self.status_message = Some("No logs to select".to_string());
            return;
        }

        // Use log_cursor if available, otherwise scroll_offset
        let anchor = self
            .log_cursor
            .unwrap_or(self.scroll_offset)
            .min(filtered.len().saturating_sub(1));

        self.visual = Some(VisualSelection { anchor, cursor: anchor });
        self.mode = AppMode::Visual;
        self.follow_mode = false;
    }

    /// Exit visual mode
    pub fn exit_visual_mode(&mut self) {
        self.visual = None;
        self.mode = AppMode::Normal;
    }

    /// Move visual selection cursor up
    pub fn visual_move_up(&mut self) {
        if let Some(ref mut sel) = self.visual
            && sel.cursor > 0
        {
            sel.cursor -= 1;
            // Auto-scroll if cursor goes above visible area
            if sel.cursor < self.scroll_offset {
                self.scroll_offset = sel.cursor;
            }
        }
    }

    /// Move visual selection cursor down
    pub fn visual_move_down(&mut self, visible_height: usize) {
        // Get max before borrowing visual mutably
        let max = self.logs.filtered_entries(&self.selected_services()).len();

        if let Some(ref mut sel) = self.visual
            && sel.cursor < max.saturating_sub(1)
        {
            sel.cursor += 1;
            // Auto-scroll if cursor goes below visible area
            let visible_end = self.scroll_offset + visible_height;
            if sel.cursor >= visible_end {
                self.scroll_offset = sel.cursor.saturating_sub(visible_height) + 1;
            }
        }
    }

    /// Copy selected lines to clipboard
    pub fn copy_selection(&mut self) -> Result<()> {
        use arboard::Clipboard;

        let Some(ref sel) = self.visual else {
            return Ok(());
        };

        let (start, end) = sel.range();
        let filtered = self.logs.filtered_entries(&self.selected_services());

        if end >= filtered.len() {
            return Ok(());
        }

        let lines: Vec<String> = filtered[start..=end]
            .iter()
            .map(|e| {
                format!(
                    "[{}] {} {}",
                    e.service,
                    e.timestamp.format("%H:%M:%S"),
                    e.content
                )
            })
            .collect();

        let text = lines.join("\n");
        let line_count = lines.len();

        match Clipboard::new() {
            Ok(mut clipboard) => {
                if let Err(e) = clipboard.set_text(&text) {
                    self.status_message = Some(format!("Copy failed: {}", e));
                } else {
                    self.status_message = Some(format!("Copied {} line(s)", line_count));
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Clipboard error: {}", e));
            }
        }

        Ok(())
    }
}

/// Spawn a service and send logs to channel
async fn spawn_service_with_channel(
    name: &str,
    path: &std::path::Path,
    log_file: PathBuf,
    color: Color,
    log_tx: mpsc::UnboundedSender<LogMessage>,
) -> Result<(Child, u32)> {
    use std::process::Stdio;
    use tokio::fs::OpenOptions;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::process::Command;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Ensure logs directory exists
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
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    // Create a new process group so we can kill all children
    #[cfg(unix)]
    cmd.process_group(0);

    let mut child = cmd.spawn()?;
    let pid = child.id().unwrap_or(0);

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Spawn stdout reader
    if let Some(stdout) = stdout {
        let name = name.to_string();
        let log_tx = log_tx.clone();
        let log_writer = Arc::clone(&log_writer);
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = log_tx.send(LogMessage {
                    service: name.clone(),
                    content: line.clone(),
                    is_stderr: false,
                    timestamp: Local::now(),
                    color,
                });
                // Write to log file
                let mut file = log_writer.lock().await;
                let _ = file.write_all(format!("[{}] {}\n", name, line).as_bytes()).await;
                let _ = file.flush().await;
            }
        });
    }

    // Spawn stderr reader
    if let Some(stderr) = stderr {
        let name = name.to_string();
        let log_tx = log_tx.clone();
        let log_writer = Arc::clone(&log_writer);
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = log_tx.send(LogMessage {
                    service: name.clone(),
                    content: line.clone(),
                    is_stderr: true,
                    timestamp: Local::now(),
                    color,
                });
                // Write to log file
                let mut file = log_writer.lock().await;
                let _ = file.write_all(format!("[{}] {}\n", name, line).as_bytes()).await;
                let _ = file.flush().await;
            }
        });
    }

    Ok((child, pid))
}
