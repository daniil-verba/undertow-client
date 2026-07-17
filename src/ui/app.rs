//! ## TUI Application / TUI-приложение
//!
//! Beautiful terminal interface for the Undertow node.
//! / Красивый терминальный интерфейс для узла Undertow.

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
use std::sync::Arc;
use undertow_protocol::network::lan_beacon::LanBeacon;

/// Application state / Состояние приложения
pub struct App {
    /// My PeerId / Мой PeerId
    pub peer_id: String,
    /// My short PeerId / Мой короткий PeerId
    pub peer_id_short: String,
    /// My username / Моё имя пользователя
    pub username: String,
    /// Local IP addresses / Локальные IP-адреса
    pub local_addrs: Vec<String>,
    /// External IP:port from STUN / Внешний IP:порт от STUN
    pub external_addr: Option<String>,
    /// NAT type / Тип NAT
    pub nat_type: String,
    /// Connected beacon address / Адрес подключённого маяка
    pub beacon_addr: Option<String>,
    /// Chat messages: (sender_short, sender_full, text_with_time)
    pub messages: Vec<(String, String, String)>,
    /// Input buffer / Буфер ввода
    pub input: String,
    /// Input mode / Режим ввода
    pub input_mode: InputMode,
    /// Scroll position / Позиция прокрутки
    pub scroll: usize,
    /// Connected peers / Подключённые пиры
    pub connected_peers: Vec<String>,
    /// Connected peers with usernames / Подключённые пиры с именами
    pub connected_peers_info: Vec<(String, String)>, // (peer_id, username)
    /// Status message / Статусное сообщение
    pub status: String,
    /// Show help popup / Показать всплывающую справку
    pub show_help: bool,
    /// LAN peers
    pub lan_peers: Vec<(String, String)>, // (peer_id, username)
    /// LAN beacon reference (optional)
    pub lan_beacon: Option<Arc<LanBeacon>>,
    /// Change name mode
    pub changing_name: bool,
    /// New name buffer
    pub new_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

impl App {
    /// Creates a new App with node info.
    pub fn new(
        peer_id: String,
        username: String,
        local_addrs: Vec<String>,
        external_addr: Option<String>,
        nat_type: String,
        beacon_addr: Option<String>,
    ) -> Self {
        let peer_id_short = peer_id.chars().take(8).collect();

        Self {
            peer_id,
            peer_id_short,
            username,
            local_addrs,
            external_addr,
            nat_type,
            beacon_addr,
            messages: Vec::new(),
            input: String::new(),
            input_mode: InputMode::Normal,
            scroll: 0,
            connected_peers: Vec::new(),
            connected_peers_info: Vec::new(),
            status: "Ready / Готов".to_string(),
            show_help: false,
            lan_peers: Vec::new(),
            lan_beacon: None,
            changing_name: false,
            new_name: String::new(),
        }
    }

    /// Adds a received chat message.
    pub fn add_message(&mut self, sender_full: String, text: String) {
        let sender_short: String = sender_full.chars().take(8).collect();
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.messages
            .push((sender_short, sender_full, format!("[{}] {}", time, text)));
        if self.messages.len() > 500 {
            self.messages.remove(0);
        }
    }

    /// Adds a system message (yellow SYS prefix).
    pub fn add_system_message(&mut self, text: String) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.messages.push((
            "SYS".to_string(),
            "SYSTEM".to_string(),
            format!("[{}] {}", time, text),
        ));
    }

    /// Adds a connected peer with username.
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

    /// Removes a connected peer.
    pub fn remove_peer(&mut self, peer_id: &str) {
        self.connected_peers_info.retain(|(id, _)| id != peer_id);
        self.connected_peers.retain(|p| !p.contains(peer_id));
    }
    /// Updates peer list from beacon info.
    pub fn update_peers(&mut self, peers: Vec<(String, String)>) {
        self.connected_peers.clear();
        self.connected_peers_info.clear();
        for (peer_id, username) in peers {
            self.add_peer(peer_id, username);
        }
    }

    /// Updates LAN peers list
    pub fn update_lan_peers(&mut self, peers: Vec<(String, String)>) {
        self.lan_peers = peers;
    }

    /// Adds a LAN peer
    pub fn add_lan_peer(&mut self, peer_id: String, username: String) {
        if !self.lan_peers.iter().any(|(id, _)| id == &peer_id) {
            self.lan_peers.push((peer_id, username));
        }
    }

    /// Removes a LAN peer
    pub fn remove_lan_peer(&mut self, peer_id: &str) {
        self.lan_peers.retain(|(id, _)| id != peer_id);
    }

    /// Starts changing name mode
    pub fn start_change_name(&mut self) {
        self.changing_name = true;
        self.new_name = self.username.clone();
        self.input_mode = InputMode::Editing;
        self.status = "Enter new username (ESC to cancel, Enter to confirm)".to_string();
    }

    /// Confirms name change
    pub fn confirm_name_change(&mut self) -> Result<(), String> {
        let new_name = self.new_name.trim();
        if new_name.is_empty() {
            return Err("Username cannot be empty".to_string());
        }
        if new_name.len() > 32 {
            return Err("Username too long (max 32 chars)".to_string());
        }

        // Обновляем имя в профиле
        // Это будет делаться в main.rs
        self.status = format!("Username changed to: {}", new_name);
        self.changing_name = false;
        self.input_mode = InputMode::Normal;
        Ok(())
    }

    /// Cancels name change
    pub fn cancel_name_change(&mut self) {
        self.changing_name = false;
        self.new_name.clear();
        self.input_mode = InputMode::Normal;
        self.status = "Name change cancelled".to_string();
    }
}

/// Runs the TUI event loop.
pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<(String, App)> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = std::time::Duration::from_millis(250);

    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('q') => return Ok(("/quit".to_string(), app)),
                            KeyCode::Char('e') => {
                                app.input_mode = InputMode::Editing;
                                app.status = "Editing mode / Режим ввода".to_string();
                            }
                            KeyCode::Char('h') => app.show_help = !app.show_help,
                            KeyCode::Up => {
                                if app.scroll > 0 {
                                    app.scroll -= 1;
                                }
                            }
                            KeyCode::Down => app.scroll += 1,
                            _ => {}
                        },
                        InputMode::Editing => match key.code {
                            KeyCode::Enter => {
                                let input = app.input.trim().to_string();
                                app.input.clear();
                                app.input_mode = InputMode::Normal;
                                app.status = "Ready / Готов".to_string();
                                if !input.is_empty() {
                                    return Ok((input, app));
                                }
                            }
                            KeyCode::Esc => {
                                app.input.clear();
                                app.input_mode = InputMode::Normal;
                                app.status = "Ready / Готов".to_string();
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

/// Renders the complete UI layout.
fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(8), // Info panel
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Input
            Constraint::Length(1), // Status
        ])
        .split(f.size());

    // === TITLE BAR ===
    let title = Paragraph::new(Text::from(vec![Line::from(vec![
        Span::styled(" 🌊 ", Style::default().fg(Color::Cyan)),
        Span::styled(
            "UNDERTOW PROTOCOL",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" — {} ", app.username),
            Style::default().fg(Color::LightGreen),
        ),
        Span::styled(
            "— Decentralized P2P Messenger",
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

    // === INFO PANEL ===
    let info_text = format!(
        "👤 {} ({})\n\
         🏠 Local: {}\n\
         🌍 External: {}\n\
         🔥 NAT: {} | 🗼 Beacon: {} | 📡 Peers: {}",
        app.username,
        app.peer_id_short,
        app.local_addrs.join(", "),
        app.external_addr.as_deref().unwrap_or("Unknown"),
        app.nat_type,
        app.beacon_addr.as_deref().unwrap_or("None"),
        app.connected_peers.len(),
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

    // === MAIN CONTENT: Chat + Peers ===
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[2]);

    // Chat messages
    let messages_text: Vec<Line> = app
        .messages
        .iter()
        .map(|(short, full, text)| {
            let color = if short == "SYS" {
                Color::Yellow
            } else if short == &app.peer_id_short || short == &app.username {
                Color::Green
            } else {
                Color::Cyan
            };
            let prefix = if short == &app.username || short == &app.peer_id_short {
                "You"
            } else {
                short
            };
            Line::from(vec![
                Span::styled(
                    format!("{} ", prefix),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(text.clone(), Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let messages = Paragraph::new(Text::from(messages_text))
        .block(
            Block::default()
                .title(" Chat ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .wrap(Wrap { trim: true })
        .scroll((app.scroll as u16, 0));
    f.render_widget(messages, main_chunks[0]);

    // Peers list with usernames
    // В функции ui() найти эту секцию:
    // Peers list with usernames
    let peers_text = if app.connected_peers.is_empty() {
        "No peers connected".to_string()
    } else {
        app.connected_peers.join("\n")
    };

    let peers = Paragraph::new(peers_text)
        .block(
            Block::default()
                .title(format!(" Peers [{}] ", app.connected_peers.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(peers, main_chunks[1]);

    // И ЗАМЕНИТЬ её на:

    // Peers list with usernames (LAN + Beacon)
    let mut peers_text = String::new();

    // LAN Peers
    if !app.lan_peers.is_empty() {
        peers_text.push_str("🌐 LAN Peers:\n");
        for (id, username) in &app.lan_peers {
            let short_id = if id.len() > 8 {
                format!("{}...", &id[..8])
            } else {
                id.clone()
            };
            peers_text.push_str(&format!("  🟢 {} (@{})\n", short_id, username));
        }
        peers_text.push_str("\n");
    }

    // Beacon Peers
    if !app.connected_peers.is_empty() {
        peers_text.push_str("📡 Beacon Peers:\n");
        for peer in &app.connected_peers {
            peers_text.push_str(&format!("  {}\n", peer));
        }
    }

    if peers_text.is_empty() {
        peers_text = "No peers connected".to_string();
    }

    let peers = Paragraph::new(peers_text)
        .block(
            Block::default()
                .title(format!(
                    " Peers [LAN: {}, Beacon: {}] ",
                    app.lan_peers.len(),
                    app.connected_peers.len()
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(peers, main_chunks[1]);

    // === INPUT BAR ===
    let input_style = if app.input_mode == InputMode::Editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let input_label = if app.input_mode == InputMode::Editing {
        " Send message (ESC=Cancel) "
    } else {
        " 'e'=write, 'h'=help, 'q'=quit "
    };

    let input = Paragraph::new(app.input.clone()).style(input_style).block(
        Block::default()
            .title(input_label)
            .borders(Borders::ALL)
            .border_style(input_style),
    );
    f.render_widget(input, chunks[3]);

    // === STATUS BAR ===
    let status = Paragraph::new(format!(" {} ", app.status))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status, chunks[4]);

    // === HELP POPUP ===
    if app.show_help {
        let help_text = "\
        🌊 UNDERTOW PROTOCOL — Help\n\n\
        Keys:\n\
          e       — Start writing\n\
          Enter   — Send\n\
          ESC     — Cancel\n\
          h       — Toggle help\n\
          ↑,↓     — Scroll chat\n\
          q       — Quit\n\n\
        Commands:\n\
          /name <username>      — Change your username\n\
          /lan                  — Show LAN peers\n\
          /msg-lan <name> <msg> — Send message to LAN peer\n\
          /broadcast <msg>      — Broadcast to all LAN peers\n\
          /msg <username> <msg> — Send via beacon\n\
          /send <peer_id> <msg> — Send by PeerId\n\
          /peers                — List connected peers\n\
          /ping <addr>          — Ping node\n\
          /nat                  — Detect NAT\n\
          /help                 — Show this help\n\
        ";

        let area = centered_rect(55, 65, f.size());
        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title(" Help ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(Clear, area);
        f.render_widget(help, area);
    }
}

/// Creates a centered rectangle for popups.
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
