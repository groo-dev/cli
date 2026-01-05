use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{App, AppMode, PaneFocus};
use super::logs::LogLevel;

/// Main render function
pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(5),    // Main content
            Constraint::Length(2), // Footer
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_main(f, app, chunks[1]);
    render_footer(f, app, chunks[2]);

    // Render overlays
    if app.mode == AppMode::Help {
        render_help_overlay(f);
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let title = format!(
        " groo dev - {} ",
        app.project_name
    );

    let running = app.running_count();
    let total = app.services.len();
    let status = format!(" {}/{} running ", running, total);

    let help_hint = " [?] Help ";

    // Calculate spacing
    let title_len = title.len();
    let status_len = status.len();
    let help_len = help_hint.len();
    let spacing = area.width as usize - title_len - status_len - help_len;

    let line = Line::from(vec![
        Span::styled(title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" ".repeat(spacing.max(1))),
        Span::styled(status, Style::default().fg(Color::Green)),
        Span::styled(help_hint, Style::default().fg(Color::DarkGray)),
    ]);

    let header = Paragraph::new(line)
        .style(Style::default().bg(Color::DarkGray));

    f.render_widget(header, area);
}

fn render_main(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(25), // Services sidebar
            Constraint::Min(40),    // Logs
        ])
        .split(area);

    render_services(f, app, chunks[0]);
    render_logs(f, app, chunks[1]);
}

fn render_services(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .services
        .iter()
        .enumerate()
        .map(|(i, service)| {
            let checkbox = if service.selected { "[x]" } else { "[ ]" };
            let status_symbol = service.status.symbol();
            let status_color = service.status.color();
            let port_str = service
                .port
                .map(|p| format!(":{}", p))
                .unwrap_or_default();

            let is_current = i == app.cursor;

            let style = if is_current {
                Style::default().add_modifier(Modifier::BOLD).bg(Color::DarkGray)
            } else {
                Style::default()
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", checkbox),
                    if service.selected {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(format!("{} ", status_symbol), Style::default().fg(status_color)),
                Span::styled(&service.name, Style::default().fg(service.color)),
                Span::styled(port_str, Style::default().fg(Color::DarkGray)),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    // Build stats footer
    let aggregate = app.stats.aggregate();
    let stats_line = if aggregate.cpu_percent > 0.0 || aggregate.memory_mb > 0 {
        format!(
            "CPU: {:.0}%  MEM: {}MB",
            aggregate.cpu_percent,
            aggregate.memory_mb
        )
    } else {
        String::new()
    };

    // Highlight title when focused
    let title_style = if app.focus == PaneFocus::Services {
        Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };

    let block = Block::default()
        .borders(Borders::RIGHT)
        .title(" Services ")
        .title_style(title_style);

    let list = List::new(items).block(block);

    // Split area for list and stats
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(if stats_line.is_empty() { 0 } else { 2 }),
        ])
        .split(area);

    f.render_widget(list, chunks[0]);

    if !stats_line.is_empty() {
        let stats = Paragraph::new(stats_line)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::RIGHT | Borders::TOP));
        f.render_widget(stats, chunks[1]);
    }
}

fn render_logs(f: &mut Frame, app: &App, area: Rect) {
    let selected = app.selected_services();
    let selected_names: String = if selected.is_empty() {
        "none".to_string()
    } else if selected.len() <= 3 {
        selected.join(", ")
    } else {
        format!("{} services", selected.len())
    };

    let title = match app.mode {
        AppMode::Search => format!(" Logs ({}) [/] {} ", selected_names, app.search.query),
        AppMode::Visual => {
            let count = app.visual.as_ref().map(|s| s.line_count()).unwrap_or(0);
            format!(" Logs ({}) [VISUAL {}L] ", selected_names, count)
        }
        _ => format!(" Logs ({}) ", selected_names),
    };

    // Highlight title when focused
    let title_style = if app.focus == PaneFocus::Logs {
        Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };

    let block = Block::default()
        .borders(Borders::NONE)
        .title(title)
        .title_style(title_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Get filtered logs
    let entries = app.logs.filtered_entries(&selected);
    let visible_height = inner.height as usize;

    // Calculate scroll position
    let total_entries = entries.len();
    let start_idx = if app.follow_mode {
        total_entries.saturating_sub(visible_height)
    } else {
        app.scroll_offset.min(total_entries.saturating_sub(visible_height))
    };
    let end_idx = (start_idx + visible_height).min(total_entries);

    let visible_entries = &entries[start_idx..end_idx];

    let lines: Vec<Line> = visible_entries
        .iter()
        .enumerate()
        .map(|(display_idx, entry)| {
            let actual_idx = start_idx + display_idx;
            let is_match = app.search.matches.contains(&actual_idx);
            let is_current_match = app.mode == AppMode::Search
                && app.search.matches.get(app.search.current_match) == Some(&actual_idx);

            // Check if line is in visual selection
            let is_selected = if let Some(ref sel) = app.visual {
                let (sel_start, sel_end) = sel.range();
                actual_idx >= sel_start && actual_idx <= sel_end
            } else {
                false
            };

            // Check if this is the log cursor line (only when logs pane is focused)
            let is_cursor_line = app.focus == PaneFocus::Logs
                && app.mode == AppMode::Normal
                && app.log_cursor == Some(actual_idx);

            let time = entry.timestamp.format("%H:%M:%S").to_string();
            let level_color = match entry.level() {
                LogLevel::Error => Color::Red,
                LogLevel::Warn => Color::Yellow,
                LogLevel::Debug => Color::DarkGray,
                LogLevel::Info => Color::Reset,
            };

            // Determine line style based on state (in priority order)
            let content_style = if is_selected {
                // Visual selection - inverse colors
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
            } else if is_current_match {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if is_cursor_line {
                // Log cursor line - highlighted
                Style::default()
                    .fg(level_color)
                    .bg(Color::DarkGray)
            } else if is_match {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default().fg(level_color)
            };

            // Show cursor indicator when focused on logs
            let prefix = if is_cursor_line { "▶ " } else { "  " };

            Line::from(vec![
                Span::styled(
                    prefix,
                    if is_cursor_line {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(
                    format!("[{}] ", entry.service),
                    Style::default().fg(entry.color),
                ),
                Span::styled(format!("{} ", time), Style::default().fg(Color::DarkGray)),
                Span::styled(&entry.content, content_style),
            ])
        })
        .collect();

    let logs = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(logs, inner);

    // Render scroll indicator
    if total_entries > visible_height {
        let scroll_pct = if total_entries == 0 {
            100
        } else {
            ((end_idx as f64 / total_entries as f64) * 100.0) as u16
        };
        let indicator = if app.follow_mode {
            " [FOLLOW] ".to_string()
        } else {
            format!(" {}% ", scroll_pct)
        };
        let indicator_area = Rect::new(
            area.x + area.width - indicator.len() as u16 - 1,
            area.y,
            indicator.len() as u16,
            1,
        );
        let indicator_widget = Paragraph::new(indicator)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(indicator_widget, indicator_area);
    }
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.mode {
        AppMode::Search => {
            let match_info = if app.search.matches.is_empty() {
                "no matches".to_string()
            } else {
                format!(
                    "{}/{}",
                    app.search.current_match + 1,
                    app.search.matches.len()
                )
            };
            format!(
                " Search: {} | {} | [Enter] done  [n/N] next/prev  [Esc] cancel ",
                app.search.query,
                match_info
            )
        }
        AppMode::Visual => {
            let count = app.visual.as_ref().map(|s| s.line_count()).unwrap_or(0);
            format!(" [VISUAL] {} line(s) | [j/k] select  [y] copy  [Esc] cancel ", count)
        }
        AppMode::Help => " Press any key to close help ".to_string(),
        AppMode::Normal => {
            match app.focus {
                PaneFocus::Services => {
                    " [Tab] logs  [Space] toggle  [o]pen  [r]estart  [s]top  [q]uit ".to_string()
                }
                PaneFocus::Logs => {
                    " [Tab] services  [j/k] navigate  [v]isual  [y] copy  [/]search  [q]uit ".to_string()
                }
            }
        }
    };

    let footer = Paragraph::new(help_text)
        .style(Style::default().bg(Color::DarkGray));

    f.render_widget(footer, area);

    // Show status message if any
    if let Some(ref msg) = app.status_message {
        let msg_area = Rect::new(
            area.x + area.width - msg.len() as u16 - 2,
            area.y,
            msg.len() as u16 + 2,
            1,
        );
        let msg_widget = Paragraph::new(format!(" {} ", msg))
            .style(Style::default().fg(Color::Yellow).bg(Color::DarkGray));
        f.render_widget(msg_widget, msg_area);
    }
}

fn render_help_overlay(f: &mut Frame) {
    let area = f.area();
    let overlay_width = 50;
    let overlay_height = 20;
    let x = (area.width.saturating_sub(overlay_width)) / 2;
    let y = (area.height.saturating_sub(overlay_height)) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    // Clear the area
    f.render_widget(Clear, overlay_area);

    let help_text = vec![
        "",
        "  Navigation",
        "    Tab          Switch focus (services/logs)",
        "    j/k, ↑/↓     Move cursor in focused pane",
        "",
        "  Services Pane",
        "    Space        Toggle service log visibility",
        "    a            Select all services",
        "    n            Select none",
        "    o            Open in browser",
        "    r/R          Restart service/all",
        "    s/S          Stop service/all",
        "",
        "  Logs Pane",
        "    j/k          Move log cursor",
        "    g/G          Jump to top/bottom",
        "    v            Visual mode (select lines)",
        "    y            Copy line (or selection)",
        "    /            Search in logs",
        "    PgUp/PgDn    Scroll logs",
        "",
        "  General",
        "    ?            Toggle this help",
        "    q            Quit (stops all services)",
        "",
    ];

    let help = Paragraph::new(help_text.join("\n"))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .title_style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .style(Style::default().bg(Color::Black));

    f.render_widget(help, overlay_area);
}

/// Render shutdown overlay
pub fn render_shutdown(f: &mut Frame, service_count: usize) {
    let area = f.area();

    // Center the message
    let msg = format!("Stopping {} service(s)...", service_count);
    let width = (msg.len() + 6) as u16;
    let height = 3;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let overlay_area = Rect::new(x, y, width, height);

    // Clear the area
    f.render_widget(Clear, overlay_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let text = Paragraph::new(msg)
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center)
        .block(block);

    f.render_widget(text, overlay_area);
}
