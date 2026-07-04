use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct DecryptedItem {
    pub id: String,
    pub text: String,
    pub files: Vec<DecryptedFile>,
    pub created_at: i64, // milliseconds since epoch
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields reserved for future features
pub struct DecryptedFile {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
    pub r2_key: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatusType {
    Success,
    Error,
    Info,
}

/// App mode for handling different UI states
pub enum AppMode {
    Normal,
    ConfirmDelete(String), // item_id
    DirectoryPicker(DirPickerState),
}

/// State for the directory picker overlay
pub struct DirPickerState {
    pub current_dir: PathBuf,
    pub entries: Vec<DirEntry>,
    pub selected: usize,
}

/// A directory entry in the picker
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
}

impl DirPickerState {
    pub fn new(start_dir: PathBuf) -> std::io::Result<Self> {
        let mut state = Self {
            current_dir: start_dir,
            entries: Vec::new(),
            selected: 0,
        };
        state.refresh()?;
        Ok(state)
    }

    pub fn refresh(&mut self) -> std::io::Result<()> {
        self.entries.clear();

        // Add parent directory entry
        if let Some(parent) = self.current_dir.parent() {
            self.entries.push(DirEntry {
                name: "..".to_string(),
                path: parent.to_path_buf(),
            });
        }

        // Read directory entries (directories only)
        let mut dirs: Vec<DirEntry> = std::fs::read_dir(&self.current_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.metadata().map(|m| m.is_dir()).unwrap_or(false))
            .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
            .map(|entry| DirEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path: entry.path(),
            })
            .collect();

        // Sort alphabetically (case-insensitive)
        dirs.sort_by_key(|a| a.name.to_lowercase());
        self.entries.extend(dirs);

        self.selected = 0;
        Ok(())
    }

    pub fn navigate_into(&mut self) -> std::io::Result<()> {
        if let Some(entry) = self.entries.get(self.selected) {
            self.current_dir = entry.path.clone();
            self.refresh()?;
        }
        Ok(())
    }

    pub fn select_next(&mut self) {
        if self.selected < self.entries.len().saturating_sub(1) {
            self.selected += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
}

#[allow(dead_code)] // Some fields reserved for future features
pub struct App {
    pub items: Vec<DecryptedItem>,
    pub selected: usize,
    pub key: [u8; 32],
    pub token: String,
    pub should_quit: bool,
    pub status_message: Option<(String, StatusType, Instant)>,
    pub mode: AppMode,
    pub scroll_offset: usize,
}

impl App {
    pub fn new(items: Vec<DecryptedItem>, key: [u8; 32], token: String) -> Self {
        Self {
            items,
            selected: 0,
            key,
            token,
            should_quit: false,
            status_message: None,
            mode: AppMode::Normal,
            scroll_offset: 0,
        }
    }

    pub fn selected_item(&self) -> Option<&DecryptedItem> {
        self.items.get(self.selected)
    }

    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1).min(self.items.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn set_status(&mut self, message: &str, status_type: StatusType) {
        self.status_message = Some((message.to_string(), status_type, Instant::now()));
    }

    pub fn set_error(&mut self, message: &str) {
        self.set_status(message, StatusType::Error);
    }

    pub fn set_success(&mut self, message: &str) {
        self.set_status(message, StatusType::Success);
    }

    pub fn clear_status_if_expired(&mut self) {
        if let Some((_, _, created)) = &self.status_message
            && created.elapsed().as_secs() >= 3
        {
            self.status_message = None;
        }
    }

    pub fn remove_item(&mut self, id: &str) {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            self.items.remove(idx);
            if self.selected >= self.items.len() && self.selected > 0 {
                self.selected -= 1;
            }
        }
    }

    pub fn start_delete_confirm(&mut self) {
        if let Some(item) = self.selected_item() {
            self.mode = AppMode::ConfirmDelete(item.id.clone());
            self.set_status("Press 'y' to confirm delete, any other key to cancel", StatusType::Info);
        }
    }

    pub fn start_dir_picker(&mut self) {
        let start = dirs::download_dir().unwrap_or_else(|| PathBuf::from("."));
        match DirPickerState::new(start) {
            Ok(state) => self.mode = AppMode::DirectoryPicker(state),
            Err(e) => self.set_error(&format!("Failed to open directory: {}", e)),
        }
    }

    pub fn cancel_mode(&mut self) {
        self.mode = AppMode::Normal;
        self.status_message = None;
    }

    pub fn update_items(&mut self, items: Vec<DecryptedItem>) {
        self.items = items;
        // Adjust selection if needed
        if self.selected >= self.items.len() && !self.items.is_empty() {
            self.selected = self.items.len() - 1;
        } else if self.items.is_empty() {
            self.selected = 0;
        }
    }
}
