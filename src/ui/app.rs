//! ## TUI Application / TUI-приложение
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

pub struct App {
    pub peer_id: String,
    pub peer_id_short: String,
    pub username: String,
    pub local_addrs: Vec<String>,
    pub external_addr: Option<String>,
    pub nat_type: String,
    pub beacon_addr: Option<String>,
    pub lan_mode: bool,
    pub messages: Vec<(String, String, String)>, // (sender_short, sender_full, text_with_time)
    pub input: String,
    pub input_mode: InputMode,
    pub scroll: usize,
    pub connected_peers: Vec<String>,
    pub connected_peers_info: Vec<(String, String)>, // (peer_id_hex, username)
    pub lan_peer_count: usize,
    pub status: String,
    pub show_help: bool,
    pub show_peers: bool,
    pub mode: String,
    pub selected_chat: Option<String>, // None = broadcast, Some(peer_id_hex) = DM
    pub peer_cursor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

impl App {
    pub fn new(
        peer_id: String,
        username: String,
        local_addrs: Vec<String>,
        external_addr: Option<String>,
        nat_type: String,
        beacon_addr: Option<String>,
    ) -> Self {
        let peer_id_short = peer_id.chars().take(8).collect();
        let lan_mode = beacon_addr.is_none();
        let mode = if lan_mode { "🏠 LAN" } else { "🗼 Beacon" };
        Self {
            peer_id,
            peer_id_short,
            username,
            local_addrs,
            external_addr,
            nat_type,
            beacon_addr,
            lan_mode,
            messages: Vec::new(),
            input: String::new(),
            input_mode: InputMode::Normal,
            scroll: 0,
            connected_peers: Vec::new(),
            connected_peers_info: Vec::new(),
            lan_peer_count: 0,
            status: "Ready / Готов".to_string(),
            show_help: false,
            show_peers: false,
            mode: mode.to_string(),
            selected_chat: None,
            peer_cursor: 0,
        }
    }

    pub fn add_message(&mut self, sender_full: String, text: String) {
        let sender_short: String = sender_full.chars().take(8).collect();
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.messages
            .push((sender_short, sender_full, format!("[{}] {}", time, text)));
        if self.messages.len() > 500 {
            self.messages.remove(0);
        }
        self.scroll = self.messages.len().saturating_sub(1);
    }

    pub fn add_system_message(&mut self, text: String) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.messages.push((
            "SYS".to_string(),
            "SYSTEM".to_string(),
            format!("[{}] {}", time, text),
        ));
        self.scroll = self.messages.len().saturating_sub(1);
    }

    pub fn add_peer(&mut self, peer_id: String, username: String) {
        let short_id = if peer_id.len() > 8 {
            format!("{}...", &peer_id[..8])
        } else {
            peer_id.clone()
        };
        let display = format!("{} (@{})", short_id, username);
        if !self.connected_peers.contains(&display) {
            self.connected_peers.push(display);
            self.connected_peers_info.push((peer_id, username));
        }
    }

    pub fn remove_peer(&mut self, peer_id: &str) {
        self.connected_peers_info.retain(|(id, _)| id != peer_id);
        self.connected_peers.retain(|p| !p.contains(peer_id));
        if self.peer_cursor >= self.connected_peers_info.len() {
            self.peer_cursor = self.connected_peers_info.len().saturating_sub(1);
        }
    }

    pub fn set_lan_peer_count(&mut self, count: usize) {
        self.lan_peer_count = count;
    }

    pub fn total_peers(&self) -> usize {
        self.connected_peers.len()
    }

    pub fn select_chat(&mut self, peer_id: Option<String>) {
        self.selected_chat = peer_id;
    }

    pub fn get_chat_target_label(&self) -> String {
        if let Some(ref id) = self.selected_chat {
            if let Some((_, name)) = self.connected_peers_info.iter().find(|(pid, _)| pid == id) {
                return format!("💬 DM: @{}", name);
            }
            return format!("💬 DM: {}...", &id[..8]);
        }
        "📢 Broadcast (All LAN)".to_string()
    }

    pub fn move_peer_cursor(&mut self, delta: i32) {
        if self.connected_peers_info.is_empty() {
            return;
        }
        let len = self.connected_peers_info.len();
        self.peer_cursor = ((self.peer_cursor as i32 + delta).rem_euclid(len as i32)) as usize;
    }

    pub fn select_peer_at_cursor(&mut self) {
        if self.connected_peers_info.is_empty() {
            return;
        }
        let peer_id = self.connected_peers_info[self.peer_cursor].0.clone();
        let username = self.connected_peers_info[self.peer_cursor].1.clone();
        self.select_chat(Some(peer_id.clone()));
        self.add_system_message(format!("💬 Switched to DM with @{}", username));
    }

    pub fn switch_to_broadcast(&mut self) {
        if self.selected_chat.is_some() {
            self.select_chat(None);
            self.add_system_message("📢 Switched to Broadcast mode".to_string());
        }
    }
}

/// Runs the TUI event loop. Returns Some(input) when user presses Enter with text.
pub fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<Option<String>> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = std::time::Duration::from_millis(200);
    loop {
        terminal.draw(|f| ui(f, app))?;
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('q') => return Ok(Some("/quit".to_string())),
                            KeyCode::Char('e') => {
                                app.input_mode = InputMode::Editing;
                                app.status = "✏️ Editing".to_string();
                            }
                            KeyCode::Char('h') => app.show_help = !app.show_help,
                            KeyCode::Char('p') => app.show_peers = !app.show_peers,
                            KeyCode::Up => {
                                if app.show_peers {
                                    app.move_peer_cursor(-1);
                                } else if app.scroll > 0 {
                                    app.scroll -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if app.show_peers {
                                    app.move_peer_cursor(1);
                                } else {
                                    app.scroll += 1;
                                }
                            }
                            KeyCode::Tab => {
                                if app.selected_chat.is_some() {
                                    app.switch_to_broadcast();
                                } else if !app.connected_peers_info.is_empty() {
                                    app.select_peer_at_cursor();
                                }
                            }
                            KeyCode::Enter => {
                                if app.show_peers && !app.connected_peers_info.is_empty() {
                                    app.select_peer_at_cursor();
                                    app.show_peers = false;
                                }
                            }
                            KeyCode::Char('c') => {
                                if !app.connected_peers_info.is_empty() {
                                    app.select_peer_at_cursor();
                                }
                            }
                            KeyCode::Char('b') => {
                                app.switch_to_broadcast();
                            }
                            _ => {}
                        },
                        InputMode::Editing => match key.code {
                            KeyCode::Enter => {
                                let input = app.input.trim().to_string();
                                app.input.clear();
                                app.input_mode = InputMode::Normal;
                                app.status = "Ready".to_string();
                                if !input.is_empty() {
                                    return Ok(Some(input));
                                }
                            }
                            KeyCode::Esc => {
                                app.input.clear();
                                app.input_mode = InputMode::Normal;
                                app.status = "Ready".to_string();
                            }
                            KeyCode::Char(c) => app.input.push(c),
                            KeyCode::Backspace => {
                                app.input.pop();
                            }
                            _ => {}
                        },
                    }
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.size());

    // Title
    let title_text = if app.lan_mode {
        format!(" LAN MODE — {}", app.username)
    } else {
        format!("🗼 BEACON MODE — {}", app.username)
    };
    let title_color = if app.lan_mode {
        Color::Green
    } else {
        Color::Yellow
    };
    let title = Paragraph::new(Text::from(vec![Line::from(vec![
        Span::styled(" 🌊 ", Style::default().fg(Color::Cyan)),
        Span::styled(
            "UNDERTOW",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" — {} ", title_text),
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("(Peers: {})", app.total_peers() + app.lan_peer_count),
            Style::default().fg(Color::Gray),
        ),
    ])]))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(title, chunks[0]);

    // Info
    let beacon_status = if let Some(ref beacon) = app.beacon_addr {
        format!("🟢 {}", beacon)
    } else {
        "🔴 None (LAN mode)".to_string()
    };
    let info_text = format!(
        "👤 {} ({})\n📡 {}\n Local: {}\n🌍 External: {} | 🔥 NAT: {}",
        app.username,
        app.peer_id_short,
        beacon_status,
        app.local_addrs.join(", "),
        app.external_addr.as_deref().unwrap_or("Unknown"),
        app.nat_type,
    );
    let info = Paragraph::new(info_text)
        .block(
            Block::default()
                .title(" Node Info ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(info, chunks[1]);

    // Main content
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[2]);

    // Chat
    let messages_text: Vec<Line> = app
        .messages
        .iter()
        .map(|(short, _full, text)| {
            let color = if short == "SYS" {
                Color::Yellow
            } else if short == &app.peer_id_short || short == &app.username {
                Color::Green
            } else {
                Color::Cyan
            };
            let prefix = if short == &app.username || short == &app.peer_id_short {
                "You"
            } else if short == "SYS" {
                "SYS"
            } else {
                short
            };
            let style = if text.contains("🏠") || text.contains("LAN") {
                Style::default().fg(Color::LightGreen)
            } else if text.contains("💬") {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(vec![
                Span::styled(
                    format!("{} ", prefix),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(text.clone(), style),
            ])
        })
        .collect();
    let messages = Paragraph::new(Text::from(messages_text))
        .block(
            Block::default()
                .title(format!(" Chat [{}] ", app.messages.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .wrap(Wrap { trim: true })
        .scroll((app.scroll as u16, 0));
    f.render_widget(messages, main_chunks[0]);

    // Peers
    let mut peers_text = String::new();
    if !app.connected_peers_info.is_empty() {
        peers_text.push_str(&format!(
            "🌐 Network Peers ({})\n",
            app.connected_peers_info.len()
        ));
        for (i, (peer_id, username)) in app.connected_peers_info.iter().enumerate() {
            let short_id = if peer_id.len() > 8 {
                format!("{}...", &peer_id[..8])
            } else {
                peer_id.clone()
            };
            let marker = if app.selected_chat.as_deref() == Some(peer_id.as_str()) {
                "▶ "
            } else if i == app.peer_cursor {
                "• "
            } else {
                "  "
            };
            peers_text.push_str(&format!("{}{} (@{})\n", marker, short_id, username));
        }
    }
    if peers_text.is_empty() {
        peers_text = "No peers connected\nНет подключённых пиров".to_string();
        if app.lan_mode {
            peers_text.push_str("\n🔍 Searching LAN...");
        }
    }
    let peers = Paragraph::new(peers_text)
        .block(
            Block::default()
                .title(format!(
                    " Peers [{}] ",
                    app.total_peers() + app.lan_peer_count
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(peers, main_chunks[1]);

    // Input
    let input_style = if app.input_mode == InputMode::Editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    let input_label = if app.input_mode == InputMode::Editing {
        " ✏️ Type message (ESC to cancel) "
    } else {
        " [e]dit [h]elp [p]eers [↑↓]nav [Tab]chat [q]uit "
    };
    let input_title = format!("{} | {}", input_label, app.get_chat_target_label());
    let input = Paragraph::new(app.input.clone()).style(input_style).block(
        Block::default()
            .title(input_title)
            .borders(Borders::ALL)
            .border_style(input_style),
    );
    f.render_widget(input, chunks[3]);

    // Status
    let status_text = if app.lan_mode {
        format!(" 🏠 LAN Mode | {}", app.status)
    } else {
        format!(" 🗼 Beacon Mode | {}", app.status)
    };
    let status =
        Paragraph::new(status_text).style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status, chunks[4]);

    // Help popup
    if app.show_help {
        let help_text = "\
🌊 UNDERTOW PROTOCOL — Help\n\
════════════════════════════════\n\n\
🎮 Keys:\n\
  e       — Start writing\n\
  Enter   — Send\n\
  ESC     — Cancel\n\
  h       — Toggle help\n\
  p       — Toggle peers list\n\
  ↑,↓     — Scroll chat OR navigate peers\n\
  Tab     — Switch Broadcast ↔ DM\n\
  Enter   — (in peers popup) select peer\n\
  c       — Chat with peer under cursor\n\
  b       — Back to broadcast mode\n\
  q       — Quit\n\n\
📝 Commands:\n\
  /chat <name>    — Start DM with user\n\
  /broadcast      — Switch to broadcast\n\
  /peers          — Show peers list\n\
  /lan            — Show LAN peers\n\
  /nat            — Detect NAT\n\
  /help           — Show this help\n\
  (text)          — Send message to current chat\n";
        let area = centered_rect(60, 80, f.size());
        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title(" Help (press h to close)")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(Clear, area);
        f.render_widget(help, area);
    }

    // Peers popup
    if app.show_peers {
        let mut peers_text = String::from("🌐 Connected Peers\n");
        peers_text.push_str("════════════════════════════════\n\n");
        if app.connected_peers_info.is_empty() {
            peers_text.push_str("  No peers connected\n");
        } else {
            for (i, (peer_id, username)) in app.connected_peers_info.iter().enumerate() {
                let short_id = if peer_id.len() > 16 {
                    format!("{}...{}", &peer_id[..8], &peer_id[peer_id.len() - 8..])
                } else {
                    peer_id.clone()
                };
                let marker = if i == app.peer_cursor { "▶" } else { " " };
                let is_selected = app.selected_chat.as_deref() == Some(peer_id.as_str());
                let sel_mark = if is_selected { " [DM]" } else { "" };
                peers_text.push_str(&format!(
                    "  {} {:<2} @{} ({}){}\n",
                    marker,
                    i + 1,
                    username,
                    short_id,
                    sel_mark
                ));
            }
        }
        peers_text.push_str(&format!(
            "\n  Total: {} peers",
            app.connected_peers_info.len()
        ));
        peers_text.push_str("\n\n  ↑↓ navigate | Enter select | p close");
        let area = centered_rect(50, 60, f.size());
        let peers = Paragraph::new(peers_text)
            .block(
                Block::default()
                    .title(" Peers ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(Clear, area);
        f.render_widget(peers, area);
    }
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
