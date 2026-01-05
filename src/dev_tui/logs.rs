use chrono::{DateTime, Local};
use ratatui::style::Color;
use std::collections::HashMap;

/// A single log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub service: String,
    pub content: String,
    pub is_stderr: bool,
    pub color: Color,
}

/// Log level parsed from content
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

impl LogEntry {
    /// Parse log level from content
    pub fn level(&self) -> LogLevel {
        let lower = self.content.to_lowercase();
        if self.is_stderr || lower.contains("error") || lower.contains("err:") {
            LogLevel::Error
        } else if lower.contains("warn") || lower.contains("warning") {
            LogLevel::Warn
        } else if lower.contains("debug") {
            LogLevel::Debug
        } else {
            LogLevel::Info
        }
    }
}

/// Message sent from process to TUI
#[derive(Debug, Clone)]
pub struct LogMessage {
    pub service: String,
    pub content: String,
    pub is_stderr: bool,
    pub timestamp: DateTime<Local>,
    pub color: Color,
}

/// Ring buffer for log entries
pub struct LogBuffer {
    entries: Vec<LogEntry>,
    max_entries: usize,
    /// Index of entries per service for quick filtering
    service_indexes: HashMap<String, Vec<usize>>,
}

impl LogBuffer {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_entries),
            max_entries,
            service_indexes: HashMap::new(),
        }
    }

    /// Push a new log message
    pub fn push(&mut self, msg: LogMessage) {
        let entry = LogEntry {
            timestamp: msg.timestamp,
            service: msg.service.clone(),
            content: msg.content,
            is_stderr: msg.is_stderr,
            color: msg.color,
        };

        // If at capacity, remove oldest entry
        if self.entries.len() >= self.max_entries {
            // Remove oldest entry and update indexes
            let removed = self.entries.remove(0);
            if let Some(indexes) = self.service_indexes.get_mut(&removed.service) {
                indexes.remove(0);
                // Decrement all indexes
                for idx in indexes.iter_mut() {
                    *idx -= 1;
                }
            }
            // Decrement indexes for all other services
            for (name, indexes) in self.service_indexes.iter_mut() {
                if name != &removed.service {
                    for idx in indexes.iter_mut() {
                        *idx -= 1;
                    }
                }
            }
        }

        let index = self.entries.len();
        self.entries.push(entry);

        // Update service index
        self.service_indexes
            .entry(msg.service)
            .or_default()
            .push(index);
    }

    /// Get all entries
    #[allow(dead_code)]
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    /// Get entries filtered by selected services
    pub fn filtered_entries(&self, selected_services: &[&str]) -> Vec<&LogEntry> {
        if selected_services.is_empty() {
            return self.entries.iter().collect();
        }

        self.entries
            .iter()
            .filter(|e| selected_services.contains(&e.service.as_str()))
            .collect()
    }

    /// Get entries matching a search query
    pub fn search(&self, query: &str, selected_services: &[&str]) -> Vec<(usize, &LogEntry)> {
        let query_lower = query.to_lowercase();
        self.filtered_entries(selected_services)
            .into_iter()
            .enumerate()
            .filter(|(_, e)| e.content.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Total number of entries
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if buffer is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.entries.clear();
        self.service_indexes.clear();
    }
}
