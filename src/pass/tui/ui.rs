use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use super::app::{App, AppMode, ItemFilter, StatusType};
use crate::pass::types::VaultItem;

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(5),    // Content (list + preview)
            Constraint::Length(2), // Footer
        ])
        .split(f.area());

    render_header(f, chunks[0], app);
    render_content(f, chunks[1], app);
    render_footer(f, chunks[2], app);
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let items = app.filtered_items();
    let total = app.vault.items.iter().filter(|i| i.deleted_at().is_none()).count();

    let filter_info = if app.filter == ItemFilter::All {
        format!("{} items", total)
    } else {
        format!("{} {} ({} total)", items.len(), app.filter.label().to_lowercase(), total)
    };

    let title = match &app.mode {
        AppMode::Search(query) => {
            format!(" 🔍 Search: {} ", query)
        }
        AppMode::Normal => {
            format!(" 🔐 Groo Pass ({}) ", filter_info)
        }
    };

    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(header, area);
}

fn render_content(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // List
            Constraint::Percentage(60), // Preview
        ])
        .split(area);

    render_list(f, chunks[0], app);
    render_preview(f, chunks[1], app);
}

fn render_list(f: &mut Frame, area: Rect, app: &App) {
    let filtered = app.filtered_items();

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let selected = i == app.selected;
            let prefix = if selected { "▸" } else { " " };
            let icon = item.type_icon();
            let name = item.name();


            // Build display line
            let username_display = match item {
                VaultItem::Password(p) if !p.username.is_empty() => {
                    Span::styled(format!(" ({})", truncate(&p.username, 20)), Style::default().fg(Color::DarkGray))
                }
                VaultItem::Card(c) => {
                    let last4 = if c.number.len() >= 4 {
                        &c.number[c.number.len() - 4..]
                    } else {
                        &c.number
                    };
                    Span::styled(format!(" ****{}", last4), Style::default().fg(Color::DarkGray))
                }
                VaultItem::BankAccount(b) => {
                    Span::styled(format!(" ({})", truncate(&b.bank_name, 15)), Style::default().fg(Color::DarkGray))
                }
                _ => Span::raw(""),
            };

            let style = if selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let line = Line::from(vec![
                Span::styled(format!("{} {} ", prefix, icon), style),
                Span::styled(truncate(name, 25), style),
                username_display,
            ]);

            ListItem::new(line)
        })
        .collect();

    let title = format!(" Items [{}] ", app.filter.label());
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL));

    f.render_widget(list, area);
}

fn render_preview(f: &mut Frame, area: Rect, app: &App) {
    let content = if let Some(item) = app.selected_item() {
        match item {
            VaultItem::Password(p) => {
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(&p.name),
                    ]),
                    Line::from(vec![
                        Span::styled("Username: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(&p.username),
                    ]),
                    Line::from(vec![
                        Span::styled("Password: ", Style::default().fg(Color::DarkGray)),
                        Span::styled("••••••••••••", Style::default().fg(Color::Yellow)),
                        Span::styled("  [c] to copy", Style::default().fg(Color::DarkGray)),
                    ]),
                ];

                if !p.urls.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled("URLs:", Style::default().fg(Color::DarkGray))));
                    for url in &p.urls {
                        lines.push(Line::from(format!("  • {}", url)));
                    }
                }

                if p.totp.is_some() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("TOTP: ", Style::default().fg(Color::DarkGray)),
                        Span::styled("Configured", Style::default().fg(Color::Green)),
                        Span::styled("  [t] to copy code", Style::default().fg(Color::DarkGray)),
                    ]));
                }

                if let Some(ref notes) = p.notes
                    && !notes.is_empty() {
                        lines.push(Line::from(""));
                        lines.push(Line::from(Span::styled("Notes:", Style::default().fg(Color::DarkGray))));
                        for line in notes.lines().take(5) {
                            lines.push(Line::from(format!("  {}", line)));
                        }
                    }

                lines
            }
            VaultItem::Card(c) => {
                let masked_number = mask_card_number(&c.number);
                vec![
                    Line::from(vec![
                        Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(&c.name),
                    ]),
                    Line::from(vec![
                        Span::styled("Cardholder: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(&c.cardholder_name),
                    ]),
                    Line::from(vec![
                        Span::styled("Number: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(masked_number, Style::default()),
                        Span::styled("  [c] to copy", Style::default().fg(Color::DarkGray)),
                    ]),
                    Line::from(vec![
                        Span::styled("Expiry: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(format!("{}/{}", c.exp_month, c.exp_year)),
                    ]),
                    Line::from(vec![
                        Span::styled("CVV: ", Style::default().fg(Color::DarkGray)),
                        Span::styled("•••", Style::default().fg(Color::Yellow)),
                    ]),
                ]
            }
            VaultItem::Note(n) => {
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(&n.name),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled("Content:", Style::default().fg(Color::DarkGray))),
                ];
                for line in n.content.lines().take(10) {
                    lines.push(Line::from(format!("  {}", line)));
                }
                lines
            }
            VaultItem::BankAccount(b) => {
                let masked_account = mask_account(&b.account_number);
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(&b.name),
                    ]),
                    Line::from(vec![
                        Span::styled("Bank: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(&b.bank_name),
                    ]),
                    Line::from(vec![
                        Span::styled("Account: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(masked_account, Style::default()),
                        Span::styled("  [c] to copy", Style::default().fg(Color::DarkGray)),
                    ]),
                ];
                if let Some(ref routing) = b.routing_number {
                    lines.push(Line::from(vec![
                        Span::styled("Routing: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(routing),
                    ]));
                }
                lines
            }
            _ => {
                vec![Line::from(format!("{} {}", item.type_icon(), item.name()))]
            }
        }
    } else {
        vec![Line::from(Span::styled("No items", Style::default().fg(Color::DarkGray)))]
    };

    let preview = Paragraph::new(content)
        .wrap(Wrap { trim: false })
        .block(Block::default().title(" Details ").borders(Borders::ALL));

    f.render_widget(preview, area);
}

fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let status_text = if let Some((msg, status_type, _)) = &app.status_message {
        let style = match status_type {
            StatusType::Success => Style::default().fg(Color::Green),
            StatusType::Error => Style::default().fg(Color::Red),
            StatusType::Info => Style::default().fg(Color::Yellow),
        };
        Span::styled(msg.clone(), style)
    } else {
        let help = match app.mode {
            AppMode::Search(_) => "[Esc] cancel  [Enter] search",
            AppMode::Normal => "[↑↓/jk] navigate  [c] copy  [u] username  [t] totp  [/] search  [f] filter  [q] quit",
        };
        Span::styled(help, Style::default().fg(Color::DarkGray))
    };

    let footer = Paragraph::new(Line::from(status_text));
    f.render_widget(footer, area);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}…", &s[..max_len - 1])
    } else {
        s.to_string()
    }
}

fn mask_card_number(number: &str) -> String {
    if number.len() >= 4 {
        let last4 = &number[number.len() - 4..];
        format!("•••• •••• •••• {}", last4)
    } else {
        "•••• •••• •••• ••••".to_string()
    }
}

fn mask_account(number: &str) -> String {
    if number.len() > 4 {
        let last4 = &number[number.len() - 4..];
        format!("****{}", last4)
    } else {
        "****".to_string()
    }
}
