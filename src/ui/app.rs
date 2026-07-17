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
    /// LAN mode flag / Флаг LAN режима
    pub lan_mode: bool,
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
    /// LAN peers count / Количество LAN пиров
    pub lan_peer_count: usize,
    /// Status message / Статусное сообщение
    pub status: String,
    /// Show help popup / Показать всплывающую справку
    pub show_help: bool,
    /// Show peers popup / Показать список пиров
    pub show_peers: bool,
    /// Mode indicator / Индикатор режима
    pub mode: String,
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

    /// Sets LAN peer count.
    pub fn set_lan_peer_count(&mut self, count: usize) {
        self.lan_peer_count = count;
    }

    /// Returns total connected peers count.
    pub fn total_peers(&self) -> usize {
        self.connected_peers.len()
    }
}

/// Runs the TUI event loop, mutating app state in place.
pub fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<Option<String>> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = std::time::Duration::from_millis(250);
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
                                app.status = "✏️ Editing mode / Режим ввода".to_string();
                            }
                            KeyCode::Char('h') => app.show_help = !app.show_help,
                            KeyCode::Char('p') => app.show_peers = !app.show_peers,
                            KeyCode::Up => {
                                if app.scroll > 0 {
                                    app.scroll -= 1;
                                }
                            }
                            KeyCode::Down => app.scroll += 1,
                            KeyCode::Char('r') => {
                                app.status = "🔄 Refreshing...".to_string();
                            }
                            _ => {}
                        },
                        InputMode::Editing => match key.code {
                            KeyCode::Enter => {
                                let input = app.input.trim().to_string();
                                app.input.clear();
                                app.input_mode = InputMode::Normal;
                                app.status = "Ready / Готов".to_string();
                                if !input.is_empty() {
                                    return Ok(Some(input));
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
            Constraint::Length(6), // Info panel
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Input
            Constraint::Length(1), // Status
        ])
        .split(f.size());

    // === TITLE BAR ===
    let title_text = if app.lan_mode {
        format!("🏠 LAN MODE — {}", app.username)
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

    // === INFO PANEL ===
    let beacon_status = if let Some(ref beacon) = app.beacon_addr {
        format!("🟢 {}", beacon)
    } else {
        "🔴 None (LAN mode)".to_string()
    };

    let info_text = format!(
        "👤 {} ({})\n\
         📡 {}\n\
         🏠 Local: {}\n\
         🌍 External: {} | 🔥 NAT: {}",
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
            } else if short == "SYS" {
                "SYS"
            } else {
                short
            };

            // Check if this is a system message with emoji indicators
            let style = if text.contains("🏠") || text.contains("LAN") {
                Style::default().fg(Color::LightGreen)
            } else if text.contains("🗼") || text.contains("beacon") {
                Style::default().fg(Color::Yellow)
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

    // Peers list with indicators
    let mut peers_text = String::new();

    if !app.connected_peers.is_empty() {
        peers_text.push_str(&format!(
            "🌐 Network Peers ({})\n",
            app.connected_peers.len()
        ));
        for peer in &app.connected_peers {
            peers_text.push_str(&format!("  • {}\n", peer));
        }
    }

    if app.lan_peer_count > 0 {
        peers_text.push_str(&format!("\n🏠 LAN Peers ({})\n", app.lan_peer_count));
        // LAN peers are shown with green indicator
        for peer in &app.connected_peers {
            if peer.contains("LAN") || peer.contains("🏠") {
                peers_text.push_str(&format!("  🟢 {}\n", peer));
            }
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

    // === INPUT BAR ===
    let input_style = if app.input_mode == InputMode::Editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let input_label = if app.input_mode == InputMode::Editing {
        " ✏️ Type message (ESC to cancel) "
    } else {
        " [e]dit  [h]elp  [p]eers  [r]efresh  [q]uit "
    };

    let input = Paragraph::new(app.input.clone()).style(input_style).block(
        Block::default()
            .title(input_label)
            .borders(Borders::ALL)
            .border_style(input_style),
    );
    f.render_widget(input, chunks[3]);

    // === STATUS BAR ===
    let status_text = if app.lan_mode {
        format!(" 🏠 LAN Mode | {}", app.status)
    } else {
        format!(" 🗼 Beacon Mode | {}", app.status)
    };
    let status =
        Paragraph::new(status_text).style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status, chunks[4]);

    // === HELP POPUP ===
    if app.show_help {
        let help_text = "\
        🌊 UNDERTOW PROTOCOL — Help / Справка\n\
        ════════════════════════════════════════════\n\n\
        🎮 Keys / Клавиши:\n\
          e       — Start writing / Начать ввод\n\
          Enter   — Send / Отправить\n\
          ESC     — Cancel / Отмена\n\
          h       — Toggle help / Переключить справку\n\
          p       — Toggle peers / Список пиров\n\
          ↑,↓     — Scroll chat / Прокрутка чата\n\
          r       — Refresh / Обновить\n\
          q       — Quit / Выход\n\n\
        📝 Commands / Команды:\n\
          /msg <username> <text>  — Send to user by name\n\
          /send <peer_id> <text>  — Send by PeerId\n\
          /peers                  — List all peers\n\
          /lan                    — Show LAN peers only\n\
          /ping <addr>            — Ping node\n\
          /nat                    — Detect NAT\n\
          /help                   — Show this help\n\n\
        🏠 LAN Discovery:\n\
          • Automatic peer discovery in local network\n\
          • No beacon required\n\
          • Announces every 10 seconds\n\
          • Direct P2P communication\n\n\
        🌐 Beacon Mode:\n\
          • Global peer discovery\n\
          • Relay through VPS\n\
          • Works across the internet\n\
        ";

        let area = centered_rect(60, 80, f.size());
        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title(" Help / Справка (press h to close)")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(Clear, area);
        f.render_widget(help, area);
    }

    // === PEERS POPUP ===
    if app.show_peers {
        let mut peers_text = String::from("🌐 Connected Peers / Подключённые пиры\n");
        peers_text.push_str(&format!("════════════════════════════════\n\n"));

        if app.connected_peers_info.is_empty() {
            peers_text.push_str("  No peers connected\n  Нет подключённых пиров\n");
        } else {
            for (i, (peer_id, username)) in app.connected_peers_info.iter().enumerate() {
                let short_id = if peer_id.len() > 16 {
                    format!("{}...{}", &peer_id[..8], &peer_id[peer_id.len() - 8..])
                } else {
                    peer_id.clone()
                };
                let is_lan = app.lan_mode; // Simplified
                let indicator = if is_lan { "🏠" } else { "🌐" };
                peers_text.push_str(&format!(
                    "  {:<2} {} @{} ({}...)\n",
                    i + 1,
                    indicator,
                    username,
                    &peer_id[..8]
                ));
            }
        }

        peers_text.push_str(&format!(
            "\n  Total: {} peers",
            app.connected_peers_info.len()
        ));
        if app.lan_peer_count > 0 {
            peers_text.push_str(&format!(" ({} LAN)", app.lan_peer_count));
        }
        peers_text.push_str("\n\n  Press 'p' to close");

        let area = centered_rect(50, 60, f.size());
        let peers = Paragraph::new(peers_text)
            .block(
                Block::default()
                    .title(" Peers / Пиры ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(Clear, area);
        f.render_widget(peers, area);
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
