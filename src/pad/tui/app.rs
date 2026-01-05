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

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    None,
    Delete(String), // item_id
}

#[allow(dead_code)] // Some fields reserved for future features
pub struct App {
    pub items: Vec<DecryptedItem>,
    pub selected: usize,
    pub key: [u8; 32],
    pub token: String,
    pub should_quit: bool,
    pub status_message: Option<(String, StatusType, Instant)>,
    pub confirm_action: ConfirmAction,
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
            confirm_action: ConfirmAction::None,
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
        if let Some((_, _, created)) = &self.status_message {
            if created.elapsed().as_secs() >= 3 {
                self.status_message = None;
            }
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
            self.confirm_action = ConfirmAction::Delete(item.id.clone());
            self.set_status("Press 'y' to confirm delete, any other key to cancel", StatusType::Info);
        }
    }

    pub fn cancel_confirm(&mut self) {
        self.confirm_action = ConfirmAction::None;
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
