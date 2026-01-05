use chrono::{TimeZone, Utc};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use super::app::{App, AppMode, DirPickerState, StatusType};

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(5),     // Content (list + preview)
            Constraint::Length(2),  // Footer
        ])
        .split(f.area());

    render_header(f, chunks[0], app);
    render_content(f, chunks[1], app);
    render_footer(f, chunks[2], app);

    // Render overlay if in picker mode
    if let AppMode::DirectoryPicker(ref picker) = app.mode {
        render_dir_picker(f, picker);
    }
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let item_count = app.items.len();
    let title = format!(" Groo Pad ({} items) ", item_count);

    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(header, area);
}

fn render_content(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // List
            Constraint::Percentage(60), // Preview
        ])
        .split(area);

    render_list(f, chunks[0], app);
    render_preview(f, chunks[1], app);
}

fn render_list(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let selected = i == app.selected;
            let prefix = if selected { ">" } else { " " };

            // Truncate text preview
            let text_preview: String = item.text
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(40)
                .collect();
            let text_preview = if item.text.len() > 40 {
                format!("{}...", text_preview)
            } else {
                text_preview
            };

            // File count
            let file_info = if item.files.is_empty() {
                "-".to_string()
            } else if item.files.len() == 1 {
                "1 file".to_string()
            } else {
                format!("{} files", item.files.len())
            };

            // Relative time
            let time = format_relative_time(item.created_at);

            let content = format!(
                "{} {:40}  {:8}  {:>6}",
                prefix,
                text_preview,
                file_info,
                time
            );

            let style = if selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title(" Items ").borders(Borders::ALL));

    f.render_widget(list, area);
}

fn render_preview(f: &mut Frame, area: Rect, app: &App) {
    let content = if let Some(item) = app.selected_item() {
        let mut lines = vec![item.text.clone()];

        if !item.files.is_empty() {
            lines.push(String::new());
            lines.push("Files:".to_string());
            for file in &item.files {
                lines.push(format!("  • {} ({})", file.name, format_size(file.size)));
            }
        }

        lines.join("\n")
    } else {
        "No items".to_string()
    };

    let preview = Paragraph::new(content)
        .wrap(Wrap { trim: false })
        .block(Block::default().title(" Preview ").borders(Borders::ALL));

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
        Span::styled(
            "[↑↓] Navigate  [c] Copy  [d] Download  [x] Delete  [r] Refresh  [q] Quit",
            Style::default().fg(Color::DarkGray),
        )
    };

    let footer = Paragraph::new(Line::from(status_text))
        .style(Style::default());

    f.render_widget(footer, area);
}

fn format_relative_time(timestamp_ms: i64) -> String {
    let now = Utc::now();
    let then = Utc.timestamp_millis_opt(timestamp_ms).single();

    let Some(then) = then else {
        return "?".to_string();
    };

    let duration = now.signed_duration_since(then);

    if duration.num_seconds() < 60 {
        "now".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{}m", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{}h", duration.num_hours())
    } else if duration.num_days() < 30 {
        format!("{}d", duration.num_days())
    } else if duration.num_days() < 365 {
        format!("{}mo", duration.num_days() / 30)
    } else {
        format!("{}y", duration.num_days() / 365)
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn render_dir_picker(f: &mut Frame, picker: &DirPickerState) {
    let area = centered_rect(60, 70, f.area());

    // Clear background
    f.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Path display
            Constraint::Min(5),    // Directory list
            Constraint::Length(2), // Help
        ])
        .split(area);

    // Current path
    let path_display = Paragraph::new(format!(" {}", picker.current_dir.display()))
        .style(Style::default().fg(Color::Cyan))
        .block(
            Block::default()
                .title(" Select Directory ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
    f.render_widget(path_display, chunks[0]);

    // Directory list
    let items: Vec<ListItem> = picker
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let prefix = if i == picker.selected { ">" } else { " " };
            let icon = if entry.name == ".." { ".." } else { "/" };
            let display = if entry.name == ".." {
                "..".to_string()
            } else {
                format!("{}{}", entry.name, icon)
            };

            let style = if i == picker.selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(format!(" {} {}", prefix, display)).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(list, chunks[1]);

    // Help text
    let help = Paragraph::new(" [↑↓] Navigate  [Enter] Open  [Space] Select  [~] Home  [Esc] Cancel")
        .style(Style::default().fg(Color::DarkGray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
    f.render_widget(help, chunks[2]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
