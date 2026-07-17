//! ## UI Widgets / Виджеты интерфейса
//!
//! Helper widgets for the TUI.
//! / Вспомогательные виджеты для TUI.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

/// Creates a colored status line.
pub fn status_line<'a>(label: &'a str, value: &'a str, color: Color) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{}: ", label), Style::default().fg(Color::Gray)),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ])
}

/// Formats a peer ID for display (first 8 chars + ...).
pub fn format_peer_id(id: &str) -> String {
    if id.len() > 12 {
        format!("{}...", &id[..12])
    } else {
        id.to_string()
    }
}

/// Formats bytes to human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
