use std::time::Instant;

use crate::pass::types::{Vault, VaultItem};

#[derive(Debug, Clone, PartialEq)]
pub enum StatusType {
    Success,
    Error,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemFilter {
    All,
    Passwords,
    Cards,
    Notes,
    BankAccounts,
}

impl ItemFilter {
    pub fn label(&self) -> &'static str {
        match self {
            ItemFilter::All => "All",
            ItemFilter::Passwords => "Passwords",
            ItemFilter::Cards => "Cards",
            ItemFilter::Notes => "Notes",
            ItemFilter::BankAccounts => "Bank",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            ItemFilter::All => ItemFilter::Passwords,
            ItemFilter::Passwords => ItemFilter::Cards,
            ItemFilter::Cards => ItemFilter::Notes,
            ItemFilter::Notes => ItemFilter::BankAccounts,
            ItemFilter::BankAccounts => ItemFilter::All,
        }
    }
}

/// App mode for handling different UI states
#[derive(Debug)]
pub enum AppMode {
    Normal,
    Search(String),
}

pub struct App {
    pub vault: Vault,
    pub key: [u8; 32],
    pub version: u32,
    pub selected: usize,
    pub filter: ItemFilter,
    pub should_quit: bool,
    pub status_message: Option<(String, StatusType, Instant)>,
    pub mode: AppMode,
    pub scroll_offset: usize,
}

impl App {
    pub fn new(vault: Vault, key: [u8; 32], version: u32) -> Self {
        Self {
            vault,
            key,
            version,
            selected: 0,
            filter: ItemFilter::All,
            should_quit: false,
            status_message: None,
            mode: AppMode::Normal,
            scroll_offset: 0,
        }
    }

    /// Get filtered items (excluding deleted)
    pub fn filtered_items(&self) -> Vec<&VaultItem> {
        let search_query = match &self.mode {
            AppMode::Search(q) => Some(q.to_lowercase()),
            AppMode::Normal => None,
        };

        self.vault
            .items
            .iter()
            .filter(|item| item.deleted_at().is_none())
            .filter(|item| {
                match self.filter {
                    ItemFilter::All => true,
                    ItemFilter::Passwords => matches!(item, VaultItem::Password(_)),
                    ItemFilter::Cards => matches!(item, VaultItem::Card(_)),
                    ItemFilter::Notes => matches!(item, VaultItem::Note(_)),
                    ItemFilter::BankAccounts => matches!(item, VaultItem::BankAccount(_)),
                }
            })
            .filter(|item| {
                if let Some(ref query) = search_query {
                    let name_match = item.name().to_lowercase().contains(query);
                    let username_match = match item {
                        VaultItem::Password(p) => p.username.to_lowercase().contains(query),
                        _ => false,
                    };
                    let url_match = match item {
                        VaultItem::Password(p) => {
                            p.urls.iter().any(|u| u.to_lowercase().contains(query))
                        }
                        _ => false,
                    };
                    name_match || username_match || url_match
                } else {
                    true
                }
            })
            .collect()
    }

    pub fn selected_item(&self) -> Option<&VaultItem> {
        self.filtered_items().get(self.selected).copied()
    }

    pub fn select_next(&mut self) {
        let items = self.filtered_items();
        if !items.is_empty() {
            self.selected = (self.selected + 1).min(items.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    pub fn select_last(&mut self) {
        let items = self.filtered_items();
        if !items.is_empty() {
            self.selected = items.len() - 1;
        }
    }

    pub fn cycle_filter(&mut self) {
        self.filter = self.filter.next();
        self.selected = 0;
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

    pub fn enter_search(&mut self) {
        self.mode = AppMode::Search(String::new());
    }

    pub fn exit_search(&mut self) {
        self.mode = AppMode::Normal;
        self.selected = 0;
    }

    pub fn search_input(&mut self, c: char) {
        if let AppMode::Search(ref mut query) = self.mode {
            query.push(c);
            self.selected = 0;
        }
    }

    pub fn search_backspace(&mut self) {
        if let AppMode::Search(ref mut query) = self.mode {
            query.pop();
            self.selected = 0;
        }
    }

    pub fn search_query(&self) -> Option<&str> {
        match &self.mode {
            AppMode::Search(q) => Some(q),
            _ => None,
        }
    }
}
